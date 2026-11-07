//! One sync session (SPEC 7.4): hello, vectors and device names, then everything
//! the peer lacks, `CaughtUp`, and live pushes until either side goes away.
//!
//! Sending and receiving run side by side, so two peers with big backlogs never wait
//! on each other's full socket buffers.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_bytes::ByteBuf;
use tailcopy_core::hlc::wall_ms;
use tailcopy_core::model::blake3_hash;
use tailcopy_core::{DeviceId, DeviceName, Hash32, Op, OpBody, PeerSeen, VersionVector};
use tokio::sync::{mpsc, oneshot};

use tailcopy_core::{PeerKey, Role};

use crate::engine::{Inner, Me, SyncEvent, now_ms};
use crate::frame::{DirEntry, ErrorCode, Frame, MAX_OPS_PER_FRAME, PROTO_MAJOR, PROTO_MINOR};
use crate::identity::{new_challenge, verify_proof};
use crate::transport::{PeerIdentity, Pinned};
use crate::ws::{FrameSink, FrameStream};

const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
const PING_EVERY: Duration = Duration::from_secs(15);
/// No frame for this long means the session is dead.
const DEAD_AFTER: Duration = Duration::from_secs(45);
/// Peers in a phone's `Directory` were seen this recently (SPEC 21.3.5).
const DIRECTORY_MAX_AGE_MS: u64 = 30 * 24 * 3600 * 1000;
/// Live ops further than this from local time never touch the clipboard (SPEC 7.6).
const LIVE_WINDOW_MS: u64 = 10_000;
/// Inline blobs held while waiting for the `Ops` frame that needs them.
const MAX_PENDING_BLOB_BYTES: usize = 32 * 1024 * 1024;

async fn send(sink: &mut FrameSink, frame: &Frame) -> anyhow::Result<()> {
    sink.send(frame.encode()?).await
}

async fn recv(stream: &mut FrameStream) -> anyhow::Result<Frame> {
    match tokio::time::timeout(DEAD_AFTER, stream.next()).await {
        Err(_) => anyhow::bail!("no frame for {} s", DEAD_AFTER.as_secs()),
        Ok(None) => anyhow::bail!("the peer closed the session"),
        Ok(Some(bytes)) => Ok(Frame::decode(&bytes?)?),
    }
}

/// What the peer's `Hello` carried beyond who it is.
struct PeerHello {
    minor: u16,
    key: Option<[u8; 32]>,
    challenge: Option<[u8; 32]>,
    proof: Option<ByteBuf>,
}

fn bytes32(b: Option<ByteBuf>) -> Option<[u8; 32]> {
    b.and_then(|b| b.as_slice().try_into().ok())
}

