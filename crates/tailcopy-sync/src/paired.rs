//! A phone's [`Transport`] (SPEC 21.3): no whois and no listener. Candidates are
//! the paired and vouched desktops and relays in `peer_keys`, each checked by its
//! identity key when the session starts.

use std::net::{IpAddr, SocketAddr};

use async_trait::async_trait;
use tailcopy_core::Role;

use crate::store::StoreHandle;
use crate::transport::{Candidate, PeerIdentity, Pinned, Transport};

/// This device's tailnet addresses, or `None` when it isn't on the tailnet.
pub type TailnetAddrs = Box<dyn Fn() -> Option<Vec<IpAddr>> + Send + Sync>;

pub struct PairedTransport {
    store: StoreHandle,
    tailnet: TailnetAddrs,
}

impl PairedTransport {
    pub fn new(store: StoreHandle, tailnet: TailnetAddrs) -> Self {
        Self { store, tailnet }
    }
}

/// Whether `ip` is a Tailscale address (`100.64.0.0/10` or `fd7a:115c:a1e0::/48`).
pub fn is_tailscale_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            o[0] == 100 && (o[1] & 0xc0) == 64
        }
        IpAddr::V6(v6) => {
            let s = v6.segments();
            s[0] == 0xfd7a && s[1] == 0x115c && s[2] == 0xa1e0
        }
    }
}

#[async_trait]
impl Transport for PairedTransport {
    async fn listen_addrs(&self) -> anyhow::Result<Vec<SocketAddr>> {
        match (self.tailnet)() {
            Some(ips) if !ips.is_empty() => Ok(Vec::new()),
            _ => anyhow::bail!("Tailscale isn't connected"),
        }
    }

    async fn whois(&self, _remote: SocketAddr) -> anyhow::Result<PeerIdentity> {
        anyhow::bail!("a phone can't ask Tailscale who a peer is")
    }

    async fn candidates(&self) -> anyhow::Result<Vec<Candidate>> {
        if (self.tailnet)().is_none_or(|ips| ips.is_empty()) {
            anyhow::bail!("Tailscale isn't connected");
        }
        let keys = self.store.call(|s| s.peer_keys()).await??;
        Ok(keys
            .into_iter()
            .filter(|k| k.trusted && k.role != Role::Mobile)
            .filter_map(|k| {
                let addr: SocketAddr = k.addrs.iter().find_map(|a| a.parse().ok())?;
                Some(Candidate {
                    node_name: k.display_name.clone(),
                    stable_id: k.device_id.0.clone(),
                    addr,
                    relay_tagged: k.role == Role::Relay,
                    pinned: Some(Pinned {
                        device_id: k.device_id,
                        key: k.key,
                    }),
                })
            })
            .collect())
    }

    fn dial_from(&self, peer: IpAddr) -> Option<IpAddr> {
        (self.tailnet)()?
            .into_iter()
            .find(|ip| ip.is_ipv4() == peer.is_ipv4())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tailscale_ranges() {
        for ip in ["100.64.0.1", "100.127.255.254", "fd7a:115c:a1e0::1"] {
            assert!(is_tailscale_ip(ip.parse().unwrap()), "{ip}");
        }
        for ip in [
            "100.63.255.255",
            "100.128.0.1",
            "192.168.1.2",
            "fd7a:115c:a1e1::1",
        ] {
            assert!(!is_tailscale_ip(ip.parse().unwrap()), "{ip}");
        }
    }
}
