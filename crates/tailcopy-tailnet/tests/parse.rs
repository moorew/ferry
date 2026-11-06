use std::net::IpAddr;

use tailcopy_tailnet::{Path, Status, Whois};

const RELAY_TAG: &str = "tag:tailcopy-relay";
const CAPABILITY: &str = "moorew.github.io/cap/tailcopy";

fn status(name: &str) -> Status {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    Status::from_json(&std::fs::read(path).unwrap()).unwrap()
}

fn whois(name: &str) -> Whois {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    Whois::from_json(&std::fs::read(path).unwrap()).unwrap()
}

fn ip(s: &str) -> IpAddr {
    s.parse().unwrap()
}

fn peer<'a>(status: &'a Status, short: &str) -> &'a tailcopy_tailnet::Node {
    status
        .peers
        .iter()
        .find(|p| p.short_name() == short)
        .unwrap_or_else(|| panic!("no peer {short}"))
}

#[test]
fn running_status() {
    let s = status("status-running.json");
    assert!(s.running());
    assert_eq!(s.version, "1.102.4-tc0ffee0a1-g0123456789");
    let me = s.self_node.as_ref().unwrap();
    assert_eq!(me.stable_id, "nSelfLaptop1CNTRL");
    assert_eq!(me.short_name(), "laptop");
    assert_eq!(me.ips, [ip("100.64.0.1"), ip("fd7a:115c:a1e0::1")]);
    assert_eq!(me.ipv4(), Some(ip("100.64.0.1")));
    assert!(me.tags.is_empty());
    assert_eq!(s.self_login(), Some("owner@example.com"));
    assert_eq!(s.peers.len(), 8);
    assert_eq!(s.users[&2002].login_name, "tagged-devices");
    assert_eq!(s.users[&1001].display_name, "Owner Example");
}

#[test]
fn peers_are_sorted_and_fields_read() {
    let s = status("status-running.json");
    let names: Vec<_> = s.peers.iter().map(|p| p.short_name()).collect();
    assert_eq!(
        names,
        [
            "builder",
            "desktop",
            "friends-iphone",
            "nas",
            "old-laptop",
            "phone",
            "studio",
            "tailcopy-relay"
        ]
    );
    let desktop = peer(&s, "desktop");
    assert_eq!(desktop.stable_id, "nDesktop2CNTRL");
    assert_eq!(desktop.host_name, "DESKTOP-ABC123");
    assert_eq!(desktop.os, "windows");
    assert_eq!(desktop.user_id, 1001);
    let relay = peer(&s, "tailcopy-relay");
    assert!(relay.has_tag(RELAY_TAG));
    assert_eq!(relay.relay, "fra");
}

#[test]
fn paths() {
    let s = status("status-running.json");
    assert_eq!(
        peer(&s, "desktop").path(),
        Path::Direct("192.0.2.2:41641".into())
    );
    assert_eq!(peer(&s, "phone").path(), Path::Derp("tor".into()));
    assert_eq!(peer(&s, "nas").path(), Path::Idle);
    assert_eq!(
        peer(&s, "studio").path(),
        Path::PeerRelay("198.51.100.7:40001".into())
    );
    assert_eq!(peer(&s, "tailcopy-relay").path(), Path::Idle);
}

#[test]
fn machine_name_follows_the_tailnet() {
    let mut node = tailcopy_tailnet::Node {
        host_name: "Venus".into(),
        dns_name: "venus.example.ts.net.".into(),
        ..Default::default()
    };
    assert_eq!(node.machine_name(), "Venus");

    // Renamed in the admin console, or tailscaled running in a container.
    node.host_name = "sempa".into();
    node.dns_name = "blackbox.example.ts.net.".into();
    assert_eq!(node.machine_name(), "blackbox");

    // Two machines with the same OS hostname get distinct MagicDNS names.
    node.host_name = "FRACTAL".into();
    node.dns_name = "fractal-wsl.example.ts.net.".into();
    assert_eq!(node.machine_name(), "fractal-wsl");

    node.dns_name = String::new();
    assert_eq!(node.machine_name(), "FRACTAL");
}

#[test]
fn candidates() {
    let s = status("status-running.json");
    let names: Vec<_> = s
        .candidates(RELAY_TAG)
        .iter()
        .map(|p| p.short_name())
        .collect();
    // Not old-laptop (offline), builder (other tag) or friends-iphone (other user).
    assert_eq!(
        names,
        ["desktop", "nas", "phone", "studio", "tailcopy-relay"]
    );

    let names: Vec<_> = s
        .candidates("tag:something-else")
        .iter()
        .map(|p| p.short_name())
        .collect();
    assert_eq!(names, ["desktop", "nas", "phone", "studio"]);
}

