use async_trait::async_trait;
use futures::StreamExt;
use http::StatusCode;
use hyper::body::HttpBody;
use hyper::{client::HttpConnector, Body, Client, Request};
use hyper_rustls::HttpsConnector;
use libflate::deflate::Decoder as DeflateDecoder;
use libflate::gzip::Decoder as GzipDecoder;
use std::io::Read;
use std::pin::Pin;
use url::Url;

pub enum Downloaded {
    Page {
        content: Vec<u8>,
        status_code: StatusCode,
    },
    BadStatus {
        status_code: StatusCode,
    },
    Redirect {
        location: String,
        status_code: StatusCode,
    },
}

#[async_trait]
pub trait Downloader: 'static + Send + Sync {
    async fn download(&self, page_url: &Url) -> Result<Downloaded, anyhow::Error>;
}

pub struct DummyDownloader;

#[async_trait]
impl Downloader for DummyDownloader {
    async fn download(&self, _page_url: &Url) -> Result<Downloaded, anyhow::Error> {
        panic!("cannot use DummyDownloader")
    }
}

// #[async_trait]
// impl Downloader for Box<dyn Downloader> {
//     async fn download(&self, page_url: &Url) -> Result<Downloaded, anyhow::Error> {
//         self.as_ref().download(page_url).await
//     }
// }

pub struct SimpleDownloader {
    user_agent: String,
    max_body_size: usize,
    client: Client<HttpsConnector<HttpConnector>, Body>,
}

impl SimpleDownloader {
    pub fn new(user_agent: String, max_body_size: usize) -> SimpleDownloader {
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_or_http()
            .enable_http1()
            .build();

        let client = Client::builder()
            .pool_max_idle_per_host(1) // very stringent, but useful.
            .build(https);

        SimpleDownloader {
            user_agent,
            max_body_size,
            client,
        }
    }
}

#[async_trait]
impl Downloader for SimpleDownloader {
    async fn download(&self, page_url: &Url) -> Result<Downloaded, anyhow::Error> {
        // Make the request.
        let uri: hyper::Uri = page_url.as_str().parse()?; // uh! patchy
        let builder = Request::get(uri);
        let request = builder
            .header("User-Agent", &self.user_agent)
            .header("Accept-Encoding", "gzip, deflate")
            // .header("Connection", "Keep-Alive")
            // .header("Keep-Alive", format!("timeout={}, max=100", 10))
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
                .ok_or_else(|| anyhow::anyhow!("no Location header on redirect"))?;

            // Force UTF-8, dammit!
            let location = String::from_utf8_lossy(location_value.as_bytes()).into_owned();

            Ok(Downloaded::Redirect {
                location,
                status_code,
            })
        } else if status_code.is_success() {
            // Get encoding:
            // Force UTF-8, dammit!
            let encoding_value = headers.get(http::header::CONTENT_ENCODING);
            let encoding = encoding_value
                .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned())
                .unwrap_or_else(|| "identity".to_owned());

            // Download contents:
            let mut body = response.into_body();
            // TEMPORARY HACK: due to a temporary incompatiblity between Futures and Hyper.
            let mut stream =
                futures::stream::poll_fn(move |ctx| Pin::new(&mut body).poll_data(ctx));
            let mut content = vec![];

            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;

                if content.len() + chunk.len() > self.max_body_size {
                    log::debug!("at {}: Got very big body. Truncating...", page_url);

                    let truncated = &chunk[..self.max_body_size - content.len()];
                    // self.task_counter.add_to_download_count(truncated.len());
                    content.extend(truncated);

                    break;
                }

                // self.task_counter.add_to_download_count(chunk.len());
                content.extend(chunk);
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
                _ => return Err(anyhow::anyhow!("unknown content encoding {encoding}")),
            };

            Ok(Downloaded::Page {
                content,
                status_code,
            })
        } else {
            Ok(Downloaded::BadStatus { status_code })
        }
    }
}
