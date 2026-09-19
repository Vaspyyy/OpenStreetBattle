//! Versioned transactional checkpoints, not yet the 3.5 strategic campaign simulation.
use osb_sim::Snapshot;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::{io::Read, path::Path, time::Duration};
const APPLICATION_ID: i64 = 0x4f534231;
const DB_VERSION: i64 = 1;
const MAX_BYTES: usize = 128 * 1024 * 1024;
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("save database: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("save I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("save JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("save state: {0}")]
    Simulation(#[from] osb_sim::SimError),
    #[error("invalid save: {0}")]
    Invalid(String),
}
fn identify(connection: &Connection) -> Result<(), SaveError> {
    let app: i64 = connection.pragma_query_value(None, "application_id", |row| row.get(0))?;
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if app != APPLICATION_ID || version != DB_VERSION {
        return Err(SaveError::Invalid(format!(
            "not an OpenStreetBattle v{DB_VERSION} database (app={app}, version={version})"
        )));
    }
    Ok(())
}
pub fn save(path: impl AsRef<Path>, snapshot: &Snapshot) -> Result<(), SaveError> {
    snapshot.validate()?;
    let raw = serde_json::to_vec(snapshot)?;
    if raw.len() > MAX_BYTES {
        return Err(SaveError::Invalid(
            "checkpoint exceeds 128 MiB safety limit".into(),
        ));
    }
    let digest = blake3::hash(&raw).to_hex().to_string();
    let compressed = zstd::stream::encode_all(raw.as_slice(), 3)?;
    let mut connection = Connection::open(path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "synchronous", "FULL")?;
    let app: i64 = connection.pragma_query_value(None, "application_id", |row| row.get(0))?;
    let tables: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        [],
        |row| row.get(0),
    )?;
    if app != 0 || tables != 0 {
        identify(&connection)?;
    }
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    transaction.pragma_update(None, "application_id", APPLICATION_ID)?;
    transaction.pragma_update(None, "user_version", DB_VERSION)?;
    transaction.execute_batch("CREATE TABLE IF NOT EXISTS snapshots (id INTEGER PRIMARY KEY AUTOINCREMENT, tick TEXT NOT NULL, digest TEXT NOT NULL, payload BLOB NOT NULL);")?;
    transaction.execute(
        "INSERT INTO snapshots(tick,digest,payload) VALUES (?1,?2,?3)",
        params![snapshot.tick.to_string(), digest, compressed],
    )?;
    // Keep one previous checkpoint. Full historical replay requires a future archival event sink.
    transaction.execute(
        "DELETE FROM snapshots WHERE id NOT IN (SELECT id FROM snapshots ORDER BY id DESC LIMIT 2)",
        [],
    )?;
    transaction.commit()?;
    Ok(())
}
pub fn load(path: impl AsRef<Path>) -> Result<Snapshot, SaveError> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    identify(&connection)?;
    let row: Option<(String, Vec<u8>)> = connection
        .query_row(
            "SELECT digest,payload FROM snapshots ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((digest, payload)) = row else {
        return Err(SaveError::Invalid("database contains no checkpoint".into()));
    };
    if payload.len() > MAX_BYTES {
        return Err(SaveError::Invalid(
            "compressed checkpoint is too large".into(),
        ));
    }
    let decoder = zstd::stream::read::Decoder::new(payload.as_slice())?;
    let mut raw = Vec::new();
    decoder.take((MAX_BYTES + 1) as u64).read_to_end(&mut raw)?;
    if raw.len() > MAX_BYTES {
        return Err(SaveError::Invalid(
            "decompressed checkpoint exceeds safety limit".into(),
        ));
    }
    if blake3::hash(&raw).to_hex().as_str() != digest {
        return Err(SaveError::Invalid("checkpoint checksum mismatch".into()));
    }
    let snapshot: Snapshot = serde_json::from_slice(&raw)?;
    snapshot.validate()?;
    Ok(snapshot)
}
#[cfg(test)]
mod tests {
    use super::*;
    use osb_sim::{Scenario, Simulation};
    #[test]
    fn sqlite_resume_is_exact() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("campaign.osb");
        let mut a = Simulation::new(Scenario::demo(), 42).unwrap();
        a.advance(35).unwrap();
        save(&p, &a.snapshot()).unwrap();
        let mut b = Simulation::restore(load(&p).unwrap()).unwrap();
        a.advance(50).unwrap();
        b.advance(50).unwrap();
        assert_eq!(a.fingerprint().unwrap(), b.fingerprint().unwrap());
    }
    #[test]
    fn invalid_snapshot_does_not_replace_save() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("campaign.osb");
        let s = Simulation::new(Scenario::demo(), 42).unwrap();
        let good = s.snapshot();
        save(&p, &good).unwrap();
        let mut bad = good.clone();
        bad.schema_version = 99;
        assert!(save(&p, &bad).is_err());
        assert_eq!(load(&p).unwrap(), good);
    }
    #[test]
    fn rejects_unrelated_database() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("other.sqlite");
        let c = Connection::open(&p).unwrap();
        c.execute("CREATE TABLE unrelated (id INTEGER)", [])
            .unwrap();
        let s = Simulation::new(Scenario::demo(), 42).unwrap();
        assert!(save(&p, &s.snapshot()).is_err());
        assert!(load(&p).is_err());
    }
    #[test]
    fn verifies_digest() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("bad.osb");
        let s = Simulation::new(Scenario::demo(), 42).unwrap();
        save(&p, &s.snapshot()).unwrap();
        let c = Connection::open(&p).unwrap();
        c.execute("UPDATE snapshots SET digest='broken'", [])
            .unwrap();
        assert!(load(&p).is_err());
    }
    #[test]
    fn retains_two_checkpoints() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("save.osb");
        let mut s = Simulation::new(Scenario::demo(), 42).unwrap();
        for _ in 0..3 {
            s.step().unwrap();
            save(&p, &s.snapshot()).unwrap();
        }
        let c = Connection::open(&p).unwrap();
        let count: i64 = c
            .query_row("SELECT COUNT(*) FROM snapshots", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
        assert_eq!(load(&p).unwrap().tick, 3);
    }
    #[test]
    fn missing_save_is_not_created() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("absent.osb");
        assert!(load(&p).is_err());
        assert!(!p.exists());
    }
}
