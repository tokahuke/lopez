#[macro_use]
mod db;
mod master;
mod ranker;
mod worker;
mod error;

use std::sync::Arc;

use lib_lopez::backend::{async_trait, Backend, WorkerBackendFactory};

use crate::db::DbConfig;
use crate::error::Error;

use self::master::PostgresMasterBackend;
use self::ranker::PostgresPageRanker;
use self::worker::PostgresWorkerBackend;

pub struct PostgresBackend {
    config: Arc<DbConfig>,
    wave: String,
}

#[async_trait(?Send)]
impl Backend for PostgresBackend {
    type Error = crate::Error;
    type Config = DbConfig;
    type Master = PostgresMasterBackend;
    type WorkerFactory = PostgresWorkerFactory;
    type Ranker = PostgresPageRanker;

    async fn init(config: Self::Config, wave: &str) -> Result<Self, crate::Error> {
        // Make db Arc:
        let config = Arc::new(config);

        // Make sure db exists and is up to date:
        // This thing blocks, but no problemo; it is not critical:
        config.ensure_create_db().await?;
        config.clone().sync_migrations().await?;

        Ok(PostgresBackend {
            config,
            wave: wave.to_owned(),
        })
    }

    async fn build_master(&self) -> Result<Self::Master, crate::Error> {
        Ok(PostgresMasterBackend::init(self.config.connect().await?, &self.wave).await?)
    }

    fn build_worker_factory(&self, wave_id: i32) -> Self::WorkerFactory {
        PostgresWorkerFactory {
            config: self.config.clone(),
            wave_id,
        }
    }

    async fn build_ranker(&self, wave_id: i32) -> Result<Self::Ranker, crate::Error> {
        Ok(PostgresPageRanker::init(self.config.connect().await?, wave_id).await?)
    }
}

pub struct PostgresWorkerFactory {
    config: Arc<DbConfig>,
    wave_id: i32,
}

#[async_trait(?Send)]
impl WorkerBackendFactory for PostgresWorkerFactory {
    type Error = crate::Error;
    type Worker = PostgresWorkerBackend;
    async fn build(&self) -> Result<Self::Worker, crate::Error> {
        Ok(PostgresWorkerBackend::init(self.config.connect().await?, self.wave_id).await?)
    }
}
