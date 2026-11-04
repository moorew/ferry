//! Read-side queries for the UI and sync.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, Row, params};

use super::blobs::DataDir;
use crate::error::Result;
use crate::look::Look;
use crate::model::{
    ClipId, ClipKind, DeviceId, DeviceInfo, Hash32, Op, Role, VersionVector, hash_from_slice, mime,
};

/// One row of the list: a display group (rule 7), shown as its newest member.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayRow {
    /// The member shown; pasting the row uses this clip.
    pub clip_id: ClipId,
    pub group_key: Hash32,
    pub kind: ClipKind,
    pub preview: String,
    /// Label of the newest member that has one.
    pub label: Option<String>,
    /// Any member is pinned.
    pub pinned: bool,
    pub origin: DeviceId,
    pub created_hlc: u64,
    pub sort_hlc: u64,
    pub byte_size: u64,
    /// Not synced ("not synced" marker).
    pub local_only: bool,
    pub source_app: Option<String>,
    /// Live clips in the group.
    pub members: u32,
    /// Hash of the PNG format, for the thumbnail.
    pub image_hash: Option<Hash32>,
    /// Pastes and copies of the member shown, on every device.
    pub use_count: u32,
    /// Derived from kind and preview, never stored (see [`crate::look`]).
    pub look: Look,
    /// Reads better in a monospace font.
    pub mono: bool,
    /// When this device first held the member shown, by its own clock.
    pub received_ms: Option<u64>,
}

impl DisplayRow {
    /// Cursor for the page after this row.
    pub fn cursor(&self) -> PageCursor {
        PageCursor {
            sort_hlc: self.sort_hlc,
            clip_id: self.clip_id.clone(),
        }
    }
}

/// Keyset paging position: rows strictly after this one in list order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageCursor {
    pub sort_hlc: u64,
    pub clip_id: ClipId,
}

/// Every column of one clip row except `rid` and `plain_text`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipRow {
    pub id: ClipId,
    pub origin: DeviceId,
    pub created_hlc: u64,
    pub last_used_hlc: u64,
    pub sort_hlc: u64,
    pub group_key: Hash32,
    pub kind: ClipKind,
    pub preview: String,
    pub byte_size: u64,
    pub pinned: bool,
    pub pin_hlc: u64,
    pub pin_origin: Option<DeviceId>,
    pub label: Option<String>,
    pub label_hlc: u64,
    pub label_origin: Option<DeviceId>,
    pub deleted_hlc: Option<u64>,
    pub local_only: bool,
    pub source_app: Option<String>,
    /// Pastes and copies from TailCopy, on every device.
    pub use_count: u32,
    /// When this device first held it, by this device's clock.
    pub received_ms: Option<u64>,
}

/// One stored format of a clip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatInfo {
    pub mime: String,
    pub blob_hash: Hash32,
    pub size: u64,
    /// The blob is on disk (otherwise it still has to be fetched).
    pub present: bool,
}

/// A clip with its formats.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipDetail {
    pub clip: ClipRow,
    pub formats: Vec<FormatInfo>,
}

impl ClipDetail {
    pub fn format(&self, mime: &str) -> Option<&FormatInfo> {
        self.formats.iter().find(|f| f.mime == mime)
    }
}

const CLIP_COLUMNS: &str =
    "id, origin, created_hlc, last_used_hlc, sort_hlc, group_key, kind, preview,
    byte_size, pinned, pin_hlc, pin_origin, label, label_hlc, label_origin, deleted_hlc,
    local_only, source_app, use_count, received_ms";

fn clip_row(r: &Row<'_>) -> rusqlite::Result<ClipRow> {
    Ok(ClipRow {
        id: ClipId(r.get(0)?),
        origin: DeviceId(r.get(1)?),
        created_hlc: r.get::<_, i64>(2)? as u64,
        last_used_hlc: r.get::<_, i64>(3)? as u64,
        sort_hlc: r.get::<_, i64>(4)? as u64,
        group_key: blob_hash(r, 5)?,
        kind: kind(r, 6)?,
        preview: r.get(7)?,
        byte_size: r.get::<_, i64>(8)? as u64,
        pinned: r.get(9)?,
        pin_hlc: r.get::<_, i64>(10)? as u64,
        pin_origin: r.get::<_, Option<String>>(11)?.map(DeviceId),
        label: r.get(12)?,
        label_hlc: r.get::<_, i64>(13)? as u64,
        label_origin: r.get::<_, Option<String>>(14)?.map(DeviceId),
        deleted_hlc: r.get::<_, Option<i64>>(15)?.map(|v| v as u64),
        local_only: r.get(16)?,
        source_app: r.get(17)?,
        use_count: r.get(18)?,
        received_ms: r.get::<_, Option<i64>>(19)?.map(|v| v as u64),
    })
}

