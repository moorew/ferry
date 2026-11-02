-- SPEC 6.2. journal_mode and foreign_keys are set per connection by the store.

CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value BLOB NOT NULL
); -- schema_version, device_id, last_hlc, created_at

CREATE TABLE devices (
  device_id     TEXT PRIMARY KEY,
  display_name  TEXT NOT NULL,
  role          TEXT NOT NULL CHECK (role IN ('device','relay')),
  ts_node_id    TEXT,
  ts_hostname   TEXT,
  last_ts_ip    TEXT,
  last_seen_ms  INTEGER,
  is_self       INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE version_vector (
  origin   TEXT PRIMARY KEY,
  max_seq  INTEGER NOT NULL
);

CREATE TABLE ops (
  origin   TEXT    NOT NULL,
  seq      INTEGER NOT NULL,
  hlc      INTEGER NOT NULL,
  kind     INTEGER NOT NULL,
  clip_id  TEXT,                -- NULL for HistoryCleared
  payload  BLOB    NOT NULL,    -- MessagePack of the Op
  PRIMARY KEY (origin, seq)
);
CREATE INDEX ops_clip ON ops(clip_id);
CREATE INDEX ops_hlc  ON ops(hlc);

CREATE TABLE clips (
  rid            INTEGER PRIMARY KEY,   -- stable rowid for FTS5
  id             TEXT    NOT NULL UNIQUE,
  origin         TEXT    NOT NULL,
  created_hlc    INTEGER NOT NULL,
  last_used_hlc  INTEGER NOT NULL,
  sort_hlc       INTEGER NOT NULL,   -- max(created_hlc, last_used_hlc)
  group_key      BLOB    NOT NULL,
  kind           TEXT    NOT NULL CHECK (kind IN ('text','rich','image')),
  preview        TEXT    NOT NULL,   -- first 500 chars of plain text, or "image WxH"
  plain_text     TEXT,               -- full plain text up to the text cap, for FTS
  byte_size      INTEGER NOT NULL,
  pinned         INTEGER NOT NULL DEFAULT 0,
  pin_hlc        INTEGER NOT NULL DEFAULT 0,
  pin_origin     TEXT,
  label          TEXT,
  label_hlc      INTEGER NOT NULL DEFAULT 0,
  label_origin   TEXT,
  deleted_hlc    INTEGER,            -- tombstone
  local_only     INTEGER NOT NULL DEFAULT 0,
  source_app     TEXT                -- local only unless the user opts in
);
CREATE INDEX clips_order ON clips(deleted_hlc, sort_hlc DESC);
CREATE INDEX clips_group ON clips(group_key);

CREATE TABLE formats (
  clip_id    TEXT NOT NULL REFERENCES clips(id) ON DELETE CASCADE,
  mime       TEXT NOT NULL,          -- text/plain | text/html | text/rtf | image/png
  blob_hash  BLOB NOT NULL,
  size       INTEGER NOT NULL,
  PRIMARY KEY (clip_id, mime)
);
CREATE INDEX formats_blob ON formats(blob_hash);

CREATE TABLE blobs (
  hash     BLOB PRIMARY KEY,
  size     INTEGER NOT NULL,
  present  INTEGER NOT NULL DEFAULT 0  -- 0 = known but not fetched yet
);

CREATE VIRTUAL TABLE clips_fts USING fts5(
  plain_text, content='clips', content_rowid='rid'
);

-- External-content triggers: only insert, delete and updates of plain_text, so
-- ClipUsed and pin changes never reindex.
CREATE TRIGGER clips_fts_ai AFTER INSERT ON clips BEGIN
  INSERT INTO clips_fts(rowid, plain_text) VALUES (new.rid, new.plain_text);
END;
CREATE TRIGGER clips_fts_ad AFTER DELETE ON clips BEGIN
  INSERT INTO clips_fts(clips_fts, rowid, plain_text) VALUES ('delete', old.rid, old.plain_text);
END;
CREATE TRIGGER clips_fts_au AFTER UPDATE OF plain_text ON clips BEGIN
  INSERT INTO clips_fts(clips_fts, rowid, plain_text) VALUES ('delete', old.rid, old.plain_text);
  INSERT INTO clips_fts(rowid, plain_text) VALUES (new.rid, new.plain_text);
END;
