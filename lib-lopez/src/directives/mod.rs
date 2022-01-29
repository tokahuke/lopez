mod directives;
mod error;
mod expressions;
mod extractor;
mod parse;
mod parse_common;
mod parse_utils;
mod variable;

// Note on where to put parseable items: if it has an impl-block, it goes
// Somewhere Else©; if it does not have an impl-block, it stays in `parse`.

pub use self::directives::Directives;
pub use self::error::Error;

use lazy_static::lazy_static;
use scraper::{Html, Selector};
use serde_json::Value;
use url::Url;

use crate::crawler::{
    Boundaries, Configuration, Downloader, Parameters, Parsed, Parser, Reason, SimpleDownloader,
};
use crate::Type;

use self::directives::{Analyzer, Boundaries as DirectiveBoundaries};
use self::extractor::Extractor;
use self::variable::{SetVariables, Variable};

/// Finds all "hrefs" in an HTML and run all analyses.
fn tree_search(html: &Html) -> Vec<(Reason, String)> {
    lazy_static! {
        static ref ANCHOR: Selector =
            Selector::parse("a").expect("failed to parse statics selector");
        static ref CANONICAL: Selector =
            Selector::parse("link[rel=\"canonical\"]").expect("failed to parse statics selector");
    }

    let anchors = html
        .select(&ANCHOR)
        .filter_map(|element| element.value().attr("href"))
        .map(|link| (Reason::Ahref, link.to_owned()));
    let canonicals = html
        .select(&CANONICAL)
        .filter_map(|element| element.value().attr("href"))
        .map(|link| (Reason::Canonical, link.to_owned()));

    anchors.chain(canonicals).collect()
}

impl Parser for Analyzer {
    fn parse(&self, page_url: &Url, content: &[u8]) -> Parsed {
        let html = Html::parse_document(&String::from_utf8_lossy(&content));

        // Search HTML:
        let links = tree_search(&html);
        log::debug!("found: {:?}", links);

        let analyses = self.analyze(page_url, &html);

        Parsed::Accepted { links, analyses }
    }
}

impl Boundaries for DirectiveBoundaries {
    fn is_allowed(&self, url: &Url) -> bool {
        self.is_allowed(url)
    }

    fn is_frontier(&self, url: &Url) -> bool {
        self.is_frontier(url)
    }

    fn clean_query_params(&self, url: Url) -> Url {
        self.filter_query_params(url)
    }
}

#[derive(Debug)]
pub struct DirectivesConfiguration {
    directives: Directives,
    variables: SetVariables,
}

impl DirectivesConfiguration {
    pub fn new(directives: Directives) -> DirectivesConfiguration {
        DirectivesConfiguration {
            variables: directives.set_variables(),
            directives,
        }
    }
}

impl Configuration for DirectivesConfiguration {
    fn downloader(&self) -> Box<dyn Downloader> {
        Box::new(SimpleDownloader::new(
            self.variables
                .get_as_str(Variable::UserAgent)
                .expect("bad val")
                .to_owned(),
            self.variables
                .get_as_u64(Variable::MaxBodySize)
                .expect("bad val") as usize,
        ))
    }

    fn parser(&self) -> Box<dyn Parser> {
        Box::new(self.directives.analyzer())
    }

    fn boundaries(&self) -> Box<dyn Boundaries> {
        Box::new(self.directives.boundaries())
    }

    fn seeds(&self) -> Vec<Url> {
        self.directives.seeds()
    }

    fn analyzes(&self) -> Vec<(String, Type)> {
        self.directives.rules()
    }

    fn parameters(&self) -> Parameters {
        Parameters {
            max_hits_per_sec: self
                .variables
                .get_as_positive_f64(Variable::MaxHitsPerSec)
                .expect("bad val"),
            quota: self.variables.get_as_u64(Variable::Quota).expect("bad val") as usize,
            request_timeout: self
                .variables
                .get_as_positive_f64(Variable::RequestTimeout)
                .expect("bad val"),
            max_depth: self
                .variables
                .get_as_u64(Variable::MaxDepth)
                .expect("bad val") as i16,
            enable_page_rank: self
                .variables
                .get_as_bool(Variable::EnablePageRank)
                .expect("bad val"),
        }
    }
}