fn conversion_err(idx: usize, msg: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        idx,
        rusqlite::types::Type::Blob,
        msg.to_owned().into(),
    )
}

fn blob_hash(r: &Row<'_>, idx: usize) -> rusqlite::Result<Hash32> {
    let bytes: Vec<u8> = r.get(idx)?;
    hash_from_slice(&bytes).map_err(|_| conversion_err(idx, "hash is not 32 bytes"))
}

fn kind(r: &Row<'_>, idx: usize) -> rusqlite::Result<ClipKind> {
    let s: String = r.get(idx)?;
    ClipKind::parse(&s).ok_or_else(|| conversion_err(idx, "unknown clip kind"))
}

/// Grouped live rows. `?1`/`?2` are the cursor (NULL for the first page), `?3` the
/// limit, `?4` an optional group key filter.
const DISPLAY_SQL: &str = "
WITH live AS (
  SELECT id, group_key, kind, preview, origin, created_hlc, sort_hlc, byte_size, local_only,
    source_app, use_count, received_ms,
    ROW_NUMBER() OVER (PARTITION BY group_key ORDER BY sort_hlc DESC, id DESC) AS rn,
    MAX(pinned) OVER (PARTITION BY group_key) AS any_pinned,
    COUNT(*) OVER (PARTITION BY group_key) AS members
  FROM clips
  WHERE deleted_hlc IS NULL AND (?4 IS NULL OR group_key = ?4)
)
SELECT id, group_key, kind, preview,
  (SELECT l.label FROM clips l
     WHERE l.group_key = live.group_key AND l.deleted_hlc IS NULL AND l.label IS NOT NULL
     ORDER BY l.sort_hlc DESC, l.id DESC LIMIT 1),
  any_pinned, origin, created_hlc, sort_hlc, byte_size, local_only, source_app, members,
  (SELECT f.blob_hash FROM formats f WHERE f.clip_id = live.id AND f.mime = 'image/png'),
  use_count, received_ms
FROM live
WHERE rn = 1 AND (?1 IS NULL OR (sort_hlc, id) < (?1, ?2))
ORDER BY sort_hlc DESC, id DESC
LIMIT ?3";

fn display_row(r: &Row<'_>) -> rusqlite::Result<DisplayRow> {
    let kind = kind(r, 2)?;
    let preview: String = r.get(3)?;
    let (look, mono) = crate::look::classify(kind, &preview);
    Ok(DisplayRow {
        clip_id: ClipId(r.get(0)?),
        group_key: blob_hash(r, 1)?,
        kind,
        preview,
        label: r.get(4)?,
        pinned: r.get(5)?,
        origin: DeviceId(r.get(6)?),
        created_hlc: r.get::<_, i64>(7)? as u64,
        sort_hlc: r.get::<_, i64>(8)? as u64,
        byte_size: r.get::<_, i64>(9)? as u64,
        local_only: r.get(10)?,
        source_app: r.get(11)?,
        members: r.get(12)?,
        image_hash: r
            .get::<_, Option<Vec<u8>>>(13)?
            .map(|b| hash_from_slice(&b))
            .transpose()
            .map_err(|_| conversion_err(13, "hash is not 32 bytes"))?,
        use_count: r.get(14)?,
        look,
        mono,
        received_ms: r.get::<_, Option<i64>>(15)?.map(|v| v as u64),
    })
}

/// Queries over a connection (the store's or a [`Reader`]'s).
#[derive(Clone, Copy)]
pub struct ReadView<'a> {
    conn: &'a Connection,
    dir: &'a DataDir,
}

