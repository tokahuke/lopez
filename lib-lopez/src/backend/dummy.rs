use super::*;
use serde_derive::{Deserialize, Serialize};

/// A backend implementation which is actually not a backend at all and will
/// panic if used.
#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DummyBackend;
#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DummyMasterBackend;
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct DummyWorkerBackendFactory;
#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DummyWorkerBackend;
#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DummyPageRanker;

#[derive(StructOpt, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DummyConfig {}

#[async_trait(?Send)]
impl Backend for DummyBackend {
    type Config = DummyConfig;
    type Ranker = DummyPageRanker;

    async fn init(_config: Self::Config, _wave: &str) -> Result<Self, anyhow::Error> {
        Ok(DummyBackend)
    }

    async fn build_master(&mut self) -> Result<Box<dyn MasterBackend>, anyhow::Error> {
        Ok(Box::new(DummyMasterBackend))
    }

    fn build_worker_factory(&mut self, _wave_id: i32) -> Box<dyn WorkerBackendFactory> {
        Box::new(DummyWorkerBackendFactory)
    }

    async fn build_ranker(&mut self, _wave_id: i32) -> Result<Self::Ranker, anyhow::Error> {
        Ok(DummyPageRanker)
    }
}

#[async_trait(?Send)]
impl MasterBackend for DummyMasterBackend {
    fn wave_id(&mut self) -> i32 {
        0
    }

    async fn ensure_seeded(&mut self, _seeds: &[Url]) -> Result<(), anyhow::Error> {
        panic!("cannot use DummyMasterBackend");
    }

    async fn create_analyses(
        &mut self,
        _analysis_names: &[(String, Type)],
    ) -> Result<(), anyhow::Error> {
        panic!("cannot use DummyMasterBackend");
    }

    async fn count_crawled(&mut self) -> Result<usize, anyhow::Error> {
        panic!("cannot use DummyMasterBackend");
    }

    async fn reset_queue(&mut self) -> Result<(), anyhow::Error> {
        panic!("cannot use DummyMasterBackend");
    }

    async fn exists_taken(&mut self) -> Result<bool, anyhow::Error> {
        panic!("cannot use DummyMasterBackend");
    }

    async fn fetch(
        &mut self,
        _batch_size: i64,
        _max_depth: i16,
    ) -> Result<Vec<(Url, u16)>, anyhow::Error> {
        panic!("cannot use DummyMasterBackend");
    }
}

#[typetag::serde]
#[async_trait(?Send)]
impl WorkerBackendFactory for DummyWorkerBackendFactory {
    async fn build(&self) -> Result<Box<dyn WorkerBackend>, anyhow::Error> {
        Ok(Box::new(DummyWorkerBackend))
    }
}

#[async_trait(?Send)]
impl WorkerBackend for DummyWorkerBackend {
    async fn ensure_active(&self, _url: &Url) -> Result<(), anyhow::Error> {
        panic!("cannot use DummyWorkerBackend");
    }

    async fn ensure_analyzed(
        &self,
        _url: &Url,
        _analyses: Vec<(String, Value)>,
    ) -> Result<(), anyhow::Error> {
        panic!("cannot use DummyWorkerBackend");
    }

    async fn ensure_explored(
        &self,
        _from_url: &Url,
        _status_code: StatusCode,
        _link_depth: u16,
        _links: Vec<(Reason, Url)>,
    ) -> Result<(), anyhow::Error> {
        panic!("cannot use DummyWorkerBackend");
    }

    async fn ensure_error(&self, _url: &Url) -> Result<(), anyhow::Error> {
        panic!("cannot use DummyWorkerBackend");
    }
}

#[async_trait(?Send)]
impl PageRanker for DummyPageRanker {
    type PageId = i32;

    async fn linkage(
        &mut self,
    ) -> Result<Box<dyn Iterator<Item = (Self::PageId, Self::PageId)>>, anyhow::Error> {
        panic!("cannot use DummyPageRanker");
    }

    async fn push_page_ranks(
        &mut self,
        _ranked: &[(Self::PageId, f64)],
    ) -> Result<(), anyhow::Error> {
        panic!("cannot use DummyPageRanker");
    }
}
