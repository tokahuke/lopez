//! Remember: idempotent atomic operations are the key.

#![feature(never_type, async_closure)]

mod cli;
mod logger;

use ansi_term::Color::{Green, Red};
use std::sync::Arc;
use structopt::StructOpt;

use lib_lopez::backend::Backend;
use lib_lopez::Directives;

use crate::cli::{Cli, LopezApp};

pub type BackendImpl = postgres_lopez::PostgresBackend;

#[tokio::main(basic_scheduler)]
async fn main() -> Result<(), lib_lopez::Error> {
    // Environment interpretation:
    let cli = Cli::from_args();

    // Prepare filesystem:
    lib_lopez::prepare_fs(&cli.data)?;

    match cli.app {
        LopezApp::Validate { source } => {
            // Open directives:
            match Directives::load(source) {
                Ok(_directives) => println!("{}", Green.bold().paint("Ok")),
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
            crate::logger::init_logger();

            // Open directives:
            let directives = Arc::new(Directives::load(source)?);

            // Create backend:
            let backend = BackendImpl::init(config, &wave_name)
                .await?;

            // Do the thing!
            lib_lopez::start(Arc::new(profile), directives, backend)
                .await?;
        }
    }

    Ok(())
}
