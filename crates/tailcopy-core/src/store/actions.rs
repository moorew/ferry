//! Local actions: capture, use, pin, label, delete, clear, retention (SPEC 6.4, 6.5).

use rusqlite::{OptionalExtension, params};

use super::Store;
use crate::capture::{Capture, normalize, thumbnail_png};
use crate::error::{Error, Result};
use crate::hlc;
use crate::merge::{Clear, clamp_label};
use crate::model::{ClipId, FormatRef, Hash32, Op, OpBody, OpKind, blake3_hash, hash_from_slice};

/// Result of [`Store::capture`].
// Returned once per capture; boxing the op would only add an allocation.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureOutcome {
    /// Nothing worth storing (empty text, undecodable image, no supported format).
    Ignored,
    Added {
        clip_id: ClipId,
        group_key: Hash32,
        /// Stored locally only (rule match or over a size cap); no op emitted.
        local_only: bool,
        /// The `ClipAdded` to send to peers, if synced.
        op: Option<Op>,
    },
}

/// Local retention policy (SPEC 6.5, `[retention]`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Unpinned clips kept.
    pub max_items: usize,
    /// Unpinned clips older than this (by `sort_hlc`) are trimmed.
    pub max_age_days: u64,
    /// Tombstone rows are dropped after this many days.
    pub tombstone_days: u64,
}

impl RetentionPolicy {
    pub const DESKTOP: Self = Self {
        max_items: 1000,
        max_age_days: 30,
        tombstone_days: 30,
    };
    pub const RELAY: Self = Self {
        max_items: 10_000,
        max_age_days: 90,
        tombstone_days: 30,
    };
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self::DESKTOP
    }
}

const DAY_MS: u64 = 86_400_000;

impl Store {
    /// Emits a local op: next seq, fresh HLC, stored and applied.
    fn emit(&mut self, body: OpBody) -> Result<Op> {
        let hlc = self.hlc.tick();
        let seq = self.max_seq(&self.device_id.clone())? + 1;
        let op = Op {
            origin: self.device_id.clone(),
            seq,
            hlc,
            body,
        };
        self.insert_op(&op)?;
        match op.body.clip_id() {
            Some(id) => self.recompute(&id.clone())?,
            None => {
                let clear = Clear::from_op(&op).expect("only HistoryCleared has no clip id");
                self.clears.push(clear.clone());
                for id in self.clear_targets(&clear)? {
                    self.recompute(&id)?;
                }
                self.clear_local_only(&clear)?;
            }
        }
        Ok(op)
    }

