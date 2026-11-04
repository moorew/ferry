use std::{net::SocketAddr, path::PathBuf, time::Duration};

use crate::{Backend, Error, Status, Whois};

const TIMEOUT: Duration = Duration::from_secs(3);

impl Backend {
    pub(crate) async fn status(&self) -> Result<Status, Error> {
        match self {
            Backend::Cli { path } => {
                let out = run_cli(path.as_deref(), &["status", "--json"]).await?;
                status_from_cli(out.status.success(), &out.stdout, &out.stderr)
            }
            Backend::LocalApi { socket } => {
                Status::from_json(&localapi::get(socket, "/localapi/v0/status").await?)
            }
        }
    }

    pub(crate) async fn whois(&self, addr: SocketAddr) -> Result<Whois, Error> {
        let addr = addr.to_string();
        match self {
            Backend::Cli { path } => {
                let out = run_cli(path.as_deref(), &["whois", "--json", &addr]).await?;
                if !out.status.success() {
                    return Err(cli_failure(&out.stderr));
                }
                Whois::from_json(&out.stdout)
            }
            Backend::LocalApi { socket } => {
                let query = addr.replace('[', "%5B").replace(']', "%5D");
                let path = format!("/localapi/v0/whois?addr={query}");
                Whois::from_json(&localapi::get(socket, &path).await?)
            }
        }
    }
}

/// The CLI exits non-zero when Tailscale is stopped but may still print the status.
pub(crate) fn status_from_cli(
    success: bool,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<Status, Error> {
    match Status::from_json(stdout) {
        Ok(status) => Ok(status),
        Err(_) if !success => Err(cli_failure(stderr)),
        Err(e) => Err(e),
    }
}

pub(crate) fn cli_failure(stderr: &[u8]) -> Error {
    let text = String::from_utf8_lossy(stderr);
    let line = text.lines().map(str::trim).find(|l| !l.is_empty());
    let msg: String = line
        .unwrap_or("tailscale failed")
        .chars()
        .take(200)
        .collect();
    if msg.contains("failed to connect") || msg.contains("doesn't appear to be running") {
        Error::Stopped(msg)
    } else {
        Error::Other(msg)
    }
}

fn cli_path(over: Option<&std::path::Path>) -> PathBuf {
    if let Some(p) = over {
        return p.to_path_buf();
    }
    if cfg!(target_os = "macos") {
        return PathBuf::from("/Applications/Tailscale.app/Contents/MacOS/Tailscale");
    }
    if cfg!(windows) {
        let installed = PathBuf::from(r"C:\Program Files\Tailscale\tailscale.exe");
        if installed.exists() {
            return installed;
        }
    }
    PathBuf::from("tailscale")
}

async fn run_cli(
    over: Option<&std::path::Path>,
    args: &[&str],
) -> Result<std::process::Output, Error> {
    let path = cli_path(over);
    let mut cmd = tokio::process::Command::new(&path);
    cmd.args(args).kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    match tokio::time::timeout(TIMEOUT, cmd.output()).await {
        Err(_) => Err(Error::Other(format!(
            "{} didn't answer within {} s",
            path.display(),
            TIMEOUT.as_secs()
        ))),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => Err(Error::Missing),
        Ok(Err(e)) => Err(Error::Other(format!(
            "couldn't run {}: {e}",
            path.display()
        ))),
        Ok(Ok(out)) => Ok(out),
    }
}

#[cfg(unix)]
mod localapi {
    use std::{error::Error as _, io, path::Path};

    use super::TIMEOUT;
    use crate::Error;

    pub(super) async fn get(socket: &Path, path: &str) -> Result<Vec<u8>, Error> {
        let client = reqwest::Client::builder()
            .unix_socket(socket)
            .timeout(TIMEOUT)
            .build()
            .map_err(|e| Error::Other(format!("LocalAPI client: {e}")))?;
        let resp = client
            .get(format!("http://local-tailscaled.sock{path}"))
            .send()
            .await
            .map_err(|e| send_error(socket, &e))?;
        let code = resp.status();
        let body = resp
            .bytes()
            .await
            .map_err(|e| Error::Other(format!("LocalAPI {path}: {e}")))?;
        if !code.is_success() {
            let text = String::from_utf8_lossy(&body);
            let text: String = text.trim().chars().take(200).collect();
            return Err(Error::Other(format!("LocalAPI returned {code}: {text}")));
        }
        Ok(body.to_vec())
    }

    fn send_error(socket: &Path, e: &reqwest::Error) -> Error {
        if e.is_timeout() {
            return Error::Other(format!(
                "tailscaled didn't answer within {} s",
                TIMEOUT.as_secs()
            ));
        }
        let mut source = e.source();
        while let Some(s) = source {
            if let Some(io) = s.downcast_ref::<io::Error>() {
                if io.kind() == io::ErrorKind::PermissionDenied {
                    return Error::Other(format!("no permission to open {}", socket.display()));
                }
                return Error::Stopped(format!("can't reach tailscaled at {}", socket.display()));
            }
            source = s.source();
        }
        Error::Other(format!("LocalAPI request failed: {e}"))
    }
}

#[cfg(not(unix))]
mod localapi {
    use std::path::Path;

    use crate::Error;

    pub(super) async fn get(_socket: &Path, _path: &str) -> Result<Vec<u8>, Error> {
        Err(Error::Other(
            "the LocalAPI backend needs a Unix socket; use the CLI backend here".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopped_cli_still_prints_status() {
        let json = br#"{"BackendState":"Stopped","Self":null,"Peer":null,"User":null}"#;
        let status = status_from_cli(false, json, b"").unwrap();
        assert_eq!(status.backend_state, "Stopped");
        assert!(!status.running());
    }

    #[test]
    fn unreachable_daemon_is_stopped() {
        let stderr = b"failed to connect to local tailscaled; it doesn't appear to be running (sudo systemctl start tailscaled ?)\n";
        assert!(matches!(
            status_from_cli(false, b"", stderr),
            Err(Error::Stopped(_))
        ));
        let stderr = b"failed to connect to local Tailscale service; is Tailscale running?\n";
        assert!(matches!(cli_failure(stderr), Error::Stopped(_)));
    }

    #[test]
    fn other_failures_keep_the_first_stderr_line() {
        assert_eq!(
            cli_failure(b"\npeer not found\nmore\n"),
            Error::Other("peer not found".into())
        );
        assert!(matches!(
            status_from_cli(true, b"not json", b""),
            Err(Error::Other(_))
        ));
    }

    #[test]
    fn override_path_wins() {
        let p = std::path::Path::new("/opt/tailscale/bin/tailscale");
        assert_eq!(cli_path(Some(p)), p);
    }

    #[tokio::test]
    async fn missing_binary_is_reported() {
        let backend = Backend::Cli {
            path: Some("/nonexistent/tailscale".into()),
        };
        assert_eq!(backend.status().await.unwrap_err(), Error::Missing);
    }
}
