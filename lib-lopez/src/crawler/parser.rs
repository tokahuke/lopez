use url::Url;

use super::Reason;

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

pub struct DummyParser;

impl Parser for DummyParser {
    fn parse(&self, _page_url: &Url, _content: &[u8]) -> Parsed {
        panic!("cannot use DummyParser")
    }
}