    /// Normalizes and stores a capture (SPEC 9.6). Capture always adds: identical
    /// content gets a new clip id and groups with the old one in the list.
    pub fn capture(&mut self, capture: &Capture) -> Result<CaptureOutcome> {
        let Some(n) = normalize(&capture.formats, &self.limits) else {
            return Ok(CaptureOutcome::Ignored);
        };
        let local_only = capture.local_only || n.over_cap;
        let formats: Vec<FormatRef> = n
            .formats
            .iter()
            .map(|(m, bytes)| FormatRef {
                mime: (*m).to_owned(),
                blob_hash: blake3_hash(bytes),
                size: bytes.len() as u64,
            })
            .collect();
        for (f, (_, bytes)) in formats.iter().zip(&n.formats) {
            self.dir.write_blob(&f.blob_hash, bytes)?;
        }
        if let (Some(image), Some(png)) = (
            &n.image,
            formats.iter().find(|f| f.mime == crate::model::mime::PNG),
        ) && let Some(thumb) = thumbnail_png(image)
        {
            self.dir.write_thumb(&png.blob_hash, &thumb)?;
        }
        let source_app = capture.source_app.clone();
        let sync_source_app = self.sync_source_app;

        self.write(|s| {
            for f in &formats {
                s.conn.execute(
                    "INSERT INTO blobs (hash, size, present) VALUES (?1, ?2, 1)
                     ON CONFLICT(hash) DO UPDATE SET present = 1",
                    params![f.blob_hash.as_slice(), f.size as i64],
                )?;
            }
            let clip_id;
            let op = if local_only {
                let hlc = s.hlc.tick();
                clip_id = ClipId::new_v7(hlc::wall_ms(hlc));
                s.conn.execute(
                    "INSERT INTO clips (id, origin, created_hlc, last_used_hlc, sort_hlc, group_key,
                       kind, preview, plain_text, byte_size, local_only, source_app, received_ms)
                     VALUES (?1, ?2, ?3, 0, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10)",
                    params![
                        clip_id.0,
                        s.device_id.0,
                        hlc as i64,
                        n.group_key.as_slice(),
                        n.kind.as_str(),
                        n.preview,
                        n.plain_text,
                        formats.iter().map(|f| f.size).sum::<u64>() as i64,
                        source_app,
                        hlc::wall_ms(hlc) as i64,
                    ],
                )?;
                for f in &formats {
                    s.conn.execute(
                        "INSERT INTO formats (clip_id, mime, blob_hash, size) VALUES (?1, ?2, ?3, ?4)",
                        params![clip_id.0, f.mime, f.blob_hash.as_slice(), f.size as i64],
                    )?;
                }
                s.dirty_groups.insert(n.group_key);
                None
            } else {
                clip_id = ClipId::new_v7(s.hlc.now_ms());
                let op = s.emit(OpBody::ClipAdded {
                    clip_id: clip_id.clone(),
                    kind: n.kind,
                    group_key: n.group_key,
                    preview: n.preview.clone(),
                    formats: formats.clone(),
                    source_app: source_app.clone().filter(|_| sync_source_app),
                    trimmed: false,
                })?;
                if source_app.is_some() {
                    s.conn.execute(
                        "UPDATE clips SET source_app = ?1 WHERE id = ?2",
                        params![source_app, clip_id.0],
                    )?;
                }
                Some(op)
            };
            tracing::debug!(%clip_id, kind = n.kind.as_str(), local_only, "captured clip");
            Ok(CaptureOutcome::Added {
                clip_id,
                group_key: n.group_key,
                local_only,
                op,
            })
        })
    }

    fn live_row(&self, clip_id: &ClipId) -> Result<(bool, Hash32)> {
        let row: Option<(bool, Vec<u8>)> = self
            .conn
            .query_row(
                "SELECT local_only, group_key FROM clips WHERE id = ?1 AND deleted_hlc IS NULL",
                [&clip_id.0],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (local, key) = row.ok_or_else(|| Error::UnknownClip(clip_id.0.clone()))?;
        Ok((local, hash_from_slice(&key)?))
    }

    /// Live members of a display group: `(clip_id, local_only)`.
    fn group_members(&self, key: &Hash32) -> Result<Vec<(ClipId, bool)>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, local_only FROM clips WHERE group_key = ?1 AND deleted_hlc IS NULL",
        )?;
        let rows = stmt.query_map([key.as_slice()], |r| Ok((ClipId(r.get(0)?), r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The user pasted or copied `clip_id` (the member shown for its row) from the
    /// picker or manager. Moves it to the top.
    pub fn use_clip(&mut self, clip_id: &ClipId) -> Result<Option<Op>> {
        self.write(|s| {
            let (local, key) = s.live_row(clip_id)?;
            if local {
                let hlc = s.hlc.tick() as i64;
                s.conn.execute(
                    "UPDATE clips SET last_used_hlc = ?1, sort_hlc = max(created_hlc, ?1),
                       use_count = use_count + 1 WHERE id = ?2",
                    params![hlc, clip_id.0],
                )?;
                s.dirty_groups.insert(key);
                return Ok(None);
            }
            Ok(Some(s.emit(OpBody::ClipUsed {
                clip_id: clip_id.clone(),
            })?))
        })
    }

    /// Pins or unpins the row containing `clip_id`: every member of its group.
    pub fn set_pinned(&mut self, clip_id: &ClipId, pinned: bool) -> Result<Vec<Op>> {
        self.write(|s| {
            let (_, key) = s.live_row(clip_id)?;
            let mut ops = Vec::new();
            for (id, local) in s.group_members(&key)? {
                if local {
                    s.conn.execute(
                        "UPDATE clips SET pinned = ?1 WHERE id = ?2",
                        params![pinned, id.0],
                    )?;
                    s.dirty_groups.insert(key);
                } else {
                    ops.push(s.emit(OpBody::PinSet {
                        clip_id: id,
                        pinned,
                    })?);
                }
            }
            Ok(ops)
        })
    }

    /// Labels the row containing `clip_id` (every member of its group). Labels are
    /// trimmed and cut to 120 chars; empty clears.
    pub fn set_label(&mut self, clip_id: &ClipId, label: Option<&str>) -> Result<Vec<Op>> {
        let label = clamp_label(label);
        self.write(|s| {
            let (_, key) = s.live_row(clip_id)?;
            let mut ops = Vec::new();
            for (id, local) in s.group_members(&key)? {
                if local {
                    s.conn.execute(
                        "UPDATE clips SET label = ?1 WHERE id = ?2",
                        params![label, id.0],
                    )?;
                    s.dirty_groups.insert(key);
                } else {
                    ops.push(s.emit(OpBody::LabelSet {
                        clip_id: id,
                        label: label.clone(),
                    })?);
                }
            }
            Ok(ops)
        })
    }

    /// Deletes the row containing `clip_id`: `ClipDeleted` for every member.
    pub fn delete_row(&mut self, clip_id: &ClipId) -> Result<Vec<Op>> {
        self.write(|s| {
            let (_, key) = s.live_row(clip_id)?;
            let mut ops = Vec::new();
            for (id, local) in s.group_members(&key)? {
                if local {
                    s.remove_row(&id)?;
                } else {
                    ops.push(s.emit(OpBody::ClipDeleted { clip_id: id })?);
                }
            }
            Ok(ops)
        })
    }

    /// Clears history up to now, keeping pinned clips unless `include_pinned`.
    pub fn clear_history(&mut self, include_pinned: bool) -> Result<Op> {
        self.write(|s| {
            // Everything held now goes, including clips stamped ahead of the local
            // clock by a skewed peer.
            let newest: Option<i64> = s.conn.query_row(
                "SELECT max(m) FROM (SELECT max(hlc) AS m FROM ops WHERE kind = ?1
                 UNION ALL SELECT max(created_hlc) FROM clips)",
                [OpKind::ClipAdded as i64],
                |r| r.get(0),
            )?;
            let before_hlc = s.hlc.last().max(newest.unwrap_or(0) as u64);
            // D-018: the op names what it covers and which pinned clips it kept, so a
            // pin or clip that arrives later can't change what the clear removed.
            let seen = s.view().version_vector()?;
            let spared = if include_pinned {
                Vec::new()
            } else {
                let mut stmt = s.conn.prepare(
                    "SELECT id FROM clips WHERE pinned = 1 AND deleted_hlc IS NULL
                     AND local_only = 0 ORDER BY id",
                )?;
                stmt.query_map([], |r| Ok(ClipId(r.get(0)?)))?
                    .collect::<Result<_, _>>()?
            };
            s.emit(OpBody::HistoryCleared {
                before_hlc,
                include_pinned,
                seen: Some(seen),
                spared,
            })
        })
    }

    /// Trims unpinned clips beyond `max_items` or older than `max_age_days` (by
    /// `sort_hlc`) and drops tombstones older than `tombstone_days`. Trimming stubs
    /// the clip's `ClipAdded` and emits no ops. Returns the number of clips trimmed.
    pub fn enforce_retention(&mut self, policy: &RetentionPolicy) -> Result<usize> {
        let now = self.hlc.now_ms();
        let age_cutoff = hlc::pack(now.saturating_sub(policy.max_age_days * DAY_MS), 0);
        let tomb_cutoff = hlc::pack(now.saturating_sub(policy.tombstone_days * DAY_MS), 0);
        self.write(|s| {
            let candidates: Vec<(ClipId, u64)> = {
                let mut stmt = s.conn.prepare(
                    "SELECT id, sort_hlc FROM clips WHERE deleted_hlc IS NULL AND pinned = 0
                     ORDER BY sort_hlc DESC, id DESC",
                )?;
                stmt.query_map([], |r| Ok((ClipId(r.get(0)?), r.get::<_, i64>(1)? as u64)))?
                    .collect::<Result<_, _>>()?
            };
            let mut trimmed = 0;
            for (i, (id, sort_hlc)) in candidates.into_iter().enumerate() {
                if i >= policy.max_items || sort_hlc < age_cutoff {
                    s.trim(&id)?;
                    trimmed += 1;
                }
            }
            let purged = s.conn.execute(
                "DELETE FROM clips WHERE deleted_hlc IS NOT NULL AND deleted_hlc < ?1",
                [tomb_cutoff as i64],
            )?;
            if trimmed > 0 || purged > 0 {
                tracing::info!(trimmed, purged, "applied retention");
            }
            Ok(trimmed)
        })
    }

    /// Removes one clip's content locally without deleting it (SPEC 6.5 trimming).
    fn trim(&mut self, clip_id: &ClipId) -> Result<()> {
        let (local, _) = self.live_row(clip_id)?;
        if local {
            return self.remove_row(clip_id);
        }
        let payloads: Vec<Vec<u8>> = self
            .conn
            .prepare_cached("SELECT payload FROM ops WHERE clip_id = ?1 AND kind = ?2")?
            .query_map(params![clip_id.0, OpKind::ClipAdded as i64], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        for payload in payloads {
            self.stub_op(&Op::decode(&payload)?)?;
        }
        self.recompute(clip_id)
    }
}
