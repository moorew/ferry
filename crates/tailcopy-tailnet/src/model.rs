use std::{collections::HashMap, net::IpAddr};

use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::Error;

/// Parsed `tailscale status --json` (or LocalAPI `/status`).
#[derive(Debug, Clone, Default)]
pub struct Status {
    /// `Running`, `Stopped`, `NeedsLogin`, `Starting`, and so on.
    pub backend_state: String,
    /// Tailscale version of the local client.
    pub version: String,
    /// Health warnings reported by tailscaled.
    pub health: Vec<String>,
    pub self_node: Option<Node>,
    /// Peers sorted by DNS name.
    pub peers: Vec<Node>,
    pub users: HashMap<u64, User>,
}

/// A node from `Self` or the `Peer` map of the status.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Node {
    /// The stable node ID (status `ID`, whois `Node.StableID`).
    #[serde(rename = "ID")]
    pub stable_id: String,
    #[serde(rename = "HostName")]
    pub host_name: String,
    #[serde(rename = "DNSName")]
    pub dns_name: String,
    #[serde(rename = "TailscaleIPs", deserialize_with = "null_default")]
    pub ips: Vec<IpAddr>,
    #[serde(rename = "UserID")]
    pub user_id: u64,
    #[serde(rename = "Tags", deserialize_with = "null_default")]
    pub tags: Vec<String>,
    #[serde(rename = "Online")]
    pub online: bool,
    #[serde(rename = "Active")]
    pub active: bool,
    #[serde(rename = "CurAddr")]
    pub cur_addr: String,
    /// Home DERP region code, whether or not it is in use.
    #[serde(rename = "Relay")]
    pub relay: String,
    #[serde(rename = "PeerRelay")]
    pub peer_relay: String,
    #[serde(rename = "OS")]
    pub os: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct User {
    #[serde(rename = "LoginName")]
    pub login_name: String,
    #[serde(rename = "DisplayName")]
    pub display_name: String,
}

/// How traffic to a peer currently flows (DECISIONS D-009 rule 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Path {
    /// Direct, with the peer's current endpoint.
    Direct(String),
    /// Through a Tailscale peer relay.
    PeerRelay(String),
    /// Through the named DERP region.
    Derp(String),
    /// No recent traffic, so the path isn't known.
    Idle,
}

/// Parsed `tailscale whois --json` (or LocalAPI `/whois`).
#[derive(Debug, Clone, Default)]
pub struct Whois {
    /// The node's full MagicDNS name, as whois reports it.
    pub node_name: String,
    pub stable_id: String,
    pub tags: Vec<String>,
    /// `UserProfile.LoginName`; a placeholder for tagged nodes.
    pub login_name: String,
    /// Capabilities the peer holds toward this node.
    pub cap_map: HashMap<String, Vec<Value>>,
}

impl Status {
    pub fn from_json(json: &[u8]) -> Result<Status, Error> {
        let raw: RawStatus = serde_json::from_slice(json)
            .map_err(|e| Error::Other(format!("Tailscale status isn't valid JSON: {e}")))?;
        let mut peers: Vec<Node> = raw.peer.into_values().collect();
        peers.sort_by(|a, b| a.dns_name.cmp(&b.dns_name));
        Ok(Status {
            backend_state: raw.backend_state,
            version: raw.version,
            health: raw.health,
            self_node: raw.self_node,
            peers,
            users: raw.user,
        })
    }

    pub fn running(&self) -> bool {
        self.backend_state == "Running"
    }

    /// This device's login name. `None` when unknown or when this node is tagged,
    /// since tagged nodes only have a placeholder profile.
    pub fn self_login(&self) -> Option<&str> {
        let me = self.self_node.as_ref().filter(|n| n.tags.is_empty())?;
        let login = self.users.get(&me.user_id)?.login_name.as_str();
        (!login.is_empty()).then_some(login)
    }

    /// Online peers that are untagged and owned by this device's user, or that carry
    /// `relay_tag` (SPEC 8.4).
    pub fn candidates(&self, relay_tag: &str) -> Vec<&Node> {
        let self_user = self
            .self_node
            .as_ref()
            .filter(|n| n.tags.is_empty())
            .map(|n| n.user_id);
        self.peers
            .iter()
            .filter(|p| {
                p.online
                    && ((p.tags.is_empty() && Some(p.user_id) == self_user) || p.has_tag(relay_tag))
            })
            .collect()
    }