/// Runs a session to its end. `dialer` is this side's role; `expected` is the peer's
/// device id from its `/v1/hello`, when this side dialled. A phone passes the paired
/// key it expects the listener to prove (SPEC 21.3.4).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run(
    inner: &Inner,
    mut sink: FrameSink,
    mut stream: FrameStream,
    dialer: bool,
    who: PeerIdentity,
    addr: SocketAddr,
    expected: Option<DeviceId>,
    pinned: Option<Pinned>,
) -> anyhow::Result<()> {
    let me = inner.me().await?;
    let identity = inner.cfg().identity;
    let challenge = match pinned {
        Some(_) => Some(new_challenge()?),
        None => None,
    };
    let my_hello = |proof: Option<[u8; 64]>| Frame::Hello {
        proto_major: PROTO_MAJOR,
        proto_minor: PROTO_MINOR,
        device_id: me.device_id.clone(),
        role: me.role,
        display_name: me.display_name.clone(),
        key: identity
            .as_ref()
            .map(|i| ByteBuf::from(i.public_key().to_vec())),
        challenge: challenge.map(|c| ByteBuf::from(c.to_vec())),
        proof: proof.map(|p| ByteBuf::from(p.to_vec())),
    };
    if dialer {
        send(&mut sink, &my_hello(None)).await?;
    }
    let (peer, extra) = match tokio::time::timeout(HELLO_TIMEOUT, recv(&mut stream)).await {
        Err(_) => anyhow::bail!("no hello"),
        Ok(frame) => match frame? {
            Frame::Hello {
                proto_major,
                proto_minor,
                device_id,
                role,
                display_name,
                key,
                challenge,
                proof,
            } => {
                if proto_major != PROTO_MAJOR {
                    let _ = send(
                        &mut sink,
                        &Frame::Error {
                            code: ErrorCode::ProtoMismatch,
                            message: format!("this side speaks {PROTO_MAJOR}.x"),
                        },
                    )
                    .await;
                    inner.with_peer(&device_id, |p| {
                        p.error = Some(format!("Update TailCopy on {display_name}"));
                    });
                    anyhow::bail!("protocol {proto_major} from {}", who.node_name);
                }
                (
                    Me {
                        device_id,
                        display_name,
                        role,
                    },
                    PeerHello {
                        minor: proto_minor,
                        key: bytes32(key),
                        challenge: bytes32(challenge),
                        proof,
                    },
                )
            }
            other => anyhow::bail!("expected Hello, got {}", other.kind()),
        },
    };
    if !dialer {
        let proof = match (&identity, &extra.challenge) {
            (Some(id), Some(c)) => Some(id.prove(c, &me.device_id, &peer.device_id)),
            _ => None,
        };
        send(&mut sink, &my_hello(proof)).await?;
    }
    if let (Some(p), Some(c)) = (&pinned, &challenge) {
        let proven = peer.device_id == p.device_id
            && extra.proof.as_ref().is_some_and(|proof| {
                verify_proof(&p.key, c, &peer.device_id, &me.device_id, proof)
            });
        if !proven {
            let _ = send(
                &mut sink,
                &Frame::Error {
                    code: ErrorCode::Unauthorized,
                    message: "the identity proof didn't verify".into(),
                },
            )
            .await;
            inner.note_peer_error(
                &p.device_id,
                &who,
                format!("{} couldn't prove it's your device", who.node_name),
            );
            anyhow::bail!("{} failed the identity proof", who.node_name);
        }
    }
    if peer.device_id == me.device_id {
        anyhow::bail!("connected to itself");
    }
    if expected.as_ref().is_some_and(|e| *e != peer.device_id) {
        anyhow::bail!("the peer's id changed between hello and session");
    }
    let dialer_id = if dialer {
        me.device_id.clone()
    } else {
        peer.device_id.clone()
    };
    let http_addr = if dialer {
        addr
    } else {
        SocketAddr::new(addr.ip(), inner.cfg().port)
    };
    let Some((sid, mut stop)) = inner.register(&me.device_id, &peer, &dialer_id, &who, http_addr)
    else {
        return Ok(());
    };
    tracing::info!(peer = %who.node_name, dialer, "session open");
    let seen = PeerSeen {
        device_id: peer.device_id.clone(),
        display_name: peer.display_name.clone(),
        role: Some(peer.role),
        ts_node_id: Some(who.stable_id.clone()).filter(|s| !s.is_empty()),
        ts_hostname: Some(who.node_name.clone()).filter(|s| !s.is_empty()),
        ip: Some(addr.ip().to_string()),
        os: None,
        now_ms: now_ms(),
    };
    let _ = inner.store.call(move |s| s.record_peer(&seen)).await;
    // A peer's key is as trustworthy as the session: 8.5 on desktops and relays, the
    // proof on phones (SPEC 21.3.2).
    if let Some(key) = extra.key
        && peer.role != Role::Mobile
    {
        let entry = PeerKey {
            device_id: peer.device_id.clone(),
            display_name: peer.display_name.clone(),
            role: peer.role,
            key,
            addrs: vec![http_addr.to_string()],
            trusted: pinned.is_some(),
            updated_ms: now_ms(),
        };
        let _ = inner.store.call(move |s| s.record_peer_key(&entry)).await;
    }
    inner.emit(SyncEvent::PeersChanged);

    let peer_has = Mutex::new(VersionVector::new());
    let (vv_tx, vv_rx) = oneshot::channel();
    let (pong_tx, pong_rx) = mpsc::channel(4);
    let ctx = Ctx {
        inner,
        me: &me,
        peer: &peer,
        who: &who,
        peer_has: &peer_has,
        sid,
        peer_minor: extra.minor,
        proven: pinned.is_some(),
    };
    let result = tokio::select! {
        r = ctx.writer(&mut sink, vv_rx, pong_rx) => r,
        r = ctx.reader(&mut stream, vv_tx, pong_tx) => r,
        _ = stop.wait_for(|s| *s) => Ok(()),
    };
    let _ = sink.close().await;
    let error = match &result {
        Err(e) if e.to_string().starts_with("peer error") => Some(e.to_string()),
        _ => None,
    };
    inner.unregister(&peer.device_id, sid, error);
    tracing::info!(peer = %who.node_name, "session closed");
    inner.emit(SyncEvent::PeersChanged);
    result
}

