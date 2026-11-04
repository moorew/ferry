//! Headless relay (SPEC 12): core, sync and tailnet with no UI and no clipboard. It
//! accepts authorized devices, never dials, and keeps a longer history so devices
//! that were asleep catch up.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tailcopy_core::names::{
    DEFAULT_CAPABILITY, DEFAULT_RELAY_TAG, DEFAULT_SYNC_PORT, RELAY_BINARY,
};
use tailcopy_core::{RetentionPolicy, Role, Store, StoreOptions};
use tailcopy_sync::{Engine, EngineConfig, spawn_store_thread};
use tailcopy_tailnet::{Backend, Options, Tailnet, TailnetTransport};

const RETENTION_EVERY: Duration = Duration::from_secs(3600);

struct Args {
    data_dir: PathBuf,
    port: u16,
    retention: RetentionPolicy,
    socket: PathBuf,
    strict: bool,
}

fn usage() -> ! {
    eprintln!(
        "usage: {RELAY_BINARY} --data-dir DIR [--port {DEFAULT_SYNC_PORT}] [--max-items 10000] \
         [--max-age-days 90] [--tailscale-socket {}] [--allow-capability]",
        tailcopy_tailnet::DEFAULT_SOCKET
    );
    std::process::exit(2)
}

fn parse() -> Args {
    let mut args = Args {
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

#[tokio::main]
async fn main() {
    let args = parse();
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
    let tailnet = Tailnet::new(Options {
        backend: Backend::LocalApi {
            socket: args.socket.clone(),
        },
        capability: DEFAULT_CAPABILITY.into(),
        relay_tag: DEFAULT_RELAY_TAG.into(),
    });
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
