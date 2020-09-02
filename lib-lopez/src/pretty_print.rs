//! A crate for pretty printing reports.

use ansi_term::Color::{self, Blue, Green, Purple, Red, White, Yellow};
use colored_json::to_colored_json_auto;
use hyper::StatusCode;
use url::Url;

use crate::crawler::{Crawled, ReportType, TestRunReport};

fn color_for_code(code: &StatusCode) -> Color {
    if code.is_informational() {
        White
    } else if code.is_success() {
        Green
    } else if code.is_redirection() {
        Blue
    } else if code.is_client_error() {
        Yellow
    } else if code.is_server_error() {
        Red
    } else {
        Purple
    }
}

fn print_status(status_code: &StatusCode) {
    let color = color_for_code(status_code);
    let code = status_code.as_u16();

    if let Some(reason_phrase) = status_code.canonical_reason() {
        println!(
            "Status code: {}{}{}",
            color.bold().paint(format!("⏺ {}", code)),
            White.paint(" - "),
            White.paint(reason_phrase),
        );
    } else {
        println!("Status code: {}", code)
    }
}

fn print_list_of_url<'a, I: IntoIterator<Item = &'a Url>>(list: I, color: Color, limit: usize) {
    let collected = list
        .into_iter()
        .map(|url| color.paint(url.to_string()).to_string())
        .collect::<Vec<_>>();
    let len = collected.len();

    if len == 0 {
        println!("    <empty>");
    } else if len <= limit {
        println!("    {}", collected.join("\n    "));
    } else {
        println!(
            "    {}",
            collected
                .into_iter()
                .take(limit)
                .collect::<Vec<_>>()
                .join("\n    ")
        );
        println!("    ... and {} more.", len - limit);
    }
}

impl TestRunReport {
    pub fn pretty_print(&self) {
        println!(
            "Actual url: {}",
            White.bold().paint(self.actual_url.to_string())
        );

        match &self.report {
            ReportType::DisallowedByDirectives => println!(
                "Status: {}",
                Yellow.bold().paint("disallowed by directives")
            ),
            ReportType::DisallowedByOrigin => println!(
                "Status: {} (robots.txt)",
                Red.bold().paint("disallowed by origin")
            ),
            ReportType::Crawled(Crawled::Error(error)) => {
                println!("Status: error");
                println!("Error: {}", error);
            }
            ReportType::Crawled(Crawled::TimedOut) => {
                println!("Status: {}", Red.bold().paint("timed out"))
            }
            ReportType::Crawled(Crawled::Redirect {
                status_code,
                location,
            }) => {
                print_status(status_code);
                println!("Location: {}", Blue.paint(location));
            }
            ReportType::Crawled(Crawled::BadStatus { status_code }) => print_status(status_code),
            ReportType::Crawled(Crawled::Success {
                status_code,
                links,
                analyses,
            }) => {
                print_status(status_code);
                println!("Canonical:");
                print_list_of_url(
                    links
                        .iter()
                        .filter(|(reason, _)| reason.is_canonical())
                        .map(|(_, url)| url),
                    Red,
                    3,
                );
                println!("Links:");
                print_list_of_url(
                    links
                        .iter()
                        .filter(|(reason, _)| reason.is_ahref())
                        .map(|(_, url)| url),
                    Blue,
                    10,
                );

                let pretty_analises = analyses
                    .iter()
                    .map(|(name, result)| {
                        format!(
                            "{}: {}",
                            name,
                            to_colored_json_auto(result)
                                .expect("can serialize")
                                .replace("\n", "\n    ")
                        )
                    })
                    .collect::<Vec<_>>();

                if pretty_analises.is_empty() {
                    println!("Analyses:\n    <empty>");
                } else {
                    println!("Analyses:\n    {}", pretty_analises.join("\n    "),);
                }
            }
        }
    }
}
