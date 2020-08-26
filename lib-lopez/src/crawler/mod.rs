mod counter;
mod reason;
mod worker;

pub use counter::Counter;
pub use reason::Reason;

use futures::prelude::*;
use std::sync::Arc;
use tokio::time::{self, Duration};
use url::Url;

use crate::backend::{Backend, MasterBackend, PageRanker};
use crate::cli::Profile;
use crate::directives::Directives;
use crate::origins::Origins;

use self::counter::log_stats;
use self::worker::{CrawlWorker, TestRunReport};

/// Does the crawling.
pub async fn start<B: Backend>(
    profile: Arc<Profile>,
    directives: Arc<Directives>,
    backend: B,
) -> Result<(), crate::Error> {
    // Set panics to be logged:
    crate::panic::log_panics();

    // Set global (transient) information on origins:
    let origins = Arc::new(Origins::new(
        profile.max_hits_per_sec,
        profile.user_agent().to_owned(),
    ));

    // Load data model:
    let master_model = backend.build_master().await.map_err(|err| err.into())?;
    let worker_model_factory = Arc::new(backend.build_worker_factory(master_model.wave_id()));

    // Creates a counter to get stats:
    let counter = Arc::new(Counter::default());

    // Creates crawlers:
    let crawl_counter = counter.clone();
    let consumed = master_model
        .count_crawled()
        .await
        .map_err(|err| err.into())?;
    let remaining_quota = (profile.quota as usize).saturating_sub(consumed);

    // Spawn task that will log stats from time to time:
    let _stats_handle = tokio::spawn(log_stats(counter.clone(), consumed, profile.clone()));

    let crawl_profile = profile.clone();
    let crawl_directives = directives.clone();
    let (mut senders, handles): (Vec<_>, Vec<_>) = (0..profile.workers)
        .map(move |worker_id| {
            CrawlWorker::new(
                crawl_counter.clone(),
                crawl_profile.clone(),
                crawl_directives.clone(),
                worker_model_factory.clone(),
                origins.clone(),
            )
            .run(worker_id)
        })
        .unzip();

    // Ensure that the search was started:
    let seeds = directives.seeds();
    log::info!(
        "Seeding: \n\t- {}",
        seeds
            .iter()
            .map(|seed| seed.as_str())
            .collect::<Vec<_>>()
            .join("\n\t- ")
    );
    master_model
        .ensure_seeded(&directives.seeds())
        .await
        .map_err(|err| err.into())?;

    // Ensure that all analysis names exist:
    master_model
        .create_analyses(&directives.rule_names())
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
            .fetch(profile.batch_size as i64, profile.max_depth as i16)
            .await
        {
            Err(error) => {
                log::error!("error while fetching: {}", error.into());
                break 'master;
            }
            Ok(batch) => {
                // TODO this is most probably buggy in a very, very clever way...
                if batch.is_empty() {
                    // If everything sent is done (or error), then... go away!
                    if n_sent == counter.n_done() {
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
                    time::delay_for(Duration::from_secs(1)).await;
                    continue 'master;
                } else {
                    // "Cancel the Apocalypse..."
                    has_been_empty = false;
                }

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
                    if counter.n_done() >= remaining_quota {
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

    if !is_interrupted {
        log::info!("crawl done");

        // Now, do page rank.
        backend
            .build_ranker(master_model.wave_id())
            .await
            .map_err(|err| err.into())?
            .page_rank()
            .await
            .map_err(|err| err.into())?;

        Ok(())
    } else {
        log::info!("crawl was interrupted");
        Err(crate::Error::Custom("crawl was interrupted".to_owned()))
    }
}

/// Tests a URL and says what is happening.
pub async fn test_url(
    profile: Arc<Profile>,
    directives: Arc<Directives>,
    url: Url,
) -> TestRunReport {
    // Set global (transient) information on origins:
    let origins = Arc::new(Origins::new(
        profile.max_hits_per_sec,
        profile.user_agent().to_owned(),
    ));

    // Load dummy data model:
    let backend = crate::backend::DummyBackend::default();
    let master_model = backend
        .build_master()
        .await
        .expect("can always build DummyMasterBackend");
    let worker_model_factory = Arc::new(backend.build_worker_factory(master_model.wave_id()));

    // Creates a counter to get stats:
    let counter = Arc::new(Counter::default());

    CrawlWorker::new(counter, profile, directives, worker_model_factory, origins)
        .test_url(url)
        .await
}
