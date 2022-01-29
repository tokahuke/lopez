use futures::channel::mpsc;
use futures::future;
use futures::prelude::*;
use hyper::StatusCode;
use serde_derive::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};
use url::{ParseError, Url};

use crate::backend::{WorkerBackend, WorkerBackendFactory};
use crate::cancel::{spawn_onto_thread, Canceler};
use crate::cli::Profile;

use super::boundaries::Boundaries;
use super::downloader::{Downloaded, Downloader};
use super::origins::Origins;
use super::parser::{Parsed, Parser};
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

#[derive(Debug)]
pub(crate) enum Crawled {
    Success {
        status_code: StatusCode,
        links: Vec<(Reason, Url)>,
        analyses: Vec<(String, serde_json::Value)>,
    },
    BadStatus {
        status_code: StatusCode,
    },
    Redirect {
        status_code: StatusCode,
        location: String,
    },
    Error(crate::Error),
    TimedOut,
}

#[derive(Debug, Serialize)]
pub struct TestRunReport {
    pub(crate) actual_url: Url,
    pub(crate) report: ReportType,
}

#[derive(Debug, Serialize)]
pub(crate) enum ReportType {
    DisallowedByDirectives,
    DisallowedByOrigin,
    Crawled(Crawled),
}

pub struct CrawlWorker<D: Downloader, P: Parser, B: Boundaries, WF: WorkerBackendFactory> {
    downloader: D,
    parser: P,
    boundaries: B,
    task_counter: Arc<Counter>,
    profile: Arc<Profile>,
    worker_backend_factory: Arc<Mutex<WF>>,
    origins: Arc<Origins>,
    request_timeout: f64,
}

impl<D: Downloader, P: Parser, B: Boundaries, WF: WorkerBackendFactory> CrawlWorker<D, P, B, WF> {
    pub fn new(
        downloader: D,
        parser: P,
        boundaries: B,
        worker_backend_factory: Arc<Mutex<WF>>,
        task_counter: Arc<Counter>,
        profile: Arc<Profile>,
        origins: Arc<Origins>,
        request_timeout: f64,
    ) -> CrawlWorker<D, P, B, WF> {
        CrawlWorker {
            downloader,
            task_counter,
            profile,
            parser,
            boundaries,
            worker_backend_factory,
            origins,
            request_timeout,
        }
    }

    async fn crawl(&self, page_url: &Url) -> Crawled {
        // Now, download, but be quick.
        let crawl = time::timeout(
            Duration::from_secs_f64(self.request_timeout),
            self.downloader.download(page_url),
        );

        let crawled = match crawl.await {
            Ok(Ok(Downloaded::Page {
                content,
                status_code,
            })) => match self.parser.parse(page_url, &content) {
                Parsed::Accepted { links, analyses } => Crawled::Success {
                    status_code,
                    links: self.boundaries.clean_links(page_url, &links),
                    analyses,
                },
                Parsed::NotAccepted => Crawled::Success {
                    status_code,
                    links: vec![],
                    analyses: vec![],
                },
            },
            Ok(Ok(Downloaded::BadStatus { status_code, .. })) => Crawled::BadStatus { status_code },
            Ok(Ok(Downloaded::Redirect {
                location,
                status_code,
            })) => Crawled::Redirect {
                status_code,
                location,
            },
            Ok(Err(error)) => Crawled::Error(error),
            Err(_) => Crawled::TimedOut,
        };

        crawled
    }

