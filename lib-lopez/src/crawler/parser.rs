use lazy_static::lazy_static;
use scraper::{Html, Selector};
use url::Url;

use crate::directives::Analyzer;

use super::Reason;

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

#[allow(unused)]
pub enum Parsed {
    /// This parser does not parse this content.
    NotAccepted,
    /// This parser could parse the content and found this.
    Accepted {
        links: Vec<(Reason, String)>,
        analyses: Vec<(String, serde_json::Value)>,
    },
}

pub trait Parser: 'static + Send {
    fn parse(&self, page_url: &Url, content: &[u8]) -> Parsed;
}

pub struct HtmlParser {
    analyzer: Analyzer,
}

impl HtmlParser {
    pub fn new(analyzer: Analyzer) -> HtmlParser {
        HtmlParser { analyzer }
    }
}

impl Parser for HtmlParser {
    fn parse(&self, page_url: &Url, content: &[u8]) -> Parsed {
        let html = Html::parse_document(&String::from_utf8_lossy(&content));

        // Search HTML:
        let links = tree_search(&html);
        log::debug!("found: {:?}", links);

        let analyses = self.analyzer.analyze(page_url, &html);

        Parsed::Accepted { links, analyses }
    }
}
