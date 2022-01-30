//! Remember: idempotent atomic operations are the key.

#![feature(never_type)]

mod crawler;
#[macro_use]
pub mod backend;
mod cancel;
mod directives;
mod env;
mod hash;
mod page_rank;
mod panic;
#[macro_use]
mod cli;
mod logger;
mod server;

pub mod pretty_print;
pub mod r#type;

pub use ansi_term;
pub use anyhow;
pub use cli::{Mode, Profile};
pub use crawler::{CrawlMaster, DummyConfiguration, LocalHandlerFactory};
pub use directives::{Directives, DirectivesConfiguration};
pub use hash::hash;
pub use logger::init_logger;
pub use r#type::Type;
pub use serde::Serialize;
pub use server::{serve, RemoteWorkerHandlerFactory};
pub use structopt::StructOpt;

pub const fn default_user_agent() -> &'static str {
    concat!(
        env!("CARGO_PKG_NAME"),
        "/",
        env!("CARGO_PKG_VERSION"),
        " (+",
        env!("CARGO_PKG_HOMEPAGE"),
        ")",
    )
}

/// Entrypoint for Lopez. This "does the whole thing" for you, given the
/// backend implementation.
///
/// All you need to do is to make this function the `main` function of your
/// program, like so:
/// ```rust
/// lib_lopez::main! { YourBackendImplType }
/// ```
/// And you have yourself a Lopez!
#[macro_export]
macro_rules! main {
    ($backend_ty:ty) => {
        // Implements the Cli for the Backend (generics are no supported by `structopt`).
        $crate::cli_impl!($backend_ty);

        use $crate::Serialize;

        fn print_json<T: Serialize + ?Sized>(t: &T) {
            println!(
                "{}",
                serde_json::to_string_pretty(t)
                    .expect("can deserialize")
            );
        }

        #[tokio::main(flavor = "current_thread")]
        pub async fn main() {
            use $crate::ansi_term::Color::{Green, Red};

            // Environment interpretation:
            let cli = Cli::from_args();

            if cli.json {
                match run(cli).await {
                    Ok(Some(msg)) => {
                        print_json(&serde_json::json!({ "Ok": msg }));
                        std::process::exit(0)
                    }
                    Ok(None) => std::process::exit(0),
                    Err(err) => {
                        print_json(&serde_json::json!({ "Err": err.to_string() }));
                        std::process::exit(1)
                    }
                }
            } else {
                match run(cli).await {
                    Ok(Some(msg)) => {
                        println!("{}: {msg}", Green.bold().paint("ok"));
                        std::process::exit(0)
                    }
                    Ok(None) => std::process::exit(0),
                    Err(err) => {
                        println!("{}: {err}", Red.bold().paint("error"));
                        std::process::exit(1)
                    }
                }
            }
        }

        async fn run(cli: Cli) -> Result<Option<String>, $crate::anyhow::Error> {
            use std::sync::Arc;

            use $crate::ansi_term::Color::Red;
            use $crate::backend::Url;
            use $crate::Directives;

            #[cfg(windows)]
            let enabled = colored_json::enable_ansi_support();

            match cli.app {
                LopezApp::Validate { source } => {
                    // Conditionally init logging:
                    if cli.verbose {
                        $crate::init_logger(cli.verbose);
                    }

                    // Open directives:
                    Directives::load(source, cli.import_path)
                        .map(|_| Some("valid configuration".to_owned()))
                }
                LopezApp::Test {
                    source,
                    test_url,
                } => {
                    // Conditionally init logging:
                    if cli.verbose {
                        $crate::init_logger(cli.verbose);
                    }

                    match Url::parse(&test_url) {
                        // TODO: (known issue) structured output messes the expected return status...
                        Err(err) => {
                            if cli.json {
                                print_json(&Err(format!("{}", err)) as &Result<(), _>);
                                Ok(None)
                            } else {
                                Err(err.into())
                            }
                        }
                        Ok(url) => {
                            // Open directives:
                            match Directives::load(source, cli.import_path) {
                                Err(err) => {
                                    if cli.json {
                                        print_json(&Err(format!("{}", err)) as &Result<(), _>);
                                        Ok(None)
                                    } else {
                                        Err(err)
                                    }
                                }
                                Ok(directives) => {
                                    let directives = directives;
                                    let configuration = $crate::DirectivesConfiguration::new(directives);
                                    let crawl_master = $crate::CrawlMaster::new(configuration, $crate::backend::DummyBackend::default(), $crate::LocalHandlerFactory);

                                    // Create report:
                                    let report = crawl_master.test_url(Arc::new(Profile::default()), url)
                                        .await;

                                    // Show report:
                                    if cli.json {
                                        print_json(&Ok(report) as &Result<_, ()>);
                                    } else {
                                        report.pretty_print();
                                    }

                                    Ok(None)
                                }
                            }
                        }
                    }
                }
                LopezApp::Run {
                    source,
                    wave_name,
                    config,
                    profile,
                    mode,
                } => {
                    // Init logging:
                    $crate::init_logger(cli.verbose);

                    // Open directives:
                    let directives = Directives::load(source, cli.import_path)?;
                    let configuration = $crate::DirectivesConfiguration::new(directives);

                    // Create backend:
                    let backend = <$backend_ty>::init(config, &wave_name).await?;

                    // Do the thing!
                    match mode.unwrap_or_default() {
                        $crate::Mode::Local => {
                            $crate::CrawlMaster::new(
                                configuration,
                                backend,
                                $crate::LocalHandlerFactory
                            ).start(Arc::new(profile)).await?
                        },
                        $crate::Mode::Cluster { token, pool, max_retries } => {
                            $crate::CrawlMaster::new(
                                configuration,
                                backend,
                                $crate::RemoteWorkerHandlerFactory::connect(
                                    token,
                                    max_retries,
                                    &pool
                                ).await?,
                            ).start(Arc::new(profile)).await?
                        }
                    };

                    Ok(Some("crawl complete".to_owned()))
                },
                LopezApp::Serve { token, bind, max_connections } => {
                    // Init logging:
                    $crate::init_logger(cli.verbose);

                    $crate::serve(token,
                        max_connections,
                        bind,
                    ).await?;

                    Ok(Some("server ended".to_owned()))
                },
                LopezApp::Rm {
                    ignore,
                    wave_name,
                    config,
                } => {
                    if cli.verbose {
                        $crate::init_logger(cli.verbose);
                    }

                    let mut backend = <$backend_ty>::init(config, &wave_name).await?;

                    let remove_report = backend.remove().await?;

                    if cli.json {
                        print_json(&remove_report);
                        Ok(None)
                    } else {
                        if remove_report.was_removed() {
                            Ok(Some(format!("wave `{}` removed", wave_name)))
                        } else if ignore {
                            Ok(Some(format!("wave `{}` not removed (ignoring)", wave_name)))
                        } else {
                            Err(
                                $crate::anyhow::anyhow!("wave `{wave_name}` cannot be removed (does it exist?)")
                            )
                        }
                    }
                }
                LopezApp::PageRank { wave_name, config } => {
                    // Init logging:
                    $crate::init_logger(cli.verbose);

                    // Create backend:
                    let backend = <$backend_ty>::init(config, &wave_name).await?;

                    // Do the thing.
                    let crawl_master = $crate::CrawlMaster::new(
                        $crate::DummyConfiguration,
                        backend,
                        $crate::LocalHandlerFactory
                    );
                    crawl_master.page_rank().await?;

                    Ok(Some("page rank done".to_owned()))
                }
            }
        }
    };
}

/// A dummy module only to validate the expansion of the [`main!`] macro
/// against the dummy backend.
#[allow(unused)]
mod dummy {
    main! { crate::backend::DummyBackend }
}
