//! The `tailcopy` command line and single-instance handling (SPEC 5.2).
//!
//! A second launch connects to the running instance's local socket
//! (`tailcopy-<username>`), forwards its command, prints any reply and exits.

use std::io;

use interprocess::local_socket::{
    GenericFilePath, GenericNamespaced, ListenerOptions, Name, ToFsName, ToNsName,
    tokio::{Listener, Stream, prelude::*},
};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};

use tailcopy_core::names::{DESKTOP_BINARY, IPC_SOCKET_PREFIX};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Command {
    /// No arguments: focus the manager.
    Open,
    Picker,
    PastePlain,
    Pause,
    Resume,
    Status,
}

impl Command {
    pub fn parse(args: &[String]) -> Result<Command, String> {
        match args.first().map(String::as_str) {
            None => Ok(Command::Open),
            Some("picker") => Ok(Command::Picker),
            Some("paste-plain") => Ok(Command::PastePlain),
            Some("pause") => Ok(Command::Pause),
            Some("resume") => Ok(Command::Resume),
            Some("status") => Ok(Command::Status),
            Some("-h" | "--help" | "help") => Err(usage()),
            Some("-V" | "--version") => {
                Err(format!("{DESKTOP_BINARY} {}", env!("CARGO_PKG_VERSION")))
            }
            // Autostart and the OS may pass their own flags; treat those as a plain launch.
            Some(a) if a.starts_with("--") || a.starts_with("-psn") => Ok(Command::Open),
            Some(other) => Err(format!("unknown command: {other}\n\n{}", usage())),
        }
    }
}

fn usage() -> String {
    format!(
        "usage: {DESKTOP_BINARY} [COMMAND]

Without a command, starts TailCopy or brings up its window.

commands:
  picker        open the picker
  paste-plain   paste the current clipboard as plain text
  pause         stop saving what you copy
  resume        start saving again
  status        print status as JSON"
    )
}

/// A command from another process, with a way to answer it.
pub struct Request {
    pub command: Command,
    pub reply: oneshot::Sender<String>,
}

fn socket_name() -> io::Result<Name<'static>> {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "user".into());
    let name = format!("{IPC_SOCKET_PREFIX}-{user}");
    if GenericNamespaced::is_supported() {
        name.to_ns_name::<GenericNamespaced>()
    } else {
        std::env::temp_dir()
            .join(format!("{name}.sock"))
            .to_fs_name::<GenericFilePath>()
    }
}

/// Sends `command` to a running instance. `Ok(None)` means none is running.
pub async fn forward(command: Command) -> io::Result<Option<String>> {
    let Ok(conn) = Stream::connect(socket_name()?).await else {
        return Ok(None);
    };
    let mut line = serde_json::to_string(&command).map_err(io::Error::other)?;
    line.push('\n');
    (&conn).write_all(line.as_bytes()).await?;
    let mut reply = String::new();
    BufReader::new(&conn).read_line(&mut reply).await?;
    Ok(Some(reply.trim_end().to_string()))
}

/// Listens for commands from later launches; each arrives on the returned channel.
/// Must be called inside the async runtime.
pub async fn listen() -> io::Result<mpsc::Receiver<Request>> {
    let listener = ListenerOptions::new()
        .name(socket_name()?)
        .try_overwrite(true)
        .create_tokio()?;
    let (tx, rx) = mpsc::channel(16);
    tauri::async_runtime::spawn(serve(listener, tx));
    Ok(rx)
}

async fn serve(listener: Listener, tx: mpsc::Sender<Request>) {
    loop {
        let conn = match listener.accept().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("local socket accept failed: {e}");
                continue;
            }
        };
        let tx = tx.clone();
        tauri::async_runtime::spawn(async move {
            let mut send = &conn;
            let mut line = String::new();
            if BufReader::new(&conn).read_line(&mut line).await.is_err() {
                return;
            }
            let Ok(command) = serde_json::from_str::<Command>(line.trim()) else {
                let _ = send.write_all(b"error: unknown command\n").await;
                return;
            };
            let (reply, answer) = oneshot::channel();
            if tx.send(Request { command, reply }).await.is_err() {
                return;
            }
            let text = answer.await.unwrap_or_default();
            let _ = send.write_all(format!("{text}\n").as_bytes()).await;
        });
    }
}

/// Windows builds use the GUI subsystem; attach to the launching console so
/// subcommands can print.
pub fn attach_console() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_commands() {
        assert_eq!(Command::parse(&args(&[])), Ok(Command::Open));
        assert_eq!(Command::parse(&args(&["picker"])), Ok(Command::Picker));
        assert_eq!(
            Command::parse(&args(&["paste-plain"])),
            Ok(Command::PastePlain)
        );
        assert_eq!(Command::parse(&args(&["--minimized"])), Ok(Command::Open));
        assert!(Command::parse(&args(&["nope"])).is_err());
    }

    #[test]
    fn commands_round_trip() {
        for c in [
            Command::Open,
            Command::Picker,
            Command::Pause,
            Command::Status,
        ] {
            let s = serde_json::to_string(&c).unwrap();
            assert_eq!(serde_json::from_str::<Command>(&s).unwrap(), c);
        }
    }
}
