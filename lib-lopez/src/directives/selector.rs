//! Because people didn't bother with selector serialization!
//!

use std::fmt::Display;
use std::ops::Deref;
use std::str::FromStr;

#[derive(Debug)]
pub struct Selector {
    selector: scraper::Selector,
    original: String,
}

impl FromStr for Selector {
    type Err = String;
    fn from_str(i: &str) -> Result<Selector, String> {
        Ok(Selector {
            selector: scraper::Selector::parse(i).map_err(|err| format!("{err:?}"))?,
            original: i.to_owned(),
        })
    }
}

impl Deref for Selector {
    type Target = scraper::Selector;
    fn deref(&self) -> &Self::Target {
        &self.selector
    }
}

impl Display for Selector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.original)
    }
}

impl PartialEq for Selector {
    fn eq(&self, other: &Self) -> bool {
        self.selector.eq(&other.selector)
    }
}
