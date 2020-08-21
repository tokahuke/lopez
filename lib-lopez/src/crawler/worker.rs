use futures::channel::mpsc;
use futures::future;
use futures::prelude::*;
use http::Request;
use hyper::{client::HttpConnector, Body, Client, StatusCode};
use hyper_tls::HttpsConnector;
use lazy_static::lazy_static;
use libflate::deflate::Decoder as DeflateDecoder;
use libflate::gzip::Decoder as GzipDecoder;
use scraper::{Html, Selector};
use std::io::Read;
use std::rc::Rc;
use std::sync::Arc;
use tokio::time::{self, Duration};
use url::{ParseError, Url};

use crate::backend::{WorkerBackend, WorkerBackendFactory};
use crate::cancel::{spawn_onto_thread, Canceler};
use crate::directives::{Analyzer, Boundaries, Directives};
use crate::origins::Origins;
use crate::profile::Profile;

use super::Counter;
use super::Reason;

/// Performs a checked join, with all the common problems accounted for.
pub fn checked_join(base_url: &Url, raw: &str) -> Result<Url, crate::Error> {
    // Parse the thing.
    let maybe_url = raw.parse().or_else(|err| {
        if err == ParseError::RelativeUrlWithoutBase {
            base_url.join(&raw)
        } else {
            Err(err)
        }
    });

    let url = if let Ok(url) = maybe_url {
        url
    } else {
        return Err(crate::Error::Custom(format!("bad link: {}", raw)));
    };

    // Get rid of those pesky "#" section references and of weird empty strings:
    if raw.is_empty() || raw.starts_with('#') {
        return Err(crate::Error::Custom(format!("bad link: {}", raw)));
    }

    // Now, make sure this is really HTTP (not mail, ftp and what not):
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(crate::Error::Custom(format!("unaccepted scheme: {}", raw)));
    }

    // Check if internal or external.
    if url.domain().is_some() {
        Ok(url)
    } else {
        Err(crate::Error::Custom(format!("no domain: {}", raw)))
    }
}

#[test]
fn checked_join_test() {
    assert_eq!(
        checked_join(
            &Url::parse("https://querobolsa.com.br/mba").unwrap(),
            "/revista/assunto/especiais"
        )
        .unwrap(),
        Url::parse("https://querobolsa.com.br/revista/assunto/especiais").unwrap(),
    )
}

/// Finds all "hrefs" in an HTML and run all analyses.
fn tree_search<'a>(html: &'a Html) -> Vec<(Reason, &'a str)> {
    lazy_static! {
        static ref ANCHOR: Selector =
            Selector::parse("a").expect("failed to parse statics selector");
        static ref CANONICAL: Selector =
            Selector::parse("link[rel=\"canonical\"]").expect("failed to parse statics selector");
    }

    let anchors = html
        .select(&ANCHOR)
        .filter_map(|element| element.value().attr("href"))
        .map(|link| (Reason::Ahref, link));
    let canonicals = html
        .select(&CANONICAL)
        .filter_map(|element| element.value().attr("href"))
        .map(|link| (Reason::Canonical, link));

    anchors.chain(canonicals).collect()
}

enum Hit {
    Download {
        html: Html,
        status_code: StatusCode,
    },
    Redirect {
        location: String,
        status_code: StatusCode,
    },
}

pub struct CrawlWorker<WF: WorkerBackendFactory> {
    client: Client<HttpsConnector<HttpConnector>, Body>,
    task_counter: Arc<Counter>,
    profile: Arc<Profile>,
    analyzer: Analyzer,
    boundaries: Boundaries,
    worker_backend_factory: Arc<WF>,
    origins: Arc<Origins>,
}

impl<WF: WorkerBackendFactory> CrawlWorker<WF> {
    pub fn new(
        task_counter: Arc<Counter>,
        profile: Arc<Profile>,
        directives: Arc<Directives>,
        worker_backend_factory: Arc<WF>,
        origins: Arc<Origins>,
    ) -> CrawlWorker<WF> {
        let https = HttpsConnector::new();
        let client = Client::builder().build::<_, hyper::Body>(https);

        CrawlWorker {
            client,
            task_counter,
            profile,
            analyzer: directives.analyzer(),
            boundaries: directives.boundaries(),
            worker_backend_factory,
            origins,
        }
    }

    async fn download<'a>(&'a self, page_url: &'a Url) -> Result<Hit, crate::Error> {
        // Make the request.
        let uri: hyper::Uri = page_url.as_str().parse()?; // uh! patchy
        let builder = Request::get(uri);
        let request = builder
            .header("User-Agent", self.profile.user_agent())
            .header("Accept-Encoding", "gzip, deflate")
            .body(Body::from(""))
            .expect("unreachable");

        // Send the request:
        let response = self.client.request(request).await?;

        // Get status and filter redirects:
        let status_code = response.status();
        let headers = response.headers();

        if status_code.is_redirection() {
            let location_value = headers
                .get(http::header::LOCATION)
                .cloned()
                .ok_or(crate::Error::NoLocationOnRedirect)?;

            // Force UTF-8, dammit!
            let location = String::from_utf8_lossy(location_value.as_bytes()).into_owned();

            return Ok(Hit::Redirect {
                location,
                status_code,
            });
        } else {
            // Get encoding:
            // Force UTF-8, dammit!
            let encoding_value = headers.get(http::header::CONTENT_ENCODING);
            let encoding = encoding_value
                .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned())
                .unwrap_or_else(|| "identity".to_owned());

            // Download contents:
            let mut body = response.into_body();
            let mut content = vec![];

