//! The sync engine's [`Transport`] on the system Tailscale client (SPEC 7.1, 8).

use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use tailcopy_sync::{Candidate, PeerIdentity, Transport};

use crate::Tailnet;

/// Status is polled this often while running (SPEC 8.2).
const STATUS_MAX_AGE: Duration = Duration::from_secs(15);

pub struct TailnetTransport {
    tailnet: Tailnet,
    port: u16,
    /// `sync.relay`: a MagicDNS name or tailnet address that is always a candidate.
    relay: Option<String>,
    /// This node's Tailscale addresses, from the last status.
    own_ips: Mutex<Vec<IpAddr>>,
    login: Mutex<LoginGuard>,
}

/// Which Tailscale login may sync (D-027). History belongs to the login it was
/// captured under; after a switch nobody is "same user" until the user approves.
#[derive(Clone, Debug, PartialEq, Eq)]
enum LoginGuard {
    /// No guard (the relay, tests): the current login is trusted.
    Off,
    /// Guarded, but no login approved yet: refuse everyone.
    Pending,
    Approved(String),
}

impl TailnetTransport {
    pub fn new(tailnet: Tailnet, port: u16) -> Self {
        Self {
            tailnet,
            port,
            relay: None,
            own_ips: Mutex::new(Vec::new()),
            login: Mutex::new(LoginGuard::Off),
        }
    }

    /// Only allows "same user" while this machine is signed in to an approved login
    /// (see [`approve_login`](Self::approve_login)). Until then nobody is.
    pub fn with_login_guard(self) -> Self {
        *self.login.lock().unwrap() = LoginGuard::Pending;
        self
    }

    /// Allows sync while this machine is signed in to `login`.
    pub fn approve_login(&self, login: &str) {
        *self.login.lock().unwrap() = LoginGuard::Approved(login.to_owned());
    }

    /// This machine's current Tailscale login, if it has one (untagged, signed in).
    pub async fn self_login(&self) -> anyhow::Result<Option<String>> {
        Ok(self.status().await?.self_login().map(str::to_owned))
    }

    /// Always offers this relay as a candidate (SPEC 7.7). Empty means none.
    pub fn with_relay(mut self, relay: &str) -> Self {
        let relay = relay.trim().trim_end_matches('.');
        self.relay = (!relay.is_empty()).then(|| relay.to_lowercase());
        self
    }

    pub fn tailnet(&self) -> &Tailnet {
        &self.tailnet
    }

    async fn status(&self) -> anyhow::Result<std::sync::Arc<crate::Status>> {
        let status = self.tailnet.status(STATUS_MAX_AGE).await?;
        if let Some(me) = &status.self_node {
            *self.own_ips.lock().unwrap() = me.ips.clone();
        }
        Ok(status)
    }
}

fn first_label(name: &str) -> &str {
    name.split('.')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(name)
}

#[async_trait]
impl Transport for TailnetTransport {
    async fn listen_addrs(&self) -> anyhow::Result<Vec<SocketAddr>> {
        let status = self.status().await?;
        let me = status
            .self_node
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tailscale reported no addresses for this device"))?;
        Ok(me
            .ips
            .iter()
            .map(|ip| SocketAddr::new(*ip, self.port))
            .collect())
    }

    async fn whois(&self, remote: SocketAddr) -> anyhow::Result<PeerIdentity> {
        let who = self.tailnet.whois(remote).await?;
        let status = self.status().await?;
        let me = status.self_login();
        let allowed = match &*self.login.lock().unwrap() {
            LoginGuard::Off => true,
            LoginGuard::Pending => false,
            LoginGuard::Approved(l) => me == Some(l.as_str()),
        };
        let same_user = allowed && me.is_some_and(|l| who.same_user(l));
        Ok(PeerIdentity {
            node_name: first_label(&who.node_name).to_owned(),
            stable_id: who.stable_id.clone(),
            same_user,
            has_capability: who.has_capability(self.tailnet.capability()),
            relay_tagged: who.has_tag(self.tailnet.relay_tag()),
        })
    }

    async fn candidates(&self) -> anyhow::Result<Vec<Candidate>> {
        let status = self.status().await?;
        let tag = self.tailnet.relay_tag();
        let candidate = |n: &crate::Node| {
            let ip = n.ipv4().or_else(|| n.ips.first().copied())?;
            Some(Candidate {
                node_name: n.short_name().to_owned(),
                stable_id: n.stable_id.clone(),
                addr: SocketAddr::new(ip, self.port),
                relay_tagged: n.has_tag(tag),
                pinned: None,
            })
        };
        let mut out: Vec<Candidate> = status
            .candidates(tag)
            .into_iter()
            .filter_map(candidate)
            .collect();
        if let Some(relay) = &self.relay {
            let ip: Option<IpAddr> = relay.parse().ok();
            let node = status.peers.iter().find(|n| {
                ip.is_some_and(|ip| n.ips.contains(&ip))
                    || n.short_name().eq_ignore_ascii_case(relay)
                    || n.host_name.eq_ignore_ascii_case(relay)
                    || n.dns_name.trim_end_matches('.').eq_ignore_ascii_case(relay)
            });
            let pinned = match (node, ip) {
                (Some(n), _) => candidate(n),
                (None, Some(ip)) => Some(Candidate {
                    node_name: relay.clone(),
                    stable_id: relay.clone(),
                    addr: SocketAddr::new(ip, self.port),
                    relay_tagged: false,
                    pinned: None,
                }),
                (None, None) => None,
            };
            if let Some(c) = pinned
                && !out.iter().any(|o| o.stable_id == c.stable_id)
            {
                out.push(c);
            }
        }
        Ok(out)
    }

    fn dial_from(&self, peer: IpAddr) -> Option<IpAddr> {
        self.own_ips
            .lock()
            .unwrap()
            .iter()
            .copied()
            .find(|ip| ip.is_ipv4() == peer.is_ipv4())
    }
}
