use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{self, Delay, Duration};
use url::{Origin as UrlOrigin, Url};

use crate::robots::{get_robots, RobotExclusion};

pub struct Origin {
    _base_url: Option<Url>,
    exclusion: Option<RobotExclusion>,
    crawl_delay: Duration,
    block_until: Mutex<Delay>,
}

impl Origin {
    async fn load(
        url_origin: UrlOrigin,
        default_requests_per_sec: f64,
        user_agent: &str,
    ) -> Origin {
        let base_url = url_origin.ascii_serialization().parse::<Url>().ok();
        let exclusion = if let Some(base_url) = base_url.as_ref() {
            get_robots(base_url, user_agent)
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
        let block_until = Mutex::new(time::delay_for(crawl_delay));

        Origin {
            _base_url: base_url,
            exclusion,
            crawl_delay,
            block_until,
        }
    }

    pub async fn block(&self) {
        // If you block, it is because someone else is waiting, and so should you.
        let mut delay = self.block_until.lock().await;
        (&mut *delay).await;
        *delay = time::delay_until(time::Instant::now() + self.crawl_delay);
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
    user_agent: String,
    origins: Vec<RwLock<HashMap<UrlOrigin, Arc<Origin>>>>,
}

const SEGMENT_SIZE: usize = 32;

impl Origins {
    pub fn new(default_requests_per_sec: f64, user_agent: String) -> Origins {
        Origins {
            default_requests_per_sec,
            user_agent,
            origins: (0..SEGMENT_SIZE).map(|_| RwLock::new(HashMap::new())).collect::<Vec<_>>(),
        }
    }

    pub async fn get_origin_for_url(&self, url: &Url) -> Arc<Origin> {
        let url_origin = url.origin();
        let origins = &self.origins[crate::hash(&url_origin) as usize % SEGMENT_SIZE];

        let read_guard = origins.read().await;
        let origin = read_guard.get(&url_origin);

        if let Some(origin) = origin {
            return origin.clone();
        } else {
            drop(read_guard); // prevents deadlock.
            let mut write_guard = origins.write().await;

            // Recheck condition:
            if !write_guard.contains_key(&url_origin) {
                let origin = Origin::load(
                    url_origin.clone(),
                    self.default_requests_per_sec,
                    &self.user_agent,
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

            return origin.clone();
        }
    }
}
