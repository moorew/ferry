//! What the sync engine needs from the store beyond applying ops: the backlog for a
//! peer, device names and the per-device switches (SPEC 7.3, 7.4, D-023).

use std::collections::BTreeMap;

use rusqlite::{OptionalExtension, params};

use super::Store;
use crate::error::Result;
use crate::model::{DeviceId, DeviceName, Hash32, Op, PeerSeen, Role, SyncFlags, VersionVector};

impl Store {
    /// Ops a peer holding `their` lacks, in `(hlc, origin, seq)` order (which is seq
    /// order within each origin), at most `limit`. Only ops inside this store's
    /// contiguous prefix of each origin are offered, so the peer never sees a gap.
    pub fn ops_for_peer(&self, their: &VersionVector, limit: usize) -> Result<Vec<Op>> {
        let ours = self.version_vector()?;
        let mut start: Option<i64> = None;
        {
            let mut first = self
                .conn
                .prepare_cached("SELECT hlc FROM ops WHERE origin = ?1 AND seq = ?2")?;
            for (origin, &max) in &ours {
                let from = their.get(origin).copied().unwrap_or(0);
                if from >= max {
                    continue;
                }
                let hlc: Option<i64> = first
                    .query_row(params![origin.0, (from + 1) as i64], |r| r.get(0))
                    .optional()?;
                if let Some(h) = hlc {
                    start = Some(start.map_or(h, |s| s.min(h)));
                }
            }
        }
        let Some(start) = start else {
            return Ok(Vec::new());
        };
        let mut stmt = self.conn.prepare_cached(
            "SELECT origin, seq, payload FROM ops WHERE hlc >= ?1 ORDER BY hlc, origin, seq",
        )?;
        let mut rows = stmt.query([start])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            let origin = DeviceId(row.get(0)?);
            let seq = row.get::<_, i64>(1)? as u64;
            let have = their.get(&origin).copied().unwrap_or(0);
            let max = ours.get(&origin).copied().unwrap_or(0);
            if seq <= have || seq > max {
                continue;
            }
            out.push(Op::decode(&row.get::<_, Vec<u8>>(2)?)?);
            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }

    /// Ticks the HLC for a local event that isn't an op (a clipboard change the live
    /// mirror must not overwrite, SPEC 7.6) and persists it.
    pub fn tick_hlc(&mut self) -> Result<u64> {
        self.write(|s| Ok(s.hlc.tick()))
    }

    /// Whether a blob's bytes are on disk.
    pub fn blob_present(&self, hash: &Hash32) -> Result<bool> {
        Ok(self
            .conn
            .query_row(
                "SELECT present FROM blobs WHERE hash = ?1",
                [hash.as_slice()],
                |r| r.get::<_, bool>(0),
            )
            .optional()?
            .unwrap_or(false))
    }

    /// Names and roles of every device this store knows, for the `Devices` frame.
    pub fn device_names(&self) -> Result<Vec<DeviceName>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT device_id, display_name, role, name_hlc FROM devices ORDER BY device_id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, name, role, hlc) = row?;
            out.push(DeviceName {
                device_id: DeviceId(id),
                display_name: name,
                role: Role::parse(&role).unwrap_or_default(),
                updated_hlc: hlc as u64,
            });
        }
        Ok(out)
    }

    /// Merges names from a peer's `Devices` frame, last writer wins by
    /// `updated_hlc`. This device's own row is never changed. Returns whether
    /// anything changed.
    pub fn merge_device_names(&mut self, names: &[DeviceName]) -> Result<bool> {
        self.write(|s| {
            let mut changed = 0;
            for n in names {
                if n.device_id == s.device_id {
                    continue;
                }
                changed += s.conn.execute(
                    "INSERT INTO devices (device_id, display_name, role, name_hlc, color_index)
                     VALUES (?1, ?2, ?3, ?4, (SELECT coalesce(max(color_index) + 1, 0) FROM devices))
                     ON CONFLICT(device_id) DO UPDATE SET display_name = excluded.display_name,
                       role = excluded.role, name_hlc = excluded.name_hlc
                     WHERE excluded.name_hlc > devices.name_hlc AND devices.is_self = 0",
                    params![
                        n.device_id.0,
                        n.display_name,
                        n.role.as_str(),
                        n.updated_hlc as i64
                    ],
                )?;
            }
            Ok(changed > 0)
        })
    }

    /// Records a peer seen in a session: its tailnet details and last-seen time. A
    /// new device takes the name it gave; a known one keeps its gossiped name.
    pub fn record_peer(&mut self, peer: &PeerSeen) -> Result<()> {
        if peer.device_id == self.device_id {
            return Ok(());
        }
        self.write(|s| {
            s.conn.execute(
                "INSERT INTO devices (device_id, display_name, role, ts_node_id, ts_hostname,
                   last_ts_ip, last_seen_ms, os, color_index)
                 VALUES (?1, ?2, coalesce(?3, 'device'), ?4, ?5, ?6, ?7, ?8,
                   (SELECT coalesce(max(color_index) + 1, 0) FROM devices))
                 ON CONFLICT(device_id) DO UPDATE SET role = coalesce(?3, role),
                   ts_node_id = coalesce(?4, ts_node_id), ts_hostname = coalesce(?5, ts_hostname),
                   last_ts_ip = coalesce(?6, last_ts_ip), last_seen_ms = ?7, os = coalesce(?8, os),
                   display_name = CASE WHEN display_name = '' THEN ?2 ELSE display_name END",
                params![
                    peer.device_id.0,
                    peer.display_name,
                    peer.role.map(Role::as_str),
                    peer.ts_node_id,
                    peer.ts_hostname,
                    peer.ip,
                    peer.now_ms as i64,
                    peer.os,
                ],
            )?;
            Ok(())
        })
    }

    /// Per-device sync switches, for every device that has a row.
    pub fn sync_flags(&self) -> Result<BTreeMap<DeviceId, SyncFlags>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT device_id, send_enabled, receive_enabled FROM devices")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                DeviceId(r.get(0)?),
                SyncFlags {
                    send: r.get(1)?,
                    receive: r.get(2)?,
                },
            ))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Turns Send or Receive on or off for one device (D-023).
    pub fn set_sync_flags(&mut self, device: &DeviceId, flags: SyncFlags) -> Result<()> {
        self.write(|s| {
            s.conn.execute(
                "UPDATE devices SET send_enabled = ?1, receive_enabled = ?2 WHERE device_id = ?3",
                params![flags.send, flags.receive, device.0],
            )?;
            Ok(())
        })
    }

    pub(super) fn receive_enabled(&self, origin: &DeviceId) -> Result<bool> {
        Ok(self
            .conn
            .query_row(
                "SELECT receive_enabled FROM devices WHERE device_id = ?1",
                [&origin.0],
                |r| r.get::<_, bool>(0),
            )
            .optional()?
            .unwrap_or(true))
    }
}
