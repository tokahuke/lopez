use url::Url;

use crate::directives::Boundaries as ParsedBoundaries;

use super::Reason;

/// Performs a checked join, with all the common problems accounted for.
fn checked_join(base_url: &Url, raw: &str) -> Result<Url, crate::Error> {
    // Parse the thing.
    let maybe_url = raw.parse().or_else(|err| {
        if err == url::ParseError::RelativeUrlWithoutBase {
            base_url.join(&raw)
        } else {
            Err(err)
        }
    });

    let url = if let Ok(url) = maybe_url {
        url
    } else {
        return Err(crate::Error::Custom(format!("bad link: {}", raw)));
    };

    // Get rid of those pesky "#" section references and of weird empty strings:
    if raw.is_empty() || raw.starts_with('#') {
        return Err(crate::Error::Custom(format!("bad link: {}", raw)));
    }

    // Now, make sure this is really HTTP (not mail, ftp and what not):
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(crate::Error::Custom(format!("unaccepted scheme: {}", raw)));
    }

    // Check if internal or external.
    if url.domain().is_some() {
        Ok(url)
    } else {
        Err(crate::Error::Custom(format!("no domain: {}", raw)))
    }
}

pub trait Boundaries: 'static + Send {
    /// Returns `true` if the page can be downloaded.
    fn is_allowed(&self, url: &Url) -> bool;
    /// Returns `true` if no links from this page can be put in the search queue.
    fn is_frontier(&self, url: &Url) -> bool;
    /// Rmoves, adds, mixes ans matches URL query parameters according to some
    /// implementation-specific policy. This is meant to create a "canonical"
    /// representation of a URL.
    fn clean_query_params(&self, url: Url) -> Url;

    fn clean_links(&self, page_url: &Url, links: &[(Reason, String)]) -> Vec<(Reason, Url)> {
        if self.is_frontier(page_url) {
            return vec![];
        }

        // Now, parse and see what stays in and what goes away:
        let mut raw_links = links
            .iter()
            .filter_map(|(reason, raw)| match checked_join(page_url, &raw) {
                Ok(url) => Some((*reason, self.clean_query_params(url))),
                Err(err) => {
                    log::debug!("at {}: {}", page_url, err);
                    None
                }
            })
            .filter(|(_reason, url)| self.is_allowed(url))
            .map(|(reason, url)| (reason, self.clean_query_params(url)))
            .collect::<Vec<_>>();

        // Only *one* representative for each (reason, link) pair. This may ease the load
        // on the database and avoid dumb stuff in general.
        raw_links.sort_unstable();
        raw_links.dedup();

        raw_links
    }
}

pub struct DirectiveBoundaries {
    boundaries: ParsedBoundaries,
}

impl DirectiveBoundaries {
    pub fn new(boundaries: ParsedBoundaries) -> DirectiveBoundaries {
        DirectiveBoundaries { boundaries }
    }
}

impl Boundaries for DirectiveBoundaries {
    fn is_allowed(&self, url: &Url) -> bool {
        self.boundaries.is_allowed(url)
    }

    fn is_frontier(&self, url: &Url) -> bool {
        self.boundaries.is_frontier(url)
    }

    fn clean_query_params(&self, url: Url) -> Url {
        self.boundaries.filter_query_params(url)
    }
}
