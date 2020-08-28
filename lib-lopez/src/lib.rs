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

pub use ansi_term;
pub use cli::Profile;
pub use crawler::{start, test_url};
pub use directives::Directives;
pub use error::Error;
pub use hash::hash;
pub use logger::init_logger;
pub use structopt::StructOpt;

pub fn default_user_agent() -> &'static str {
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
        async fn main() -> Result<(), $crate::Error> {
            use std::sync::Arc;

            use $crate::ansi_term::Color::{Green, Red};
            use $crate::backend::Url;
            use $crate::Directives;

            // Environment interpretation:
            let cli = Cli::from_args();

            match cli.app {
                LopezApp::Validate { source } => {
                    // Open directives:
                    match Directives::load(source, cli.import_path) {
                        Ok(_directives) => {
                            println!("{}", Green.bold().paint("Valid configuration"))
                        }
                        Err(err) => println!("{}: {}", Red.bold().paint("Error"), err),
                    }
                }
                LopezApp::Test { source, test_url } => {
                    match Url::parse(&test_url) {
                        Err(err) => println!("{}: {}", Red.bold().paint("Invalid URL"), err,),
                        Ok(url) => {
                            // Open directives:
                            let directives = Arc::new(Directives::load(source, cli.import_path)?);

                            // Create report:
                            let report =
                                $crate::test_url(Arc::new(Profile::default()), directives, url)
                                    .await;

                            // Show report (TODO bad representation! make something pretty):
                            println!("{:#?}", report);
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
                    $crate::init_logger();

                    // Open directives:
                    let directives = Arc::new(Directives::load(source, cli.import_path)?);

                    // Create backend:
                    let backend = <$backend_ty>::init(config, &wave_name).await?;

                    // Do the thing!
                    $crate::start(Arc::new(profile), directives, backend).await?;
                }
            }

            Ok(())
        }
    };
}

/// A dummy module only to validate the expansion of the [`main!`] macro
/// against the dummy backend.
mod dummy {
    main! { crate::backend::DummyBackend }
}
