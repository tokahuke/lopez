//! The crawler façade.

mod boundaries;
mod counter;
mod downloader;
mod origins;
mod parser;
mod reason;
mod robots;
mod worker;

pub use self::boundaries::Boundaries;
pub use self::counter::Counter;
pub use self::downloader::{Downloader, SimpleDownloader};
pub use self::parser::{Parsed, Parser};
pub use self::reason::Reason;
pub(crate) use self::worker::{CrawlWorker, Crawled, ReportType, TestRunReport};

use futures::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};
use url::Url;

use crate::backend::{Backend, MasterBackend, PageRanker};
use crate::cli::Profile;
use crate::Type;

use self::counter::log_stats;
use self::origins::Origins;

/// Configuration parameter for crawling
pub struct Parameters {
    pub max_hits_per_sec: f64,
    pub quota: usize,
    pub request_timeout: f64,
    pub max_depth: i16,
    pub enable_page_rank: bool,
}

pub trait Configuration {
    type Downloader: Downloader;
    type Parser: Parser;
    type Boundaries: Boundaries;

    fn downloader(&self) -> Self::Downloader;
    fn parser(&self) -> Self::Parser;
    fn boundaries(&self) -> Self::Boundaries;
    fn seeds(&self) -> Vec<Url>;
    fn analyzes(&self) -> Vec<(String, Type)>;
    fn parameters(&self) -> Parameters;
}

/// Does the crawling.
pub async fn start<C: Configuration, B: Backend>(
    profile: Arc<Profile>,
    configuration: C,
    mut backend: B,
) -> Result<(), crate::Error> {
    // Set panics to be logged:
    crate::panic::log_panics();

    let configuration = Arc::new(configuration);
    let parameters = configuration.parameters();

    // Set global (transient) information on origins:
    let origins = Arc::new(Origins::new(parameters.max_hits_per_sec));

    // Load data model:
    let mut master_model = backend.build_master().await.map_err(|err| err.into())?;
    let wave_id = master_model.wave_id();
    let worker_model_factory = Arc::new(Mutex::new(backend.build_worker_factory(wave_id)));

    // Creates a counter to get stats:
    let counter = Arc::new(Counter::default());

    // Calculation of how many pages to crawl, after all:
    let crawl_counter = counter.clone();
    let consumed = master_model
        .count_crawled()
        .await
        .map_err(|err| err.into())?;
    let crawl_quota = parameters.quota;
    let max_quota = profile.max_quota.unwrap_or(std::usize::MAX);
    let effective_quota = usize::min(max_quota, crawl_quota);
    // Whether enough juice was given for the crawl to get to the end:
    let will_crawl_end = crawl_quota <= max_quota;
    let remaining_quota = (effective_quota).saturating_sub(consumed);

    // Spawn task that will log stats from time to time:
    let _stats_handle = tokio::spawn(log_stats(
        counter.clone(),
        consumed,
        effective_quota,
        profile.clone(),
    ));

    let crawl_profile = profile.clone();
    let crawl_configuration = configuration.clone();
    let request_timeout = parameters.request_timeout;
    let (mut senders, handles): (Vec<_>, Vec<_>) = (0..profile.workers)
        .map(move |worker_id| {
            CrawlWorker::new(
                crawl_configuration.downloader(),
                crawl_configuration.parser(),
                crawl_configuration.boundaries(),
                worker_model_factory.clone(),
                crawl_counter.clone(),
                crawl_profile.clone(),
                origins.clone(),
                request_timeout,
            )
            .run(worker_id)
        })
        .unzip();

    // Ensure that the search was started:
    let seeds = configuration.seeds();
    log::info!(
        "Seeding:\n    {}",
        seeds
            .iter()
            .map(|seed| seed.as_str())
            .collect::<Vec<_>>()
            .join("\n    ")
    );
    master_model
        .ensure_seeded(&configuration.seeds())
        .await
        .map_err(|err| err.into())?;

    // Ensure that all analysis names exist:
    master_model
        .create_analyses(&configuration.analyzes())
        .await
        .map_err(|err| err.into())?;

    // Reset the search queue from last run (only one process at a time)
    master_model.reset_queue().await.map_err(|err| err.into())?;

    // And now, do the thing!
    let mut n_sent = 0;
    let mut has_been_empty = false;
    let mut is_interrupted = false;

    if remaining_quota == 0 {
        log::warn!("empty crawl");
        return Ok(());
    }

    'master: while !is_interrupted {
        match master_model
            .fetch(profile.batch_size as i64, parameters.max_depth)
            .await
        {
            Err(error) => {
                log::error!("error while fetching: {}", error.into());
                break 'master;
            }
            Ok(mut batch) => {
                // TODO this is most probably buggy in a very, very clever way...
                if batch.is_empty() {
                    // If everything sent is done (or error), then... go away!
                    if n_sent == counter.n_closed() {
                        if has_been_empty {
                            log::info!(
                                "number of sents and dones are equal and the queue \
                                 has been empty. I think we are done..."
                            );
                            break 'master;
                        } else {
                            // Better give one more chance, just to be sure.
                            has_been_empty = true;
                        }
                    }

                    // This mitigates a spin lock here.
                    time::sleep(Duration::from_secs(1)).await;
                    continue 'master;
                } else {
                    // "Cancel the Apocalypse..."
                    has_been_empty = false;
                }

                batch.sort_unstable_by_key(|(_, depth)| *depth);

                // Round robin:
                '_dispatch: for (url, depth) in batch {
                    let chosen = crate::hash(&url.origin()) as usize % senders.len();

                    if senders[chosen].send((url, depth)).await.is_err() {
                        log::error!("crawler {} failed. Stopping", chosen);
                        is_interrupted = true;
                        break 'master;
                    } else {
                        n_sent += 1;
                    }

                    // Stop if quota is reached:
                    if counter.n_closed() >= remaining_quota {
                        log::info!("quota of {} reached", remaining_quota + consumed);
                        break 'master;
                    }
                }
            }
        }
    }

    // Last part: close channel (this will force all workers to end and thus...)
    drop(senders);

    // Wait for workers:
    for canceler in handles {
        canceler.cancel().await;
    }

    if is_interrupted {
        log::info!("crawl was interrupted");
        Err(crate::Error::Custom("crawl was interrupted".to_owned()))
    } else if !will_crawl_end {
        log::info!("crawl incomplete: not enough `MAX_QUOTA` given");
        Ok(())
    } else {
        log::info!("crawl done");

        // Now, do page rank, if enabled:
        if parameters.enable_page_rank {
            page_rank_for_wave_id(&mut backend, wave_id).await?
        }

        Ok(())
    }
}

