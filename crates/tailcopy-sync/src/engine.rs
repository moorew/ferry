//! The sync engine (SPEC 5.1 `sync-server` and `sync-manager`, 7.7): binds the
//! listener on the tailnet addresses, finds and dials peers, keeps one session per
//! peer and reports what it's doing.

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tailcopy_core::{DeviceId, Hash32, Op, Role};
use tokio::sync::{Notify, broadcast, watch};
use tokio::task::JoinHandle;

use crate::frame::{HelloInfo, PROTO_MAJOR, PROTO_MINOR};
use crate::store::StoreHandle;
use crate::transport::{Candidate, PeerIdentity, Transport};

/// Settings the engine reads from config.
#[derive(Clone, Debug)]
pub struct EngineConfig {
    pub role: Role,
    /// Port peers listen on; used to reach a peer that dialled us.
    pub port: u16,
    /// Only same-login devices may sync (SPEC 8.5 strict mode).
    pub strict_same_login: bool,
    /// A clip whose formats total at most this travels inline with its op.
    pub inline_blob_bytes: u64,
    /// Larger blobs are never fetched.
    pub max_fetch_bytes: u64,
    pub app_version: String,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            role: Role::Device,
            port: tailcopy_core::names::DEFAULT_SYNC_PORT,
            strict_same_login: true,
            inline_blob_bytes: 256 * 1024,
            max_fetch_bytes: 20 * 1024 * 1024,
            app_version: String::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum SyncEvent {
    /// Ops from a peer were stored. `live` holds those that may go to the clipboard
    /// (SPEC 7.6): from another origin, after the session caught up, and recent.
    Applied {
        from: DeviceId,
        applied: usize,
        live: Vec<Op>,
    },
    /// A blob that was missing is now on disk.
    BlobStored(Hash32),
    /// A session started, ended or caught up; or device names changed.
    PeersChanged,
    /// This device took a new id (SPEC 6.1); sessions restart.
    DeviceIdChanged,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "detail", rename_all = "snake_case")]
