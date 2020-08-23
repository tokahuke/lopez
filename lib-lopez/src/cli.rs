#[macro_export]
macro_rules! cli_impl {
    ($backend_ty:ty) => {
        use std::path::PathBuf;

        use $crate::backend::Backend;
        use $crate::Profile;
        use $crate::StructOpt;

        #[derive(StructOpt)]
        pub struct Cli {
            #[structopt(long, env, default_value = "/usr/share/lopez/lib")]
            pub import_path: PathBuf,
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
                config: <$backend_ty as Backend>::Config,
            },
            /// Validates a given crawl configuration.
            Validate {
                /// The name of the `.lcd` file to be used for the crawl configuration
                #[structopt(env)]
                source: PathBuf,
            },
        }
    };
}