#[test]
fn tagged_self_has_no_login_and_no_same_user_candidates() {
    let mut s = status("status-running.json");
    let me = s.self_node.as_mut().unwrap();
    me.tags = vec![RELAY_TAG.into()];
    me.user_id = 2002;
    assert_eq!(s.self_login(), None);
    let names: Vec<_> = s
        .candidates(RELAY_TAG)
        .iter()
        .map(|p| p.short_name())
        .collect();
    assert_eq!(names, ["tailcopy-relay"]);
}

#[test]
fn peer_by_ip() {
    let s = status("status-running.json");
    assert_eq!(
        s.peer_by_ip(ip("100.64.0.4")).unwrap().stable_id,
        "nNas4CNTRL"
    );
    assert_eq!(
        s.peer_by_ip(ip("fd7a:115c:a1e0::5")).unwrap().stable_id,
        "nStudio5CNTRL"
    );
    assert!(s.peer_by_ip(ip("100.64.0.1")).is_none());
    assert!(s.peer_by_ip(ip("100.64.0.99")).is_none());
}

#[test]
fn explicit_nulls_are_empty() {
    let s = status("status-nulls.json");
    assert!(s.running());
    assert!(s.peers.is_empty());
    assert!(s.users.is_empty());
    assert!(s.health.is_empty());
    let me = s.self_node.as_ref().unwrap();
    assert!(me.ips.is_empty() && me.tags.is_empty());
    assert_eq!(me.ipv4(), None);
    assert_eq!(me.short_name(), "laptop");
    assert_eq!(s.self_login(), None);
    assert!(s.candidates(RELAY_TAG).is_empty());
}

#[test]
fn not_running_states() {
    let s = status("status-needslogin.json");
    assert_eq!(s.backend_state, "NeedsLogin");
    assert!(!s.running());
    assert_eq!(s.health.len(), 1);

    let s = status("status-stopped.json");
    assert_eq!(s.backend_state, "Stopped");
    assert!(!s.running());
}

#[test]
fn minimal_and_odd_json() {
    let s = Status::from_json(b"{}").unwrap();
    assert!(!s.running() && s.self_node.is_none());
    let s = Status::from_json(br#"{"BackendState":"Running","Self":null,"Extra":[1,2]}"#).unwrap();
    assert!(s.running() && s.self_node.is_none());
    assert!(Status::from_json(b"").is_err());
    assert!(Whois::from_json(br#"{"Node":null,"UserProfile":null,"CapMap":null}"#).is_ok());
}

#[test]
fn whois_same_user() {
    let w = whois("whois-same-user.json");
    assert_eq!(w.node_name, "desktop.tail0000.ts.net.");
    assert_eq!(w.stable_id, "nDesktop2CNTRL");
    assert_eq!(w.login_name, "owner@example.com");
    assert!(w.tags.is_empty() && w.cap_map.is_empty());
    assert!(w.same_user("owner@example.com"));
    assert!(!w.same_user("friend@example.com"));
    assert!(!w.same_user(""));
    assert!(!w.has_capability(CAPABILITY));
}

#[test]
fn whois_tagged_is_never_same_user() {
    let w = whois("whois-tagged.json");
    assert_eq!(w.stable_id, "nRelay7CNTRL");
    assert!(w.has_tag(RELAY_TAG));
    assert!(!w.same_user("tagged-devices"));

    let mut w = whois("whois-same-user.json");
    w.tags = vec!["tag:server".into()];
    assert!(!w.same_user("owner@example.com"));
}

#[test]
fn whois_capability() {
    let w = whois("whois-capability.json");
    assert!(w.has_capability(CAPABILITY));
    assert!(!w.has_capability("example.com/cap/viewer-only"));
    assert!(!w.has_capability("example.com/cap/empty"));
    assert!(!w.has_capability("example.com/cap/absent"));
    assert!(w.cap_map["example.com/cap/empty"].is_empty());
}

#[test]
fn status_id_matches_whois_stable_id() {
    let s = status("status-running.json");
    let w = whois("whois-same-user.json");
    assert_eq!(peer(&s, "desktop").stable_id, w.stable_id);
}
