use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{self, Delay, Duration};
use url::{Origin as UrlOrigin, Url};

use crate::robots::{get_robots, RobotExclusion};

// #[test]
// fn link_test() {
//     let base_url: Url = "https://startse.com".parse().unwrap();
//     assert_eq!(
//         checked_join(&base_url, "https://www.startse.com"),
//         Ok("https://www.startse.com".parse().unwrap())
//     );
// }

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
        *delay = time::delay_for(self.crawl_delay);
    }

    // /// Joins the base URL for this origin with the raw string provided,
    // /// returning on first error. If raw is already absolute (i.e., has an origin),
    // /// only parsing is done.
    // pub fn join(&self, raw: &str) -> Result<Url, crate::Error> {
    //     if let Some(base_url) = &self.base_url {
    //         checked_join(base_url, raw)
    //     } else {
    //         Err(crate::Error::Custom(format!(
    //             "Joining `{}` from opaque origin",
    //             raw
    //         )))
    //     }
    // }

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
    origins: RwLock<HashMap<UrlOrigin, Arc<Origin>>>,
}

impl Origins {
    pub fn new(default_requests_per_sec: f64, user_agent: String) -> Origins {
        Origins {
            default_requests_per_sec,
            user_agent,
            origins: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get_origin_for_url(&self, url: &Url) -> Arc<Origin> {
        let url_origin = url.origin();

        let read_guard = self.origins.read().await;
        let origin = read_guard.get(&url_origin);

        if let Some(origin) = origin {
            return origin.clone();
        } else {
            drop(read_guard); // prevents deadlock.
            let mut write_guard = self.origins.write().await;

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
            let read_guard = self.origins.read().await;
            let origin = read_guard
                .get(&url_origin)
                .expect("origin should always exist by this point");

            return origin.clone();
        }
    }
}
