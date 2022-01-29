use std::collections::BTreeMap;
use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use tarpc::{context::Context, service};
use tokio::sync::{Mutex, RwLock};
use url::Url;

use crate::backend::WorkerBackendFactory;
use crate::crawler::worker::{LocalHandler, WorkerHandler, WorkerHandlerFactory};
use crate::crawler::{Configuration, Counter};
use crate::crawler::{LocalHandlerFactory, Origins, WorkerId};
use crate::Profile;

pub type RemoteWorkerId = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token(Arc<str>);

#[derive(Debug)]
#[non_exhaustive]
pub enum RpcError {
    BadToken(Token),
    NoSuchRemoteWorker(RemoteWorkerId),
    FailedToSendTask(Url, u16),
}

#[service]
pub trait CrawlerRpc {
    async fn build_worker(
        token: Token,
        configuration: Arc<dyn Configuration>,
        worker_backend_factory: Arc<dyn WorkerBackendFactory>,
        profile: Arc<Profile>,
        counter: Arc<Counter>,
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

pub struct CrawlerRpcServerInner {
    token: Token,
    origins: Arc<Origins>,
    handlers: RwLock<BTreeMap<RemoteWorkerId, Mutex<LocalHandler>>>,
}

#[derive(Clone)]
pub struct CrawlerRpcServer(Arc<CrawlerRpcServerInner>);

impl Deref for CrawlerRpcServer {
    type Target = CrawlerRpcServerInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl CrawlerRpc for CrawlerRpcServer {
    type BuildWorkerFut = Pin<Box<dyn Future<Output = Result<RemoteWorkerId, RpcError>>>>;
    type SendTaskFut = Pin<Box<dyn Future<Output = Result<(), RpcError>>>>;
    type TerminateFut = Pin<Box<dyn Future<Output = Result<(), RpcError>>>>;

    fn build_worker(
        self,
        _: Context,
        token: Token,
        configuration: Arc<dyn Configuration>,
        worker_backend_factory: Arc<dyn WorkerBackendFactory>,
        profile: Arc<Profile>,
        counter: Arc<Counter>,
        worker_id: WorkerId,
    ) -> Self::BuildWorkerFut {
        Box::pin(async move {
            if token != self.token {
                return Err(RpcError::BadToken(token));
            }

            let local_handler = LocalHandlerFactory.build(
                &*configuration,
                worker_backend_factory,
                profile,
                counter,
                self.origins.clone(),
                worker_id,
            );
            let remote_worker_id = rand::random();
            self.handlers
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
            if token != self.token {
                return Err(RpcError::BadToken(token));
            }

            if let Some(handler) = self.handlers.read().await.get(&remote_worker_id) {
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
            if token != self.token {
                return Err(RpcError::BadToken(token));
            }

            let maybe_handler = self.handlers.write().await.remove(&remote_worker_id);

            if let Some(handler) = maybe_handler {
                handler.into_inner().terminate().await;
                Ok(())
            } else {
                Err(RpcError::NoSuchRemoteWorker(remote_worker_id))
            }
        })
    }
}
