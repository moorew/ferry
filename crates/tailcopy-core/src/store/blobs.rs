//! Content-addressed blob and thumbnail files (SPEC 6.2).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{Error, Result};
use crate::model::{Hash32, hash_hex};

/// File name of the SQLite database inside the data directory.
pub const DB_FILE: &str = "tailcopy.sqlite3";

/// Paths inside the data directory.
#[derive(Clone, Debug)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn db_path(&self) -> PathBuf {
        self.root.join(DB_FILE)
    }

    /// `<data_dir>/blobs/<first 2 hex chars>/<full hex hash>`.
    pub fn blob_path(&self, hash: &Hash32) -> PathBuf {
        let hex = hash_hex(hash);
        self.root.join("blobs").join(&hex[..2]).join(hex)
    }

    /// `<data_dir>/thumbs/<hash>.png`, keyed by the PNG blob's hash.
    pub fn thumb_path(&self, hash: &Hash32) -> PathBuf {
        self.root
            .join("thumbs")
            .join(format!("{}.png", hash_hex(hash)))
    }

    pub(crate) fn create(&self) -> Result<()> {
        for dir in [
            self.root.clone(),
            self.root.join("blobs"),
            self.root.join("thumbs"),
        ] {
            fs::create_dir_all(&dir).map_err(|source| Error::IoAt { path: dir, source })?;
        }
        owner_only(&self.root)
    }

    pub(crate) fn write_blob(&self, hash: &Hash32, bytes: &[u8]) -> Result<()> {
        write_atomic(&self.blob_path(hash), bytes)
    }

    pub(crate) fn write_thumb(&self, hash: &Hash32, png: &[u8]) -> Result<()> {
        write_atomic(&self.thumb_path(hash), png)
    }

    pub(crate) fn read_blob(&self, hash: &Hash32) -> Result<Option<Vec<u8>>> {
        let path = self.blob_path(hash);
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(Error::IoAt { path, source }),
        }
    }

    /// Removes a blob and its thumbnail; missing files are fine.
    pub(crate) fn remove_blob(&self, hash: &Hash32) {
        for path in [self.blob_path(hash), self.thumb_path(hash)] {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => {
                    tracing::warn!(path = %path.display(), %err, "could not remove blob file")
                }
            }
        }
    }
}

/// Writes to a temp file next to `path`, syncs it and renames it into place. An
/// existing file is left alone: names are content hashes.
/// Clipboard history is private: on Unix only the owner may enter the directory,
/// whatever the umask. Elsewhere the per-user parent directory already restricts it.
pub(crate) fn owner_only(dir: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|source| {
            Error::IoAt {
                path: dir.to_owned(),
                source,
            }
        })?;
    }
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    if path.exists() {
        return Ok(());
    }
    let dir = path.parent().expect("blob paths have a parent");
    fs::create_dir_all(dir).map_err(|source| Error::IoAt {
        path: dir.to_owned(),
        source,
    })?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("blob");
    let tmp = dir.join(format!(
        ".{name}.{}.{}.tmp",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if let Err(source) = result {
        let _ = fs::remove_file(&tmp);
        return Err(Error::IoAt {
            path: path.to_owned(),
            source,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::blake3_hash;

    #[test]
    fn blob_layout_and_atomic_write() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = DataDir::new(tmp.path());
        dir.create().unwrap();
        let hash = blake3_hash(b"abc");
        dir.write_blob(&hash, b"abc").unwrap();
        let hex = hash_hex(&hash);
        let path = tmp.path().join("blobs").join(&hex[..2]).join(&hex);
        assert_eq!(fs::read(&path).unwrap(), b"abc");
        // No temp files left behind.
        let entries: Vec<_> = fs::read_dir(path.parent().unwrap()).unwrap().collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(dir.read_blob(&hash).unwrap().unwrap(), b"abc");
        dir.remove_blob(&hash);
        assert!(dir.read_blob(&hash).unwrap().is_none());
    }
}
