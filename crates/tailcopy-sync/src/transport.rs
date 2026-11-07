//! The `Transport` trait (SPEC 7.1): where to listen, who is on the other end, and
//! which peers to try. The system Tailscale client implements it in
//! `tailcopy-tailnet`; `tailcopy-testkit` has an in-memory tailnet.

use std::net::{IpAddr, SocketAddr};

use async_trait::async_trait;

/// What the tailnet says about the node at the other end of a connection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PeerIdentity {
    /// Tailscale node name (MagicDNS name or hostname), for logs and the UI.
    pub node_name: String,
    /// Tailscale stable node id (`StableID`).
    pub stable_id: String,
    /// The node has no tags and is signed in to this device's Tailscale login.
    pub same_user: bool,
    /// The peer holds the configured app capability with `role: peer` toward this node.
    pub has_capability: bool,
    /// The peer carries the configured relay tag.
    pub relay_tagged: bool,
}

/// A peer worth probing (SPEC 8.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub node_name: String,
    pub stable_id: String,
    /// Address of its sync listener.
    pub addr: SocketAddr,
    pub relay_tagged: bool,
    /// A phone's paired peer: dial without whois and check the session proof
    /// against this key instead (SPEC 21.3.4).
    pub pinned: Option<Pinned>,
}

/// The device a phone expects at a candidate address, and its identity key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pinned {
    pub device_id: tailcopy_core::DeviceId,
    pub key: [u8; 32],
}

#[async_trait]
pub trait Transport: Send + Sync + 'static {
    /// Addresses to bind the sync listener on (Tailscale IPv4 and IPv6). An error
    /// means the tailnet isn't usable right now (Tailscale stopped or missing).
    async fn listen_addrs(&self) -> anyhow::Result<Vec<SocketAddr>>;

    /// Identity of the remote end of an accepted or dialled connection.
    async fn whois(&self, remote: SocketAddr) -> anyhow::Result<PeerIdentity>;

    /// Candidate peers to probe, from the tailnet's peer list.
    async fn candidates(&self) -> anyhow::Result<Vec<Candidate>>;

    /// Local address to dial `peer` from, so the peer's whois sees this node.
    /// `None` lets the OS choose.
    fn dial_from(&self, peer: IpAddr) -> Option<IpAddr> {
        let _ = peer;
        None
    }
}