struct Ctx<'a> {
    inner: &'a Inner,
    me: &'a Me,
    peer: &'a Me,
    who: &'a PeerIdentity,
    peer_has: &'a Mutex<VersionVector>,
    sid: u64,
    peer_minor: u16,
    /// The peer proved a paired key (a phone's session).
    proven: bool,
}

/// Device names as a peer can decode them: minor-0 builds don't know `mobile`.
fn names_for(mut names: Vec<DeviceName>, peer_minor: u16) -> Vec<DeviceName> {
    if peer_minor == 0 {
        for n in &mut names {
            if n.role == Role::Mobile {
                n.role = Role::Device;
            }
        }
    }
    names
}

impl Ctx<'_> {
    async fn writer(
        &self,
        sink: &mut FrameSink,
        vv_rx: oneshot::Receiver<VersionVector>,
        mut pong_rx: mpsc::Receiver<u64>,
    ) -> anyhow::Result<()> {
        let mut changed = self.inner.changed.subscribe();
        let (vv, names) = self
            .inner
            .store
            .call(|s| -> tailcopy_core::Result<_> { Ok((s.version_vector()?, s.device_names()?)) })
            .await??;
        send(sink, &Frame::Vector { vv }).await?;
        send(
            sink,
            &Frame::Devices {
                devices: names_for(names.clone(), self.peer_minor),
            },
        )
        .await?;
        if self.peer.role == Role::Mobile && self.me.role != Role::Mobile {
            send(sink, &self.directory().await?).await?;
        }
        let mut sent_names = names;
        let their = tokio::time::timeout(HELLO_TIMEOUT, vv_rx)
            .await
            .map_err(|_| anyhow::anyhow!("no vector from the peer"))??;
        {
            let mut has = self.peer_has.lock().unwrap();
            for (origin, seq) in their {
                let e = has.entry(origin).or_insert(0);
                *e = (*e).max(seq);
            }
        }
        changed.mark_unchanged();
        let pushed = self.push(sink).await?;
        send(sink, &Frame::CaughtUp).await?;
        tracing::info!(peer = %self.who.node_name, ops = pushed, "sent backlog");

        let mut ping =
            tokio::time::interval_at(tokio::time::Instant::now() + PING_EVERY, PING_EVERY);
        loop {
            tokio::select! {
                r = changed.changed() => {
                    r?;
                    self.push(sink).await?;
                    let names = self.inner.store.call(|s| s.device_names()).await??;
                    if names != sent_names {
                        send(sink, &Frame::Devices { devices: names_for(names.clone(), self.peer_minor) }).await?;
                        sent_names = names;
                    }
                }
                _ = ping.tick() => send(sink, &Frame::Ping { t: now_ms() }).await?,
                Some(t) = pong_rx.recv() => send(sink, &Frame::Pong { t }).await?,
            }
        }
    }

    /// This node and the desktops and relays it knows keys for (SPEC 21.3.5).
    async fn directory(&self) -> anyhow::Result<Frame> {
        let mut peers = Vec::new();
        if let Some(id) = self.inner.cfg().identity {
            peers.push(DirEntry {
                device_id: self.me.device_id.clone(),
                display_name: self.me.display_name.clone(),
                role: self.me.role,
                key: ByteBuf::from(id.public_key().to_vec()),
                addrs: self
                    .inner
                    .listening()
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            });
        }
        let now = now_ms();
        let keys = self.inner.store.call(|s| s.peer_keys()).await??;
        peers.extend(
            keys.into_iter()
                .filter(|k| k.role != Role::Mobile && k.device_id != self.peer.device_id)
                .filter(|k| now.saturating_sub(k.updated_ms) < DIRECTORY_MAX_AGE_MS)
                .map(|k| DirEntry {
                    device_id: k.device_id,
                    display_name: k.display_name,
                    role: k.role,
                    key: ByteBuf::from(k.key.to_vec()),
                    addrs: k.addrs,
                }),
        );
        Ok(Frame::Directory { peers })
    }

    /// A phone trusts what a proven peer vouches for (SPEC 21.3.5).
    async fn learn_directory(&self, peers: Vec<DirEntry>) -> anyhow::Result<()> {
        if self.me.role != Role::Mobile || !self.proven {
            tracing::warn!(peer = %self.who.node_name, "ignored a Directory this side didn't ask for");
            return Ok(());
        }
        let now = now_ms();
        let entries: Vec<PeerKey> = peers
            .into_iter()
            .filter(|e| e.role != Role::Mobile)
            .filter_map(|e| {
                Some(PeerKey {
                    key: e.key.as_slice().try_into().ok()?,
                    device_id: e.device_id,
                    display_name: e.display_name,
                    role: e.role,
                    addrs: e.addrs,
                    trusted: true,
                    updated_ms: now,
                })
            })
            .collect();
        self.inner
            .store
            .call(move |s| -> tailcopy_core::Result<()> {
                for e in &entries {
                    s.record_peer_key(e)?;
                }
                Ok(())
            })
            .await??;
        self.inner.emit(SyncEvent::PeersChanged);
        Ok(())
    }

    /// Sends every op the peer lacks, inline blobs first. Returns how many.
    async fn push(&self, sink: &mut FrameSink) -> anyhow::Result<usize> {
        let mut total = 0;
        loop {
            let their = self.peer_has.lock().unwrap().clone();
            let peer = self.peer.device_id.clone();
            let node = self.who.stable_id.clone();
            let inline = self.inner.cfg().inline_blob_bytes;
            let (ops, blobs) = self
                .inner
                .store
                .call(move |s| outgoing(s, &their, &peer, &node, inline))
                .await??;
            if ops.is_empty() {
                return Ok(total);
            }
            for (hash, data) in blobs {
                send(
                    sink,
                    &Frame::Blob {
                        hash,
                        data: ByteBuf::from(data),
                    },
                )
                .await?;
            }
            total += ops.len();
            {
                let mut has = self.peer_has.lock().unwrap();
                for op in &ops {
                    let e = has.entry(op.origin.clone()).or_insert(0);
                    *e = (*e).max(op.seq);
                }
            }
            send(sink, &Frame::Ops { ops }).await?;
        }
    }

    async fn reader(
        &self,
        stream: &mut FrameStream,
        vv_tx: oneshot::Sender<VersionVector>,
        pong_tx: mpsc::Sender<u64>,
    ) -> anyhow::Result<()> {
        let mut vv_tx = Some(vv_tx);
        let mut caught_up = false;
        let mut pending: Vec<(Hash32, Vec<u8>)> = Vec::new();
        let mut pending_bytes = 0usize;
        loop {
            match recv(stream).await? {
                Frame::Vector { vv } => {
                    let check = vv.clone();
                    if self
                        .inner
                        .store
                        .call(move |s| s.observe_peer_vector(&check))
                        .await??
                    {
                        self.inner.device_id_changed();
                        anyhow::bail!("this device's id was reused");
                    }
                    if let Some(tx) = vv_tx.take() {
                        let _ = tx.send(vv);
                    }
                }
                Frame::Devices { devices } => {
                    let names: Vec<DeviceName> = devices;
                    if self
                        .inner
                        .store
                        .call(move |s| s.merge_device_names(&names))
                        .await??
                    {
                        self.inner.emit(SyncEvent::PeersChanged);
                    }
                }
                Frame::Blob { hash, data } => {
                    if blake3_hash(&data) != hash {
                        tracing::warn!(peer = %self.who.node_name, "dropped an inline blob with a bad hash");
                        continue;
                    }
                    pending_bytes += data.len();
                    if pending_bytes > MAX_PENDING_BLOB_BYTES {
                        anyhow::bail!("too many inline blobs before an Ops frame");
                    }
                    pending.push((hash, data.into_vec()));
                }
                Frame::Ops { ops } => {
                    if ops.len() > MAX_OPS_PER_FRAME {
                        anyhow::bail!("{} ops in one frame", ops.len());
                    }
                    pending_bytes = 0;
                    let blobs = std::mem::take(&mut pending);
                    self.apply(ops, blobs, caught_up).await?;
                }
                Frame::CaughtUp => {
                    caught_up = true;
                    let sid = self.sid;
                    self.inner.with_peer(&self.peer.device_id, |p| {
                        p.last_sync_ms = Some(now_ms());
                        if let Some(s) = p.session.as_mut().filter(|s| s.id == sid) {
                            s.caught_up = true;
                        }
                    });
                    tracing::info!(peer = %self.who.node_name, "caught up");
                    self.inner.emit(SyncEvent::PeersChanged);
                }
                Frame::Ping { t } => {
                    let _ = pong_tx.try_send(t);
                }
                Frame::Pong { .. } => {}
                Frame::Error { code, message } => {
                    anyhow::bail!("peer error {code:?}: {message}");
                }
                Frame::Hello { .. } => anyhow::bail!("a second Hello"),
                Frame::Directory { peers } => self.learn_directory(peers).await?,
            }
        }
    }

    async fn apply(
        &self,
        ops: Vec<Op>,
        blobs: Vec<(Hash32, Vec<u8>)>,
        caught_up: bool,
    ) -> anyhow::Result<()> {
        let incoming = ops.clone();
        let (report, stored) = self
            .inner
            .store
            .call(move |s| -> anyhow::Result<_> {
                // SPEC 7.4 step 6: a seq more than one past what's held is a gap.
                let mut vv = s.version_vector()?;
                for op in &ops {
                    let have = vv.get(&op.origin).copied().unwrap_or(0);
                    if op.seq > have + 1 {
                        anyhow::bail!("gap in {}'s log: got {} after {}", op.origin, op.seq, have);
                    }
                    if op.seq == have + 1 {
                        vv.insert(op.origin.clone(), op.seq);
                    }
                }
                let report = s.apply_remote_ops(&ops)?;
                let mut stored = Vec::new();
                for (hash, bytes) in blobs {
                    match s.put_blob(&hash, &bytes) {
                        Ok(true) => stored.push(hash),
                        Ok(false) => {}
                        Err(e) => tracing::warn!("couldn't store an inline blob: {e}"),
                    }
                }
                Ok((report, stored))
            })
            .await??;
        {
            let mut has = self.peer_has.lock().unwrap();
            for op in &incoming {
                let e = has.entry(op.origin.clone()).or_insert(0);
                *e = (*e).max(op.seq);
            }
        }
        if report.regenerated_device_id {
            self.inner.device_id_changed();
            anyhow::bail!("this device's id was reused");
        }
        if report.skewed > 0 {
            tracing::warn!(peer = %self.who.node_name, ops = report.skewed, "clock ahead by more than 60 s");
        }
        let now = now_ms();
        let live: Vec<Op> = if caught_up {
            incoming
                .iter()
                .filter(|op| op.origin != self.me.device_id)
                .filter(|op| matches!(op.body, OpBody::ClipAdded { .. } | OpBody::ClipUsed { .. }))
                .filter(|op| wall_ms(op.hlc).abs_diff(now) <= LIVE_WINDOW_MS)
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        let mut hinted = BTreeSet::new();
        {
            let mut st = self.inner.state.lock().unwrap();
            for op in &incoming {
                if let OpBody::ClipAdded { formats, .. } = &op.body {
                    for f in formats {
                        st.blob_hints
                            .insert(f.blob_hash, self.peer.device_id.clone());
                        hinted.insert(f.blob_hash);
                    }
                }
            }
            if let Some(p) = st.peers.get_mut(&self.peer.device_id) {
                p.last_sync_ms = Some(now);
            }
        }
        if report.applied > 0 {
            self.inner.bump_changed();
            tracing::debug!(peer = %self.who.node_name, applied = report.applied, live = live.len(), "applied ops");
            self.inner.emit(SyncEvent::Applied {
                from: self.peer.device_id.clone(),
                applied: report.applied,
                live,
            });
        }
        for hash in stored {
            self.inner.emit(SyncEvent::BlobStored(hash));
        }
        if !hinted.is_empty() {
            self.inner.fetch.notify_one();
        }
        Ok(())
    }
}

