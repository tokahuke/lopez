pub use async_trait::async_trait;
pub use hyper::StatusCode;
pub use serde_json::Value;
pub use structopt::StructOpt;
pub use url::Url;

pub use crate::crawler::Reason;

use crate::page_rank::power_iteration;

#[async_trait(?Send)]
pub trait Backend: Sized {
    type Error: Into<crate::Error>;
    type Config: StructOpt;
    type Master: MasterBackend<Error = Self::Error>;
    type WorkerFactory: WorkerBackendFactory<Error = Self::Error>;
    type Ranker: PageRanker<Error = Self::Error>;

    async fn init(config: Self::Config, wave: &str) -> Result<Self, Self::Error>;
    async fn build_master(&self) -> Result<Self::Master, Self::Error>;
    fn build_worker_factory(&self, wave_id: i32) -> Self::WorkerFactory;
    async fn build_ranker(&self, wave_id: i32) -> Result<Self::Ranker, Self::Error>;
}

#[async_trait(?Send)]
pub trait MasterBackend {
    type Error: Into<crate::Error>;

    fn wave_id(&self) -> i32;
    async fn ensure_seeded(&self, seeds: &[Url]) -> Result<(), Self::Error>;
    async fn create_analyses(&self, analysis_names: &[String]) -> Result<(), Self::Error>;
    async fn count_crawled(&self) -> Result<usize, Self::Error>;
    async fn reset_queue(&self) -> Result<(), Self::Error>;
    async fn fetch(&self, batch_size: i64, max_depth: i16) -> Result<Vec<(Url, u16)>, Self::Error>;
}

#[async_trait(?Send)]
pub trait WorkerBackendFactory: 'static + Send + Sync {
    type Error: Into<crate::Error>;
    type Worker: WorkerBackend<Error = Self::Error>;
    async fn build(&self) -> Result<Self::Worker, Self::Error>;
}

#[async_trait(?Send)]
pub trait WorkerBackend {
    type Error: Into<crate::Error>;

    async fn ensure_analyzed(
        &self,
        url: &Url,
        analyses: Vec<(String, Value)>,
    ) -> Result<(), Self::Error>;

    async fn ensure_explored(
        &self,
        from_url: &Url,
        status_code: StatusCode,
        link_depth: u16,
        links: Vec<(Reason, Url)>,
    ) -> Result<(), Self::Error>;

    async fn ensure_error(&self, url: &Url) -> Result<(), Self::Error>;
}

#[async_trait(?Send)]
pub trait PageRanker {
    type Error: Into<crate::Error>;
    type PageId: Ord + Clone;

    async fn linkage(
        &self,
    ) -> Result<Box<dyn Iterator<Item = (Self::PageId, Self::PageId)>>, Self::Error>;
    async fn push_page_ranks(&self, ranked: &[(Self::PageId, f64)]) -> Result<(), Self::Error>;

    async fn page_rank(&self) -> Result<(), Self::Error> {
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
