//! M0 tailnet spike (SPEC 8, 17 item 4).
//!
//! Reads Tailscale status and whois through the CLI or the LocalAPI socket, binds a
//! listener to the Tailscale addresses only, and exchanges GET /v1/hello with whois
//! verification on both ends.

use std::{
    collections::{BTreeMap, HashMap},
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use axum::{
    Json, Router,
    extract::{ConnectInfo, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEFAULT_PORT: u16 = 7441;
const CAPABILITY: &str = "moorew.github.io/cap/ferry";
const RELAY_TAG: &str = "tag:ferry-relay";
const DEFAULT_SOCKET: &str = "/var/run/tailscale/tailscaled.sock";

// ---- Tailscale JSON (lenient: every field defaults, unknown fields ignored) ----

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Status {
    #[serde(rename = "BackendState")]
    backend_state: String,
    #[serde(rename = "Self")]
    self_node: Option<PeerStatus>,
    #[serde(rename = "Peer")]
    #[serde(deserialize_with = "null_default")]
    peer: HashMap<String, PeerStatus>,
    #[serde(rename = "User")]
    #[serde(deserialize_with = "null_default")]
    user: HashMap<String, UserProfile>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PeerStatus {
    #[serde(rename = "ID")]
    id: String,
    #[serde(rename = "HostName")]
    host_name: String,
    #[serde(rename = "DNSName")]
    dns_name: String,
    #[serde(rename = "TailscaleIPs")]
    #[serde(deserialize_with = "null_default")]
    tailscale_ips: Vec<IpAddr>,
    #[serde(rename = "UserID")]
    user_id: u64,
    #[serde(rename = "Tags")]
    #[serde(deserialize_with = "null_default")]
    tags: Vec<String>,
    #[serde(rename = "Online")]
    online: bool,
    #[serde(rename = "Active")]
    active: bool,
    #[serde(rename = "CurAddr")]
    cur_addr: String,
    #[serde(rename = "Relay")]
    relay: String,
    #[serde(rename = "PeerRelay")]
    peer_relay: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UserProfile {
    #[serde(rename = "LoginName")]
    login_name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Whois {
    #[serde(rename = "Node")]
    node: WhoisNode,
    #[serde(rename = "UserProfile")]
    user_profile: UserProfile,
    #[serde(rename = "CapMap")]
    #[serde(deserialize_with = "null_default")]
    cap_map: HashMap<String, Vec<Value>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct WhoisNode {
    #[serde(rename = "StableID")]
    stable_id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Tags")]
    #[serde(deserialize_with = "null_default")]
    tags: Vec<String>,
}

/// Tailscale sends `null` for empty maps and lists (whois `CapMap`, for one), which
/// `#[serde(default)]` alone does not accept.
fn null_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

// ---- Backends ----

enum Backend {
    Cli(PathBuf),
    LocalApi(PathBuf),
}

impl Backend {
    fn describe(&self) -> String {
        match self {
            Backend::Cli(p) => format!("CLI {}", p.display()),
            Backend::LocalApi(p) => format!("LocalAPI {}", p.display()),
        }
    }

    async fn raw_status(&self) -> Result<Value> {
        match self {
            Backend::Cli(p) => run_cli(p, &["status", "--json"]).await,
            Backend::LocalApi(p) => localapi_get(p, "/localapi/v0/status").await,
        }
    }

    async fn raw_whois(&self, addr: &str) -> Result<Value> {
        match self {
            Backend::Cli(p) => run_cli(p, &["whois", "--json", addr]).await,
            Backend::LocalApi(p) => {
                let q = addr.replace('[', "%5B").replace(']', "%5D");
                localapi_get(p, &format!("/localapi/v0/whois?addr={q}")).await
            }
        }
    }

    async fn status(&self) -> Result<Status> {
        Ok(serde_json::from_value(self.raw_status().await?)?)
    }

    async fn whois(&self, addr: &str) -> Result<Whois> {
        Ok(serde_json::from_value(self.raw_whois(addr).await?)?)
    }
}

fn default_cli_path() -> PathBuf {
    if cfg!(target_os = "macos") {
        PathBuf::from("/Applications/Tailscale.app/Contents/MacOS/Tailscale")
    } else {
        PathBuf::from("tailscale")
    }
}

async fn run_cli(path: &Path, args: &[&str]) -> Result<Value> {
    let mut cmd = tokio::process::Command::new(path);
    cmd.args(args);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd
        .output()
        .await
        .with_context(|| format!("running {}", path.display()))?;
    if !out.status.success() {
        bail!(
            "{} {:?} failed: {}",
            path.display(),
            args,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(serde_json::from_slice(&out.stdout)?)
}

#[cfg(unix)]
async fn localapi_get(socket: &Path, path: &str) -> Result<Value> {
    let client = reqwest::Client::builder()
        .unix_socket(socket)
        .timeout(Duration::from_secs(5))
        .build()?;
    let resp = client
        .get(format!("http://local-tailscaled.sock{path}"))
        .send()
        .await
        .with_context(|| format!("connecting to {}", socket.display()))?;
    let code = resp.status();
    let body = resp.text().await?;
    if !code.is_success() {
        bail!("LocalAPI {path} returned {code}: {}", body.trim());
    }
    Ok(serde_json::from_str(&body)?)
}

#[cfg(not(unix))]
async fn localapi_get(_socket: &Path, _path: &str) -> Result<Value> {
    bail!("the LocalAPI socket backend is Linux only in this spike")
}

// ---- Authorization (SPEC 8.4, 8.5) ----

#[derive(Debug, Serialize)]
enum Decision {
    SameUser,
    AppCapability,
    Relay,
    Rejected,
}

fn self_login(status: &Status) -> String {
    let uid = status.self_node.as_ref().map(|s| s.user_id).unwrap_or(0);
    status
        .user
        .get(&uid.to_string())
        .map(|u| u.login_name.clone())
        .unwrap_or_default()
}

fn same_user(who: &Whois, login: &str) -> bool {
    who.node.tags.is_empty() && !login.is_empty() && who.user_profile.login_name == login
}

fn authorize_inbound(who: &Whois, login: &str) -> Decision {
    if same_user(who, login) {
        return Decision::SameUser;
    }
    let has_cap = who.cap_map.get(CAPABILITY).is_some_and(|entries| {
        entries
            .iter()
            .any(|e| e.get("role").and_then(Value::as_str) == Some("peer"))
    });
    if has_cap {
        Decision::AppCapability
    } else {
        Decision::Rejected
    }
}

fn authorize_outbound(who: &Whois, login: &str, hello_role: &str) -> Decision {
    if same_user(who, login) {
        Decision::SameUser
    } else if who.node.tags.iter().any(|t| t == RELAY_TAG) && hello_role == "relay" {
        Decision::Relay
    } else {
        Decision::Rejected
    }
}

fn is_candidate(peer: &PeerStatus, self_user: u64) -> bool {
    peer.online
        && ((peer.tags.is_empty() && peer.user_id == self_user)
            || peer.tags.iter().any(|t| t == RELAY_TAG))
}

fn path_label(peer: &PeerStatus) -> String {
    if !peer.cur_addr.is_empty() {
        format!("direct {}", peer.cur_addr)
    } else if !peer.peer_relay.is_empty() {
        format!("peer relay {}", peer.peer_relay)
    } else if peer.active {
        format!("DERP {}", peer.relay)
    } else {
        format!("idle (home DERP {})", peer.relay)
    }
}

// ---- Hello ----

#[derive(Debug, Serialize, Deserialize)]
struct Hello {
    proto_major: u16,
    proto_minor: u16,
    device_id: String,
    display_name: String,
    role: String,
    app_version: String,
}

struct ServerState {
    backend: Backend,
    login: String,
    hello: Hello,
}

async fn hello_handler(
    State(state): State<Arc<ServerState>>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
) -> Response {
    let started = Instant::now();
    let who = match state.backend.whois(&remote.to_string()).await {
        Ok(w) => w,
        Err(e) => {
            eprintln!("inbound {remote}: whois failed: {e:#}");
            return StatusCode::FORBIDDEN.into_response();
        }
    };
    let decision = authorize_inbound(&who, &state.login);
    println!(
        "inbound {remote}: node {} ({}) -> {decision:?}, whois {} ms",
        who.node.name,
        who.node.stable_id,
        started.elapsed().as_millis()
    );
    match decision {
        Decision::Rejected => StatusCode::FORBIDDEN.into_response(),
        _ => Json(&state.hello).into_response(),
    }
}

// ---- Commands ----

async fn cmd_status(backend: &Backend) -> Result<()> {
    let t = Instant::now();
    let status = backend.status().await?;
    println!(
        "{} status in {} ms",
        backend.describe(),
        t.elapsed().as_millis()
    );
    println!("BackendState: {}", status.backend_state);
    let me = status.self_node.as_ref().context("no Self in status")?;
    println!(
        "Self: {} id={} ips={:?} user={} login={} tags={:?}",
        me.host_name,
        me.id,
        me.tailscale_ips,
        me.user_id,
        self_login(&status),
        me.tags
    );
    let mut peers: Vec<_> = status.peer.values().collect();
    peers.sort_by(|a, b| a.host_name.cmp(&b.host_name));
    println!("Candidates (SPEC 8.4):");
    for p in peers.iter().filter(|p| is_candidate(p, me.user_id)) {
        println!("  {:<24} {:<40} {}", p.host_name, p.dns_name, path_label(p));
    }
    let others = peers
        .iter()
        .filter(|p| !is_candidate(p, me.user_id))
        .count();
    println!("({others} other peers are offline, another user's, or tagged without the relay tag)");
    Ok(())
}

async fn cmd_whois(backend: &Backend, addr: &str) -> Result<()> {
    let status = backend.status().await?;
    let t = Instant::now();
    let who = backend.whois(addr).await?;
    println!(
        "{} whois {addr} in {} ms",
        backend.describe(),
        t.elapsed().as_millis()
    );
    println!(
        "Node: {} stable_id={} tags={:?}",
        who.node.name, who.node.stable_id, who.node.tags
    );
    println!("UserProfile.LoginName: {}", who.user_profile.login_name);
    println!("CapMap keys: {:?}", who.cap_map.keys().collect::<Vec<_>>());
    println!(
        "Inbound decision: {:?}",
        authorize_inbound(&who, &self_login(&status))
    );
    Ok(())
}

async fn cmd_serve(backend: Backend, port: u16) -> Result<()> {
    let status = backend.status().await?;
    if status.backend_state != "Running" {
        bail!(
            "Tailscale is not running (BackendState={})",
            status.backend_state
        );
    }
    let me = status.self_node.as_ref().context("no Self in status")?;
    let ips = me.tailscale_ips.clone();
    let state = Arc::new(ServerState {
        login: self_login(&status),
        hello: Hello {
            proto_major: 1,
            proto_minor: 0,
            device_id: format!("spike-{}", me.id),
            display_name: me.host_name.clone(),
            role: "device".into(),
            app_version: env!("CARGO_PKG_VERSION").into(),
        },
        backend,
    });
    let app = Router::new()
        .route("/v1/hello", get(hello_handler))
        .with_state(state.clone());
    let mut tasks = Vec::new();
    for ip in ips {
        let addr = SocketAddr::new(ip, port);
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                println!("listening on {addr}");
                let svc = app
                    .clone()
                    .into_make_service_with_connect_info::<SocketAddr>();
                tasks.push(tokio::spawn(
                    async move { axum::serve(listener, svc).await },
                ));
            }
            Err(e) => eprintln!("bind {addr} failed: {e}"),
        }
    }
    if tasks.is_empty() {
        bail!("could not bind any Tailscale address");
    }
    println!(
        "serving as {} via {}; Ctrl+C to stop",
        state.hello.display_name,
        state.backend.describe()
    );
    for t in tasks {
        t.await??;
    }
    Ok(())
}

async fn cmd_hello(backend: &Backend, target: &str, port: u16) -> Result<()> {
    let status = backend.status().await?;
    let ip: IpAddr = match target.parse() {
        Ok(ip) => ip,
        Err(_) => {
            let want = target.trim_end_matches('.').to_lowercase();
            let peer = status
                .peer
                .values()
                .chain(status.self_node.as_ref())
                .find(|p| {
                    p.host_name.to_lowercase() == want
                        || p.dns_name.trim_end_matches('.').to_lowercase() == want
                        || p.dns_name.to_lowercase().starts_with(&format!("{want}."))
                })
                .with_context(|| format!("no peer named {target} in status"))?;
            *peer
                .tailscale_ips
                .first()
                .context("peer has no Tailscale IPs")?
        }
    };
    let addr = SocketAddr::new(ip, port);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()?;
    let t = Instant::now();
    let resp = client
        .get(format!("http://{addr}/v1/hello"))
        .send()
        .await
        .with_context(|| format!("GET http://{addr}/v1/hello"))?;
    let rtt = t.elapsed();
    if !resp.status().is_success() {
        bail!("{addr} answered {} (the peer rejected us)", resp.status());
    }
    let hello: Hello = resp.json().await?;
    println!("hello from {addr} in {} ms: {hello:?}", rtt.as_millis());
    let t = Instant::now();
    let who = backend.whois(&addr.to_string()).await?;
    let decision = authorize_outbound(&who, &self_login(&status), &hello.role);
    println!(
        "whois {addr}: node {} ({}) -> {decision:?}, whois {} ms",
        who.node.name,
        who.node.stable_id,
        t.elapsed().as_millis()
    );
    if matches!(decision, Decision::Rejected) {
        bail!("outbound authorization failed");
    }
    Ok(())
}

/// Writes status and whois JSON with identifying values replaced, for test fixtures.
async fn cmd_fixture(backend: &Backend, peer_addr: &str, out_dir: &str) -> Result<()> {
    let mut r = Redactor::default();
    let status = r.redact(backend.raw_status().await?);
    let whois = r.redact(backend.raw_whois(peer_addr).await?);
    std::fs::create_dir_all(out_dir)?;
    let host = whoami_host();
    for (name, v) in [("status", status), ("whois", whois)] {
        let path = format!("{out_dir}/{host}-{name}.json");
        std::fs::write(&path, serde_json::to_string_pretty(&v)?)?;
        println!("wrote {path}");
    }
    println!("Please skim both files before sending them; redaction is best effort.");
    Ok(())
}

fn whoami_host() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| std::env::consts::OS.to_string())
        .to_lowercase()
}

#[derive(Default)]
struct Redactor {
    seen: BTreeMap<String, String>,
}

impl Redactor {
    const KEYS: &[&str] = &[
        "PublicKey",
        "Key",
        "DiscoKey",
        "MachineKey",
        "HostName",
        "DNSName",
        "Name",
        "ComputedName",
        "ComputedNameWithHost",
        "LoginName",
        "DisplayName",
        "ProfilePicURL",
        "MagicDNSSuffix",
        "CertDomains",
        "Addrs",
        "CurAddr",
        "Endpoints",
        "PeerAPIURL",
        "Hostinfo",
        "CurrentTailnet",
        "AuthURL",
        "tailnet-display-name",
        "domains",
    ];

    fn replace(&mut self, s: &str, kind: &str) -> String {
        if s.is_empty() {
            return String::new();
        }
        let n = self.seen.len() + 1;
        self.seen
            .entry(s.to_string())
            .or_insert_with(|| format!("{kind}-{n}"))
            .clone()
    }

    fn redact(&mut self, v: Value) -> Value {
        self.walk(v, false)
    }

    fn walk(&mut self, v: Value, sensitive: bool) -> Value {
        match v {
            Value::Object(map) => Value::Object(
                map.into_iter()
                    .map(|(k, v)| {
                        let s = sensitive || Self::KEYS.contains(&k.as_str());
                        let v = self.walk(v, s);
                        let k = if k.starts_with("nodekey:") {
                            self.replace(&k, "nodekey")
                        } else {
                            k
                        };
                        (k, v)
                    })
                    .collect(),
            ),
            Value::Array(a) => {
                Value::Array(a.into_iter().map(|v| self.walk(v, sensitive)).collect())
            }
            Value::String(s) if sensitive => Value::String(self.replace(&s, "redacted")),
            Value::String(s) if looks_like_addr(&s) => Value::String(self.replace(&s, "addr")),
            Value::String(s) if s.contains(".ts.net") => Value::String(self.replace(&s, "host")),
            other => other,
        }
    }
}

fn looks_like_addr(s: &str) -> bool {
    let host = s.split_once('/').map_or(s, |(ip, _)| ip);
    host.parse::<IpAddr>().is_ok() || s.parse::<SocketAddr>().is_ok()
}

fn usage() -> ! {
    eprintln!(
        "usage: tailnet-spike [--localapi [SOCKET]] [--cli PATH] COMMAND
commands:
  status                    parse status, list candidate peers (SPEC 8.4)
  whois ADDR                whois an IP or IP:port and show the inbound decision
  serve [--port N]          bind /v1/hello on the Tailscale IPs only (default 7441)
  hello PEER [--port N]     GET /v1/hello from a peer (name or IP) and whois-verify it
  fixture PEER_IP [DIR]     write redacted status and whois JSON for test fixtures"
    );
    std::process::exit(2)
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut backend = Backend::Cli(default_cli_path());
    let mut port = DEFAULT_PORT;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--localapi" => {
                args.remove(i);
                let sock = if i < args.len() && args[i].starts_with('/') {
                    args.remove(i)
                } else {
                    DEFAULT_SOCKET.to_string()
                };
                backend = Backend::LocalApi(sock.into());
            }
            "--cli" if i + 1 < args.len() => {
                args.remove(i);
                backend = Backend::Cli(args.remove(i).into());
            }
            "--port" if i + 1 < args.len() => {
                args.remove(i);
                port = args.remove(i).parse().context("--port")?;
            }
            _ => i += 1,
        }
    }
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["status"] => cmd_status(&backend).await,
        ["whois", addr] => cmd_whois(&backend, addr).await,
        ["serve"] => cmd_serve(backend, port).await,
        ["hello", peer] => cmd_hello(&backend, peer, port).await,
        ["fixture", peer] => cmd_fixture(&backend, peer, "fixtures").await,
        ["fixture", peer, dir] => cmd_fixture(&backend, peer, dir).await,
        _ => usage(),
    }
}
