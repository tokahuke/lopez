//! Remember: idempotent atomic operations are the key.

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
mod robots;
#[macro_use]
mod cli;
mod logger;
pub mod pretty_print;

pub use ansi_term;
pub use cli::Profile;
pub use crawler::{page_rank, start, test_url};
pub use directives::Directives;
pub use error::Error;
pub use hash::hash;
pub use logger::init_logger;
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

        #[tokio::main(basic_scheduler)]
        pub async fn main() {
            use $crate::ansi_term::Color::{Green, Red};

            match run().await {
                Ok(Some(msg)) => {
                    println!("{}: {}", Green.bold().paint("ok"), msg);
                    std::process::exit(0)
                }
                Ok(None) => std::process::exit(1),
                Err(err) => {
                    println!("{}: {}", Red.bold().paint("error"), err);
                    std::process::exit(1)
                }
            }
        }

        async fn run() -> Result<Option<String>, $crate::Error> {
            use std::sync::Arc;

            use $crate::ansi_term::Color::Red;
            use $crate::backend::Url;
            use $crate::Directives;

            #[cfg(windows)]
            let enabled = colored_json::enable_ansi_support();

            // Environment interpretation:
            let cli = Cli::from_args();

            match cli.app {
                LopezApp::Validate { source } => {
                    // Conditionally init logging:
                    if cli.verbose {
                        $crate::init_logger(cli.verbose);
                    }

                    // Open directives:
                    Directives::load(source, cli.import_path)
                        .map(|_| Some("valid configuration".to_owned()))
                        .map_err(|err| err.into())
                }
                LopezApp::Test { source, test_url } => {
                    // Conditionally init logging:
                    if cli.verbose {
                        $crate::init_logger(cli.verbose);
                    }

                    match Url::parse(&test_url) {
                        Err(err) => Err(err.into()),
                        Ok(url) => {
                            // Open directives:
                            let directives = Arc::new(Directives::load(source, cli.import_path)?);

                            // Create report:
                            let report =
                                $crate::test_url(Arc::new(Profile::default()), directives, url)
                                    .await;

                            // Show report (TODO bad representation! make something pretty):
                            report.pretty_print();

                            Ok(None)
                        }
                    }
                }
                LopezApp::Run {
                    source,
                    wave_name,
                    config,
                    profile,
                } => {
                    // Init logging:
                    $crate::init_logger(cli.verbose);

                    // Open directives:
                    let directives = Arc::new(Directives::load(source, cli.import_path)?);

                    // Create backend:
                    let backend = <$backend_ty>::init(config, &wave_name).await?;

                    // Do the thing!
                    $crate::start(Arc::new(profile), directives, backend).await?;

                    Ok(Some("crawl complete".to_owned()))
                }
                LopezApp::Rm {
                    ignore,
                    wave_name,
                    config,
                } => {
                    if cli.verbose {
                        $crate::init_logger(cli.verbose);
                    }

                    let mut backend = <$backend_ty>::init(config, &wave_name).await?;

                    let was_removed = backend.remove().await?;

                    if was_removed {
                        Ok(Some(format!("wave `{}` removed", wave_name)))
                    } else if ignore {
                        Ok(Some(format!("wave `{}` not removed (ignoring)", wave_name)))
                    } else {
                        Err(
                            format!("wave `{}` cannot be removed (does it exist?)", wave_name)
                                .into(),
                        )
                    }
                }
                LopezApp::PageRank { wave_name, config } => {
                    // Init logging:
                    $crate::init_logger(cli.verbose);

                    // Create backend:
                    let backend = <$backend_ty>::init(config, &wave_name).await?;

                    // Do the thing.
                    $crate::page_rank(backend).await?;

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
