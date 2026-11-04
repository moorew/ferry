//! SQLite store: ops log, materialized clips, blobs, devices (SPEC 6).
//!
//! [`Store`] owns the only write connection and is synchronous; the app wraps it in
//! an actor. Reads for the UI go through [`ReadView`], either borrowed from the store
//! or from a separate read-only [`Reader`] connection.

mod actions;
mod blobs;
mod materialize;
mod read;
pub mod schema;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use rusqlite::{Connection, OpenFlags, OptionalExtension, params};

pub use actions::{CaptureOutcome, RetentionPolicy};
pub use blobs::{DB_FILE, DataDir};
pub use read::{ClipDetail, ClipRow, DisplayRow, FormatInfo, PageCursor, ReadView, Reader};

use crate::capture::SizeLimits;
use crate::error::{Error, Result};
use crate::hlc::{Clock, Hlc, SystemClock};
use crate::merge::Clear;
use crate::model::{DeviceId, DeviceInfo, Hash32, Op, OpKind, Role, VersionVector};

/// Settings the store needs from config and the platform.
#[derive(Clone)]
pub struct StoreOptions {
    pub clock: Arc<dyn Clock>,
    /// Name for this device's row on first run.
    pub display_name: String,
    pub role: Role,
    pub limits: SizeLimits,
    /// Include `source_app` in `ClipAdded` ops (`privacy.sync_source_app`).
    pub sync_source_app: bool,
}

impl Default for StoreOptions {
    fn default() -> Self {
        Self {
            clock: Arc::new(SystemClock),
            display_name: String::new(),
            role: Role::Device,
            limits: SizeLimits::default(),
            sync_source_app: false,
        }
    }
}

impl std::fmt::Debug for StoreOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreOptions")
            .field("display_name", &self.display_name)
            .field("role", &self.role)
            .field("limits", &self.limits)
            .field("sync_source_app", &self.sync_source_app)
            .finish_non_exhaustive()
    }
}

/// Outcome of [`Store::apply_remote_ops`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ApplyReport {
    /// Ops stored for the first time.
    pub applied: usize,
    /// Ops already held (or held as a stub).
    pub duplicates: usize,
    /// Ops rejected as malformed (seq 0).
    pub invalid: usize,
    /// Ops whose hlc was more than 60 s ahead of local time.
    pub skewed: usize,
    /// This device took a new id because a peer held ops from its id that it does
    /// not have (a restored or cloned data directory).
    pub regenerated_device_id: bool,
}

/// The single-writer store.
pub struct Store {
    conn: Connection,
    dir: DataDir,
    hlc: Hlc,
    device_id: DeviceId,
    clears: Vec<Clear>,
    limits: SizeLimits,
    sync_source_app: bool,
    dirty_groups: BTreeSet<Hash32>,
    pending_blob_removals: Vec<Hash32>,
}

fn configure(conn: &Connection) -> Result<()> {
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(())
}

impl Store {
    /// Opens (creating if needed) the store in `data_dir` and applies migrations.
    pub fn open(data_dir: impl Into<PathBuf>, opts: StoreOptions) -> Result<Store> {
        let dir = DataDir::new(data_dir);
        dir.create()?;
        let mut conn = Connection::open(dir.db_path())?;
        let mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            tracing::warn!(%mode, "SQLite did not switch to WAL mode");
        }
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        configure(&conn)?;
        schema::migrate(&mut conn)?;

