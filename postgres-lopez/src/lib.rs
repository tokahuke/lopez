#[macro_use]
mod db;
mod error;
mod master;
mod ranker;
mod worker;

use std::rc::Rc;
use std::sync::Arc;

use lib_lopez::backend::{async_trait, Backend, WaveRemoveReport, WorkerBackendFactory};

use crate::db::DbConfig;
use crate::error::Error;

use self::master::PostgresMasterBackend;
use self::ranker::PostgresPageRanker;
use self::worker::PostgresWorkerBackend;

const REMOVE_WAVE: &str = include_str!("sql/remove_wave.sql");

pub struct PostgresBackend {
    config: Arc<DbConfig>,
    wave: String,
}

impl PostgresBackend {
    async fn connect(&self) -> Result<Rc<tokio_postgres::Client>, crate::Error> {
        self.config.connect().await
    }

    async fn init(config: Arc<DbConfig>, wave: String) -> Result<PostgresBackend, crate::Error> {
        Ok(PostgresBackend { config, wave })
    }
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
        // This thing blocks, but no problemo: it is not critical.
        config.ensure_create_db().await?;
        config.clone().sync_migrations().await?;

        PostgresBackend::init(config, wave.to_owned()).await
    }

    async fn build_master(&mut self) -> Result<Self::Master, crate::Error> {
        Ok(PostgresMasterBackend::init(self.connect().await?, &self.wave).await?)
    }

    fn build_worker_factory(&mut self, wave_id: i32) -> Self::WorkerFactory {
        PostgresWorkerFactory {
            config: self.config.clone(),
            wave_id,
        }
    }

    async fn build_ranker(&mut self, wave_id: i32) -> Result<Self::Ranker, crate::Error> {
        Ok(PostgresPageRanker::init(self.connect().await?, wave_id).await?)
    }

    async fn remove(&mut self) -> Result<WaveRemoveReport, crate::Error> {
        let row = self
            .connect()
            .await?
            .query_opt(REMOVE_WAVE, &[&self.wave])
            .await?
            .expect("remove_wave.sql always returns one row");
        let report = if row.get::<_, Option<i32>>("wave_id").is_some() {
            WaveRemoveReport::removed(row.get::<_, i32>("n_pages") as usize)
        } else {
            WaveRemoveReport::not_removed()
        };

        Ok(report)
        // PostgresMasterBackend::init(self.connect().await?, &self.wave).await?.delete().await
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
    async fn build(&mut self) -> Result<Self::Worker, crate::Error> {
        Ok(PostgresWorkerBackend::init(self.config.connect().await?, self.wave_id).await?)
    }
}
