use serde_derive::{Deserialize, Serialize};
use std::fmt::{self, Display};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::cli::Profile;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Counter {
    /// All tasks in progress.
    open_count: AtomicUsize,
    /// All tasks finished, no matter the outcome.
    closed_count: AtomicUsize,
    /// All tasks finished with error.
    error_count: AtomicUsize,
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
        self.register_closed();
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

    pub fn n_active(&self) -> usize {
        self.open_count.load(Ordering::Acquire) - self.closed_count.load(Ordering::Acquire)
    }

    // pub fn add_to_download_count(&self, amount: usize) {
    //     self.download_count.fetch_add(amount, Ordering::Relaxed);
    // }

    pub fn n_downloaded(&self) -> usize {
        self.download_count.load(Ordering::Relaxed)
    }

    pub fn merge(&self, other: &Self) -> Self {
        Counter {
            open_count: AtomicUsize::new(
                self.open_count.load(Ordering::Acquire) + other.open_count.load(Ordering::Acquire),
            ),
            closed_count: AtomicUsize::new(
                self.closed_count.load(Ordering::Acquire)
                    + other.closed_count.load(Ordering::Acquire),
            ),
            error_count: AtomicUsize::new(
                self.error_count.load(Ordering::Acquire)
                    + other.error_count.load(Ordering::Acquire),
            ),
            download_count: AtomicUsize::new(
                self.download_count.load(Ordering::Acquire)
                    + other.download_count.load(Ordering::Acquire),
            ),
        }
    }
}
