use structopt::StructOpt;

/// See `Default` implementation for default values on fields.
#[derive(Debug, Clone, StructOpt)]
pub struct Profile {
    /// The name to be used in the User-Agent header. Defaults to `lopez/<version>`
    /// if not set.
    #[structopt(long, env)]
    pub user_agent: Option<String>,
    /// The maximum number of hits in this crawl wave. The default limit is
    /// small. You should override it.
    #[structopt(long, default_value = "1000", env)]
    pub quota: u32,
    /// The maximum distance from the seed that the crawl will go. This is
    /// necessary to avoid, e.g., long search lists with pagination.
    #[structopt(long, default_value = "7", env)]
    pub max_depth: u16,

    /// The maximum number of hits per second _per origin_. Do not set this too
    /// high or you may overload some servers and that's rude!
    #[structopt(long, default_value = "2.5", env)]
    pub max_hits_per_sec: f64,
    /// The time limit for the download of a page.
    #[structopt(long, default_value = "60", env)]
    pub request_timeout: f64,

    /// The number of worker units to be run. Each worker runs in its own
    /// thread. Just raise this if one worker is already consuming 100% CPU,
    /// otherwise it is just plain silly.
    #[structopt(long, default_value = "1", env)]
    pub workers: usize,
    /// The maximum number of concurrent tasks that a worker may run.
    #[structopt(long, default_value = "1024", env)]
    pub max_tasks_per_worker: usize,
    /// The number of worker backends used by each worker. Using more backends
    /// can make communication with a database more effective, for example.
    #[structopt(long, default_value = "2", env)]
    pub backends_per_worker: usize,
    /// Interval between consecutive stats log entries.
    #[structopt(long, env)]
    pub log_stats_every_secs: Option<f64>,

    /// The size of the batches of URL that are to be fetched from the backend.
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
