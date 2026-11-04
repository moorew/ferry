//! Tailscale adapter: status, whois and the facts authorization decisions need
//! (SPEC 8).

mod backend;
mod cache;
mod model;
mod transport;

use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub use model::{Node, Path, Status, User, Whois};
pub use transport::TailnetTransport;

use cache::WhoisCache;

/// Default tailscaled LocalAPI socket on Linux.
pub const DEFAULT_SOCKET: &str = "/var/run/tailscale/tailscaled.sock";

/// How long a whois result is reused for the same remote IP (SPEC 8.2).
pub const WHOIS_TTL: Duration = Duration::from_secs(300);

/// Where status and whois come from (SPEC 8.1).
#[derive(Debug, Clone)]
pub enum Backend {
    /// The `tailscale` CLI, at `path` or the platform's usual location.
    Cli { path: Option<PathBuf> },
    /// tailscaled's LocalAPI over a Unix socket.
    LocalApi { socket: PathBuf },
}

#[derive(Debug, Clone)]
pub struct Options {
    pub backend: Backend,
    /// App capability name checked by [`Whois::has_capability`].
    pub capability: String,
    /// Tag that marks a TailCopy relay.
    pub relay_tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("Tailscale isn't installed (the tailscale command wasn't found)")]
    Missing,
    #[error("Tailscale isn't running ({0})")]
    Stopped(String),
    #[error("Tailscale error: {0}")]
    Other(String),
}

/// Handle to the local Tailscale client. Cheap to clone.
#[derive(Clone)]
pub struct Tailnet {
    inner: Arc<Inner>,
}

struct Inner {
    opts: Options,
    latest: Mutex<Option<(Instant, Arc<Status>)>>,
    whois: Mutex<WhoisCache>,
}

impl Tailnet {
    pub fn new(opts: Options) -> Tailnet {
        Tailnet {
            inner: Arc::new(Inner {
                opts,
                latest: Mutex::new(None),
                whois: Mutex::new(WhoisCache::new(WHOIS_TTL)),
            }),
        }
    }

    pub fn capability(&self) -> &str {
        &self.inner.opts.capability
    }

    pub fn relay_tag(&self) -> &str {
        &self.inner.opts.relay_tag
    }

    /// Fresh status from the backend, stored as the latest snapshot. Fails with
    /// [`Error::Stopped`] unless the backend state is `Running`.
    pub async fn refresh(&self) -> Result<Arc<Status>, Error> {
        let result = self.inner.opts.backend.status().await.and_then(|s| {
            if s.running() {
                Ok(Arc::new(s))
            } else {
                Err(Error::Stopped(s.backend_state))
            }
        });
        let mut latest = lock(&self.inner.latest);
        match result {
            Ok(status) => {
                tracing::debug!(peers = status.peers.len(), "tailscale status refreshed");
                *latest = Some((Instant::now(), status.clone()));
                Ok(status)
            }
            Err(e) => {
                tracing::warn!("tailscale status failed: {e}");
                *latest = None;
                Err(e)
            }
        }
    }

    /// The latest status if it is younger than `max_age`, otherwise [`Self::refresh`].
    pub async fn status(&self, max_age: Duration) -> Result<Arc<Status>, Error> {
        if let Some((at, status)) = lock(&self.inner.latest).as_ref()
            && at.elapsed() < max_age
        {
            return Ok(status.clone());
        }
        self.refresh().await
    }

    /// Whois for a remote address, cached per IP for [`WHOIS_TTL`].
    pub async fn whois(&self, addr: SocketAddr) -> Result<Arc<Whois>, Error> {
        let ip = addr.ip();
        if let Some(hit) = lock(&self.inner.whois).get(ip, Instant::now()) {
            return Ok(hit);
        }
        match self.inner.opts.backend.whois(addr).await {
            Ok(w) => {
                let w = Arc::new(w);
                tracing::debug!(%ip, node = %w.node_name, "whois");
                lock(&self.inner.whois).insert(ip, w.clone(), Instant::now());
                Ok(w)
            }
            Err(e) => {
                tracing::warn!(%ip, "whois failed: {e}");
                Err(e)
            }
        }
    }

    pub fn forget_whois(&self, ip: IpAddr) {
        lock(&self.inner.whois).remove(ip);
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send<T: Send>(_: T) {}

    #[test]
    fn futures_are_send() {
        let t = Tailnet::new(Options {
            backend: Backend::Cli { path: None },
            capability: String::new(),
            relay_tag: String::new(),
        });
        assert_send(t.refresh());
        assert_send(t.status(Duration::ZERO));
        assert_send(t.whois(SocketAddr::from(([100, 64, 0, 1], 7441))));
    }
}
