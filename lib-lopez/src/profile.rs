use structopt::StructOpt;

/// See `Default` implementation for default values on fields.
#[derive(Debug, Clone, StructOpt)]
pub struct Profile {
    #[structopt(long, env)]
    pub user_agent: Option<String>,
    #[structopt(long, default_value = "1000", env)]
    pub quota: u32,
    #[structopt(long, default_value = "7", env)]
    pub max_depth: u16,

    #[structopt(long, default_value = "2.5", env)]
    pub max_hits_per_sec: f64,
    #[structopt(long, default_value = "60", env)]
    pub request_timeout: f64,

    #[structopt(long, default_value = "1", env)]
    pub workers: usize,
    #[structopt(long, default_value = "128", env)]
    pub max_tasks_per_worker: usize,
    #[structopt(long, default_value = "2", env)]
    pub backends_per_worker: usize,

    #[structopt(long, default_value = "1024", env)]
    pub batch_size: usize,
}

fn default_user_agent() -> &'static str {
    concat!(
        env!("CARGO_PKG_NAME"),
        "/",
        env!("CARGO_PKG_VERSION"),
        " (+",
        env!("CARGO_PKG_HOMEPAGE"),
        ")",
    )
}

impl Profile {
    pub fn user_agent<'a>(&'a self) -> &'a str {
        self.user_agent
            .as_ref()
            .map(|ua| ua.as_str())
            .unwrap_or_else(|| default_user_agent() as &'a str)
    }
}
