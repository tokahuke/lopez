use cached::stores::SizedCache;
use cached::Cached;
use std::sync::Arc;
use tokio::sync::Mutex;
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
    origins: Vec<Mutex<SizedCache<UrlOrigin, Arc<Origin>>>>,
}

const SEGMENT_SIZE: usize = 32;

impl Origins {
    pub fn new(default_requests_per_sec: f64, user_agent: String) -> Origins {
        Origins {
            default_requests_per_sec,
            user_agent,
            origins: (0..SEGMENT_SIZE)
                .map(|_| Mutex::new(SizedCache::with_size(10)))
                .collect::<Vec<_>>(),
        }
    }

    pub async fn get_origin_for_url(&self, url: &Url) -> Arc<Origin> {
        let url_origin = url.origin();
        let origins = &self.origins[crate::hash(&url_origin) as usize % SEGMENT_SIZE];

        let mut guard = origins.lock().await;

        if let Some(origin) = guard.cache_get(&url_origin) {
            return origin.clone();
        } else {
            let origin = Arc::new(
                Origin::load(
                    url_origin.clone(),
                    self.default_requests_per_sec,
                    &self.user_agent,
                )
                .await,
            );

            guard.cache_set(url_origin.clone(), origin.clone());

            return origin;
        }
    }
}
