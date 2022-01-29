use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{self, Duration, Interval};
use url::{Origin as UrlOrigin, Url};

use super::downloader::Downloader;
use super::robots::{get_robots, RobotExclusion};

pub struct Origin {
    // _base_url: Option<Url>,
    exclusion: Option<RobotExclusion>,
    block_until: Mutex<Interval>,
}

impl Origin {
    async fn load<D: Downloader>(
        downloader: &D,
        url_origin: UrlOrigin,
        default_requests_per_sec: f64,
    ) -> Origin {
        let base_url = url_origin.ascii_serialization().parse::<Url>().ok();
        let exclusion = if let Some(base_url) = base_url {
            get_robots(downloader, base_url)
                .await
                .ok()
                .flatten()
                .map(|robots| RobotExclusion::new(&robots))
        } else {
            log::debug!("found opaque origin");
            None
        };

        let crawl_delay_secs = f64::max(
            1. / default_requests_per_sec,
            exclusion
                .as_ref()
                .and_then(|excl| excl.crawl_delay())
                .unwrap_or(0.),
        );
        let crawl_delay = Duration::from_millis((crawl_delay_secs * 1e3) as u64);
        let block_until = Mutex::new(time::interval(crawl_delay));

        Origin {
            // _base_url: base_url,
            exclusion,
            block_until,
        }
    }

    pub async fn block(&self) {
        // If you block, it is because someone else is waiting, and so should you.
        let mut delay = self.block_until.lock().await;
        delay.tick().await;
    }

    /// Warning: this assumes that `url` is of the same origin.
    pub fn allows(&self, url: &Url) -> bool {
        self.exclusion
            .as_ref()
            .map(|exclusion| exclusion.allows(url))
            .unwrap_or(true)
    }
}

pub struct Origins {
    default_requests_per_sec: f64,
    origins: Vec<RwLock<HashMap<UrlOrigin, Arc<Origin>>>>,
}

const SEGMENT_SIZE: usize = 32;

impl Origins {
    pub fn new(default_requests_per_sec: f64) -> Origins {
        Origins {
            default_requests_per_sec,
            origins: (0..SEGMENT_SIZE)
                .map(|_| RwLock::new(HashMap::new()))
                .collect::<Vec<_>>(),
        }
    }

    pub async fn get_origin_for_url<D: Downloader>(
        &self,
        downloader: &D,
        url: &Url,
    ) -> Arc<Origin> {
        let url_origin = url.origin();
        let origins = &self.origins[crate::hash(&url_origin) as usize % SEGMENT_SIZE];

        let read_guard = origins.read().await;
        let origin = read_guard.get(&url_origin);

        if let Some(origin) = origin {
            origin.clone()
        } else {
            drop(read_guard); // prevents deadlock.
            let mut write_guard = origins.write().await;

            // Recheck condition:
            if !write_guard.contains_key(&url_origin) {
                let origin = Origin::load(
                    downloader,
                    url_origin.clone(),
                    self.default_requests_per_sec,
                )
                .await;

                write_guard.insert(url_origin.clone(), Arc::new(origin));
            }

            drop(write_guard); // prevents deadlock.

            // Now, do it again (no easy recursion within async fn yet...)
            let read_guard = origins.read().await;
            let origin = read_guard
                .get(&url_origin)
                .expect("origin should always exist by this point");

            origin.clone()
        }
    }
}
