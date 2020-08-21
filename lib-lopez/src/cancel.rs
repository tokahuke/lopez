use futures::channel::oneshot;
use futures::future::select;
use futures::prelude::*;
use std::fmt::Debug;
use tokio::runtime::Runtime;
use tokio::task::LocalSet;

use crate::panic::log_panics;

/// Runs a task on a fresh new thread and returns a handle that can stop if it hits a breakpoint.
/// Note that `Fut` need not be `Send`.
pub fn spawn_onto_thread<F, Fut, E>(name: String, f: F) -> Canceler
where
    F: 'static + Send + FnOnce() -> Fut,
    Fut: 'static + Future<Output = Result<(), E>>,
    E: Debug,
{
    let (sender, receiver) = oneshot::channel();
    let (send_back, receive_back) = oneshot::channel();

    // TODO: force that globally!
    // // Sets panics in worker to be logged.

    std::thread::Builder::new()
        .name(name.clone())
        .spawn(move || {
            log_panics(); // can only be called once per thread!
            let mut runtime = Runtime::new().expect("can always init runtime");

            LocalSet::new().block_on(&mut runtime, async move {
                // Guard to log *all* errors:
                let logged = f().map(move |result| {
                    if let Err(error) = result {
                        log::error!("{} failed: {:?}", name, error);
                    }
                });

                select(logged.boxed_local(), receiver).await;
                send_back.send(()).ok();
            });
        })
        .expect("can always spawn");

    Canceler(sender, receive_back)
}

pub struct Canceler(oneshot::Sender<()>, oneshot::Receiver<()>);

impl Canceler {
    pub async fn cancel(self) {
        if self.0.send(()).is_ok() {
            self.1.await.ok();
        }
    }
}