/// The next page of ops for a peer, with Send off applied (D-023), and the inline
/// blobs those ops need.
#[allow(clippy::type_complexity)]
fn outgoing(
    s: &mut tailcopy_core::Store,
    their: &VersionVector,
    peer: &DeviceId,
    node: &str,
    inline: u64,
) -> tailcopy_core::Result<(Vec<Op>, Vec<(Hash32, Vec<u8>)>)> {
    let ops = s.ops_for_peer(their, MAX_OPS_PER_FRAME)?;
    if ops.is_empty() {
        return Ok((ops, Vec::new()));
    }
    // The device id is the peer's own claim; Send off also follows the tailnet
    // node behind it, so a new id from the same node doesn't escape the switch.
    let send = s.sync_flags()?.get(peer).is_none_or(|f| f.send) && s.node_send_enabled(node)?;
    let me = s.device_id().clone();
    let ops: Vec<Op> = ops
        .into_iter()
        .map(|op| {
            if !send && op.origin == me {
                op.to_stub()
            } else {
                op
            }
        })
        .collect();
    let mut blobs = Vec::new();
    let mut seen = BTreeSet::new();
    let view = s.view();
    for op in &ops {
        if let OpBody::ClipAdded {
            formats,
            trimmed: false,
            ..
        } = &op.body
        {
            if formats.iter().map(|f| f.size).sum::<u64>() > inline {
                continue;
            }
            for f in formats {
                if seen.insert(f.blob_hash)
                    && let Some(bytes) = view.read_blob(&f.blob_hash)?
                {
                    blobs.push((f.blob_hash, bytes));
                }
            }
        }
    }
    Ok((ops, blobs))
}
