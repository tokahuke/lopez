use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Default)]
pub struct Counter {
    open_count: AtomicUsize,
    closed_count: AtomicUsize,
    error_count: AtomicUsize,
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
}
