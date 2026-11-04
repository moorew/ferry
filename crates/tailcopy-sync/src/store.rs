//! How the engine reaches the store: closures run on whichever thread owns it (the
//! app's store actor, or a dedicated thread in the relay and tests).

use std::sync::Arc;

use tailcopy_core::Store;
use tokio::sync::oneshot;

pub type StoreJob = Box<dyn FnOnce(&mut Store) + Send>;

/// Something that runs jobs against the one writable [`Store`], in order.
pub trait StoreRunner: Send + Sync + 'static {
    fn submit(&self, job: StoreJob);
}

#[derive(Clone)]
pub struct StoreHandle(Arc<dyn StoreRunner>);

#[derive(Debug, thiserror::Error)]
#[error("the store is gone")]
pub struct StoreGone;

impl StoreHandle {
    pub fn new(runner: impl StoreRunner) -> Self {
        Self(Arc::new(runner))
    }

    /// Runs `f` on the store's thread and waits for the result without blocking.
    pub async fn call<T, F>(&self, f: F) -> Result<T, StoreGone>
    where
        T: Send + 'static,
        F: FnOnce(&mut Store) -> T + Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        self.0.submit(Box::new(move |store| {
            let _ = tx.send(f(store));
        }));
        rx.await.map_err(|_| StoreGone)
    }
}

struct Thread(std::sync::mpsc::Sender<StoreJob>);

impl StoreRunner for Thread {
    fn submit(&self, job: StoreJob) {
        let _ = self.0.send(job);
    }
}

/// Moves `store` onto its own thread (the relay and tests; the app has its actor).
pub fn spawn_store_thread(store: Store) -> StoreHandle {
    let (tx, rx) = std::sync::mpsc::channel::<StoreJob>();
    std::thread::Builder::new()
        .name("store".into())
        .spawn(move || {
            let mut store = store;
            for job in rx {
                job(&mut store);
            }
        })
        .expect("store thread");
    StoreHandle::new(Thread(tx))
}
