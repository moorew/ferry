//! The store actor (SPEC 5.1): one thread owns the SQLite write connection and the
//! warm search index. Everything else talks to it through [`Db::run`], so the UI
//! thread never waits on disk.

use std::sync::{Arc, OnceLock, mpsc};

use tailcopy_core::{SearchIndex, Store};
use tokio::sync::oneshot;

pub struct Core {
    pub store: Store,
    pub index: SearchIndex,
}

impl Core {
    /// Brings the search index up to date with whatever the last mutation changed.
    /// Returns true when anything changed.
    pub fn refresh(&mut self) -> bool {
        let groups = self.store.take_dirty_groups();
        if groups.is_empty() {
            return false;
        }
        let view = self.store.view();
        if let Err(e) = self.index.refresh_groups(&view, &groups) {
            tracing::warn!("search index refresh failed, reloading: {e}");
            if let Err(e) = self.index.reload(&view) {
                tracing::error!("search index reload failed: {e}");
            }
        }
        true
    }
}

pub type Job = Box<dyn FnOnce(&mut Core) + Send>;
type Hook = Box<dyn Fn() + Send + Sync>;

#[derive(Clone)]
pub struct Db {
    tx: mpsc::Sender<Job>,
    on_change: Arc<OnceLock<Hook>>,
}

impl Db {
    pub fn spawn(store: Store) -> tailcopy_core::Result<Db> {
        let index = SearchIndex::load(&store.view())?;
        let (tx, rx) = mpsc::channel::<Job>();
        let on_change: Arc<OnceLock<Hook>> = Arc::default();
        let hook = on_change.clone();
        std::thread::Builder::new()
            .name("store".into())
            .spawn(move || {
                let mut core = Core { store, index };
                for job in rx {
                    // Every op, local or from a peer, moves the clock: tell sync.
                    let before = core.store.last_hlc();
                    job(&mut core);
                    if core.store.last_hlc() != before
                        && let Some(hook) = hook.get()
                    {
                        hook();
                    }
                }
            })
            .map_err(|e| tailcopy_core::Error::Invalid(format!("store thread: {e}")))?;
        Ok(Db { tx, on_change })
    }

    /// Called on the store thread after any job that changed the store's clock.
    pub fn on_change(&self, hook: impl Fn() + Send + Sync + 'static) {
        let _ = self.on_change.set(Box::new(hook));
    }

    /// Queues `job` without waiting for it.
    pub fn submit(&self, job: Job) {
        self.tx.send(job).expect("store thread is running");
    }

    /// Runs `f` on the store thread and waits for its result without blocking.
    pub async fn run<T, F>(&self, f: F) -> T
    where
        T: Send + 'static,
        F: FnOnce(&mut Core) -> T + Send + 'static,
    {
        let (reply, answer) = oneshot::channel();
        let job: Job = Box::new(move |core| {
            let _ = reply.send(f(core));
        });
        self.tx.send(job).expect("store thread is running");
        answer.await.expect("store thread answered")
    }
}
