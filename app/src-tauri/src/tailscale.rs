//! Is Tailscale running? Enough for first run and the status line until M2 adds the
//! full tailnet adapter (SPEC 8).

use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone)]
struct Info {
    state: &'static str,
    hostname: Option<String>,
}

static CACHE: Mutex<Option<(Instant, Info)>> = Mutex::new(None);
const TTL: Duration = Duration::from_secs(15);

async fn info() -> Info {
    if let Some((at, info)) = CACHE.lock().unwrap().as_ref()
        && at.elapsed() < TTL
    {
        return info.clone();
    }
    let info = query().await;
    *CACHE.lock().unwrap() = Some((Instant::now(), info.clone()));
    info
}

/// `running`, `stopped` or `missing`.
pub async fn state() -> &'static str {
    info().await.state
}

/// This machine's name on the tailnet (`Self.HostName`), while Tailscale runs.
pub async fn hostname() -> Option<String> {
    info().await.hostname
}

fn cli() -> &'static str {
    if cfg!(target_os = "macos") {
        "/Applications/Tailscale.app/Contents/MacOS/Tailscale"
    } else if cfg!(windows) {
        r"C:\Program Files\Tailscale\tailscale.exe"
    } else {
        "tailscale"
    }
}

async fn query() -> Info {
    let mut cmd = tokio::process::Command::new(cli());
    cmd.args(["status", "--json"]).kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let stopped = Info {
        state: "stopped",
        hostname: None,
    };
    match tokio::time::timeout(Duration::from_secs(3), cmd.output()).await {
        Ok(Ok(out)) => {
            let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
                return stopped;
            };
            if json.get("BackendState").and_then(|v| v.as_str()) != Some("Running") {
                return stopped;
            }
            Info {
                state: "running",
                hostname: json
                    .pointer("/Self/HostName")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
            }
        }
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => Info {
            state: "missing",
            hostname: None,
        },
        _ => stopped,
    }
}