            while let Some(chunk) = body.next().await {
                content.extend(chunk?);
            }

            // Decode contents if necessary:
            content = match encoding.as_str() {
                "identity" => content,
                "gzip" => {
                    let mut decoded = Vec::new();
                    GzipDecoder::new(&content[..])?.read_to_end(&mut decoded)?;
                    decoded
                }
                "deflate" => {
                    let mut decoded = Vec::new();
                    DeflateDecoder::new(&content[..]).read_to_end(&mut decoded)?;
                    decoded
                }
                _ => Err(crate::Error::UnknownContentEncoding(encoding))?,
            };

            // Then, parse the page.
            let html = Html::parse_document(&String::from_utf8_lossy(&content));

            Ok(Hit::Download { html, status_code })
        }
    }

    pub async fn crawl_task(
        self: Rc<Self>,
        worker_backend: Rc<WF::Worker>,
        page_url: Url,
        depth: u16,
    ) -> Result<(), crate::Error> {
        // Get origin:
        let origin = self.origins.get_origin_for_url(&page_url).await;

        // First, wait your turn!
        origin.block().await;

        // Now, download, but be quick.
        match time::timeout(
            Duration::from_secs_f64(self.profile.request_timeout),
            self.download(&page_url),
        )
        .await
        {
            Ok(Ok(Hit::Download { html, status_code })) if status_code.is_success() => {
                // Search HTML:
                let links = tree_search(&html);
                log::debug!("found: {:?}", links);

                // Now, parse and see what stays in and what goes away:
                let filtered_links = if self.boundaries.is_frontier(&page_url) {
                    vec![]
                } else {
                    links
                        .iter()
                        .filter_map(|(reason, raw)| match checked_join(&page_url, raw) {
                            Ok(url) => Some((*reason, self.boundaries.filter_query_params(url))),
                            Err(err) => {
                                log::debug!("at {}: {}", page_url, err);
                                None
                            }
                        })
                        .filter(|(_reason, url)| {
                            self.boundaries.is_allowed(url) && origin.allows(url)
                        })
                        .map(|(reason, url)| (reason, self.boundaries.filter_query_params(url)))
                        .collect::<Vec<_>>()
                };

                // Perform analyses:
                worker_backend
                    .ensure_analyzed(&page_url, self.analyzer.analyze(&page_url, &html))
                    .await
                    .map_err(|err| err.into())?;

                // Mark as explored:
                worker_backend
                    .ensure_explored(&page_url, status_code, depth + 1, filtered_links)
                    .await
                    .map_err(|err| err.into())?;
            }
            Ok(Ok(Hit::Download { status_code, .. })) => {
                worker_backend
                    .ensure_explored(&page_url, status_code, depth + 1, vec![])
                    .await
                    .map_err(|err| err.into())?;
            }
            Ok(Ok(Hit::Redirect {
                location,
                status_code,
            })) => match checked_join(&page_url, &location) {
                Ok(location) => {
                    if !self.boundaries.is_frontier(&page_url)
                        && self.boundaries.is_allowed(&location)
                        && origin.allows(&location)
                    {
                        worker_backend
                            .ensure_explored(
                                &page_url,
                                status_code,
                                depth + 1,
                                vec![(
                                    Reason::Redirect,
                                    self.boundaries.filter_query_params(location),
                                )],
                            )
                            .await
                            .map_err(|err| err.into())?;
                    }
                }
                Err(err) => log::debug!("at {}: {}", page_url, err),
            },
            Ok(Err(error)) => {
                log::warn!("at {} got: {}", page_url, error);
                worker_backend
                    .ensure_error(&page_url)
                    .await
                    .map_err(|err| err.into())?;
            }
            Err(_) => {
                log::warn!("at {}: got timeout", page_url);
                worker_backend
                    .ensure_error(&page_url)
                    .await
                    .map_err(|err| err.into())?;
            }
        }

        Ok(())
    }

    pub fn crawl(self, worker_id: usize) -> (mpsc::Sender<(Url, u16)>, Canceler) {
        let max_tasks_per_worker = self.profile.max_tasks_per_worker;
        let (url_sender, url_stream) = mpsc::channel(max_tasks_per_worker);
        let canceler = spawn_onto_thread(format!("lpz-wrk-{}", worker_id), async move || {
            log::info!("worker started");

            // Spawn all connections:
            let worker_backends = future::join_all(
                (0..self.profile.backends_per_worker).map(|_| self.worker_backend_factory.build()),
            )
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.into())?
            .into_iter()
            .map(|worker_backend| Rc::new(worker_backend))
            .collect::<Vec<_>>();

            // Now, become a reference count:
            let worker_rc = Rc::new(self);

            // NOTE: do not ever, EVER, filter elements of this stream!
            // You risk making the master never finish and that is Big Trouble (tm).
            url_stream
                .enumerate()
                .for_each_concurrent(
                    Some(max_tasks_per_worker),
                    move |(i, (page_url, depth)): (_, (Url, _))| {
                        let task_counter = worker_rc.task_counter.clone();
                        task_counter.register_open();
                        worker_rc
                            .clone()
                            .crawl_task(
                                worker_backends[i % worker_backends.len()].clone(),
                                page_url.clone(),
                                depth,
                            )
                            .map(move |result| {
                                if let Err(error) = result {
                                    task_counter.register_error();
                                    log::warn!("while crawling `{}` got: {}", page_url, error);
                                } else {
                                    task_counter.register_closed();
                                }
                            })
                    },
                )
                .await;

            log::info!("Stream dried. Worker stopping...");

            Ok(()) as Result<_, crate::Error>
        });

        (url_sender, canceler)
    }
}
