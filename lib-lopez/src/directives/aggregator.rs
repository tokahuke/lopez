use regex::{Captures, Regex};
use scraper::ElementRef;
use serde_json::{to_value, Value};
use std::collections::HashMap;

use super::parse::{Aggregator, Extractor};

impl Extractor {
    pub fn extract(&self, element_ref: ElementRef) -> Value {
        /// Puts captures into a nice JSON.
        fn capture_json(regex: &Regex, captures: Captures) -> HashMap<String, String> {
            captures
                .iter()
                .zip(regex.capture_names())
                .enumerate()
                .filter_map(|(i, (maybe_capture, maybe_name))| {
                    maybe_capture.map(|capture| {
                        (
                            maybe_name
                                .map(|name| name.to_owned())
                                .unwrap_or_else(|| i.to_string()),
                            capture.as_str().to_owned(),
                        )
                    })
                })
                .collect::<HashMap<String, String>>()
        }

        match self {
            Extractor::Name => to_value(element_ref.value().name()),
            Extractor::Attr(attr) => to_value(element_ref.value().attr(attr)),
            Extractor::Html => to_value(element_ref.html()),
            Extractor::InnerHtml => to_value(element_ref.inner_html()),
            Extractor::Text => to_value(element_ref.text().collect::<Vec<_>>().join(" ")),
            Extractor::Hash => to_value(crate::hash::hash(&element_ref.inner_html())),
            Extractor::Capture(regex) => {
                let text = element_ref.text().collect::<Vec<_>>().join(" ");
                let a_match = regex
                    .captures(&text)
                    .map(|captures| capture_json(&regex, captures));

                to_value(a_match)
            }
            Extractor::AllCaptures(regex) => {
                let text = element_ref.text().collect::<Vec<_>>().join(" ");
                let all_matches = regex
                    .captures_iter(&text)
                    .map(|captures| capture_json(&regex, captures))
                    .collect::<Vec<_>>();

                to_value(all_matches)
            }
        }
        .expect("can always serialize")
    }
}

pub enum AggregatorState<'a> {
    Count(usize),
    First(&'a Extractor, Option<Value>),
    Collect(&'a Extractor, Vec<Value>),
}

impl<'a> AggregatorState<'a> {
    pub fn new(aggregator: &Aggregator) -> AggregatorState {
        match aggregator {
            Aggregator::Count => AggregatorState::Count(0),
            Aggregator::First(extractor) => AggregatorState::First(extractor, None),
            Aggregator::Collect(extractor) => AggregatorState::Collect(extractor, vec![]),
        }
    }

    pub fn aggregate(&mut self, element_ref: ElementRef) {
        match self {
            AggregatorState::Count(count) => *count += 1,
            AggregatorState::First(extractor, maybe_value) => {
                if maybe_value.is_none() {
                    *maybe_value = Some(extractor.extract(element_ref))
                }
            }
            AggregatorState::Collect(extractor, values) => {
                values.push(extractor.extract(element_ref));
            }
        }
    }

    pub fn finalize(self) -> Value {
        match self {
            AggregatorState::Count(count) => Value::Number(count.into()),
            AggregatorState::First(_, value) => value.unwrap_or_default(),
            AggregatorState::Collect(_, collected) => Value::Array(collected),
        }
    }
}
