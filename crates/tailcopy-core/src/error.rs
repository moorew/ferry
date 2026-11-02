//! Error type shared by the crate.

use std::path::PathBuf;

/// Errors from the store, capture, config and codec layers.
///
/// Messages never include clip content.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    #[error("i/o error at {path}: {source}")]
    IoAt {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("op encode error: {0}")]
    Encode(#[from] rmp_serde::encode::Error),

    #[error("op decode error: {0}")]
    Decode(#[from] rmp_serde::decode::Error),

    #[error("image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("config parse error: {0}")]
    ConfigParse(#[from] toml_edit::TomlError),

    #[error("invalid config value for `{key}`: {reason}")]
    ConfigValue { key: String, reason: String },

    #[error("database schema version {found} is newer than this build supports ({supported})")]
    SchemaTooNew { found: i64, supported: i64 },

    #[error("blob hash mismatch")]
    BlobHashMismatch,

    #[error("unknown clip {0}")]
    UnknownClip(String),

    #[error("invalid input: {0}")]
    Invalid(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
