//! In-memory tailnet and the sync simulation harness (SPEC 14.1).
//!
//! Each simulated node gets its own loopback address (127.1.x.y), so whois can tell
//! nodes apart by the remote IP just as Tailscale does. macOS only routes 127.0.0.1
//! on loopback, so tests that need several nodes skip there.

use std::collections::{BTreeMap, BTreeSet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tailcopy_core::hlc::Clock;
use tailcopy_core::store::ClipRow;
use tailcopy_core::{
    Capture, CaptureOutcome, CapturedFormats, ClipId, DeviceId, PeerKey, Role, Store, StoreOptions,
    SyncFlags, VersionVector,
};
use tailcopy_sync::{
    Candidate, Engine, EngineConfig, Identity, PairLink, PairedTransport, PeerIdentity,
    StoreHandle, SyncEvent, Transport, spawn_store_thread,
};

/// Port every simulated node listens on (each has its own address).
pub const SIM_PORT: u16 = 17441;

static NEXT_IP: AtomicU32 = AtomicU32::new(1);

fn next_ip() -> IpAddr {
    let n = NEXT_IP.fetch_add(1, Ordering::Relaxed);
    IpAddr::V4(Ipv4Addr::new(127, 1, (n >> 8) as u8, n as u8))
}

#[derive(Clone, Debug)]
struct FakeNode {
    name: String,
    login: String,
    tags: Vec<String>,
    online: bool,
    /// Holds the app capability toward everyone.
    capability: bool,
    /// Accepts no connections (a firewall).
    no_inbound: bool,
    /// A phone: Tailscale reports it as iOS or Android, so desktops never dial it.
    phone: bool,
}

/// A tailnet: nodes, their logins and tags, who is online and who can reach whom.
#[derive(Default)]
pub struct FakeTailnet {
    nodes: Mutex<BTreeMap<IpAddr, FakeNode>>,
    cut: Mutex<BTreeSet<(IpAddr, IpAddr)>>,
}

pub const RELAY_TAG: &str = "tag:tailcopy-relay";

impl FakeTailnet {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn add(&self, name: &str, login: &str, tags: Vec<String>) -> IpAddr {
        let ip = next_ip();
        self.nodes.lock().unwrap().insert(
            ip,
            FakeNode {
                name: name.into(),
                login: login.into(),
                tags,
                online: true,
                capability: false,
                no_inbound: false,
                phone: false,
            },
        );
        ip
    }

    fn edit(&self, ip: IpAddr, f: impl FnOnce(&mut FakeNode)) {
        if let Some(n) = self.nodes.lock().unwrap().get_mut(&ip) {
            f(n);
        }
    }

    fn pair(a: IpAddr, b: IpAddr) -> (IpAddr, IpAddr) {
        (a.min(b), a.max(b))
    }

    /// The two nodes can no longer reach each other.
    pub fn cut(&self, a: IpAddr, b: IpAddr) {
        self.cut.lock().unwrap().insert(Self::pair(a, b));
    }

    pub fn heal(&self, a: IpAddr, b: IpAddr) {
        self.cut.lock().unwrap().remove(&Self::pair(a, b));
    }

    fn reachable(&self, a: IpAddr, b: IpAddr) -> bool {
        !self.cut.lock().unwrap().contains(&Self::pair(a, b))
    }

    /// Opens or closes a node's firewall for inbound connections.
    pub fn set_inbound(&self, ip: IpAddr, open: bool) {
        self.edit(ip, |n| n.no_inbound = !open);
    }

    pub fn transport(self: &Arc<Self>, ip: IpAddr) -> Arc<FakeTransport> {
        Arc::new(FakeTransport {
            net: self.clone(),
            ip,
        })
    }

    /// A phone's transport: its only tailnet knowledge is whether it's online.
    fn phone_transport(self: &Arc<Self>, ip: IpAddr, store: StoreHandle) -> Arc<dyn Transport> {
        let net = self.clone();
        Arc::new(PairedTransport::new(
            store,
            Box::new(move || {
                let nodes = net.nodes.lock().unwrap();
                nodes.get(&ip).filter(|n| n.online).map(|_| vec![ip])
            }),
        ))
    }
}

/// One node's view of a [`FakeTailnet`].
pub struct FakeTransport {
    net: Arc<FakeTailnet>,
    ip: IpAddr,
}

#[async_trait]
impl Transport for FakeTransport {
    async fn listen_addrs(&self) -> anyhow::Result<Vec<SocketAddr>> {
        let nodes = self.net.nodes.lock().unwrap();
        match nodes.get(&self.ip) {
            Some(n) if n.online && n.no_inbound => Ok(Vec::new()),
            Some(n) if n.online => Ok(vec![SocketAddr::new(self.ip, SIM_PORT)]),
            _ => anyhow::bail!("Tailscale isn't running"),
        }
    }

    async fn whois(&self, remote: SocketAddr) -> anyhow::Result<PeerIdentity> {
        if !self.net.reachable(self.ip, remote.ip()) {
            anyhow::bail!("peer not found");
        }
        let nodes = self.net.nodes.lock().unwrap();
        let me = nodes
            .get(&self.ip)
            .ok_or_else(|| anyhow::anyhow!("not on the tailnet"))?;
        let peer = nodes
            .get(&remote.ip())
            .ok_or_else(|| anyhow::anyhow!("peer not found"))?;
        Ok(PeerIdentity {
            node_name: peer.name.clone(),
            stable_id: format!("n{}", peer.name),
            same_user: peer.tags.is_empty() && me.tags.is_empty() && peer.login == me.login,
            has_capability: peer.capability,
            relay_tagged: peer.tags.iter().any(|t| t == RELAY_TAG),
        })
    }

    async fn candidates(&self) -> anyhow::Result<Vec<Candidate>> {
        let nodes = self.net.nodes.lock().unwrap();
        let me = nodes
            .get(&self.ip)
            .ok_or_else(|| anyhow::anyhow!("not on the tailnet"))?;
        if !me.online {
            anyhow::bail!("Tailscale isn't running");
        }
        Ok(nodes
            .iter()
            .filter(|(ip, n)| {
                **ip != self.ip
                    && n.online
                    && !n.phone
                    && self.net.reachable(self.ip, **ip)
                    && ((n.tags.is_empty() && n.login == me.login)
                        || n.tags.iter().any(|t| t == RELAY_TAG))
            })
            .map(|(ip, n)| Candidate {
                node_name: n.name.clone(),
                stable_id: format!("n{}", n.name),
                addr: SocketAddr::new(*ip, SIM_PORT),
                relay_tagged: n.tags.iter().any(|t| t == RELAY_TAG),
                pinned: None,
            })
            .collect())
    }

    fn dial_from(&self, _peer: IpAddr) -> Option<IpAddr> {
        Some(self.ip)
    }
}

/// System time shifted by a fixed offset (clock skew between devices).
pub struct SkewedClock(pub i64);

impl Clock for SkewedClock {
    fn now_ms(&self) -> u64 {
        let now = tailcopy_core::SystemClock.now_ms() as i64;
        (now + self.0) as u64
    }
}

/// One device or relay in a simulation.
pub struct SimNode {
    pub name: String,
    pub ip: IpAddr,
    pub role: Role,
    dir: PathBuf,
    skew_ms: i64,
    pub store: StoreHandle,
    pub engine: Engine,
    net: Arc<FakeTailnet>,
    identity: Option<Arc<Identity>>,
}

fn open_store(dir: &Path, name: &str, role: Role, skew_ms: i64) -> Store {
    Store::open(
        dir,
        StoreOptions {
            clock: Arc::new(SkewedClock(skew_ms)),
            display_name: name.into(),
            role,
            ..StoreOptions::default()
        },
    )
    .expect("store opens")
}

fn engine_config(role: Role, identity: &Option<Arc<Identity>>) -> EngineConfig {
    EngineConfig {
        role,
        port: SIM_PORT,
        app_version: "test".into(),
        identity: identity.clone(),
        ..EngineConfig::default()
    }
}

fn start_engine(
    net: &Arc<FakeTailnet>,
    ip: IpAddr,
    store: &StoreHandle,
    role: Role,
    identity: &Option<Arc<Identity>>,
) -> Engine {
    let transport: Arc<dyn Transport> = if role == Role::Mobile {
        net.phone_transport(ip, store.clone())
    } else {
        net.transport(ip)
    };
    Engine::start(store.clone(), transport, engine_config(role, identity))
}

impl SimNode {
    pub async fn device_id(&self) -> DeviceId {
        self.store.call(|s| s.device_id().clone()).await.unwrap()
    }

    /// Copies text on this device, as the capture pipeline would.
    pub async fn copy(&self, text: &str) -> ClipId {
        self.capture(CapturedFormats {
            text: Some(text.into()),
            ..Default::default()
        })
        .await
    }

    pub async fn capture(&self, formats: CapturedFormats) -> ClipId {
        let capture = Capture {
            formats,
            ..Default::default()
        };
        let outcome = self
            .store
            .call(move |s| s.capture(&capture))
            .await
            .unwrap()
            .unwrap();
        self.engine.local_changed();
        match outcome {
            CaptureOutcome::Added { clip_id, .. } => clip_id,
            CaptureOutcome::Ignored => panic!("capture ignored"),
        }
    }

    pub async fn with_store<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Store) -> T + Send + 'static,
    ) -> T {
        let out = self.store.call(f).await.unwrap();
        self.engine.local_changed();
        out
    }

    /// Visible clips without the fields that are local by design (when this device
    /// got each one, and the source app), so nodes can be compared.
    pub async fn rows(&self) -> Vec<ClipRow> {
        let mut rows = self
            .store
            .call(|s| s.view().live_clips().unwrap())
            .await
            .unwrap();
        for r in &mut rows {
            r.received_ms = None;
            r.source_app = None;
        }
        rows
    }

    /// Previews of the visible list, in display order.
    pub async fn previews(&self) -> Vec<String> {
        self.store
            .call(|s| {
                s.view()
                    .all_rows()
                    .unwrap()
                    .into_iter()
                    .map(|r| r.preview)
                    .collect()
            })
            .await
            .unwrap()
    }

    pub async fn vv(&self) -> VersionVector {
        self.store
            .call(|s| s.version_vector().unwrap())
            .await
            .unwrap()
    }

    pub async fn set_flags(&self, peer: &DeviceId, flags: SyncFlags) {
        let peer = peer.clone();
        self.store
            .call(move |s| s.set_sync_flags(&peer, flags).unwrap())
            .await
            .unwrap();
    }

    pub fn connected_to(&self, peer: &DeviceId) -> bool {
        self.engine
            .snapshot()
            .peers
            .iter()
            .any(|p| &p.device_id == peer && p.connected && p.caught_up)
    }

    /// Simulates the app quitting: the engine stops and the store closes.
    pub async fn shut_down(&self) {
        self.engine.stop();
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    /// Starts again from the same data directory.
    pub async fn restart(&mut self) {
        self.shut_down().await;
        let store = open_store(&self.dir, &self.name, self.role, self.skew_ms);
        self.store = spawn_store_thread(store);
        self.engine = start_engine(&self.net, self.ip, &self.store, self.role, &self.identity);
    }

    /// Starts again from a copy of `backup` (a restored or cloned install) on the
    /// same tailnet node. The copy goes to a fresh directory: Windows won't overwrite
    /// files SQLite still has mapped.
    pub async fn restore_from(&mut self, backup: &Path) {
        self.shut_down().await;
        let restored = self.dir.with_extension("restored");
        let _ = std::fs::remove_dir_all(&restored);
        copy_dir(backup, &restored);
        self.dir = restored;
        let store = open_store(&self.dir, &self.name, self.role, self.skew_ms);
        self.store = spawn_store_thread(store);
        self.engine = start_engine(&self.net, self.ip, &self.store, self.role, &self.identity);
    }

    /// Copies this node's data directory (while it runs; WAL is checkpointed first).
    pub async fn backup(&self, to: &Path) {
        self.store
            .call(|s| s.view().data_dir().to_path_buf())
            .await
            .map(|dir| {
                let _ = std::fs::remove_dir_all(to);
                copy_dir(&dir, to);
            })
            .unwrap();
    }

    pub fn events(&self) -> tokio::sync::broadcast::Receiver<SyncEvent> {
        self.engine.subscribe()
    }

    pub fn set_online(&self, online: bool) {
        self.net.edit(self.ip, |n| n.online = online);
    }

    /// What this desktop's or relay's pairing QR code would carry (SPEC 21.3.3).
    pub async fn pair_link(&self) -> PairLink {
        let id = self.identity.as_ref().expect("phones have no pairing code");
        PairLink {
            device_id: self.device_id().await,
            display_name: self.name.clone(),
            key: id.public_key(),
            addrs: vec![SocketAddr::new(self.ip, SIM_PORT)],
        }
    }

    /// A phone scans a pairing code.
    pub async fn pair(&self, link: &PairLink) {
        let entry = PeerKey {
            device_id: link.device_id.clone(),
            display_name: link.display_name.clone(),
            role: Role::Device,
            key: link.key,
            addrs: link.addrs.iter().map(ToString::to_string).collect(),
            trusted: true,
            updated_ms: 0,
        };
        self.store
            .call(move |s| s.record_peer_key(&entry).unwrap())
            .await
            .unwrap();
        self.engine.retry_now();
    }

    /// Gives this desktop or relay a different identity key (an impostor at a
    /// paired address).
    pub async fn replace_identity(&mut self, seed: [u8; 32]) {
        self.identity = Some(Arc::new(Identity::from_seed(seed)));
        self.restart().await;
    }

    pub async fn peer_keys(&self) -> Vec<PeerKey> {
        self.store.call(|s| s.peer_keys().unwrap()).await.unwrap()
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Builds nodes on one fake tailnet.
pub struct Sim {
    pub net: Arc<FakeTailnet>,
    tmp: tempfile::TempDir,
}

/// How a node joins the tailnet.
#[derive(Clone, Debug)]
pub struct NodeSpec {
    pub name: String,
    pub login: String,
    pub tags: Vec<String>,
    pub role: Role,
    pub skew_ms: i64,
    pub no_inbound: bool,
    pub capability: bool,
}

impl NodeSpec {
    /// A phone signed in to the owner's login.
    pub fn phone(name: &str) -> Self {
        Self {
            role: Role::Mobile,
            ..Self::device(name)
        }
    }

    pub fn device(name: &str) -> Self {
        Self {
            name: name.into(),
            login: "owner@example.com".into(),
            tags: Vec::new(),
            role: Role::Device,
            skew_ms: 0,
            no_inbound: false,
            capability: false,
        }
    }
}

impl Default for Sim {
    fn default() -> Self {
        Self::new()
    }
}

impl Sim {
    pub fn new() -> Self {
        Self {
            net: FakeTailnet::new(),
            tmp: tempfile::tempdir().unwrap(),
        }
    }

    pub fn dir(&self) -> &Path {
        self.tmp.path()
    }

    pub fn node(&self, name: &str) -> SimNode {
        self.spawn(NodeSpec::device(name))
    }

    pub fn spawn(&self, spec: NodeSpec) -> SimNode {
        let ip = self.net.add(&spec.name, &spec.login, spec.tags.clone());
        self.net.edit(ip, |n| {
            n.no_inbound = spec.no_inbound;
            n.capability = spec.capability;
        });
        self.net.edit(ip, |n| n.phone = spec.role == Role::Mobile);
        let dir = self.tmp.path().join(&spec.name);
        let store = spawn_store_thread(open_store(&dir, &spec.name, spec.role, spec.skew_ms));
        let identity = (spec.role != Role::Mobile)
            .then(|| Arc::new(Identity::generate().expect("system randomness")));
        let engine = start_engine(&self.net, ip, &store, spec.role, &identity);
        SimNode {
            name: spec.name,
            ip,
            role: spec.role,
            dir,
            skew_ms: spec.skew_ms,
            store,
            engine,
            net: self.net.clone(),
            identity,
        }
    }
}

/// Polls `cond` every 50 ms until it holds or `timeout` passes. Returns whether it held.
pub async fn eventually<F, Fut>(timeout: Duration, mut cond: F) -> bool
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = Instant::now() + timeout;
    loop {
        if cond().await {
            return true;
        }
        if Instant::now() > deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