/// Runs the PageRank algorithm on a given wave, given an existing master backend.
async fn page_rank_for_wave_id<B: Backend>(
    backend: &mut B,
    wave_id: i32,
) -> Result<(), crate::Error> {
    backend
        .build_ranker(wave_id)
        .await
        .map_err(|err| err.into())?
        .page_rank()
        .await
        .map_err(|err| err.into())?;
    Ok(())
}

pub async fn page_rank<B: Backend>(mut backend: B) -> Result<(), crate::Error> {
    let mut master_model = backend.build_master().await.map_err(|err| err.into())?;
    page_rank_for_wave_id(&mut backend, master_model.wave_id()).await
}

/// Tests a URL and says what is happening.
pub async fn test_url<C: Configuration>(
    profile: Arc<Profile>,
    configuration: C,
    url: Url,
) -> TestRunReport {
    let parameters = configuration.parameters();
    // Set global (transient) information on origins:
    let origins = Arc::new(Origins::new(parameters.max_hits_per_sec));

    // Load dummy data model:
    let mut backend = crate::backend::DummyBackend::default();
    let mut master_model = backend
        .build_master()
        .await
        .expect("can always build DummyMasterBackend");
    let worker_model_factory = Arc::new(Mutex::new(
        backend.build_worker_factory(master_model.wave_id()),
    ));

    // Creates a counter to get stats:
    let counter = Arc::new(Counter::default());

    CrawlWorker::new(
        configuration.downloader(),
        configuration.parser(),
        configuration.boundaries(),
        worker_model_factory,
        counter,
        profile,
        origins,
        parameters.request_timeout,
    )
    .test_url(url)
    .await
}
