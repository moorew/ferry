-- Phones (SPEC 21.3). SQLite can't change a CHECK constraint in place, so the devices
-- table is rebuilt to accept role 'mobile'.
CREATE TABLE devices_v4 (
  device_id       TEXT PRIMARY KEY,
  display_name    TEXT NOT NULL,
  role            TEXT NOT NULL CHECK (role IN ('device','relay','mobile')),
  ts_node_id      TEXT,
  ts_hostname     TEXT,
  last_ts_ip      TEXT,
  last_seen_ms    INTEGER,
  is_self         INTEGER NOT NULL DEFAULT 0,
  color_index     INTEGER,
  device_type     TEXT,
  os              TEXT,
  send_enabled    INTEGER NOT NULL DEFAULT 1,
  receive_enabled INTEGER NOT NULL DEFAULT 1,
  name_hlc        INTEGER NOT NULL DEFAULT 0
);
INSERT INTO devices_v4 (device_id, display_name, role, ts_node_id, ts_hostname, last_ts_ip,
  last_seen_ms, is_self, color_index, device_type, os, send_enabled, receive_enabled, name_hlc)
SELECT device_id, display_name, role, ts_node_id, ts_hostname, last_ts_ip,
  last_seen_ms, is_self, color_index, device_type, os, send_enabled, receive_enabled, name_hlc
FROM devices;
DROP TABLE devices;
ALTER TABLE devices_v4 RENAME TO devices;

-- Identity keys of desktops and relays. Local only; never synced as ops.
CREATE TABLE peer_keys (
  device_id     TEXT PRIMARY KEY,
  display_name  TEXT NOT NULL,
  role          TEXT NOT NULL,
  key           BLOB NOT NULL,
  addrs         TEXT NOT NULL,
  trusted       INTEGER NOT NULL DEFAULT 0,
  updated_ms    INTEGER NOT NULL
);
