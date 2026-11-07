//! Identity keys of desktops and relays, and which of them a phone trusts
//! (SPEC 6.2 `peer_keys`, 21.3).

use rusqlite::{OptionalExtension, params};

use super::Store;
use crate::error::Result;
use crate::model::{DeviceId, PeerKey, Role};

/// Addresses kept per peer.
const MAX_ADDRS: usize = 4;

fn row_to_key(r: &rusqlite::Row<'_>) -> rusqlite::Result<Option<PeerKey>> {
    let key: Vec<u8> = r.get(3)?;
    let Ok(key) = <[u8; 32]>::try_from(key.as_slice()) else {
        return Ok(None);
    };
    let addrs: String = r.get(4)?;
    Ok(Some(PeerKey {
        device_id: DeviceId(r.get(0)?),
        display_name: r.get(1)?,
        role: Role::parse(&r.get::<_, String>(2)?).unwrap_or_default(),
        key,
        addrs: serde_json::from_str(&addrs).unwrap_or_default(),
        trusted: r.get(5)?,
        updated_ms: r.get::<_, i64>(6)? as u64,
    }))
}

const COLUMNS: &str = "device_id, display_name, role, key, addrs, trusted, updated_ms";

impl Store {
    /// Every known identity key, most recently updated first.
    pub fn peer_keys(&self) -> Result<Vec<PeerKey>> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {COLUMNS} FROM peer_keys ORDER BY updated_ms DESC, device_id"
        ))?;
        let rows = stmt.query_map([], row_to_key)?;
        let mut out = Vec::new();
        for row in rows {
            out.extend(row?);
        }
        Ok(out)
    }

    pub fn peer_key(&self, device: &DeviceId) -> Result<Option<PeerKey>> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM peer_keys WHERE device_id = ?1"),
                [&device.0],
                row_to_key,
            )
            .optional()?
            .flatten())
    }

    /// Records a peer's key, name, role and addresses. `entry.addrs` go to the front
    /// of the known ones. Trust is only ever added here: a row that was trusted stays
    /// trusted, and `entry.trusted` makes it so. This device's own id is ignored.
    pub fn record_peer_key(&mut self, entry: &PeerKey) -> Result<()> {
        if entry.device_id == self.device_id {
            return Ok(());
        }
        self.write(|s| {
            let mut addrs = entry.addrs.clone();
            if let Some(old) = s.peer_key(&entry.device_id)? {
                for a in old.addrs {
                    if !addrs.contains(&a) {
                        addrs.push(a);
                    }
                }
            }
            addrs.truncate(MAX_ADDRS);
            let addrs = serde_json::to_string(&addrs).expect("a list of strings");
            s.conn.execute(
                "INSERT INTO peer_keys (device_id, display_name, role, key, addrs, trusted, updated_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(device_id) DO UPDATE SET display_name = excluded.display_name,
                   role = excluded.role, key = excluded.key, addrs = excluded.addrs,
                   trusted = max(peer_keys.trusted, excluded.trusted),
                   updated_ms = max(peer_keys.updated_ms, excluded.updated_ms)",
                params![
                    entry.device_id.0,
                    entry.display_name,
                    entry.role.as_str(),
                    entry.key.as_slice(),
                    addrs,
                    entry.trusted,
                    entry.updated_ms as i64,
                ],
            )?;
            Ok(())
        })
    }

    /// Forgets one peer's key (Devices > Forget, SPEC 21.3.5).
    pub fn forget_peer_key(&mut self, device: &DeviceId) -> Result<()> {
        self.write(|s| {
            s.conn
                .execute("DELETE FROM peer_keys WHERE device_id = ?1", [&device.0])?;
            Ok(())
        })
    }

    /// Forgets every key (Forget all).
    pub fn forget_all_peer_keys(&mut self) -> Result<()> {
        self.write(|s| {
            s.conn.execute("DELETE FROM peer_keys", [])?;
            Ok(())
        })
    }
}
