//! The store actor (SPEC 5.1): one thread owns the SQLite write connection and the
//! warm search index. Everything else talks to it through [`Db::run`], so the UI
//! thread never waits on disk.

use std::sync::mpsc;

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

type Job = Box<dyn FnOnce(&mut Core) + Send>;

#[derive(Clone)]
pub struct Db {
    tx: mpsc::Sender<Job>,
}

impl Db {
    pub fn spawn(store: Store) -> tailcopy_core::Result<Db> {
        let index = SearchIndex::load(&store.view())?;
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("store".into())
            .spawn(move || {
                let mut core = Core { store, index };
                for job in rx {
                    job(&mut core);
                }
            })
            .map_err(|e| tailcopy_core::Error::Invalid(format!("store thread: {e}")))?;
        Ok(Db { tx })
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
