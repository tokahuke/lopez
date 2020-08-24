use std::fmt::{self, Display};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::profile::Profile;

#[derive(Debug, Default)]
pub struct Counter {
    open_count: AtomicUsize,
    closed_count: AtomicUsize,
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

    /// closed + error
    pub fn n_done(&self) -> usize {
        self.closed_count.load(Ordering::Acquire) + self.error_count.load(Ordering::Acquire)
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

    pub fn stats(&self, last: Option<&Stats>, profile: &Profile, delta_t: f64) -> Stats {
        Stats {
            n_active: self.n_active(),
            n_done: FromTotal(self.n_done(), profile.quota as usize),
            n_errors: FromTotal(
                self.error_count.load(Ordering::Acquire),
                profile.quota as usize,
            ),
            downloaded: Human(self.download_count.load(Ordering::Relaxed) as f64, "B"),
            download_speed: Human(
                (self.n_downloaded() as f64
                    - last.map(|last| last.downloaded.0).unwrap_or_default()) / delta_t,
                "B/s",
            ),
        }
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
    downloaded: Human,
    download_speed: Human,
}

impl Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Crawl statistics:")?;
        writeln!(f, "\tn. active = {}", self.n_active)?;
        writeln!(f, "\tn. done = {}", self.n_done)?;
        writeln!(f, "\tn. errors = {}", self.n_errors)?;
        writeln!(f, "\tdownloaded = {}", self.downloaded)?;
        writeln!(f, "\tdownload speed = {}", self.download_speed)?;

        Ok(())
    }
}
