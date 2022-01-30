mod rpc;

pub use self::rpc::serve;

use async_trait::async_trait;
use futures::prelude::*;
use std::net::SocketAddr;
use std::sync::Arc;
use url::Url;

use crate::backend::WorkerBackendFactory;
use crate::crawler::{Configuration, Counter, WorkerHandler, WorkerHandlerFactory, WorkerId};
use crate::Profile;

use self::rpc::{RemoteWorkerId, Token};

pub struct RemoteWorkerHandlerFactory {
    token: Token,
    max_retries: usize,
    pool: Vec<Arc<self::rpc::CrawlerRpcClient>>,
}

impl RemoteWorkerHandlerFactory {
    pub async fn connect(
        token: String,
        max_retries: usize,
        servers: &[SocketAddr],
    ) -> Result<Self, anyhow::Error> {
        let pool = futures::stream::iter(servers)
            .then(|&socket_addr| self::rpc::connect(socket_addr))
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;

        Ok(RemoteWorkerHandlerFactory {
            token: token.into(),
            max_retries,
            pool,
        })
    }
}

#[async_trait]
impl WorkerHandlerFactory for RemoteWorkerHandlerFactory {
    type Handler = RemoteWorkerHandler;
    async fn build(
        &self,
        configuration: Arc<dyn Configuration>,
        worker_backend_factory: Arc<dyn WorkerBackendFactory>,
        profile: Arc<Profile>,
        counter: Arc<Counter>,
        worker_id: WorkerId,
    ) -> Result<Self::Handler, anyhow::Error> {
        let client = self.pool[worker_id as usize % self.pool.len()].clone();
        let token = self.token.clone();

        let remote_worker_id = client
            .build_worker(
                tarpc::context::current(),
                self.token.clone(),
                configuration,
                worker_backend_factory,
                profile,
                counter,
                worker_id,
            )
            .await??;

        Ok(RemoteWorkerHandler {
            token,
            max_retries: self.max_retries,
            remote_worker_id,
            client,
        })
    }
}

pub struct RemoteWorkerHandler {
    token: Token,
    max_retries: usize,
    remote_worker_id: RemoteWorkerId,
    client: Arc<self::rpc::CrawlerRpcClient>,
}

#[async_trait]
impl WorkerHandler for RemoteWorkerHandler {
    async fn send_task(&mut self, url: Url, depth: u16) -> Result<(), ()> {
        let mut retry = 1;

        while retry <= self.max_retries {
            let outcome = self
                .client
                .send_task(
                    tarpc::context::current(),
                    self.token.clone(),
                    self.remote_worker_id,
                    url.clone(),
                    depth,
                )
                .await;

            match outcome {
                Ok(Ok(_)) => return Ok(()),
                Ok(Err(err)) => {
                    log::error!("Error from RPC worker: {err}");
                    return Err(());
                }
                Err(err) => {
                    log::warn!("RPC transport error ({retry}/{}): {err}", self.max_retries);
                    retry += 1;
                }
            }
        }

        Err(())
    }

    async fn terminate(self) {
        let mut retry = 1;

        while retry <= self.max_retries {
            let outcome = self
                .client
                .terminate(
                    tarpc::context::current(),
                    self.token.clone(),
                    self.remote_worker_id,
                )
                .await;

            match outcome {
                Ok(Ok(_)) => return,
                Ok(Err(err)) => {
                    log::warn!("Error from RPC worker trying to terminate: {err}");
                    return;
                }
                Err(err) => {
                    log::warn!("RPC transport error ({retry}/{}): {err}", self.max_retries);
                    retry += 1;
                }
            }
        }

        log::warn!("Worker dangling");
    }
}
