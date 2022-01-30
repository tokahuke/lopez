use futures::prelude::*;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use tarpc::{context::Context, server::Channel, service};
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};
use url::Url;

use crate::backend::WorkerBackendFactory;
use crate::crawler::{
    Configuration, LocalHandler, LocalHandlerFactory, WorkerHandler, WorkerHandlerFactory, WorkerId,
};
use crate::Profile;

pub type RemoteWorkerId = u64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token(Arc<str>);

impl From<String> for Token {
    fn from(s: String) -> Token {
        Token(s.into())
    }
}

#[derive(Debug, Serialize, Deserialize, Error)]
#[non_exhaustive]
pub enum RpcError {
    #[error("bad token: {0:?}")]
    BadToken(Token),
    #[error("no such remote worker: {0}")]
    NoSuchRemoteWorker(RemoteWorkerId),
    #[error("failed to send task: depth={1} url={0}")]
    FailedToSendTask(Url, u16),
}

#[service]
pub trait CrawlerRpc {
    async fn build_worker(
        token: Token,
        configuration: Arc<dyn Configuration>,
        worker_backend_factory: Arc<dyn WorkerBackendFactory>,
        profile: Arc<Profile>,
        worker_id: WorkerId,
    ) -> Result<RemoteWorkerId, RpcError>;
    async fn send_task(
        token: Token,
        remote_worker_id: RemoteWorkerId,
        url: Url,
        depth: u16,
    ) -> Result<(), RpcError>;
    async fn terminate(token: Token, remote_worker_id: RemoteWorkerId) -> Result<(), RpcError>;
}

struct CrawlerRpcServerInner {
    token: Token,
    handlers: RwLock<BTreeMap<RemoteWorkerId, Mutex<LocalHandler>>>,
}

impl CrawlerRpcServerInner {
    pub fn new(token: String) -> CrawlerRpcServerInner {
        CrawlerRpcServerInner {
            token: token.into(),
            handlers: RwLock::default(),
        }
    }
}

#[derive(Clone)]
pub struct CrawlerRpcServer(Arc<CrawlerRpcServerInner>);

impl CrawlerRpc for CrawlerRpcServer {
    type BuildWorkerFut = Pin<Box<dyn Send + Future<Output = Result<RemoteWorkerId, RpcError>>>>;
    type SendTaskFut = Pin<Box<dyn Send + Future<Output = Result<(), RpcError>>>>;
    type TerminateFut = Pin<Box<dyn Send + Future<Output = Result<(), RpcError>>>>;

    fn build_worker(
        self,
        _: Context,
        token: Token,
        configuration: Arc<dyn Configuration>,
        worker_backend_factory: Arc<dyn WorkerBackendFactory>,
        profile: Arc<Profile>,
        worker_id: WorkerId,
    ) -> Self::BuildWorkerFut {
        Box::pin(async move {
            if token != self.0.token {
                return Err(RpcError::BadToken(token));
            }

            let local_handler = LocalHandlerFactory
                .build(configuration, worker_backend_factory, profile, worker_id)
                .await
                .expect("local handlers always succeed");

            let remote_worker_id = rand::random();
            self.0
                .handlers
                .write()
                .await
                .insert(remote_worker_id, Mutex::new(local_handler));

            Ok(remote_worker_id)
        })
    }

    fn send_task(
        self,
        _: Context,
        token: Token,
        remote_worker_id: RemoteWorkerId,
        url: Url,
        depth: u16,
    ) -> Self::SendTaskFut {
        Box::pin(async move {
            if token != self.0.token {
                return Err(RpcError::BadToken(token));
            }

            if let Some(handler) = self.0.handlers.read().await.get(&remote_worker_id) {
                let outcome = handler.lock().await.send_task(url.clone(), depth).await;
                match outcome {
                    Ok(()) => Ok(()),
                    Err(()) => Err(RpcError::FailedToSendTask(url, depth)),
                }
            } else {
                Err(RpcError::NoSuchRemoteWorker(remote_worker_id))
            }
        })
    }

    fn terminate(
        self,
        _: Context,
        token: Token,
        remote_worker_id: RemoteWorkerId,
    ) -> Self::TerminateFut {
        Box::pin(async move {
            if token != self.0.token {
                return Err(RpcError::BadToken(token));
            }

            let maybe_handler = self.0.handlers.write().await.remove(&remote_worker_id);

            if let Some(handler) = maybe_handler {
                handler.into_inner().terminate().await;
                Ok(())
            } else {
                Err(RpcError::NoSuchRemoteWorker(remote_worker_id))
            }
        })
    }
}

pub async fn connect(server_addr: SocketAddr) -> Result<Arc<CrawlerRpcClient>, anyhow::Error> {
    let transport = tarpc::serde_transport::tcp::connect(
        server_addr,
        tarpc::tokio_serde::formats::Json::default,
    )
    .await?;
    let connection = CrawlerRpcClient::new(tarpc::client::Config::default(), transport).spawn();

    Ok(Arc::new(connection))
}

pub async fn serve(
    token: String,
    max_connections: usize,
    server_addr: SocketAddr,
) -> Result<(), anyhow::Error> {
    log::info!("Server starting at {server_addr}");

    let inner = Arc::new(CrawlerRpcServerInner::new(token));

    let listener = tarpc::serde_transport::tcp::listen(
        server_addr,
        tarpc::tokio_serde::formats::Json::default,
    )
    .await?;

    listener
        // Ignore accept errors.
        .filter_map(|r| future::ready(r.ok()))
        .map(tarpc::server::BaseChannel::with_defaults)
        // serve is generated by the service attribute. It takes as input any type implementing
        // the generated World trait.
        .map(|channel| {
            let server = CrawlerRpcServer(inner.clone());
            channel.execute(server.serve())
        })
        // Max channels.
        .buffer_unordered(max_connections)
        .for_each(|_| async {})
        .await;

    Ok(())
}
