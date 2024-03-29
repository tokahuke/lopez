mod dummy;

pub use async_trait::async_trait;
pub use hyper::StatusCode;
pub use serde_json::Value;
pub use structopt::StructOpt;
pub use url::Url;

pub use typetag;

pub use crate::crawler::Reason;
pub use crate::Type;

pub use self::dummy::DummyBackend;

use serde_derive::Serialize;
use std::fmt::Debug;

use crate::page_rank::power_iteration;

#[derive(Debug, Serialize)]
pub struct WaveRemoveReport {
    was_removed: bool,
    removed_pages: usize,
}

impl WaveRemoveReport {
    pub fn not_removed() -> WaveRemoveReport {
        WaveRemoveReport {
            was_removed: false,
            removed_pages: 0,
        }
    }

    pub fn removed(removed_pages: usize) -> WaveRemoveReport {
        WaveRemoveReport {
            was_removed: true,
            removed_pages: removed_pages,
        }
    }

    pub fn was_removed(&self) -> bool {
        self.was_removed
    }
}

#[async_trait(?Send)]
pub trait Backend: Sized {
    type Config: StructOpt;
    type Ranker: PageRanker;

    async fn init(config: Self::Config, wave: &str) -> Result<Self, anyhow::Error>;
    async fn build_master(&mut self) -> Result<Box<dyn MasterBackend>, anyhow::Error>;
    fn build_worker_factory(&mut self, wave_id: i32) -> Box<dyn WorkerBackendFactory>;
    async fn build_ranker(&mut self, wave_id: i32) -> Result<Self::Ranker, anyhow::Error>;

    /// This may become a mandatory method in future releases.
    async fn remove(&mut self) -> Result<WaveRemoveReport, anyhow::Error> {
        Ok(WaveRemoveReport::not_removed())
    }
}

#[async_trait(?Send)]
pub trait MasterBackend {
    fn wave_id(&mut self) -> i32;
    async fn ensure_seeded(&mut self, seeds: &[Url]) -> Result<(), anyhow::Error>;
    async fn create_analyses(&mut self, analyses: &[(String, Type)]) -> Result<(), anyhow::Error>;
    async fn count_crawled(&mut self) -> Result<usize, anyhow::Error>;
    async fn reset_queue(&mut self) -> Result<(), anyhow::Error>;
    async fn exists_taken(&mut self) -> Result<bool, anyhow::Error>;
    async fn fetch(
        &mut self,
        batch_size: i64,
        max_depth: i16,
    ) -> Result<Vec<(Url, u16)>, anyhow::Error>;
}

#[typetag::serde(tag = "type")]
#[async_trait(?Send)]
pub trait WorkerBackendFactory: 'static + Send + Sync + Debug {
    async fn build(&self) -> Result<Box<dyn WorkerBackend>, anyhow::Error>;
}

#[async_trait(?Send)]
pub trait WorkerBackend {
    async fn ensure_active(&self, url: &Url) -> Result<(), anyhow::Error>;

    async fn ensure_analyzed(
        &self,
        url: &Url,
        analyses: Vec<(String, Value)>,
    ) -> Result<(), anyhow::Error>;

    async fn ensure_explored(
        &self,
        from_url: &Url,
        status_code: StatusCode,
        link_depth: u16,
        links: Vec<(Reason, Url)>,
    ) -> Result<(), anyhow::Error>;

    async fn ensure_error(&self, url: &Url) -> Result<(), anyhow::Error>;
}

#[async_trait(?Send)]
pub trait PageRanker {
    type PageId: Ord + Clone;

    async fn linkage(
        &mut self,
    ) -> Result<Box<dyn Iterator<Item = (Self::PageId, Self::PageId)>>, anyhow::Error>;
    async fn push_page_ranks(
        &mut self,
        ranked: &[(Self::PageId, f64)],
    ) -> Result<(), anyhow::Error>;

    async fn page_rank(&mut self) -> Result<(), anyhow::Error> {
        // Create a stream of links:
        let edges = self.linkage().await?;

        // Now, do power iteration and put the result in the DB in batches:
        let mut ranked = Vec::with_capacity(1024);
        for (from_id, rank) in power_iteration(edges, 2048, 8) {
            if ranked.len() < 1024 {
                ranked.push((from_id, rank as f64));
            } else {
                self.push_page_ranks(&ranked).await?;
                ranked.clear();
            }
        }

        // End by submitting what was missing:
        self.push_page_ranks(&ranked).await?;

        Ok(())
    }
}
