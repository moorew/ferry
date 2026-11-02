//! Is Tailscale running? Enough for first run and the status line until M2 adds the
//! full tailnet adapter (SPEC 8).

use std::sync::Mutex;
use std::time::{Duration, Instant};

static CACHE: Mutex<Option<(Instant, &'static str)>> = Mutex::new(None);
const TTL: Duration = Duration::from_secs(15);

/// `running`, `stopped` or `missing`.
pub async fn state() -> &'static str {
    if let Some((at, s)) = *CACHE.lock().unwrap()
        && at.elapsed() < TTL
    {
        return s;
    }
    let s = query().await;
    *CACHE.lock().unwrap() = Some((Instant::now(), s));
    s
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

async fn query() -> &'static str {
    let mut cmd = tokio::process::Command::new(cli());
    cmd.args(["status", "--json"]).kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    match tokio::time::timeout(Duration::from_secs(3), cmd.output()).await {
        Ok(Ok(out)) => {
            let running = serde_json::from_slice::<serde_json::Value>(&out.stdout)
                .ok()
                .and_then(|v| v.get("BackendState")?.as_str().map(|s| s == "Running"))
                .unwrap_or(false);
            if running { "running" } else { "stopped" }
        }
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => "missing",
        _ => "stopped",
    }
}
