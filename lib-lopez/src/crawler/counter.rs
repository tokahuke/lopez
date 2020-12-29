use std::fmt::{self, Display};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::cli::Profile;

#[derive(Debug, Default)]
pub struct Counter {
    /// All tasks in progress.
    open_count: AtomicUsize,
    /// All tasks finished, no matter the outcome.
    closed_count: AtomicUsize,
    /// All tasks finished with error.
    error_count: AtomicUsize,
    active_count: AtomicUsize,
    download_count: AtomicUsize,
}

impl Counter {
    pub fn register_open(&self) {
        self.open_count.fetch_add(1, Ordering::Release); // gulp! atomics.
    }

    pub fn register_closed(&self) {
        self.closed_count.fetch_add(1, Ordering::Release); // gulp! atomics.
    }

    pub fn register_error(&self) {
        self.error_count.fetch_add(1, Ordering::Release); // gulp! atomics.
    }

    /// closed
    pub fn n_closed(&self) -> usize {
        self.closed_count.load(Ordering::Acquire)
    }

    /// error
    pub fn n_error(&self) -> usize {
        self.error_count.load(Ordering::Acquire)
    }

    pub fn inc_active(&self) {
        self.active_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec_active(&self) {
        self.active_count.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn n_active(&self) -> usize {
        self.active_count.load(Ordering::Acquire)
    }

    pub fn add_to_download_count(&self, amount: usize) {
        self.download_count.fetch_add(amount, Ordering::Relaxed);
    }

    pub fn n_downloaded(&self) -> usize {
        self.download_count.load(Ordering::Relaxed)
    }
}

/// Logs stats from time to time.
pub async fn log_stats(
    counter: Arc<Counter>,
    already_done: usize,
    effective_quota: usize,
    profile: Arc<Profile>,
) {
    if profile.do_not_log_stats {
        log::info!("Not logging stats. Set `LOG_STATS_EVERY_SECS` to see them.");
    } else {
        let log_interval = profile.log_stats_every_secs;
        log::info!("Logging stats every {} seconds.", log_interval);

        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs_f64(log_interval));
        let mut tracker = StatsTracker::new(already_done, effective_quota, counter, log_interval);

        loop {
            interval.tick().await;
            tracker.tick();
            log::info!("{}", tracker.get_stats());
        }
    }
}

struct StatsTracker {
    last: Option<Stats>,
    already_done: usize,
    quota: usize,
    counter: Arc<Counter>,
    delta_t: f64,
}

impl StatsTracker {
    pub fn new(
        already_done: usize,
        quota: usize,
        counter: Arc<Counter>,
        delta_t: f64,
    ) -> StatsTracker {
        StatsTracker {
            last: None,
            already_done,
            counter,
            quota,
            delta_t,
        }
    }

    pub fn tick(&mut self) {
        let recently_done = self.already_done + self.counter.n_closed()
        - self.counter.n_error()
        - self
            .last
            .as_ref()
            .map(|last| last.n_done.0)
            .unwrap_or(self.already_done);
        
        let stats = Stats {
            n_active: self.counter.n_active(),
            n_done: FromTotal(
                self.already_done + self.counter.n_closed() - self.counter.n_error(),
                self.quota as usize,
            ),
            n_errors: FromTotal(
                self.counter.error_count.load(Ordering::Acquire),
                self.quota as usize,
            ),
            hit_rate: Human(
                (self.already_done + self.counter.n_closed()
                    - self.counter.n_error()
                    - self
                        .last
                        .as_ref()
                        .map(|last| last.n_done.0)
                        .unwrap_or(self.already_done)) as f64
                    / self.delta_t,
                "/s",
            ),
            recently_done,
            downloaded: Human(
                self.counter.download_count.load(Ordering::Relaxed) as f64,
                "B",
            ),
            download_speed: Human(
                (self.counter.n_downloaded() as f64
                    - self
                        .last
                        .as_ref()
                        .map(|last| last.downloaded.0)
                        .unwrap_or_default())
                    / self.delta_t,
                "B/s",
            ),
        };

        self.last = Some(stats);
    }

    fn get_stats(&self) -> &Stats {
        self.last
            .as_ref()
            .expect("can only be called after `StatsTracker::tick`")
    }
}

#[derive(Clone, Copy)]
pub struct Human(f64, &'static str);

impl Display for Human {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Human(quantity, unit) = self;

        if quantity.abs() < 1e3 {
            write!(f, "{:.2}{}", quantity, unit)
        } else if quantity.abs() < 1e6 {
            write!(f, "{:.2}k{}", quantity / 1e3, unit)
        } else if quantity.abs() < 1e9 {
            write!(f, "{:.2}M{}", quantity / 1e6, unit)
        } else if quantity.abs() < 1e12 {
            write!(f, "{:.2}G{}", quantity / 1e9, unit)
        } else {
            write!(f, "{:.2}T{}", quantity / 1e12, unit)
        }
    }
}

#[derive(Clone, Copy)]
pub struct FromTotal(usize, usize);

impl Display for FromTotal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}/{} (={:.2}%)",
            self.0,
            self.1,
            self.0 as f64 / self.1 as f64 * 100.0
        )
    }
}

pub struct Stats {
    n_active: usize,
    n_done: FromTotal,
    n_errors: FromTotal,
    hit_rate: Human,
    recently_done: usize,
    downloaded: Human,
    download_speed: Human,
}

impl Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Crawl statistics:")?;
        writeln!(f, "\tn. active = {}", self.n_active)?;
        writeln!(f, "\tn. done = {}", self.n_done)?;
        writeln!(f, "\tn. errors = {}", self.n_errors)?;
        writeln!(f, "\trecently done = {}", self.recently_done)?;
        writeln!(f, "\thit rate = {}", self.hit_rate)?;
        writeln!(f, "\tdownloaded = {}", self.downloaded)?;
        writeln!(f, "\tdownload speed = {}", self.download_speed)?;

        Ok(())
    }
}

// struct Smoother {
//     last_state: f64,
//     last_variance: f64,
//     state_variance: f64,
//     output_variance: f64,
// }

// fn par(a: f64, b: f64) -> f64 {
//     a * b / (a + b)
// }

// impl Smoother {
//     fn smooth(&mut self, input: f64) -> f64 {
//         let variance = self.last_variance + self.state_variance;
//         let new_state = self.last_state
//             + variance / (variance + self.output_variance) * (input - self.last_state);
//         let new_variance = par(variance, self.output_variance);

//         self.last_state = new_state;
//         self.last_variance = new_variance;

//         new_state
//     }
// }
