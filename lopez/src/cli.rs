use std::path::PathBuf;
use structopt::StructOpt;

use lib_lopez::backend::Backend;
use lib_lopez::Profile;

use crate::BackendImpl;

#[derive(StructOpt)]
pub struct Cli {
    #[structopt(long, default_value = "data")]
    pub data: PathBuf,
    #[structopt(subcommand)]
    pub app: LopezApp,
}

#[derive(StructOpt)]
pub enum LopezApp {
    /// Runs a crawl using a given crawl configuration.
    Run {
        /// The name of the `.lcd` file to be used for the crawl configuration
        #[structopt(env)]
        source: PathBuf,
        /// The name of this crawl wave. If the given wave name exists, the
        /// corresponding crawl is resumed.
        #[structopt(env)]
        wave_name: String,
        #[structopt(flatten)]
        profile: Profile,
        #[structopt(flatten)]
        config: <BackendImpl as Backend>::Config,
    },
    /// Validates a given crawl configuration.
    Validate {
        /// The name of the `.lcd` file to be used for the crawl configuration
        #[structopt(env)]
        source: PathBuf,
    },
}
