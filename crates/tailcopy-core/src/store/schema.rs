//! Numbered migrations; `schema_version` lives in `meta` (SPEC 6.2).

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{Error, Result};

/// Migration `n` (1-based) is `MIGRATIONS[n - 1]`. Append only.
pub const MIGRATIONS: &[&str] = &[include_str!("migrations/0001_initial.sql")];

/// Schema version this build writes.
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

/// Reads the schema version; 0 for an empty database.
pub fn schema_version(conn: &Connection) -> Result<i64> {
    let has_meta: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'meta')",
        [],
        |r| r.get(0),
    )?;
    if !has_meta {
        return Ok(0);
    }
    Ok(conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'schema_version'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0))
}

/// Applies every pending migration in one transaction. Returns the new version.
pub fn migrate(conn: &mut Connection) -> Result<i64> {
    migrate_with(conn, MIGRATIONS)
}

pub(crate) fn migrate_with(conn: &mut Connection, migrations: &[&str]) -> Result<i64> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let current = schema_version(&tx)?;
    let supported = migrations.len() as i64;
    if current > supported {
        return Err(Error::SchemaTooNew {
            found: current,
            supported,
        });
    }
    for (i, sql) in migrations.iter().enumerate().skip(current as usize) {
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO meta (key, value) VALUES ('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![i as i64 + 1],
        )?;
        tracing::info!(version = i + 1, "applied database migration");
    }
    tx.commit()?;
    Ok(supported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_empty_database_and_is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        assert_eq!(schema_version(&conn).unwrap(), 0);
        assert_eq!(migrate(&mut conn).unwrap(), SCHEMA_VERSION);
        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);
        assert_eq!(migrate(&mut conn).unwrap(), SCHEMA_VERSION);
        for table in [
            "meta",
            "devices",
            "version_vector",
            "ops",
            "clips",
            "formats",
            "blobs",
            "clips_fts",
        ] {
            let n: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name = ?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "{table}");
        }
    }

    #[test]
    fn applies_later_migrations_in_order() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate_with(&mut conn, &MIGRATIONS[..1]).unwrap();
        let extra = [
            MIGRATIONS[0],
            "CREATE TABLE extra_a (x INTEGER);",
            "ALTER TABLE extra_a ADD COLUMN y INTEGER;",
        ];
        assert_eq!(migrate_with(&mut conn, &extra).unwrap(), 3);
        assert_eq!(schema_version(&conn).unwrap(), 3);
        conn.execute("INSERT INTO extra_a (x, y) VALUES (1, 2)", [])
            .unwrap();
    }

    #[test]
    fn refuses_newer_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        conn.execute(
            "UPDATE meta SET value = 99 WHERE key = 'schema_version'",
            [],
        )
        .unwrap();
        assert!(matches!(
            migrate(&mut conn),
            Err(Error::SchemaTooNew { found: 99, .. })
        ));
    }

    #[test]
    fn failed_migration_rolls_back() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        let bad = [MIGRATIONS[0], "CREATE TABLE ok_t (x); THIS IS NOT SQL;"];
        assert!(migrate_with(&mut conn, &bad).is_err());
        assert_eq!(schema_version(&conn).unwrap(), 1);
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'ok_t'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0);
    }
}