impl<'a> ReadView<'a> {
    pub(crate) fn new(conn: &'a Connection, dir: &'a DataDir) -> Self {
        Self { conn, dir }
    }

    /// One page of display rows in list order (`sort_hlc DESC, id DESC`).
    pub fn list_rows(&self, after: Option<&PageCursor>, limit: usize) -> Result<Vec<DisplayRow>> {
        let mut stmt = self.conn.prepare_cached(DISPLAY_SQL)?;
        let rows = stmt.query_map(
            params![
                after.map(|c| c.sort_hlc as i64),
                after.map(|c| c.clip_id.0.as_str()),
                limit.min(i64::MAX as usize) as i64,
                Option::<&[u8]>::None,
            ],
            display_row,
        )?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Every display row, for building a search index.
    pub fn all_rows(&self) -> Result<Vec<DisplayRow>> {
        self.list_rows(None, usize::MAX)
    }

    /// The display row of one group, if it has live members.
    pub fn row_for_group(&self, group_key: &Hash32) -> Result<Option<DisplayRow>> {
        let mut stmt = self.conn.prepare_cached(DISPLAY_SQL)?;
        Ok(stmt
            .query_row(
                params![
                    Option::<i64>::None,
                    Option::<&str>::None,
                    1i64,
                    Some(group_key.as_slice())
                ],
                display_row,
            )
            .optional()?)
    }

    /// A clip (live or tombstone) with its formats.
    pub fn get_clip(&self, clip_id: &ClipId) -> Result<Option<ClipDetail>> {
        let clip = self
            .conn
            .prepare_cached(&format!("SELECT {CLIP_COLUMNS} FROM clips WHERE id = ?1"))?
            .query_row([&clip_id.0], clip_row)
            .optional()?;
        let Some(clip) = clip else { return Ok(None) };
        let mut stmt = self.conn.prepare_cached(
            "SELECT f.mime, f.blob_hash, f.size, coalesce(b.present, 0)
             FROM formats f LEFT JOIN blobs b ON b.hash = f.blob_hash
             WHERE f.clip_id = ?1 ORDER BY f.mime",
        )?;
        let formats = stmt
            .query_map([&clip_id.0], |r| {
                Ok(FormatInfo {
                    mime: r.get(0)?,
                    blob_hash: blob_hash(r, 1)?,
                    size: r.get::<_, i64>(2)? as u64,
                    present: r.get(3)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(Some(ClipDetail { clip, formats }))
    }

    /// Every live (not deleted) clip row, ungrouped, in list order.
    pub fn live_clips(&self) -> Result<Vec<ClipRow>> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {CLIP_COLUMNS} FROM clips WHERE deleted_hlc IS NULL ORDER BY sort_hlc DESC, id DESC"
        ))?;
        Ok(stmt.query_map([], clip_row)?.collect::<Result<_, _>>()?)
    }

    /// Full plain text of a clip, up to the text cap, if held.
    pub fn plain_text(&self, clip_id: &ClipId) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT plain_text FROM clips WHERE id = ?1",
                [&clip_id.0],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten())
    }

    /// Bytes of a blob, if on disk.
    pub fn read_blob(&self, hash: &Hash32) -> Result<Option<Vec<u8>>> {
        self.dir.read_blob(hash)
    }

