use async_trait::async_trait;
use hyper::{client::HttpConnector, Body, Request};
use hyper_rustls::HttpsConnector;
use serde_json::Value;
use url::Url;

use super::{Downloaded, Downloader};

const EXTRACT_SOURCE: &str = r#"
    const [callback] = arguments;
    const snapshot = () => callback(document.documentElement.outerHTML);

    if (document.readyState == "complete") {
        snapshot()
    } else {
        window.addEventListener("load", snapshot);
    }
"#;

pub struct WebDriverDownloader {
    user_agent: String,
    webdriver_url: String,
    hyper_client: hyper::Client<HttpsConnector<HttpConnector>, Body>,
}

impl WebDriverDownloader {
    pub fn new(webdriver_url: String, user_agent: String) -> WebDriverDownloader {
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_or_http()
            .enable_http1()
            .build();

        let hyper_client = hyper::Client::builder()
            .pool_max_idle_per_host(1) // very stringent, but useful.
            .build(https);

        WebDriverDownloader {
            webdriver_url,
            user_agent,
            hyper_client,
        }
    }

    pub async fn download_source(&self, page_url: &Url) -> Result<String, anyhow::Error> {
        let mut client = fantoccini::ClientBuilder::rustls()
            .connect(&self.webdriver_url)
            .await?;
        client.set_ua(&self.user_agent).await?;
        client.goto(&page_url.to_string()).await?;

        let result = client.execute_async(EXTRACT_SOURCE, vec![]).await?;

        match result {
            Value::String(source) => Ok(source),
            unexpected => Err(anyhow::anyhow!("unexpected value from js: {unexpected}")),
        }
    }
}

#[async_trait]
impl Downloader for WebDriverDownloader {
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
        let response = self.hyper_client.request(request).await?;

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
            let content = self.download_source(page_url).await?.as_bytes().to_owned();
            Ok(Downloaded::Page {
                content,
                status_code,
            })
        } else {
            Ok(Downloaded::BadStatus { status_code })
        }
    }
}
