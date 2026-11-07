//! Headless relay (SPEC 12): core, sync and tailnet with no UI and no clipboard. It
//! accepts authorized devices, never dials, and keeps a longer history so devices
//! that were asleep catch up.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use std::net::SocketAddr;

use tailcopy_core::names::{
    DEFAULT_CAPABILITY, DEFAULT_RELAY_TAG, DEFAULT_SYNC_PORT, IDENTITY_KEY_FILE, RELAY_BINARY,
};
use tailcopy_core::{Reader, RetentionPolicy, Role, Store, StoreOptions};
use tailcopy_sync::{Engine, EngineConfig, Identity, PairLink, spawn_store_thread};
use tailcopy_tailnet::{Backend, Options, Tailnet, TailnetTransport};

const RETENTION_EVERY: Duration = Duration::from_secs(3600);

struct Args {
    /// `pair`: print a phone pairing code and exit (SPEC 21.3.3).
    pair: bool,
    data_dir: PathBuf,
    port: u16,
    retention: RetentionPolicy,
    socket: PathBuf,
    strict: bool,
}

fn usage() -> ! {
    eprintln!(
        "usage: {RELAY_BINARY} [pair] --data-dir DIR [--port {DEFAULT_SYNC_PORT}] [--max-items 10000] \
         [--max-age-days 90] [--tailscale-socket {}] [--allow-capability]\n\n\
         `pair` prints a QR code a phone scans to pair with this relay.",
        tailcopy_tailnet::DEFAULT_SOCKET
    );
    std::process::exit(2)
}

fn parse() -> Args {
    let mut args = Args {
        pair: false,
        data_dir: PathBuf::new(),
        port: DEFAULT_SYNC_PORT,
        retention: RetentionPolicy::RELAY,
        socket: tailcopy_tailnet::DEFAULT_SOCKET.into(),
        strict: true,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().unwrap_or_else(|| usage());
        match flag.as_str() {
            "pair" => args.pair = true,
            "--data-dir" => args.data_dir = value().into(),
            "--port" => args.port = value().parse().unwrap_or_else(|_| usage()),
            "--max-items" => args.retention.max_items = value().parse().unwrap_or_else(|_| usage()),
            "--max-age-days" => {
                args.retention.max_age_days = value().parse().unwrap_or_else(|_| usage())
            }
            "--tailscale-socket" => args.socket = value().into(),
            // A tagged relay admits devices by the app capability (SPEC 8.5 rule 2).
            "--allow-capability" => args.strict = false,
            "--version" => {
                println!("{RELAY_BINARY} {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0)
            }
            _ => usage(),
        }
    }
    if args.data_dir.as_os_str().is_empty() {
        usage();
    }
    args
}

fn tailnet(args: &Args) -> Tailnet {
    Tailnet::new(Options {
        backend: Backend::LocalApi {
            socket: args.socket.clone(),
        },
        capability: DEFAULT_CAPABILITY.into(),
        relay_tag: DEFAULT_RELAY_TAG.into(),
    })
}

fn identity(args: &Args) -> Identity {
    let path = args.data_dir.join(IDENTITY_KEY_FILE);
    Identity::load_or_create(&path).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    })
}

/// Prints the pairing link and its QR code for a phone to scan.
async fn pair(args: &Args) {
    let me = Reader::open(&args.data_dir)
        .and_then(|r| r.view().devices())
        .ok()
        .and_then(|d| d.into_iter().find(|d| d.is_self))
        .unwrap_or_else(|| {
            eprintln!(
                "no relay data in {}; start the relay once first",
                args.data_dir.display()
            );
            std::process::exit(1)
        });
    let status = tailnet(args).refresh().await.unwrap_or_else(|e| {
        eprintln!("couldn't read Tailscale's status: {e}");
        std::process::exit(1)
    });
    let mut ips = status
        .self_node
        .as_ref()
        .map(|n| n.ips.clone())
        .unwrap_or_default();
    ips.sort_by_key(|ip| !ip.is_ipv4());
    if ips.is_empty() {
        eprintln!("Tailscale reports no addresses for this machine");
        std::process::exit(1);
    }
    let link = PairLink {
        device_id: me.device_id,
        display_name: me.display_name,
        key: identity(args).public_key(),
        addrs: ips
            .into_iter()
            .map(|ip| SocketAddr::new(ip, args.port))
            .collect(),
    }
    .to_url();
    let code = qrcode::QrCode::new(link.as_bytes()).expect("a pairing link fits in a QR code");
    // Light modules as blocks, for the usual dark terminal.
    let art = code
        .render::<qrcode::render::unicode::Dense1x2>()
        .dark_color(qrcode::render::unicode::Dense1x2::Light)
        .light_color(qrcode::render::unicode::Dense1x2::Dark)
        .build();
    println!("{art}\n\nScan this with TailCopy on your phone, or open:\n{link}");
}

#[tokio::main]
async fn main() {
    let args = parse();
    if args.pair {
        return pair(&args).await;
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "{RELAY_BINARY} starting"
    );
    let store = match Store::open(
        &args.data_dir,
        StoreOptions {
            display_name: "relay".into(),
            role: Role::Relay,
            ..StoreOptions::default()
        },
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("couldn't open {}: {e}", args.data_dir.display());
            std::process::exit(1);
        }
    };
    let store = spawn_store_thread(store);
    let identity = Arc::new(identity(&args));
    let tailnet = tailnet(&args);
    if let Ok(status) = tailnet.refresh().await
        && let Some(me) = &status.self_node
    {
        let (id, name) = (me.stable_id.clone(), me.short_name().to_owned());
        let _ = store
            .call(move |s| {
                let _ = s.bind_tailscale_node(&id);
                s.set_self_display_name(&name)
            })
            .await;
    }
    let engine = Engine::start(
        store.clone(),
        Arc::new(TailnetTransport::new(tailnet, args.port)),
        EngineConfig {
            role: Role::Relay,
            port: args.port,
            strict_same_login: args.strict,
            app_version: env!("CARGO_PKG_VERSION").into(),
            identity: Some(identity),
            ..EngineConfig::default()
        },
    );
    let policy = args.retention;
    let retention = tokio::spawn(async move {
        loop {
            match store.call(move |s| s.enforce_retention(&policy)).await {
                Ok(Ok(n)) if n > 0 => tracing::info!(trimmed = n, "retention"),
                Ok(Err(e)) => tracing::warn!("retention failed: {e}"),
                _ => {}
            }
            tokio::time::sleep(RETENTION_EVERY).await;
        }
    });
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("stopping");
    retention.abort();
    engine.stop();
    tokio::time::sleep(Duration::from_millis(200)).await;
}
