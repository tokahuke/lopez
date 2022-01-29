//! The crawler façade.

mod boundaries;
mod counter;
mod downloader;
mod master;
mod origins;
mod parser;
mod reason;
mod robots;
mod server;
mod worker;

pub use self::boundaries::{Boundaries, DummyBoundaries};
pub use self::counter::Counter;
pub use self::downloader::{Downloader, DummyDownloader, SimpleDownloader};
pub use self::master::CrawlMaster;
pub use self::origins::Origins;
pub use self::parser::{DummyParser, Parsed, Parser};
pub use self::reason::Reason;
pub use self::worker::LocalHandlerFactory;
pub(crate) use self::worker::{CrawlWorker, Crawled, ReportType, TestRunReport, WorkerId};

use std::fmt::Debug;
use url::Url;

use crate::Type;

/// Configuration parameters for crawling
pub struct Parameters {
    pub max_hits_per_sec: f64,
    pub quota: usize,
    pub request_timeout: f64,
    pub max_depth: i16,
    pub enable_page_rank: bool,
}

pub trait Configuration: Debug {
    fn downloader(&self) -> Box<dyn Downloader>;
    fn parser(&self) -> Box<dyn Parser>;
    fn boundaries(&self) -> Box<dyn Boundaries>;
    fn seeds(&self) -> Vec<Url>;
    fn analyzes(&self) -> Vec<(String, Type)>;
    fn parameters(&self) -> Parameters;
}

#[derive(Debug)]
pub struct DummyConfiguration;

impl Configuration for DummyConfiguration {
    fn downloader(&self) -> Box<dyn Downloader> {
        Box::new(DummyDownloader)
    }

    fn parser(&self) -> Box<dyn Parser> {
        Box::new(DummyParser)
    }

    fn boundaries(&self) -> Box<dyn Boundaries> {
        Box::new(DummyBoundaries)
    }

    fn seeds(&self) -> Vec<Url> {
        panic!("cannot use DummyConfiguration")
    }

    fn analyzes(&self) -> Vec<(String, Type)> {
        panic!("cannot use DummyConfiguration")
    }

    fn parameters(&self) -> Parameters {
        panic!("cannot use DummyConfiguration")
    }
}

// pub struct DynConfiguration;

// impl Configuration for DynConfiguration {

//     fn downloader(&self) -> Self::Downloader {
//         Box::new(DummyDownloader)
//     }

//     fn parser(&self) -> Self::Parser {
//         Box::new(DummyParser)
//     }

//     fn boundaries(&self) -> Self::Boundaries {
//         Box::new(DummyBoundaries)
//     }

//     fn seeds(&self) -> Vec<Url> {
//         panic!("cannot use DummyConfiguration")
//     }

//     fn analyzes(&self) -> Vec<(String, Type)> {
//         panic!("cannot use DummyConfiguration")
//     }

//     fn parameters(&self) -> Parameters {
//         panic!("cannot use DummyConfiguration")
//     }
// }
