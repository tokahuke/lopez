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
mod profile;
mod robots;
#[macro_use]
mod cli;
mod logger;

pub use ansi_term;
pub use crawler::start;
pub use directives::Directives;
pub use error::Error;
pub use hash::hash;
pub use logger::init_logger;
pub use profile::Profile;
pub use structopt::StructOpt;

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
            use $crate::Directives;

            // Environment interpretation:
            let cli = Cli::from_args();

            match cli.app {
                LopezApp::Validate { source } => {
                    // Open directives:
                    match Directives::load(source, cli.import_path) {
                        Ok(directives) => println!(
                            "{}: {:#?}\n{}",
                            Green.bold().paint("Interpreted"),
                            directives,
                            Green.bold().paint("Valid configuration")
                        ),
                        Err(err) => println!("{}: {}", Red.bold().paint("Error"), err),
                    }
                }
                LopezApp::Run {
                    source,
                    wave_name,
                    profile,
                    config,
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
