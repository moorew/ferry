//! Rolling log files in `<data_dir>/logs` (SPEC 16). Never log clip content, previews
//! or labels: only ids, kinds, sizes, MIME types and app ids.

use std::path::Path;

use tracing_appender::{non_blocking::WorkerGuard, rolling};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Keeps the background writer alive; drop it at exit to flush.
pub struct Guard(#[allow(dead_code)] WorkerGuard);

pub fn init(log_dir: &Path) -> Option<Guard> {
    let appender = rolling::Builder::new()
        .rotation(rolling::Rotation::DAILY)
        .filename_prefix("tailcopy")
        .filename_suffix("log")
        .max_log_files(7)
        .build(log_dir)
        .ok()?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let filter = EnvFilter::try_from_env("TAILCOPY_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info,tao=warn,wry=warn,zbus=warn"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(writer).with_ansi(false))
        .with(fmt::layer().with_writer(std::io::stderr))
        .try_init()
        .ok()?;
    Some(Guard(guard))
}
