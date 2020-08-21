//! Remember: idempotent atomic operations are the key.

#![feature(never_type, async_closure)]

mod crawler;
#[macro_use]
pub mod backend;
mod cancel;
mod directives;
mod env;
mod error;
mod hash;
mod origins;
mod page_rank;
mod panic;
mod profile;
mod robots;

pub use directives::Directives;
pub use crawler::start;
pub use env::prepare_fs;
pub use error::Error;
pub use hash::hash;
pub use profile::Profile;