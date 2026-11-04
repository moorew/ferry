//! Keeping the `clips` table equal to the merge of the ops held (SPEC 6.4, 6.5).
//!
//! Every change to a clip goes through [`Store::recompute`], which rebuilds that
//! clip's row from its ops and every `HistoryCleared` with [`merge::clip_state`].

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{OptionalExtension, params};

use super::Store;
use crate::capture::{decode_png, thumbnail_png, truncate_bytes};
use crate::error::Result;
use crate::merge::{self, Clear, ClipState};
use crate::model::{ClipId, DeviceId, FormatRef, Hash32, Op, OpKind, hash_from_slice, mime};

/// The parts of an existing row that recomputation needs.
struct Existing {
    local_only: bool,
    deleted_hlc: Option<u64>,
    group_key: Hash32,
}

impl Store {
    /// Stores an op if `(origin, seq)` is new and advances the version vector to the
    /// highest contiguous seq. Returns whether it was new.
    pub(super) fn insert_op(&mut self, op: &Op) -> Result<bool> {
        let payload = op.encode()?;
        let inserted = self.conn.execute(
            "INSERT OR IGNORE INTO ops (origin, seq, hlc, kind, clip_id, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                op.origin.0,
                op.seq as i64,
                op.hlc as i64,
                op.body.kind() as i64,
                op.body.clip_id().map(|c| c.0.as_str()),
                payload
            ],
        )?;
        if inserted == 0 {
            return Ok(false);
        }
        let mut max = self.max_seq(&op.origin)?;
        if op.seq == max + 1 {
            let mut stmt = self.conn.prepare_cached(
                "SELECT EXISTS(SELECT 1 FROM ops WHERE origin = ?1 AND seq = ?2)",
            )?;
            max += 1;
            while stmt.query_row(params![op.origin.0, (max + 1) as i64], |r| {
                r.get::<_, bool>(0)
            })? {
                max += 1;
            }
            drop(stmt);
            self.conn.execute(
                "INSERT INTO version_vector (origin, max_seq) VALUES (?1, ?2)
                 ON CONFLICT(origin) DO UPDATE SET max_seq = max(max_seq, excluded.max_seq)",
                params![op.origin.0, max as i64],
            )?;
        }
        Ok(true)
    }

    /// Ids of every clip a clear can affect: those whose `ClipAdded` is at or before
    /// `before_hlc` (the op's hlc is the clip's `created_hlc`).
    pub(super) fn clear_targets(&self, clear: &Clear) -> Result<Vec<ClipId>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT DISTINCT clip_id FROM ops WHERE kind = ?1 AND hlc <= ?2")?;
        let rows = stmt.query_map(
            params![OpKind::ClipAdded as i64, clear.before_hlc as i64],
            |r| r.get::<_, String>(0),
        )?;
        Ok(rows.map(|r| r.map(ClipId)).collect::<Result<_, _>>()?)
    }

    /// Clears also remove local-only clips (which have no ops), using their current
    /// pin state.
    pub(super) fn clear_local_only(&mut self, clear: &Clear) -> Result<()> {
        let ids: Vec<String> = {
            let mut stmt = self.conn.prepare_cached(
                "SELECT id FROM clips WHERE local_only = 1 AND created_hlc <= ?1
                 AND (?2 OR pinned = 0)",
            )?;
            stmt.query_map(
                params![clear.before_hlc as i64, clear.include_pinned],
                |r| r.get(0),
            )?
            .collect::<Result<_, _>>()?
        };
        for id in ids {
            self.remove_row(&ClipId(id))?;
        }
        Ok(())
    }

    fn clip_ops(&self, clip_id: &ClipId) -> Result<Vec<Op>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT payload FROM ops WHERE clip_id = ?1")?;
        let rows = stmt.query_map([&clip_id.0], |r| r.get::<_, Vec<u8>>(0))?;
        let mut ops = Vec::new();
        for payload in rows {
            ops.push(Op::decode(&payload?)?);
        }
        Ok(ops)
    }

    fn existing(&self, clip_id: &ClipId) -> Result<Option<Existing>> {
        self.conn
            .prepare_cached("SELECT local_only, deleted_hlc, group_key FROM clips WHERE id = ?1")?
            .query_row([&clip_id.0], |r| {
                Ok((
                    r.get::<_, bool>(0)?,
                    r.get::<_, Option<i64>>(1)?,
                    r.get::<_, Vec<u8>>(2)?,
                ))
            })
            .optional()?
            .map(|(local_only, deleted, key)| -> Result<Existing> {
                Ok(Existing {
                    local_only,
                    deleted_hlc: deleted.map(|d| d as u64),
                    group_key: hash_from_slice(&key)?,
                })
            })
            .transpose()
    }

    /// Rewrites a held `ClipAdded` as a stub (SPEC 6.5).
    pub(super) fn stub_op(&mut self, op: &Op) -> Result<()> {
        if op.is_stub() {
            return Ok(());
        }
        self.conn.execute(
            "UPDATE ops SET payload = ?1 WHERE origin = ?2 AND seq = ?3",
            params![op.to_stub().encode()?, op.origin.0, op.seq as i64],
        )?;
        Ok(())
    }

    /// Rebuilds one clip's row from its ops.
    pub(super) fn recompute(&mut self, clip_id: &ClipId) -> Result<()> {
        let existing = self.existing(clip_id)?;
        if existing.as_ref().is_some_and(|e| e.local_only) {
            return Ok(());
        }
        let ops = self.clip_ops(clip_id)?;
        let Some(added) = merge::pick_added(&ops) else {
            // Ops for a clip whose ClipAdded hasn't arrived: held until it does.
            if existing.is_some() {
                self.remove_row(clip_id)?;
            }
            return Ok(());
        };
        let state = merge::clip_state(added, &ops, &self.clears);
        if let Some(e) = &existing {
            self.dirty_groups.insert(e.group_key);
        }
        self.dirty_groups.insert(state.group_key);

        if let Some(deleted_hlc) = state.deleted_hlc {
            // Deleted text must not linger in the log.
            let added = added.clone();
            self.stub_op(&added)?;
            match existing {
                Some(e) if e.deleted_hlc.is_none() => self.tombstone_row(&state, deleted_hlc)?,
                Some(e) if e.deleted_hlc != Some(deleted_hlc) => {
                    self.conn.execute(
                        "UPDATE clips SET deleted_hlc = ?1 WHERE id = ?2",
                        params![deleted_hlc as i64, clip_id.0],
                    )?;
                }
                Some(_) => {}
                None => self.insert_row(&state, Some(deleted_hlc), None)?,
            }
            return Ok(());
        }
        if state.stub {
            if existing.is_some() {
                self.remove_row(clip_id)?;
            }
            return Ok(());
        }
        match existing {
            Some(e) if e.deleted_hlc.is_none() => self.update_live_row(&state),
            Some(_) => {
                self.remove_row(clip_id)?;
                self.insert_live(&state)
            }
            None => self.insert_live(&state),
        }
    }

    fn insert_live(&mut self, state: &ClipState) -> Result<()> {
        for f in &state.formats {
            self.conn.execute(
                "INSERT OR IGNORE INTO blobs (hash, size, present) VALUES (?1, ?2, 0)",
                params![f.blob_hash.as_slice(), f.size as i64],
            )?;
        }
        let plain_text = self.plain_text_for(&state.formats)?;
        self.insert_row(state, None, plain_text.as_deref())?;
        for f in &state.formats {
            self.conn.execute(
                "INSERT OR IGNORE INTO formats (clip_id, mime, blob_hash, size) VALUES (?1, ?2, ?3, ?4)",
                params![state.clip_id.0, f.mime, f.blob_hash.as_slice(), f.size as i64],
            )?;
        }
        Ok(())
    }

    /// Plain text for FTS from the `text/plain` blob, if present.
    fn plain_text_for(&self, formats: &[FormatRef]) -> Result<Option<String>> {
        let Some(f) = formats.iter().find(|f| f.mime == mime::TEXT) else {
            return Ok(None);
        };
        let present: bool = self
            .conn
            .prepare_cached("SELECT present FROM blobs WHERE hash = ?1")?
            .query_row([f.blob_hash.as_slice()], |r| r.get(0))
            .optional()?
            .unwrap_or(false);
        if !present {
            return Ok(None);
        }
        Ok(self
            .dir
            .read_blob(&f.blob_hash)?
            .map(|bytes| self.cap_text(&bytes)))
    }

    pub(super) fn cap_text(&self, bytes: &[u8]) -> String {
        let text = String::from_utf8_lossy(bytes);
        truncate_bytes(&text, self.limits.max_text_bytes as usize).to_owned()
    }

    fn insert_row(
        &mut self,
        state: &ClipState,
        deleted_hlc: Option<u64>,
        plain_text: Option<&str>,
    ) -> Result<()> {
        let live = deleted_hlc.is_none();
        let byte_size: u64 = if live {
            state.formats.iter().map(|f| f.size).sum()
        } else {
            0
        };
        self.conn.execute(
            "INSERT INTO clips (id, origin, created_hlc, last_used_hlc, sort_hlc, group_key, kind,
               preview, plain_text, byte_size, pinned, pin_hlc, pin_origin, label, label_hlc,
               label_origin, deleted_hlc, local_only, source_app, use_count, received_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, 0, ?18,
               ?19, ?20)",
            params![
                state.clip_id.0,
                state.origin.0,
                state.created_hlc as i64,
                state.last_used_hlc as i64,
                state.sort_hlc as i64,
                state.group_key.as_slice(),
                state.kind.as_str(),
                if live { state.preview.as_str() } else { "" },
                plain_text,
                byte_size as i64,
                state.pinned,
                state.pin_hlc as i64,
                state.pin_origin.as_ref().map(|d| d.0.as_str()),
                state.label,
                state.label_hlc as i64,
                state.label_origin.as_ref().map(|d| d.0.as_str()),
                deleted_hlc.map(|d| d as i64),
                if live { state.source_app.as_deref() } else { None },
                state.use_count,
                self.hlc.now_ms() as i64,
            ],
        )?;
        Ok(())
    }

    fn update_live_row(&mut self, state: &ClipState) -> Result<()> {
        self.conn
            .prepare_cached(
                "UPDATE clips SET last_used_hlc = ?1, sort_hlc = ?2, pinned = ?3, pin_hlc = ?4,
                   pin_origin = ?5, label = ?6, label_hlc = ?7, label_origin = ?8, use_count = ?10
                 WHERE id = ?9",
            )?
            .execute(params![
                state.last_used_hlc as i64,
                state.sort_hlc as i64,
                state.pinned,
                state.pin_hlc as i64,
                state.pin_origin.as_ref().map(|d| d.0.as_str()),
                state.label,
                state.label_hlc as i64,
                state.label_origin.as_ref().map(|d| d.0.as_str()),
                state.clip_id.0,
                state.use_count,
            ])?;
        Ok(())
    }

    /// Turns a live row into a tombstone: formats and content gone, row kept.
    fn tombstone_row(&mut self, state: &ClipState, deleted_hlc: u64) -> Result<()> {
        let hashes = self.remove_formats(&state.clip_id)?;
        self.conn.execute(
            "UPDATE clips SET last_used_hlc = 0, sort_hlc = ?1, preview = '', plain_text = NULL,
               byte_size = 0, pinned = 0, pin_hlc = 0, pin_origin = NULL, label = NULL,
               label_hlc = 0, label_origin = NULL, deleted_hlc = ?2, source_app = NULL,
               use_count = 0
             WHERE id = ?3",
            params![
                state.created_hlc as i64,
                deleted_hlc as i64,
                state.clip_id.0
            ],
        )?;
        self.gc_blobs(hashes)
    }

    /// Deletes a row and its formats, collecting unreferenced blobs.
    pub(super) fn remove_row(&mut self, clip_id: &ClipId) -> Result<()> {
        if let Some(e) = self.existing(clip_id)? {
            self.dirty_groups.insert(e.group_key);
        }
        let hashes = self.remove_formats(clip_id)?;
        self.conn
            .execute("DELETE FROM clips WHERE id = ?1", [&clip_id.0])?;
        self.gc_blobs(hashes)
    }

    fn remove_formats(&mut self, clip_id: &ClipId) -> Result<Vec<Hash32>> {
        let hashes: Vec<Vec<u8>> = self
            .conn
            .prepare_cached("SELECT blob_hash FROM formats WHERE clip_id = ?1")?
            .query_map([&clip_id.0], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        self.conn
            .execute("DELETE FROM formats WHERE clip_id = ?1", [&clip_id.0])?;
        hashes.iter().map(|h| hash_from_slice(h)).collect()
    }

    /// Drops blobs no remaining format references; files go after commit.
    fn gc_blobs(&mut self, hashes: Vec<Hash32>) -> Result<()> {
        for hash in hashes {
            let referenced: bool = self
                .conn
                .prepare_cached("SELECT EXISTS(SELECT 1 FROM formats WHERE blob_hash = ?1)")?
                .query_row([hash.as_slice()], |r| r.get(0))?;
            if !referenced {
                self.conn
                    .execute("DELETE FROM blobs WHERE hash = ?1", [hash.as_slice()])?;
                self.pending_blob_removals.push(hash);
            }
        }
        Ok(())
    }

    /// A fetched blob is on disk: mark it present, index text, make thumbnails.
    pub(super) fn blob_arrived(&mut self, hash: &Hash32, bytes: &[u8]) -> Result<()> {
        self.conn.execute(
            "UPDATE blobs SET present = 1 WHERE hash = ?1",
            [hash.as_slice()],
        )?;
        let users: Vec<(String, String, Vec<u8>)> = self
            .conn
            .prepare(
                "SELECT f.clip_id, f.mime, c.group_key FROM formats f JOIN clips c ON c.id = f.clip_id
                 WHERE f.blob_hash = ?1",
            )?
            .query_map([hash.as_slice()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?;
        let mut thumb_done = false;
        for (clip_id, format_mime, key) in users {
            self.dirty_groups.insert(hash_from_slice(&key)?);
            if format_mime == mime::TEXT {
                let text = self.cap_text(bytes);
                self.conn.execute(
                    "UPDATE clips SET plain_text = ?1 WHERE id = ?2 AND plain_text IS NULL",
                    params![text, clip_id],
                )?;
            } else if format_mime == mime::PNG && !thumb_done {
                thumb_done = true;
                match decode_png(bytes).and_then(|img| thumbnail_png(&img)) {
                    Some(png) => self.dir.write_thumb(hash, &png)?,
                    None => tracing::warn!(%clip_id, "fetched PNG did not decode; no thumbnail"),
                }
            }
        }
        Ok(())
    }
}

/// A row as recomputation would write it, for comparison.
#[derive(Debug, PartialEq, Eq)]
struct Expected {
    origin: DeviceId,
    created_hlc: u64,
    last_used_hlc: u64,
    sort_hlc: u64,
    use_count: u32,
    group_key: Hash32,
    kind: String,
    preview: String,
    byte_size: u64,
    pinned: bool,
    pin_hlc: u64,
    pin_origin: Option<String>,
    label: Option<String>,
    label_hlc: u64,
    label_origin: Option<String>,
    deleted_hlc: Option<u64>,
    formats: BTreeSet<(String, Hash32, u64)>,
}

fn expected(state: &ClipState) -> Expected {
    let live = state.deleted_hlc.is_none();
    Expected {
        origin: state.origin.clone(),
        created_hlc: state.created_hlc,
        last_used_hlc: state.last_used_hlc,
        sort_hlc: state.sort_hlc,
        use_count: state.use_count,
        group_key: state.group_key,
        kind: state.kind.as_str().to_owned(),
        preview: if live {
            state.preview.clone()
        } else {
            String::new()
        },
        byte_size: if live {
            state.formats.iter().map(|f| f.size).sum()
        } else {
            0
        },
        pinned: state.pinned,
        pin_hlc: state.pin_hlc,
        pin_origin: state.pin_origin.as_ref().map(|d| d.0.clone()),
        label: state.label.clone(),
        label_hlc: state.label_hlc,
        label_origin: state.label_origin.as_ref().map(|d| d.0.clone()),
        deleted_hlc: state.deleted_hlc,
        formats: if live {
            state
                .formats
                .iter()
                .map(|f| (f.mime.clone(), f.blob_hash, f.size))
                .collect()
        } else {
            BTreeSet::new()
        },
    }
}

pub(super) fn check_consistency(store: &Store) -> Result<(), String> {
    let err = |e: crate::Error| e.to_string();
    let view = store.view();
    let ops = view.all_ops().map_err(err)?;
    let states = merge::materialize(&ops);

    let mut actual: BTreeMap<ClipId, Expected> = BTreeMap::new();
    let conn = &store.conn;
    let mut stmt = conn
        .prepare(
            "SELECT id, origin, created_hlc, last_used_hlc, sort_hlc, group_key, kind, preview,
               byte_size, pinned, pin_hlc, pin_origin, label, label_hlc, label_origin, deleted_hlc,
               use_count
             FROM clips WHERE local_only = 0",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                Expected {
                    origin: DeviceId(r.get(1)?),
                    created_hlc: r.get::<_, i64>(2)? as u64,
                    last_used_hlc: r.get::<_, i64>(3)? as u64,
                    sort_hlc: r.get::<_, i64>(4)? as u64,
                    group_key: hash_from_slice(&r.get::<_, Vec<u8>>(5)?).unwrap_or_default(),
                    kind: r.get(6)?,
                    preview: r.get(7)?,
                    byte_size: r.get::<_, i64>(8)? as u64,
                    pinned: r.get(9)?,
                    pin_hlc: r.get::<_, i64>(10)? as u64,
                    pin_origin: r.get(11)?,
                    label: r.get(12)?,
                    label_hlc: r.get::<_, i64>(13)? as u64,
                    label_origin: r.get(14)?,
                    deleted_hlc: r.get::<_, Option<i64>>(15)?.map(|d| d as u64),
                    use_count: r.get(16)?,
                    formats: BTreeSet::new(),
                },
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (id, row) = row.map_err(|e| e.to_string())?;
        actual.insert(ClipId(id), row);
    }
    let mut fstmt = conn
        .prepare("SELECT clip_id, mime, blob_hash, size FROM formats")
        .map_err(|e| e.to_string())?;
    let frows = fstmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Vec<u8>>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    for f in frows {
        let (id, m, h, size) = f.map_err(|e| e.to_string())?;
        let hash = hash_from_slice(&h).map_err(err)?;
        if let Some(row) = actual.get_mut(&ClipId(id.clone())) {
            row.formats.insert((m, hash, size as u64));
        } else {
            let local: bool = conn
                .query_row("SELECT local_only FROM clips WHERE id = ?1", [&id], |r| {
                    r.get(0)
                })
                .map_err(|e| format!("format row for missing clip {id}: {e}"))?;
            if !local {
                return Err(format!("format row for missing clip {id}"));
            }
        }
    }

    for (id, state) in &states {
        let row = actual.remove(id);
        match (state.visible(), state.deleted_hlc, row) {
            (true, _, Some(row)) => {
                let want = expected(state);
                if row != want {
                    return Err(format!("clip {id}: row {row:?} != recomputed {want:?}"));
                }
            }
            (true, _, None) => return Err(format!("clip {id} is visible but has no row")),
            // Tombstones may already have been purged.
            (false, Some(_), None) => {}
            (false, Some(_), Some(row)) => {
                let want = expected(state);
                if row != want {
                    return Err(format!(
                        "tombstone {id}: row {row:?} != recomputed {want:?}"
                    ));
                }
            }
            (false, None, None) => {}
            (false, None, Some(_)) => return Err(format!("trimmed clip {id} still has a row")),
        }
        if state.deleted_hlc.is_some() && !state.stub {
            return Err(format!(
                "deleted clip {id} still has full content in the log"
            ));
        }
    }
    if let Some((id, _)) = actual.into_iter().next() {
        return Err(format!("row {id} has no ClipAdded"));
    }
    conn.execute_batch("INSERT INTO clips_fts(clips_fts, rank) VALUES ('integrity-check', 1)")
        .map_err(|e| format!("FTS integrity check failed: {e}"))?;
    Ok(())
}