    async fn store(
        &self,
        worker_backend: &WF::Worker,
        page_url: &Url,
        depth: u16,
        crawled: Crawled,
    ) -> Result<(), crate::Error> {
        match crawled {
            Crawled::Success {
                status_code,
                links,
                analyses,
            } => {
                // Perform analyses:
                worker_backend
                    .ensure_analyzed(page_url, analyses)
                    .await
                    .map_err(|err| err.into())?;

                // Mark as explored:
                worker_backend
                    .ensure_explored(page_url, status_code, depth + 1, links)
                    .await
                    .map_err(|err| err.into())?;
            }
            Crawled::BadStatus { status_code } => {
                worker_backend
                    .ensure_explored(page_url, status_code, depth + 1, vec![])
                    .await
                    .map_err(|err| err.into())?;
            }
            Crawled::Redirect {
                status_code,
                location,
            } => match checked_join(page_url, &location) {
                Ok(location) => {
                    if !self.boundaries.is_frontier(page_url)
                        && self.boundaries.is_allowed(&location)
                    {
                        worker_backend
                            .ensure_explored(
                                page_url,
                                status_code,
                                depth + 1,
                                vec![(
                                    Reason::Redirect,
                                    self.boundaries.clean_query_params(location),
                                )],
                            )
                            .await
                            .map_err(|err| err.into())?;
                    }
                }
                Err(err) => log::debug!("at {}: {}", page_url, err),
            },
            Crawled::Error(error) => {
                log::debug!("at {} got: {}", page_url, error);
                worker_backend
                    .ensure_error(page_url)
                    .await
                    .map_err(|err| err.into())?;

                // This needs to be the last thing (because of `?`).
                self.task_counter.register_error();
            }
            Crawled::TimedOut => {
                log::debug!("at {}: got timeout", page_url);
                worker_backend
                    .ensure_error(page_url)
                    .await
                    .map_err(|err| err.into())?;

                // This needs to be the last thing (because of `?`).
                self.task_counter.register_error();
            }
        }

        Ok(())
    }

    pub async fn crawl_task(
        &self,
        worker_backend: &WF::Worker,
        page_url: &Url,
        depth: u16,
    ) -> Result<(), crate::Error> {
        // Get origin:
        let origin = self
            .origins
            .get_origin_for_url(&self.downloader, &page_url)
            .await;

        // Do not do anything if disallowed:
        if !origin.allows(page_url) {
            return Ok(());
        }

        // First, wait your turn!
        origin.block().await;

        // Then, you crawl:
        self.task_counter.inc_active();
        let crawled = self.crawl(page_url).await;
        self.task_counter.dec_active();

        // Finally, you store!
        self.store(worker_backend, page_url, depth, crawled).await?;

        Ok(())
    }

    pub fn run(self, worker_id: usize) -> (mpsc::Sender<(Url, u16)>, Canceler) {
        let max_tasks_per_worker = self.profile.max_tasks_per_worker;
        let (url_sender, url_stream) = mpsc::channel(2 * max_tasks_per_worker);
        let canceler = spawn_onto_thread(format!("lpz-wrk-{}", worker_id), move || async move {
            log::info!("worker started");

            // Spawn all connections:
            let worker_backends = future::join_all(
                (0..self.profile.backends_per_worker)
                    .map(|_| async { self.worker_backend_factory.lock().await.build().await }),
            )
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.into())?
            .into_iter()
            .collect::<Vec<_>>();

            // NOTE: do not ever, EVER, filter elements of this stream!
            // You risk making the master never finish and that is Big Trouble (tm).
            let worker_backends = &worker_backends;
            let worker_ref = &self; // apeasses borrow checker.
            url_stream
                .enumerate()
                .for_each_concurrent(
                    Some(max_tasks_per_worker),
                    move |(i, (page_url, depth)): (_, (Url, _))| async move {
                        // Register open task:
                        worker_ref.task_counter.register_open();

                        // Run the task:
                        let result = worker_ref
                            .crawl_task(
                                &worker_backends[i % worker_backends.len()],
                                &page_url,
                                depth,
                            )
                            .await;

                        // Register close, no matter the status.
                        worker_ref.task_counter.register_closed();

                        // Now, analyze results:
                        if let Err(error) = result {
                            worker_ref.task_counter.register_error();
                            log::debug!("while crawling `{}` got: {}", page_url, error);
                        }
                    },
                )
                .await;

            log::info!("Stream dried. Worker stopping...");

            Ok(()) as Result<_, crate::Error>
        });

        (url_sender, canceler)
    }

    pub async fn test_url(self, url: Url) -> TestRunReport {
        let actual_url = self.boundaries.clean_query_params(url);

        if !self.boundaries.is_allowed(&actual_url) {
            return TestRunReport {
                actual_url,
                report: ReportType::DisallowedByDirectives,
            };
        }

        // Get origin:
        let origin = self
            .origins
            .get_origin_for_url(&self.downloader, &actual_url)
            .await;

        // Do not do anything if disallowed:
        if !origin.allows(&actual_url) {
            return TestRunReport {
                actual_url,
                report: ReportType::DisallowedByOrigin,
            };
        }

        let crawled = self.crawl(&actual_url).await;

        TestRunReport {
            actual_url,
            report: ReportType::Crawled(crawled),
        }
    }
}