pub enum TailnetState {
    Starting,
    Listening,
    Unavailable(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PeerSnapshot {
    pub device_id: DeviceId,
    pub display_name: String,
    pub role: Role,
    pub node_name: String,
    pub stable_id: String,
    pub connected: bool,
    pub caught_up: bool,
    /// This side dialled the session.
    pub dialled: bool,
    pub last_sync_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SyncSnapshot {
    pub tailnet: TailnetState,
    pub listening: Vec<SocketAddr>,
    pub peers: Vec<PeerSnapshot>,
    /// Node names refused by the authorization check (SPEC 8.5).
    pub rejected: Vec<String>,
}

pub(crate) struct Slot {
    pub id: u64,
    pub dialer: DeviceId,
    pub stop: watch::Sender<bool>,
    pub caught_up: bool,
}

pub(crate) struct Peer {
    pub display_name: String,
    pub role: Role,
    pub node_name: String,
    pub stable_id: String,
    pub http_addr: Option<SocketAddr>,
    pub session: Option<Slot>,
    pub last_sync_ms: Option<u64>,
    pub error: Option<String>,
}

struct Cand {
    candidate: Candidate,
    online_since: Instant,
    next_try: Instant,
    backoff: Duration,
    busy: bool,
    round: u64,
}

#[derive(Default)]
pub(crate) struct State {
    pub peers: BTreeMap<DeviceId, Peer>,
    cands: HashMap<String, Cand>,
    pub rejected: BTreeMap<String, Instant>,
    tailnet: Option<TailnetState>,
    listening: Vec<SocketAddr>,
    pub blob_hints: HashMap<Hash32, DeviceId>,
    pub fetch_failed: HashMap<Hash32, Instant>,
}

/// This device as peers see it.
#[derive(Clone, Debug)]
pub(crate) struct Me {
    pub device_id: DeviceId,
    pub display_name: String,
    pub role: Role,
}

pub(crate) struct Inner {
    pub store: StoreHandle,
    pub transport: Arc<dyn Transport>,
    pub cfg: Mutex<EngineConfig>,
    pub state: Mutex<State>,
    /// Bumped whenever the store holds something new to push.
    pub changed: watch::Sender<u64>,
    pub events: broadcast::Sender<SyncEvent>,
    pub fetch: Notify,
    pub shutdown: watch::Sender<bool>,
    next_session: AtomicU64,
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

const DISCOVERY_TICK: Duration = Duration::from_secs(2);
const LISTEN_TICK: Duration = Duration::from_secs(5);
/// The higher-id device dials too after this long without a session (SPEC 7.7).
const DIAL_ANYWAY_AFTER: Duration = Duration::from_secs(10);
const MAX_BACKOFF: Duration = Duration::from_secs(60);
/// A session that lasted this long resets the backoff.
const STABLE_SESSION: Duration = Duration::from_secs(30);
const REJECTED_KEPT: Duration = Duration::from_secs(24 * 3600);

fn jitter(d: Duration) -> Duration {
    d.mul_f64(0.8 + fastrand::f64() * 0.4)
}

impl Inner {
    pub fn cfg(&self) -> EngineConfig {
        self.cfg.lock().unwrap().clone()
    }

    pub fn emit(&self, event: SyncEvent) {
        let _ = self.events.send(event);
    }

    pub fn bump_changed(&self) {
        self.changed.send_modify(|n| *n = n.wrapping_add(1));
    }

    pub async fn me(&self) -> anyhow::Result<Me> {
        let role = self.cfg().role;
        Ok(self
            .store
            .call(move |s| -> tailcopy_core::Result<Me> {
                let id = s.device_id().clone();
                let name = s
                    .device_names()?
                    .into_iter()
                    .find(|n| n.device_id == id)
                    .map(|n| n.display_name)
                    .unwrap_or_default();
                Ok(Me {
                    device_id: id,
                    display_name: name,
                    role,
                })
            })
            .await??)
    }

    pub async fn hello_info(&self) -> anyhow::Result<HelloInfo> {
        let me = self.me().await?;
        Ok(HelloInfo {
            proto_major: PROTO_MAJOR,
            proto_minor: PROTO_MINOR,
            device_id: me.device_id,
            display_name: me.display_name,
            role: me.role.as_str().into(),
            app_version: self.cfg().app_version,
        })
    }

    /// SPEC 8.5 for a connection we accepted.
    pub fn authorize_inbound(&self, who: &PeerIdentity) -> bool {
        let strict = self.cfg().strict_same_login;
        let ok = who.same_user || (!strict && who.has_capability);
        if !ok {
            self.reject(&who.node_name);
        }
        ok
    }

    /// SPEC 8.5 for a connection we dialled, given the role its hello reported.
    pub fn authorize_outbound(&self, who: &PeerIdentity, role: Role) -> bool {
        let strict = self.cfg().strict_same_login;
        let ok = who.same_user || (!strict && who.relay_tagged && role == Role::Relay);
        if !ok {
            self.reject(&who.node_name);
        }
        ok
    }

    fn reject(&self, node: &str) {
        tracing::warn!(node, "refused a peer that isn't authorized to sync");
        let mut st = self.state.lock().unwrap();
        let fresh = st
            .rejected
            .insert(node.to_owned(), Instant::now())
            .is_none();
        drop(st);
        if fresh {
            self.emit(SyncEvent::PeersChanged);
        }
    }

    /// Registers a session after the hello exchange, applying the one-session rule
    /// (SPEC 7.7). Returns its id and stop signal, or `None` if the session should
    /// close because a better one exists.
    pub fn register(
        &self,
        me: &DeviceId,
        hello: &Me,
        dialer: &DeviceId,
        who: &PeerIdentity,
        http_addr: SocketAddr,
    ) -> Option<(u64, watch::Receiver<bool>)> {
        let mut st = self.state.lock().unwrap();
        let peer = st.peers.entry(hello.device_id.clone()).or_insert(Peer {
            display_name: hello.display_name.clone(),
            role: hello.role,
            node_name: who.node_name.clone(),
            stable_id: who.stable_id.clone(),
            http_addr: None,
            session: None,
            last_sync_ms: None,
            error: None,
        });
        if let Some(old) = &peer.session {
            let lower = me.min(&hello.device_id);
            let replace = old.dialer == *dialer || dialer == lower;
            if !replace {
                tracing::debug!(peer = %who.node_name, "keeping the existing session");
                return None;
            }
            let _ = old.stop.send(true);
        }
        let id = self.next_session.fetch_add(1, Ordering::Relaxed);
        let (stop, stop_rx) = watch::channel(false);
        peer.display_name = hello.display_name.clone();
        peer.role = hello.role;
        peer.node_name = who.node_name.clone();
        peer.stable_id = who.stable_id.clone();
        peer.http_addr = Some(http_addr);
        peer.error = None;
        peer.session = Some(Slot {
            id,
            dialer: dialer.clone(),
            stop,
            caught_up: false,
        });
        Some((id, stop_rx))
    }

    pub fn unregister(&self, peer: &DeviceId, id: u64, error: Option<String>) {
        let mut st = self.state.lock().unwrap();
        if let Some(p) = st.peers.get_mut(peer) {
            if p.session.as_ref().is_some_and(|s| s.id == id) {
                p.session = None;
            }
            if error.is_some() {
                p.error = error;
            }
        }
    }

    pub fn with_peer(&self, peer: &DeviceId, f: impl FnOnce(&mut Peer)) {
        if let Some(p) = self.state.lock().unwrap().peers.get_mut(peer) {
            f(p);
        }
    }

    /// Ends every session; used when this device's id changes.
    pub fn stop_sessions(&self) {
        let st = self.state.lock().unwrap();
        for p in st.peers.values() {
            if let Some(s) = &p.session {
                let _ = s.stop.send(true);
            }
        }
    }

    pub fn device_id_changed(&self) {
        tracing::warn!("this device took a new id; restarting sessions");
        self.stop_sessions();
        self.emit(SyncEvent::DeviceIdChanged);
    }

    fn connected_stable(&self, stable_id: &str) -> bool {
        self.state
            .lock()
            .unwrap()
            .peers
            .values()
            .any(|p| p.stable_id == stable_id && p.session.is_some())
    }

    pub fn is_shutdown(&self) -> bool {
        *self.shutdown.borrow()
    }
}

/// A running sync engine. Dropping it doesn't stop it; call [`Engine::stop`].
#[derive(Clone)]
pub struct Engine {
    pub(crate) inner: Arc<Inner>,
}

impl Engine {
    /// Starts the listener, discovery and blob fetch tasks on the current tokio
    /// runtime.
    pub fn start(store: StoreHandle, transport: Arc<dyn Transport>, cfg: EngineConfig) -> Engine {
        let (changed, _) = watch::channel(0);
        let (events, _) = broadcast::channel(256);
        let (shutdown, _) = watch::channel(false);
        let inner = Arc::new(Inner {
            store,
            transport,
            cfg: Mutex::new(cfg),
            state: Mutex::new(State::default()),
            changed,
            events,
            fetch: Notify::new(),
            shutdown,
            next_session: AtomicU64::new(1),
        });
        tokio::spawn(listen_loop(inner.clone()));
        tokio::spawn(discovery_loop(inner.clone()));
        tokio::spawn(crate::blobs::fetch_loop(inner.clone()));
        Engine { inner }
    }

    /// The store has new local ops (or a new device name) to push to peers.
    pub fn local_changed(&self) {
        self.inner.bump_changed();
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SyncEvent> {
        self.inner.events.subscribe()
    }

    pub fn set_config(&self, cfg: EngineConfig) {
        let old = self.inner.cfg();
        let tightened = cfg.strict_same_login && !old.strict_same_login;
        *self.inner.cfg.lock().unwrap() = cfg;
        if tightened {
            // Sessions are authorized when they start (SPEC 8.5); make them
            // reconnect so the new rules apply to peers already connected.
            tracing::info!("sync authorization settings changed; reconnecting peers");
            self.inner.stop_sessions();
        }
    }

    /// Ends every session; peers reconnect only if they pass authorization again.
    pub fn reconnect_all(&self) {
        self.inner.stop_sessions();
    }

    /// Retry every peer now (Tailscale came back, settings changed).
    pub fn retry_now(&self) {
        let mut st = self.inner.state.lock().unwrap();
        let now = Instant::now();
        for c in st.cands.values_mut() {
            c.next_try = now;
            c.backoff = Duration::from_secs(1);
        }
        st.fetch_failed.clear();
        drop(st);
        self.inner.fetch.notify_one();
    }

    /// Ends the session with one peer (it reconnects as usual). For tests and
    /// toggles that need a fresh backlog.
    pub fn drop_session(&self, peer: &DeviceId) {
        let st = self.inner.state.lock().unwrap();
        if let Some(s) = st.peers.get(peer).and_then(|p| p.session.as_ref()) {
            let _ = s.stop.send(true);
        }
    }

    /// Stops listening, dialling and every session.
    pub fn stop(&self) {
        let _ = self.inner.shutdown.send(true);
        self.inner.stop_sessions();
    }

    /// Fetches a blob that isn't here yet from any connected peer, waiting at most
    /// `timeout` (SPEC 7.5: pasting a clip whose content hasn't arrived).
    pub async fn fetch_blob(&self, hash: Hash32, timeout: Duration) -> bool {
        crate::blobs::fetch_now(&self.inner, hash, timeout).await
    }

    pub fn snapshot(&self) -> SyncSnapshot {
        let mut st = self.inner.state.lock().unwrap();
        st.rejected.retain(|_, at| at.elapsed() < REJECTED_KEPT);
        SyncSnapshot {
            tailnet: st.tailnet.clone().unwrap_or(TailnetState::Starting),
            listening: st.listening.clone(),
            peers: st
                .peers
                .iter()
                .map(|(id, p)| PeerSnapshot {
                    device_id: id.clone(),
                    display_name: p.display_name.clone(),
                    role: p.role,
                    node_name: p.node_name.clone(),
                    stable_id: p.stable_id.clone(),
                    connected: p.session.is_some(),
                    caught_up: p.session.as_ref().is_some_and(|s| s.caught_up),
                    dialled: p.session.as_ref().is_some_and(|s| s.dialer != *id),
                    last_sync_ms: p.last_sync_ms,
                    error: p.error.clone(),
                })
                .collect(),
            rejected: st.rejected.keys().cloned().collect(),
        }
    }
}

async fn wait_or_shutdown(inner: &Inner, d: Duration) -> bool {
    let mut shutdown = inner.shutdown.subscribe();
    tokio::select! {
        () = tokio::time::sleep(d) => false,
        _ = shutdown.wait_for(|s| *s) => true,
    }
}

fn set_tailnet(inner: &Inner, state: TailnetState, listening: Vec<SocketAddr>) {
    let mut st = inner.state.lock().unwrap();
    let changed = st.tailnet.as_ref() != Some(&state) || st.listening != listening;
    st.tailnet = Some(state);
    st.listening = listening;
    drop(st);
    if changed {
        inner.emit(SyncEvent::PeersChanged);
    }
}

/// Binds the listener on the tailnet addresses and rebinds when they change
/// (SPEC 8.3). Never binds a wildcard address.
async fn listen_loop(inner: Arc<Inner>) {
    let router = crate::server::router(inner.clone());
    let mut bound: Vec<SocketAddr> = Vec::new();
    let mut servers: Vec<JoinHandle<()>> = Vec::new();
    loop {
        match inner.transport.listen_addrs().await {
            Ok(addrs) => {
                let addrs: Vec<SocketAddr> = addrs
                    .into_iter()
                    .filter(|a| !a.ip().is_unspecified())
                    .collect();
                if addrs != bound || servers.iter().any(JoinHandle::is_finished) {
                    servers.drain(..).for_each(|s| s.abort());
                    let mut listening = Vec::new();
                    for addr in &addrs {
                        match tokio::net::TcpListener::bind(addr).await {
                            Ok(listener) => {
                                let svc = router
                                    .clone()
                                    .into_make_service_with_connect_info::<SocketAddr>();
                                let addr = *addr;
                                servers.push(tokio::spawn(async move {
                                    if let Err(e) = axum::serve(listener, svc).await {
                                        tracing::warn!(%addr, "sync listener stopped: {e}");
                                    }
                                }));
                                listening.push(addr);
                            }
                            Err(e) => tracing::warn!(%addr, "couldn't listen: {e}"),
                        }
                    }
                    if !listening.is_empty() {
                        tracing::info!(?listening, "sync listener up");
                    }
                    bound = addrs;
                    let state = if listening.is_empty() {
                        TailnetState::Unavailable("couldn't listen on the tailnet address".into())
                    } else {
                        TailnetState::Listening
                    };
                    set_tailnet(&inner, state, listening);
                }
            }
            Err(e) => {
                if !bound.is_empty() {
                    tracing::info!("tailnet unavailable, listener down: {e}");
                }
                servers.drain(..).for_each(|s| s.abort());
                bound.clear();
                set_tailnet(&inner, TailnetState::Unavailable(e.to_string()), Vec::new());
            }
        }
        if wait_or_shutdown(&inner, LISTEN_TICK).await {
            servers.drain(..).for_each(|s| s.abort());
            return;
        }
    }
}

/// Probes candidates and dials the ones this side should (SPEC 7.7).
async fn discovery_loop(inner: Arc<Inner>) {
    let mut round = 0u64;
    loop {
        if wait_or_shutdown(
            &inner,
            if round == 0 {
                Duration::ZERO
            } else {
                DISCOVERY_TICK
            },
        )
        .await
        {
            return;
        }
        round += 1;
        if inner.cfg().role == Role::Relay {
            continue;
        }
        let candidates = match inner.transport.candidates().await {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!("no candidates: {e}");
                continue;
            }
        };
        let now = Instant::now();
        let mut ready = Vec::new();
        {
            let mut st = inner.state.lock().unwrap();
            for c in candidates {
                let entry = st.cands.entry(c.stable_id.clone()).or_insert_with(|| Cand {
                    candidate: c.clone(),
                    online_since: now,
                    next_try: now,
                    backoff: Duration::from_secs(1),
                    busy: false,
                    round,
                });
                entry.candidate = c;
                entry.round = round;
            }
            // Gone from the list (offline): coming back starts fresh (SPEC 7.7).
            st.cands.retain(|_, c| c.round == round || c.busy);
            for c in st.cands.values_mut() {
                if !c.busy && now >= c.next_try {
                    c.busy = true;
                    ready.push((c.candidate.clone(), c.online_since));
                }
            }
        }
        for (cand, online_since) in ready {
            let inner = inner.clone();
            tokio::spawn(async move {
                let started = Instant::now();
                let outcome = if inner.connected_stable(&cand.stable_id) {
                    Attempt::Wait(Duration::from_secs(5))
                } else {
                    crate::dial::attempt(&inner, &cand, online_since).await
                };
                let mut st = inner.state.lock().unwrap();
                let Some(c) = st.cands.get_mut(&cand.stable_id) else {
                    return;
                };
                c.busy = false;
                let now = Instant::now();
                match outcome {
                    Attempt::Ended { lasted } => {
                        if lasted >= STABLE_SESSION {
                            c.backoff = Duration::from_secs(1);
                        }
                        c.next_try = now + jitter(c.backoff);
                        c.backoff = (c.backoff * 2).min(MAX_BACKOFF);
                    }
                    Attempt::Wait(d) => c.next_try = now + d,
                    Attempt::Failed => {
                        c.next_try = now + jitter(c.backoff);
                        c.backoff = (c.backoff * 2).min(MAX_BACKOFF);
                    }
                }
                tracing::trace!(peer = %cand.node_name, took = ?started.elapsed(), "attempt done");
            });
        }
    }
}

pub(crate) enum Attempt {
    /// A session ran and ended.
    Ended {
        lasted: Duration,
    },
    /// Nothing to do yet; look again after this long.
    Wait(Duration),
    Failed,
}

pub(crate) const DIAL_ANYWAY: Duration = DIAL_ANYWAY_AFTER;
