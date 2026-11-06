//! The one Tailscale client handle the app shares: sync's transport, the status line,
//! first run and the Devices view all read the same cached status (SPEC 8).

use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tailcopy_tailnet::{Backend, Error, Options, Status, Tailnet};

static TAILNET: OnceLock<Tailnet> = OnceLock::new();
static STALE: AtomicBool = AtomicBool::new(false);
const MAX_AGE: Duration = Duration::from_secs(15);

/// Sets up the shared handle from config. Later calls keep the first one; changing
/// the CLI path or socket takes a restart.
pub fn init(config: &tailcopy_core::Config) -> Tailnet {
    TAILNET
        .get_or_init(|| {
            let t = &config.tailscale;
            let backend = if cfg!(target_os = "linux") && !t.localapi_socket.is_empty() {
                Backend::LocalApi {
                    socket: PathBuf::from(&t.localapi_socket),
                }
            } else {
                Backend::Cli {
                    path: (!t.cli_path.is_empty()).then(|| PathBuf::from(&t.cli_path)),
                }
            };
            Tailnet::new(Options {
                backend,
                capability: config.sync.capability.clone(),
                relay_tag: config.sync.relay_tag.clone(),
            })
        })
        .clone()
}

fn tailnet() -> &'static Tailnet {
    TAILNET.get().expect("tailscale::init runs at startup")
}

/// The latest status, or why there isn't one.
pub async fn status() -> Result<std::sync::Arc<Status>, Error> {
    let max_age = if STALE.swap(false, Ordering::Relaxed) {
        Duration::ZERO
    } else {
        MAX_AGE
    };
    tailnet().status(max_age).await
}

/// Forgets the cached status so the next call asks Tailscale again.
pub fn forget() {
    STALE.store(true, Ordering::Relaxed);
}

/// Opens the Tailscale app, where it has one (Windows and macOS).
pub fn open_app() -> std::io::Result<()> {
    if cfg!(target_os = "macos") {
        std::process::Command::new("open")
            .args(["-a", "Tailscale"])
            .spawn()?;
    } else if cfg!(windows) {
        std::process::Command::new(r"C:\Program Files\Tailscale\tailscale-ipn.exe").spawn()?;
    } else {
        return Err(std::io::ErrorKind::Unsupported.into());
    }
    Ok(())
}

/// `running`, `stopped` or `missing`.
pub async fn state() -> &'static str {
    match status().await {
        Ok(_) => "running",
        Err(Error::Missing) => "missing",
        Err(_) => "stopped",
    }
}

/// This machine's name on the tailnet (its MagicDNS label), while Tailscale runs.
pub async fn hostname() -> Option<String> {
    status()
        .await
        .ok()?
        .self_node
        .as_ref()
        .map(|n| n.machine_name().to_owned())
        .filter(|s| !s.is_empty())
}
