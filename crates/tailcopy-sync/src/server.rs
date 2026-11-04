//! HTTP endpoints (SPEC 7.2). Every request is authorized by whois first (SPEC 8.5).

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tailcopy_core::Hash32;

use crate::engine::Inner;
use crate::frame::MAX_FRAME_BYTES;
use crate::transport::PeerIdentity;

pub(crate) fn router(inner: Arc<Inner>) -> Router {
    Router::new()
        .route("/v1/hello", get(hello))
        .route("/v1/sync", get(sync))
        .route("/v1/blob/{hash}", get(blob))
        .with_state(inner)
}

/// The peer's identity if it may sync; refusals are logged and get a 403.
async fn authorize(inner: &Inner, remote: SocketAddr) -> Option<PeerIdentity> {
    match inner.transport.whois(remote).await {
        Ok(who) if inner.authorize_inbound(&who) => Some(who),
        Ok(_) => None,
        Err(e) => {
            tracing::warn!(%remote, "whois failed, refusing: {e}");
            None
        }
    }
}

fn forbidden() -> Response {
    StatusCode::FORBIDDEN.into_response()
}

async fn hello(
    State(inner): State<Arc<Inner>>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
) -> Response {
    if authorize(&inner, remote).await.is_none() {
        return forbidden();
    }
    match inner.hello_info().await {
        Ok(info) => axum::Json(info).into_response(),
        Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

async fn sync(
    State(inner): State<Arc<Inner>>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    ws: WebSocketUpgrade,
) -> Response {
    let Some(who) = authorize(&inner, remote).await else {
        return forbidden();
    };
    ws.max_message_size(MAX_FRAME_BYTES)
        .max_frame_size(MAX_FRAME_BYTES)
        .on_upgrade(move |socket| async move {
            let (sink, stream) = crate::ws::from_axum(socket);
            let node = who.node_name.clone();
            if let Err(e) =
                crate::session::run(&inner, sink, stream, false, who, remote, None).await
            {
                tracing::info!(peer = %node, "session ended: {e:#}");
            }
        })
}

pub(crate) fn parse_hex(hex: &str) -> Option<Hash32> {
    if hex.len() != 64 || !hex.is_ascii() {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..32)
        .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok())
        .collect();
    tailcopy_core::model::hash_from_slice(&bytes?).ok()
}

async fn blob(
    State(inner): State<Arc<Inner>>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    Path(hex): Path<String>,
) -> Response {
    let Some(who) = authorize(&inner, remote).await else {
        return forbidden();
    };
    let Some(hash) = parse_hex(&hex) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let peer = {
        let st = inner.state.lock().unwrap();
        st.peers
            .iter()
            .find(|(_, p)| p.stable_id == who.stable_id && !who.stable_id.is_empty())
            .map(|(id, _)| id.clone())
    };
    let bytes = inner
        .store
        .call(move |s| -> tailcopy_core::Result<Option<Vec<u8>>> {
            // Send off for this device (D-023): its content stays here.
            if let Some(peer) = peer
                && s.sync_flags()?.get(&peer).is_some_and(|f| !f.send)
            {
                return Ok(None);
            }
            s.view().read_blob(&hash)
        })
        .await;
    match bytes {
        Ok(Ok(Some(b))) => b.into_response(),
        Ok(Ok(None)) => StatusCode::NOT_FOUND.into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
