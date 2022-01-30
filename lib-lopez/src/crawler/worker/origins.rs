use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;
use tokio::time::{self, Duration};
use url::{Origin as UrlOrigin, Url};

use crate::crawler::robots::{get_robots, RobotExclusion};
use crate::crawler::Downloader;

#[derive(Debug)]
pub struct Origin {
    // _base_url: Option<Url>,
    exclusion: Option<RobotExclusion>,
    crawl_delay: Duration,
    last_instant: RefCell<Instant>,
}

impl Origin {
    async fn load(
        downloader: &dyn Downloader,
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

        Origin {
            // _base_url: base_url,
            exclusion,
            crawl_delay,
            last_instant: RefCell::new(Instant::now()),
        }
    }

    pub async fn block(&self) {
        let last_instant = *self.last_instant.borrow();
        let this_instant = Instant::now();

        if last_instant > this_instant {
            // Will have to wait.
            // Increment last instant.
            *self.last_instant.borrow_mut() = last_instant + self.crawl_delay;

            time::sleep(last_instant - this_instant).await; // no Ref's here!
        } else {
            *self.last_instant.borrow_mut() = this_instant + self.crawl_delay;
        }
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
    origins: RefCell<HashMap<UrlOrigin, Rc<Origin>>>,
}

impl Origins {
    pub fn new(default_requests_per_sec: f64) -> Origins {
        Origins {
            default_requests_per_sec,
            origins: RefCell::new(HashMap::new()),
        }
    }

    pub async fn get_origin_for_url(&self, downloader: &dyn Downloader, url: &Url) -> Rc<Origin> {
        // RefCell + Async quick tip: Ref cannot survive .await breakpoint (otherwise, panic!)

        let url_origin = url.origin();

        let should_dowload = !self.origins.borrow().contains_key(&url_origin);

        if should_dowload {
            // Do the downloady thingy:
            let origin = Origin::load(
                downloader,
                url_origin.clone(),
                self.default_requests_per_sec,
            )
            .await; // no Ref's here!

            self.origins
                .borrow_mut()
                .insert(url_origin.clone(), Rc::new(origin));
        }

        self.origins.borrow()[&url_origin].clone()
    }
}
