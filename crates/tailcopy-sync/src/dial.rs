//! Probing and dialling a candidate (SPEC 7.7, 8.5 outbound).

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use tailcopy_core::{DeviceId, Role};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

use crate::engine::{Attempt, DIAL_ANYWAY, Inner};
use crate::frame::{HelloInfo, MAX_FRAME_BYTES, PROTO_MAJOR};
use crate::transport::Candidate;

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn http_client(inner: &Inner, peer: SocketAddr, timeout: Duration) -> reqwest::Client {
    // No TLS here, but reqwest may be built with rustls (see Cargo.toml), and then
    // it needs a provider; installing one twice is a no-op.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut b = reqwest::Client::builder()
        .no_proxy()
        .timeout(timeout)
        .connect_timeout(CONNECT_TIMEOUT);
    if let Some(from) = inner.transport.dial_from(peer.ip()) {
        b = b.local_address(from);
    }
    b.build().expect("an HTTP client with no TLS")
}

/// `GET /v1/hello` with the SPEC 7.7 two-second timeout.
pub(crate) async fn probe(inner: &Inner, addr: SocketAddr) -> anyhow::Result<HelloInfo> {
    let resp = http_client(inner, addr, PROBE_TIMEOUT)
        .get(format!("http://{addr}/v1/hello"))
        .send()
        .await?;
    if !resp.status().is_success() {
        anyhow::bail!("answered {}", resp.status());
    }
    Ok(serde_json::from_slice(&resp.bytes().await?)?)
}

pub(crate) async fn attempt(inner: &Inner, cand: &Candidate, online_since: Instant) -> Attempt {
    let me = match inner.me().await {
        Ok(me) => me,
        Err(_) => return Attempt::Failed,
    };
    let hello = match probe(inner, cand.addr).await {
        Ok(h) => h,
        Err(e) => {
            tracing::debug!(peer = %cand.node_name, "no TailCopy answering: {e}");
            return Attempt::Failed;
        }
    };
    if hello.device_id == me.device_id {
        return Attempt::Wait(Duration::from_secs(3600));
    }
    let set_error = |msg: Option<String>| {
        inner.with_peer(&hello.device_id, |p| p.error = msg);
    };
    if hello.proto_major != PROTO_MAJOR {
        tracing::warn!(peer = %cand.node_name, theirs = hello.proto_major, "protocol mismatch");
        set_error(Some(format!("Update TailCopy on {}", hello.display_name)));
        return Attempt::Wait(Duration::from_secs(300));
    }
    let role = Role::parse(&hello.role).unwrap_or(Role::Device);
    if role == Role::Mobile {
        // Phones dial; nobody dials them (SPEC 21.3.1).
        return Attempt::Wait(Duration::from_secs(3600));
    }
    let who = if let Some(p) = &cand.pinned {
        // A phone can't whois; the session proof checks the key (SPEC 21.3.4).
        if hello.proto_minor < 1 {
            set_error(Some(format!("Update TailCopy on {}", hello.display_name)));
            return Attempt::Wait(Duration::from_secs(300));
        }
        if hello.device_id != p.device_id {
            tracing::warn!(peer = %cand.node_name, "a different device answers at a paired address");
            inner.note_peer_error(
                &p.device_id,
                &crate::PeerIdentity {
                    node_name: cand.node_name.clone(),
                    stable_id: cand.stable_id.clone(),
                    ..Default::default()
                },
                format!("{} isn't at its last address", cand.node_name),
            );
            return Attempt::Wait(Duration::from_secs(60));
        }
        crate::PeerIdentity {
            node_name: cand.node_name.clone(),
            stable_id: cand.stable_id.clone(),
            same_user: true,
            ..Default::default()
        }
    } else {
        let who = match inner.transport.whois(cand.addr).await {
            Ok(w) => w,
            Err(e) => {
                tracing::warn!(peer = %cand.node_name, "whois failed: {e}");
                return Attempt::Failed;
            }
        };
        if !inner.authorize_outbound(&who, role) {
            return Attempt::Wait(Duration::from_secs(300));
        }
        who
    };
    let lower = me.device_id < hello.device_id;
    let dial = me.role == Role::Mobile
        || role == Role::Relay
        || lower
        || online_since.elapsed() >= DIAL_ANYWAY;
    if !dial {
        // The lower id dials; give it time before dialling from this side too.
        return Attempt::Wait(DIAL_ANYWAY.saturating_sub(online_since.elapsed()));
    }
    if is_connected(inner, &hello.device_id) {
        return Attempt::Wait(Duration::from_secs(5));
    }
    let socket = match connect(inner, cand.addr).await {
        Ok(s) => s,
        Err(e) => {
            tracing::info!(peer = %cand.node_name, "couldn't open a session: {e}");
            return Attempt::Failed;
        }
    };
    let started = Instant::now();
    let (sink, stream) = crate::ws::from_tungstenite(socket);
    let result = crate::session::run(
        inner,
        sink,
        stream,
        true,
        who,
        cand.addr,
        Some(hello.device_id),
        cand.pinned.clone(),
    )
    .await;
    if let Err(e) = result {
        tracing::info!(peer = %cand.node_name, "session ended: {e:#}");
    }
    Attempt::Ended {
        lasted: started.elapsed(),
    }
}

fn is_connected(inner: &Inner, peer: &DeviceId) -> bool {
    inner
        .state
        .lock()
        .unwrap()
        .peers
        .get(peer)
        .is_some_and(|p| p.session.is_some())
}

async fn connect(
    inner: &Inner,
    addr: SocketAddr,
) -> anyhow::Result<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>> {
    let socket = if addr.is_ipv4() {
        tokio::net::TcpSocket::new_v4()?
    } else {
        tokio::net::TcpSocket::new_v6()?
    };
    if let Some(from) = inner.transport.dial_from(addr.ip()) {
        socket.bind(SocketAddr::new(from, 0))?;
    }
    let stream = tokio::time::timeout(CONNECT_TIMEOUT, socket.connect(addr)).await??;
    stream.set_nodelay(true)?;
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME_BYTES))
        .max_frame_size(Some(MAX_FRAME_BYTES));
    let (ws, _) = tokio::time::timeout(
        CONNECT_TIMEOUT,
        tokio_tungstenite::client_async_with_config(
            format!("ws://{addr}/v1/sync"),
            stream,
            Some(config),
        ),
    )
    .await??;
    Ok(ws)
}