    /// Bytes of one format of a clip, if the blob is on disk.
    pub fn read_format(&self, clip_id: &ClipId, mime: &str) -> Result<Option<Vec<u8>>> {
        let hash: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT blob_hash FROM formats WHERE clip_id = ?1 AND mime = ?2",
                params![clip_id.0, mime],
                |r| r.get(0),
            )
            .optional()?;
        match hash {
            Some(h) => self.dir.read_blob(&hash_from_slice(&h)?),
            None => Ok(None),
        }
    }

    /// Path of the thumbnail for a PNG blob, if one exists.
    pub fn thumb_path(&self, png_hash: &Hash32) -> Option<PathBuf> {
        let path = self.dir.thumb_path(png_hash);
        path.exists().then_some(path)
    }

    pub fn data_dir(&self) -> &Path {
        self.dir.root()
    }

    /// Clips whose full text matches an FTS5 query built from `text` (each word as a
    /// prefix term, all required). Returns `(clip_id, group_key)` of live clips, best
    /// match first.
    pub fn fts_search(&self, text: &str, limit: usize) -> Result<Vec<(ClipId, Hash32)>> {
        let query = fts_query(text);
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let mut stmt = self.conn.prepare_cached(
            "SELECT c.id, c.group_key FROM clips_fts
             JOIN clips c ON c.rid = clips_fts.rowid
             WHERE clips_fts MATCH ?1 AND c.deleted_hlc IS NULL
             ORDER BY clips_fts.rank LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![query, limit as i64], |r| {
            Ok((ClipId(r.get(0)?), blob_hash(r, 1)?))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn devices(&self) -> Result<Vec<DeviceInfo>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT device_id, display_name, role, ts_node_id, ts_hostname, last_ts_ip,
               last_seen_ms, is_self, coalesce(color_index, 0), device_type, os
             FROM devices ORDER BY display_name, device_id",
        )?;
        let rows = stmt.query_map([], |r| {
            let role: String = r.get(2)?;
            Ok(DeviceInfo {
                device_id: DeviceId(r.get(0)?),
                display_name: r.get(1)?,
                role: Role::parse(&role).ok_or_else(|| conversion_err(2, "unknown role"))?,
                ts_node_id: r.get(3)?,
                ts_hostname: r.get(4)?,
                last_ts_ip: r.get(5)?,
                last_seen_ms: r.get::<_, Option<i64>>(6)?.map(|v| v as u64),
                is_self: r.get(7)?,
                color_index: r.get(8)?,
                device_type: r.get(9)?,
                os: r.get(10)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn version_vector(&self) -> Result<VersionVector> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT origin, max_seq FROM version_vector")?;
        let rows = stmt.query_map([], |r| {
            Ok((DeviceId(r.get(0)?), r.get::<_, i64>(1)? as u64))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Ops from `origin` with `seq > after_seq`, in seq order.
    pub fn ops_after(&self, origin: &DeviceId, after_seq: u64, limit: usize) -> Result<Vec<Op>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT payload FROM ops WHERE origin = ?1 AND seq > ?2 ORDER BY seq LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![
                origin.0,
                after_seq as i64,
                limit.min(i64::MAX as usize) as i64
            ],
            |r| r.get::<_, Vec<u8>>(0),
        )?;
        rows.map(|p| Op::decode(&p?)).collect()
    }

    /// Every op held, ordered by origin and seq.
    pub fn all_ops(&self) -> Result<Vec<Op>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT payload FROM ops ORDER BY origin, seq")?;
        let rows = stmt.query_map([], |r| r.get::<_, Vec<u8>>(0))?;
        rows.map(|p| Op::decode(&p?)).collect()
    }

    /// Blobs referenced by clips but not on disk yet.
    pub fn missing_blobs(&self) -> Result<Vec<(Hash32, u64)>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT hash, size FROM blobs WHERE present = 0 ORDER BY size")?;
        let rows = stmt.query_map([], |r| Ok((blob_hash(r, 0)?, r.get::<_, i64>(1)? as u64)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Whether a clip has a PNG format (for thumbnails); convenience for the UI.
    pub fn png_hash(&self, clip_id: &ClipId) -> Result<Option<Hash32>> {
        let h: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT blob_hash FROM formats WHERE clip_id = ?1 AND mime = ?2",
                params![clip_id.0, mime::PNG],
                |r| r.get(0),
            )
            .optional()?;
        h.map(|b| hash_from_slice(&b)).transpose()
    }
}

/// Each whitespace-separated word as a quoted prefix term.
fn fts_query(text: &str) -> String {
    text.split_whitespace()
        .map(|w| w.replace('"', ""))
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\"*"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A read-only connection for UI queries, separate from the store's writer.
pub struct Reader {
    pub(crate) conn: Connection,
    dir: DataDir,
}

impl Reader {
    /// Opens the database in `data_dir` read-only. The store must have created it.
    pub fn open(data_dir: impl Into<PathBuf>) -> Result<Reader> {
        let dir = DataDir::new(data_dir);
        let conn = super::open_read_only(&dir)?;
        Ok(Reader { conn, dir })
    }

    pub fn view(&self) -> ReadView<'_> {
        ReadView::new(&self.conn, &self.dir)
    }
}
