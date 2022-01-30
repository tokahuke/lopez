use futures::prelude::*;
use std::sync::Arc;
use tokio::time::{self, Duration};
use url::Url;

use crate::backend::{Backend, PageRanker, WorkerBackendFactory};
use crate::cli::Profile;

use super::counter::log_stats;
use super::worker::{WorkerHandler, WorkerHandlerFactory, WorkerId};
use super::{Configuration, Counter, CrawlWorker, TestRunReport};

pub struct CrawlMaster<B, WHF> {
    configuration: Arc<dyn Configuration>,
    backend: B,
    worker_handler_factory: WHF,
}

impl<B, WHF> CrawlMaster<B, WHF>
where
    B: Backend,
    WHF: WorkerHandlerFactory,
{
    pub fn new<C: 'static + Configuration>(
        configuration: C,
        backend: B,
        worker_handler_factory: WHF,
    ) -> Self {
        CrawlMaster {
            configuration: Arc::new(configuration),
            backend,
            worker_handler_factory,
        }
    }

    pub async fn start(mut self, profile: Arc<Profile>) -> Result<(), anyhow::Error> {
        // Set panics to be logged:
        crate::panic::log_panics();

        let parameters = self.configuration.parameters();

        // Load data model:
        let mut master_model = self.backend.build_master().await?;
        let wave_id = master_model.wave_id();
        let worker_backend_factory: Arc<dyn WorkerBackendFactory> =
            self.backend.build_worker_factory(wave_id).into();

        // Creates a counter to get stats:
        let counter = Arc::new(Counter::default());

        // Calculation of how many pages to crawl, after all:
        let consumed = master_model.count_crawled().await?;
        let crawl_quota = parameters.quota;
        let max_quota = profile.max_quota.unwrap_or(std::usize::MAX);
        let effective_quota = usize::min(max_quota, crawl_quota);
        // Whether enough juice was given for the crawl to get to the end:
        let will_crawl_end = crawl_quota <= max_quota;
        let remaining_quota = (effective_quota).saturating_sub(consumed);

        // Spawn task that will log stats from time to time:
        tokio::spawn(log_stats(
            counter.clone(),
            consumed,
            effective_quota,
            profile.clone(),
        ));

        let crawl_profile = &profile;
        let crawl_counter = &counter;
        let crawl_configuration = &self.configuration;
        let worker_handler_factory = &self.worker_handler_factory;
        let mut handlers = futures::stream::iter(0..profile.workers)
            .then(move |worker_id| {
                worker_handler_factory.build(
                    crawl_configuration.clone(),
                    worker_backend_factory.clone(),
                    crawl_profile.clone(),
                    crawl_counter.clone(),
                    worker_id as WorkerId,
                )
            })
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;

        // Ensure that the search was started:
        let seeds = self.configuration.seeds();
        log::info!(
            "Seeding:\n    {}",
            seeds
                .iter()
                .map(|seed| seed.as_str())
                .collect::<Vec<_>>()
                .join("\n    ")
        );
        master_model
            .ensure_seeded(&self.configuration.seeds())
            .await?;

        // Ensure that all analysis names exist:
        master_model
            .create_analyses(&self.configuration.analyzes())
            .await?;

        // Reset the search queue from last run (only one process at a time)
        master_model.reset_queue().await?;

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
                    log::error!("error while fetching: {}", error);
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
                        let chosen = crate::hash(&url.origin()) as usize % handlers.len();

                        if handlers[chosen].send_task(url, depth).await.is_err() {
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

        // Wait for workers:
        for handler in handlers {
            handler.terminate().await;
        }

        if is_interrupted {
            log::info!("crawl was interrupted");
            Err(anyhow::anyhow!("crawl was interrupted"))
        } else if !will_crawl_end {
            log::info!("crawl incomplete: not enough `MAX_QUOTA` given");
            Ok(())
        } else {
            log::info!("crawl done");

            // Now, do page rank, if enabled:
            if parameters.enable_page_rank {
                self.page_rank_for_wave_id(wave_id).await?
            }

            Ok(())
        }
    }

    /// Runs the PageRank algorithm on a given wave, given an existing master backend.
    async fn page_rank_for_wave_id(&mut self, wave_id: i32) -> Result<(), anyhow::Error> {
        self.backend
            .build_ranker(wave_id)
            .await?
            .page_rank()
            .await?;
        Ok(())
    }

    pub async fn page_rank(mut self) -> Result<(), anyhow::Error> {
        let mut master_model = self.backend.build_master().await?;
        self.page_rank_for_wave_id(master_model.wave_id()).await
    }

    /// Tests a URL and says what is happening.
    pub async fn test_url(mut self, profile: Arc<Profile>, url: Url) -> TestRunReport {
        // Load dummy data model:
        let mut master_model = self
            .backend
            .build_master()
            .await
            .expect("failed to build master backend");
        let worker_backend_factory: Arc<_> = self
            .backend
            .build_worker_factory(master_model.wave_id())
            .into();

        // Creates a counter to get stats:
        let counter = Arc::new(Counter::default());

        CrawlWorker::new(
            self.configuration.as_ref(),
            worker_backend_factory,
            counter,
            profile,
        )
        .test_url(url)
        .await
    }
}