        let last_hlc = meta_i64(&conn, "last_hlc")?.unwrap_or(0) as u64;
        let hlc = Hlc::new(opts.clock.clone(), last_hlc);
        let mut store = Store {
            conn,
            dir,
            hlc,
            device_id: DeviceId(String::new()),
            clears: Vec::new(),
            limits: opts.limits,
            sync_source_app: opts.sync_source_app,
            dirty_groups: BTreeSet::new(),
            pending_blob_removals: Vec::new(),
        };
        store.clears = store.load_clears()?;
        match meta_text(&store.conn, "device_id")? {
            Some(id) => store.device_id = DeviceId(id),
            None => store.write(|s| s.first_run(&opts.display_name, opts.role))?,
        }
        Ok(store)
    }

    fn first_run(&mut self, display_name: &str, role: Role) -> Result<()> {
        let now = self.hlc.now_ms();
        self.device_id = DeviceId::new_v7(now);
        set_meta(&self.conn, "device_id", &self.device_id.0)?;
        set_meta(&self.conn, "created_at", &(now as i64))?;
        self.conn.execute(
            "INSERT INTO devices (device_id, display_name, role, is_self, color_index)
             VALUES (?1, ?2, ?3, 1, (SELECT coalesce(max(color_index) + 1, 0) FROM devices))",
            params![self.device_id.0, display_name, role.as_str()],
        )?;
        tracing::info!(device_id = %self.device_id, "created device id");
        Ok(())
    }

    /// Opens a separate read-only connection on the same database.
    pub fn open_reader(&self) -> Result<Reader> {
        Reader::open(self.dir.root())
    }

    /// Queries on the write connection.
    pub fn view(&self) -> ReadView<'_> {
        ReadView::new(&self.conn, &self.dir)
    }

    pub fn data_dir(&self) -> &DataDir {
        &self.dir
    }

    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    /// The last HLC issued or observed (also persisted in `meta`).
    pub fn last_hlc(&self) -> u64 {
        self.hlc.last()
    }

    pub fn now_ms(&self) -> u64 {
        self.hlc.now_ms()
    }

    pub fn limits(&self) -> SizeLimits {
        self.limits
    }

    pub fn set_limits(&mut self, limits: SizeLimits) {
        self.limits = limits;
    }

    pub fn set_sync_source_app(&mut self, on: bool) {
        self.sync_source_app = on;
    }

    /// Display groups changed since the last call, for refreshing a [`crate::SearchIndex`].
    pub fn take_dirty_groups(&mut self) -> Vec<Hash32> {
        std::mem::take(&mut self.dirty_groups).into_iter().collect()
    }

    /// Runs `f` in an immediate transaction, persisting the HLC. Blob files of
    /// garbage-collected blobs are removed only after commit.
    fn write<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let saved_device = self.device_id.clone();
        let saved_clears = self.clears.len();
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = f(self).and_then(|v| {
            set_meta(&self.conn, "last_hlc", &(self.hlc.last() as i64))?;
            self.conn.execute_batch("COMMIT")?;
            Ok(v)
        });
        match result {
            Ok(v) => {
                for hash in std::mem::take(&mut self.pending_blob_removals) {
                    self.dir.remove_blob(&hash);
                }
                Ok(v)
            }
            Err(e) => {
                if !self.conn.is_autocommit() {
                    let _ = self.conn.execute_batch("ROLLBACK");
                }
                self.device_id = saved_device;
                self.clears.truncate(saved_clears);
                self.pending_blob_removals.clear();
                Err(e)
            }
        }
    }

    fn load_clears(&self) -> Result<Vec<Clear>> {
        let mut stmt = self
            .conn
            .prepare("SELECT payload FROM ops WHERE kind = ?1")?;
        let rows = stmt.query_map([OpKind::HistoryCleared as i64], |r| r.get::<_, Vec<u8>>(0))?;
        let mut clears = Vec::new();
        for payload in rows {
            clears.extend(Clear::from_op(&Op::decode(&payload?)?));
        }
        Ok(clears)
    }

    // ---- sync-facing API -------------------------------------------------------

    /// Highest contiguous seq held per origin.
    pub fn version_vector(&self) -> Result<VersionVector> {
        self.view().version_vector()
    }

    /// Ops from `origin` with `seq > after_seq`, in seq order, at most `limit`.
    pub fn ops_after(&self, origin: &DeviceId, after_seq: u64, limit: usize) -> Result<Vec<Op>> {
        self.view().ops_after(origin, after_seq, limit)
    }

    /// Applies ops received from a peer. Idempotent and order-independent.
    pub fn apply_remote_ops(&mut self, ops: &[Op]) -> Result<ApplyReport> {
        self.write(|s| {
            let mut report = ApplyReport::default();
            let local_max = s.max_seq(&s.device_id.clone())?;
            if ops
                .iter()
                .any(|op| op.origin == s.device_id && op.seq > local_max)
            {
                s.regenerate_device_id("a peer sent ops from this device id that it never wrote")?;
                report.regenerated_device_id = true;
            }
            let mut clips = BTreeSet::new();
            let mut new_clears = Vec::new();
            for op in ops {
                if op.seq == 0 {
                    report.invalid += 1;
                    continue;
                }
                if !s.insert_op(op)? {
                    report.duplicates += 1;
                    continue;
                }
                report.applied += 1;
                if s.hlc.receive(op.hlc).clamped {
                    report.skewed += 1;
                    tracing::warn!(origin = %op.origin, seq = op.seq, "remote clock more than 60 s ahead; clamped");
                }
                match op.body.clip_id() {
                    Some(id) => {
                        clips.insert(id.clone());
                    }
                    None => new_clears.extend(Clear::from_op(op)),
                }
            }
            for clear in &new_clears {
                s.clears.push(clear.clone());
            }
            for clear in &new_clears {
                clips.extend(s.clear_targets(clear)?);
                s.clear_local_only(clear)?;
            }
            for id in &clips {
                s.recompute(id)?;
            }
            Ok(report)
        })
    }

    /// Regenerates the device id if a peer's version vector shows a seq for this
    /// device above the local maximum (SPEC 6.1). Returns whether it did.
    pub fn observe_peer_vector(&mut self, vector: &VersionVector) -> Result<bool> {
        let local = self.max_seq(&self.device_id.clone())?;
        if vector
            .get(&self.device_id)
            .is_some_and(|&peer| peer > local)
        {
            self.write(|s| s.regenerate_device_id("a peer holds newer ops from this device id"))?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Records the Tailscale node ID this device runs under. If a different one is
    /// stored, the data directory was cloned or restored: take a new device id
    /// (SPEC 6.1). Returns whether the id was regenerated.
    pub fn bind_tailscale_node(&mut self, node_id: &str) -> Result<bool> {
        self.write(|s| {
            let stored = meta_text(&s.conn, "ts_node_id")?;
            let regenerate = stored.as_deref().is_some_and(|old| old != node_id);
            if regenerate {
                s.regenerate_device_id("Tailscale node ID changed")?;
            }
            set_meta(&s.conn, "ts_node_id", &node_id)?;
            s.conn.execute(
                "UPDATE devices SET ts_node_id = ?1 WHERE device_id = ?2",
                params![node_id, s.device_id.0],
            )?;
            Ok(regenerate)
        })
    }

    fn regenerate_device_id(&mut self, reason: &str) -> Result<()> {
        let old = self.device_id.clone();
        let new = DeviceId::new_v7(self.hlc.now_ms());
        set_meta(&self.conn, "device_id", &new.0)?;
        self.conn.execute(
            "INSERT INTO devices (device_id, display_name, role, ts_node_id, ts_hostname, last_ts_ip,
               last_seen_ms, is_self, color_index, device_type, os)
             SELECT ?1, display_name, role, ts_node_id, ts_hostname, last_ts_ip, last_seen_ms, 1,
               color_index, device_type, os
             FROM devices WHERE device_id = ?2",
            params![new.0, old.0],
        )?;
        self.conn.execute(
            "UPDATE devices SET is_self = 0 WHERE device_id = ?1",
            params![old.0],
        )?;
        tracing::warn!(old = %old, new = %new, reason, "regenerated device id");
        self.device_id = new;
        Ok(())
    }

    /// Adds or updates a device row (peers learned from the tailnet or hello).
    pub fn upsert_device(&mut self, device: &DeviceInfo) -> Result<()> {
        self.write(|s| {
            s.conn.execute(
                "INSERT INTO devices (device_id, display_name, role, ts_node_id, ts_hostname, last_ts_ip,
                   last_seen_ms, is_self, os, color_index)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, (SELECT coalesce(max(color_index) + 1, 0) FROM devices))
                 ON CONFLICT(device_id) DO UPDATE SET display_name = excluded.display_name,
                   role = excluded.role, ts_node_id = excluded.ts_node_id,
                   ts_hostname = excluded.ts_hostname, last_ts_ip = excluded.last_ts_ip,
                   last_seen_ms = excluded.last_seen_ms, os = coalesce(excluded.os, os)",
                params![
                    device.device_id.0,
                    device.display_name,
                    device.role.as_str(),
                    device.ts_node_id,
                    device.ts_hostname,
                    device.last_ts_ip,
                    device.last_seen_ms.map(|v| v as i64),
                    device.device_id == s.device_id,
                    device.os,
                ],
            )?;
            Ok(())
        })
    }

    /// Renames this device.
    pub fn set_self_display_name(&mut self, name: &str) -> Result<()> {
        self.write(|s| {
            s.conn.execute(
                "UPDATE devices SET display_name = ?1 WHERE device_id = ?2",
                params![name, s.device_id.0],
            )?;
            Ok(())
        })
    }

    /// Blobs referenced by live clips that are not on disk yet, for fetching.
    pub fn missing_blobs(&self) -> Result<Vec<(Hash32, u64)>> {
        self.view().missing_blobs()
    }

    /// Stores a fetched blob. Returns `false` if no clip references it. Fills in
    /// plain text for search and makes thumbnails for images.
    pub fn put_blob(&mut self, hash: &Hash32, bytes: &[u8]) -> Result<bool> {
        if crate::model::blake3_hash(bytes) != *hash {
            return Err(Error::BlobHashMismatch);
        }
        let known: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM blobs WHERE hash = ?1)",
            [hash.as_slice()],
            |r| r.get(0),
        )?;
        if !known {
            return Ok(false);
        }
        self.dir.write_blob(hash, bytes)?;
        self.write(|s| s.blob_arrived(hash, bytes))?;
        Ok(true)
    }

    /// Checks that the materialized `clips` table equals a full recomputation from
    /// the ops held, and that the FTS index is consistent. For tests and diagnostics.
    pub fn check_consistency(&self) -> Result<(), String> {
        materialize::check_consistency(self)
    }

    fn max_seq(&self, origin: &DeviceId) -> Result<u64> {
        Ok(self
            .conn
            .query_row(
                "SELECT max_seq FROM version_vector WHERE origin = ?1",
                [&origin.0],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0) as u64)
    }
}

pub(crate) fn meta_i64(conn: &Connection, key: &str) -> Result<Option<i64>> {
    Ok(conn
        .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| {
            r.get::<_, i64>(0)
        })
        .optional()?)
}

pub(crate) fn meta_text(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| {
            r.get::<_, String>(0)
        })
        .optional()?)
}

pub(crate) fn set_meta(conn: &Connection, key: &str, value: &dyn rusqlite::ToSql) -> Result<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub(crate) fn open_read_only(dir: &DataDir) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        dir.db_path(),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    configure(&conn)?;
    conn.pragma_update(None, "query_only", "ON")?;
    Ok(conn)
}

#[cfg(test)]
mod tests;