    pub fn peer_by_ip(&self, ip: IpAddr) -> Option<&Node> {
        self.peers.iter().find(|p| p.ips.contains(&ip))
    }
}

impl Node {
    pub fn path(&self) -> Path {
        if !self.cur_addr.is_empty() {
            Path::Direct(self.cur_addr.clone())
        } else if !self.peer_relay.is_empty() {
            Path::PeerRelay(self.peer_relay.clone())
        } else if self.active {
            Path::Derp(self.relay.clone())
        } else {
            Path::Idle
        }
    }

    /// The first label of the MagicDNS name, or the host name if there is none.
    pub fn short_name(&self) -> &str {
        match self.dns_name.split('.').next() {
            Some(label) if !label.is_empty() => label,
            _ => &self.host_name,
        }
    }

    /// The name the tailnet knows this machine by: the MagicDNS label, which the
    /// admin console can rename and which is unique. The OS hostname can differ (a
    /// node renamed in the console, or tailscaled in a container), so it is only
    /// used for its capitalisation when it matches the label (D-029).
    pub fn machine_name(&self) -> &str {
        let label = self.short_name();
        if self.host_name.eq_ignore_ascii_case(label) {
            &self.host_name
        } else {
            label
        }
    }

    pub fn ipv4(&self) -> Option<IpAddr> {
        self.ips.iter().copied().find(IpAddr::is_ipv4)
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

impl Whois {
    pub fn from_json(json: &[u8]) -> Result<Whois, Error> {
        let raw: RawWhois = serde_json::from_slice(json)
            .map_err(|e| Error::Other(format!("Tailscale whois isn't valid JSON: {e}")))?;
        Ok(Whois {
            node_name: raw.node.name,
            stable_id: raw.node.stable_id,
            tags: raw.node.tags,
            login_name: raw.user_profile.login_name,
            cap_map: raw.cap_map,
        })
    }

    /// Untagged and signed in as `self_login` (SPEC 8.5 rule 1).
    pub fn same_user(&self, self_login: &str) -> bool {
        self.tags.is_empty() && !self_login.is_empty() && self.login_name == self_login
    }

    /// `CapMap` has `capability` with an entry whose role is `peer` (SPEC 8.5 rule 2).
    pub fn has_capability(&self, capability: &str) -> bool {
        self.cap_map.get(capability).is_some_and(|entries| {
            entries
                .iter()
                .any(|e| e.get("role").and_then(Value::as_str) == Some("peer"))
        })
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RawStatus {
    #[serde(rename = "BackendState")]
    backend_state: String,
    #[serde(rename = "Version")]
    version: String,
    #[serde(rename = "Health", deserialize_with = "null_default")]
    health: Vec<String>,
    #[serde(rename = "Self")]
    self_node: Option<Node>,
    #[serde(rename = "Peer", deserialize_with = "null_default")]
    peer: HashMap<String, Node>,
    #[serde(rename = "User", deserialize_with = "null_default")]
    user: HashMap<u64, User>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RawWhois {
    #[serde(rename = "Node", deserialize_with = "null_default")]
    node: RawWhoisNode,
    #[serde(rename = "UserProfile", deserialize_with = "null_default")]
    user_profile: User,
    #[serde(rename = "CapMap", deserialize_with = "cap_map")]
    cap_map: HashMap<String, Vec<Value>>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RawWhoisNode {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "StableID")]
    stable_id: String,
    #[serde(rename = "Tags", deserialize_with = "null_default")]
    tags: Vec<String>,
}

/// Tailscale sends `null` for empty collections, which `#[serde(default)]` alone
/// rejects.
fn null_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

fn cap_map<'de, D>(d: D) -> Result<HashMap<String, Vec<Value>>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: Option<HashMap<String, Option<Vec<Value>>>> = Option::deserialize(d)?;
    Ok(map
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| (k, v.unwrap_or_default()))
        .collect())
}
