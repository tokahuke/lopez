//! From https://github.com/sfackler/rust-log-panics/

use backtrace::Backtrace;
use std::{fmt, panic, thread};

struct Shim(Backtrace);

impl fmt::Debug for Shim {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "\n{:?}", self.0)
    }
}

/// Sets rust to log panics in log::error.
pub fn log_panics() {
    panic::set_hook(Box::new(|info| {
        let backtrace = Backtrace::new();

        let thread = thread::current();
        let thread = thread.name().unwrap_or("unnamed");

        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &**s,
                None => "Box<Any>",
            },
        };

        match info.location() {
            Some(location) => {
                log::error!(
                    target: "panic", "thread '{}' panicked at '{}': {}:{}{:?}",
                    thread,
                    msg,
                    location.file(),
                    location.line(),
                    Shim(backtrace)
                );
            }
            None => log::error!(
                target: "panic",
                "thread '{}' panicked at '{}'{:?}",
                thread,
                msg,
                Shim(backtrace)
            ),
        }
    }));
}
