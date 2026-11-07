//! The store actor (SPEC 21.2): one thread owns the write connection and the warm
//! search index, as on desktop. On iOS a suspended app must not hold a SQLite lock
//! in the shared container, so the actor can close the database and reopen it on
//! the next call from the app. Jobs from the sync engine never reopen it.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};

use tailcopy_core::{Role, SearchIndex, Store, StoreOptions};
use tailcopy_sync::{StoreJob, StoreRunner};
use tokio::sync::oneshot;

pub struct Core {
    store: Option<Store>,
    index: SearchIndex,
    dir: PathBuf,
    display_name: String,
    /// Closed for suspension; only the app reopens it.
    suspended: bool,
}

impl Core {
    fn open(&mut self) -> tailcopy_core::Result<&mut Store> {
        if self.store.is_none() {
            let store = Store::open(
                &self.dir,
                StoreOptions {
                    display_name: self.display_name.clone(),
                    role: Role::Mobile,
                    ..StoreOptions::default()
                },
            )?;
            self.index.reload(&store.view())?;
            self.store = Some(store);
        }
        Ok(self.store.as_mut().expect("just opened"))
    }

    /// The open store, reopening it if the app closed it for suspension.
    pub fn store(&mut self) -> tailcopy_core::Result<&mut Store> {
        self.suspended = false;
        self.open()
    }

    /// The store and the search index together, for queries that need both.
    pub fn store_and_index(&mut self) -> tailcopy_core::Result<(&mut Store, &mut SearchIndex)> {
        self.store()?;
        Ok((self.store.as_mut().expect("just opened"), &mut self.index))
    }

    fn refresh(&mut self) {
        let Some(store) = self.store.as_mut() else {
            return;
        };
        let groups = store.take_dirty_groups();
        if groups.is_empty() {
            return;
        }
        let view = store.view();
        if let Err(e) = self.index.refresh_groups(&view, &groups) {
            tracing::warn!("search index refresh failed, reloading: {e}");
            if let Err(e) = self.index.reload(&view) {
                tracing::error!("search index reload failed: {e}");
            }
        }
    }

    fn last_hlc(&self) -> u64 {
        self.store.as_ref().map_or(0, Store::last_hlc)
    }
}

type Job = Box<dyn FnOnce(&mut Core) + Send>;
type Hook = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone)]
pub struct Actor {
    tx: mpsc::Sender<Job>,
    on_change: Arc<Mutex<Option<Hook>>>,
}

impl Actor {
    pub fn spawn(dir: PathBuf, display_name: String) -> tailcopy_core::Result<Actor> {
        let mut core = Core {
            store: None,
            index: SearchIndex::new(),
            dir,
            display_name,
            suspended: false,
        };
        core.open()?;
        let (tx, rx) = mpsc::channel::<Job>();
        let on_change: Arc<Mutex<Option<Hook>>> = Arc::default();
        let hook = on_change.clone();
        std::thread::Builder::new()
            .name("store".into())
            .spawn(move || {
                for job in rx {
                    let before = core.last_hlc();
                    job(&mut core);
                    core.refresh();
                    if core.last_hlc() != before {
                        let hook = hook.lock().unwrap().clone();
                        if let Some(hook) = hook {
                            hook();
                        }
                    }
                }
            })
            .map_err(|e| tailcopy_core::Error::Invalid(format!("store thread: {e}")))?;
        Ok(Actor { tx, on_change })
    }

    /// Called on the store thread after any job that moved the store's clock.
    pub fn on_change(&self, hook: impl Fn() + Send + Sync + 'static) {
        *self.on_change.lock().unwrap() = Some(Arc::new(hook));
    }

    /// Runs `f` on the store thread and waits without blocking.
    pub async fn run<T, F>(&self, f: F) -> T
    where
        T: Send + 'static,
        F: FnOnce(&mut Core) -> T + Send + 'static,
    {
        let (reply, answer) = oneshot::channel();
        let _ = self.tx.send(Box::new(move |core| {
            let _ = reply.send(f(core));
        }));
        answer.await.expect("the store thread answers")
    }

    /// Closes the database until the app's next call (iOS suspension).
    pub async fn close(&self) {
        self.run(|core| {
            core.store = None;
            core.suspended = true;
        })
        .await;
    }
}

/// The sync engine's view of the actor: its jobs are dropped while suspended, so
/// a session winding down can't reopen the database.
impl StoreRunner for Actor {
    fn submit(&self, job: StoreJob) {
        let _ = self.tx.send(Box::new(move |core| {
            if core.suspended {
                return;
            }
            match core.open() {
                Ok(store) => job(store),
                Err(e) => tracing::warn!("store unavailable for sync: {e}"),
            }
        }));
    }
}
