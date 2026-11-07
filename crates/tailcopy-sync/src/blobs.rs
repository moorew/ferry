//! Fetching blobs that didn't travel inline (SPEC 7.5).

use std::sync::Arc;
use std::time::{Duration, Instant};

use tailcopy_core::model::hash_hex;
use tailcopy_core::{Hash32, Role};

use crate::engine::{Inner, SyncEvent};

/// A blob no peer could serve isn't tried again for this long, unless asked.
const RETRY_AFTER: Duration = Duration::from_secs(30);
const IDLE_CHECK: Duration = Duration::from_secs(30);

pub(crate) async fn fetch_loop(inner: Arc<Inner>) {
    let mut shutdown = inner.shutdown.subscribe();
    loop {
        tokio::select! {
            () = inner.fetch.notified() => {}
            () = tokio::time::sleep(IDLE_CHECK) => {}
            _ = shutdown.wait_for(|s| *s) => return,
        }
        let Ok(Ok(missing)) = inner.store.call(|s| s.missing_blobs()).await else {
            continue;
        };
        let max = inner.cfg().max_fetch_bytes;
        for (hash, size) in missing {
            if size > max || inner.is_shutdown() {
                continue;
            }
            let recent = inner
                .state
                .lock()
                .unwrap()
                .fetch_failed
                .get(&hash)
                .is_some_and(|at| at.elapsed() < RETRY_AFTER);
            if recent {
                continue;
            }
            if !fetch_one(&inner, hash, size).await {
                inner
                    .state
                    .lock()
                    .unwrap()
                    .fetch_failed
                    .insert(hash, Instant::now());
            }
        }
    }
}

/// Tries the peer that sent the op first, then every other connected peer.
async fn fetch_one(inner: &Inner, hash: Hash32, size: u64) -> bool {
    let addrs = {
        let st = inner.state.lock().unwrap();
        let hint = st.blob_hints.get(&hash);
        let mut peers: Vec<_> = st
            .peers
            .iter()
            .filter(|(_, p)| p.session.is_some() && p.role != Role::Mobile)
            .filter_map(|(id, p)| Some((Some(id) != hint, p.http_addr?, p.node_name.clone())))
            .collect();
        peers.sort();
        peers
    };
    let hex = hash_hex(&hash);
    // The size in the op is the peer's claim; the cap bounds what we buffer either way.
    let max = inner.cfg().max_fetch_bytes;
    let limit = if size == 0 { max } else { size.min(max) };
    let timeout = Duration::from_secs(10) + Duration::from_millis(size / 1000);
    for (_, addr, node) in addrs {
        let client = crate::dial::http_client(inner, addr, timeout);
        let resp = match client
            .get(format!("http://{addr}/v1/blob/{hex}"))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                tracing::debug!(peer = %node, status = %r.status(), "blob not there");
                continue;
            }
            Err(e) => {
                tracing::debug!(peer = %node, "blob fetch failed: {e}");
                continue;
            }
        };
        let Some(bytes) = read_limited(resp, limit).await else {
            tracing::warn!(peer = %node, "a blob was larger than its clip declared; trying another peer");
            continue;
        };
        let len = bytes.len();
        match inner.store.call(move |s| s.put_blob(&hash, &bytes)).await {
            Ok(Ok(_)) => {
                tracing::debug!(peer = %node, bytes = len, "fetched a blob");
                inner.state.lock().unwrap().fetch_failed.remove(&hash);
                inner.emit(SyncEvent::BlobStored(hash));
                return true;
            }
            Ok(Err(
                tailcopy_core::Error::BlobHashMismatch | tailcopy_core::Error::BlobTooLarge,
            )) => {
                tracing::warn!(peer = %node, "a fetched blob had the wrong hash; trying another peer");
            }
            Ok(Err(e)) => tracing::warn!("couldn't store a fetched blob: {e}"),
            Err(_) => return false,
        }
    }
    false
}

/// Reads a response body, giving up as soon as it passes `limit` bytes.
async fn read_limited(mut resp: reqwest::Response, limit: u64) -> Option<Vec<u8>> {
    if resp.content_length().is_some_and(|l| l > limit) {
        return None;
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.ok()? {
        if (body.len() + chunk.len()) as u64 > limit {
            return None;
        }
        body.extend_from_slice(&chunk);
    }
    Some(body)
}

pub(crate) async fn fetch_now(inner: &Inner, hash: Hash32, timeout: Duration) -> bool {
    let present = inner.store.call(move |s| s.blob_present(&hash)).await;
    if matches!(present, Ok(Ok(true))) {
        return true;
    }
    inner.state.lock().unwrap().fetch_failed.remove(&hash);
    tokio::time::timeout(timeout, fetch_one(inner, hash, 0))
        .await
        .unwrap_or(false)
}
