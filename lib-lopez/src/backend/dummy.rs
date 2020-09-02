use super::*;

/// A backend implementation which is actually not a backend at all and will
/// panic if used.
#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DummyBackend {}
#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DummyMasterBackend {}
#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DummyWorkerBackendFactory {}
#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DummyWorkerBackend {}
#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DummyPageRanker {}

#[derive(StructOpt, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DummyConfig {}

#[async_trait(?Send)]
impl Backend for DummyBackend {
    type Error = crate::Error;
    type Config = DummyConfig;
    type Master = DummyMasterBackend;
    type WorkerFactory = DummyWorkerBackendFactory;
    type Ranker = DummyPageRanker;

    async fn init(_config: Self::Config, _wave: &str) -> Result<Self, Self::Error> {
        Ok(DummyBackend {})
    }

    async fn build_master(&mut self) -> Result<Self::Master, Self::Error> {
        Ok(DummyMasterBackend {})
    }

    fn build_worker_factory(&mut self, _wave_id: i32) -> Self::WorkerFactory {
        DummyWorkerBackendFactory {}
    }

    async fn build_ranker(&mut self, _wave_id: i32) -> Result<Self::Ranker, Self::Error> {
        Ok(DummyPageRanker {})
    }
}

#[async_trait(?Send)]
impl MasterBackend for DummyMasterBackend {
    type Error = crate::Error;

    fn wave_id(&mut self) -> i32 {
        0
    }

    async fn ensure_seeded(&mut self, _seeds: &[Url]) -> Result<(), Self::Error> {
        panic!("cannot use DummyMasterBackend");
    }

    async fn create_analyses(&mut self, _analysis_names: &[String]) -> Result<(), Self::Error> {
        panic!("cannot use DummyMasterBackend");
    }

    async fn count_crawled(&mut self) -> Result<usize, Self::Error> {
        panic!("cannot use DummyMasterBackend");
    }

    async fn reset_queue(&mut self) -> Result<(), Self::Error> {
        panic!("cannot use DummyMasterBackend");
    }

    async fn fetch(
        &mut self,
        _batch_size: i64,
        _max_depth: i16,
    ) -> Result<Vec<(Url, u16)>, Self::Error> {
        panic!("cannot use DummyMasterBackend");
    }
}

#[async_trait(?Send)]
impl WorkerBackendFactory for DummyWorkerBackendFactory {
    type Error = crate::Error;
    type Worker = DummyWorkerBackend;

    async fn build(&mut self) -> Result<Self::Worker, Self::Error> {
        Ok(DummyWorkerBackend {})
    }
}

#[async_trait(?Send)]
impl WorkerBackend for DummyWorkerBackend {
    type Error = crate::Error;

    async fn ensure_analyzed(
        &self,
        _url: &Url,
        _analyses: Vec<(String, Value)>,
    ) -> Result<(), Self::Error> {
        panic!("cannot use DummyWorkerBackend");
    }

    async fn ensure_explored(
        &self,
        _from_url: &Url,
        _status_code: StatusCode,
        _link_depth: u16,
        _links: Vec<(Reason, Url)>,
    ) -> Result<(), Self::Error> {
        panic!("cannot use DummyWorkerBackend");
    }

    async fn ensure_error(&self, _url: &Url) -> Result<(), Self::Error> {
        panic!("cannot use DummyWorkerBackend");
    }
}

#[async_trait(?Send)]
impl PageRanker for DummyPageRanker {
    type Error = crate::Error;
    type PageId = i32;

    async fn linkage(
        &mut self,
    ) -> Result<Box<dyn Iterator<Item = (Self::PageId, Self::PageId)>>, Self::Error> {
        panic!("cannot use DummyPageRanker");
    }

    async fn push_page_ranks(
        &mut self,
        _ranked: &[(Self::PageId, f64)],
    ) -> Result<(), Self::Error> {
        panic!("cannot use DummyPageRanker");
    }
}
