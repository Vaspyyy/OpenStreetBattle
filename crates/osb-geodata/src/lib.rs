//! Offline, content-addressed geographic snapshots. No HTTP or renderer dependency.
mod compile;
pub use compile::compile;
use osb_world::{GeoRegion, Map, MapSource, valid_digest};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
pub const MAX_SOURCE_BYTES: usize = 32 * 1024 * 1024;
pub const COMPILER_VERSION: u32 = 1;
const SCHEMA: u32 = 1;
#[derive(Debug, thiserror::Error)]
pub enum GeoError {
    #[error("geography: {0}")]
    Invalid(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    World(#[from] osb_world::WorldError),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub compiler_version: u32,
    pub id: String,
    pub region: GeoRegion,
    pub source_hash: String,
    pub compiled_hash: String,
    pub acquired_unix_seconds: u64,
    pub source_endpoint: String,
    pub osm_base_timestamp: Option<String>,
    pub attribution: String,
}
#[derive(Clone, Debug)]
pub struct GeographicSnapshot {
    pub manifest: Manifest,
    pub map: Map,
}
fn hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}
fn identity(source_hash: &str, region: GeoRegion, endpoint: &str) -> Result<String, GeoError> {
    Ok(hash(&serde_json::to_vec(&(
        SCHEMA,
        COMPILER_VERSION,
        region,
        source_hash,
        endpoint,
    ))?))
}
pub fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, GeoError> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(GeoError::Invalid(format!(
            "{} exceeds input size limit",
            path.display()
        )));
    }
    Ok(bytes)
}
fn durable_write(path: &Path, data: &[u8]) -> Result<(), GeoError> {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    f.write_all(data)?;
    f.sync_all()?;
    Ok(())
}
#[derive(Clone, Debug)]
pub struct SnapshotStore {
    root: PathBuf,
}
impl SnapshotStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    fn path(&self, id: &str) -> Result<PathBuf, GeoError> {
        if !valid_digest(id) {
            return Err(GeoError::Invalid("invalid snapshot identifier".into()));
        }
        Ok(self.root.join(id))
    }
    /// Cache compiled geometry only after complete validation, in an atomic directory rename.
    pub fn store(
        &self,
        source: &[u8],
        region: GeoRegion,
        endpoint: &str,
        acquired: u64,
    ) -> Result<GeographicSnapshot, GeoError> {
        if endpoint.len() > 2048 {
            return Err(GeoError::Invalid("source endpoint too long".into()));
        }
        let mut map = compile(source, region)?;
        let source_hash = hash(source);
        let id = identity(&source_hash, region, endpoint)?;
        let path = self.path(&id)?;
        if path.exists() {
            return self.load(&id);
        }
        map.source = Some(MapSource {
            snapshot_id: id.clone(),
            source_hash: source_hash.clone(),
            compiler_version: COMPILER_VERSION,
            acquired_unix_seconds: acquired,
            region,
        });
        map.validate()?;
        let compiled = serde_json::to_vec(&map)?;
        let doc: serde_json::Value = serde_json::from_slice(source)?;
        let manifest = Manifest {
            schema: SCHEMA,
            compiler_version: COMPILER_VERSION,
            id: id.clone(),
            region,
            source_hash,
            compiled_hash: hash(&compiled),
            acquired_unix_seconds: acquired,
            source_endpoint: endpoint.into(),
            osm_base_timestamp: doc
                .pointer("/osm3s/timestamp_osm_base")
                .and_then(|s| s.as_str())
                .map(str::to_owned),
            attribution: map.attribution.clone(),
        };
        fs::create_dir_all(&self.root)?;
        let staging = tempfile::Builder::new()
            .prefix(".pending-")
            .tempdir_in(&self.root)?;
        durable_write(&staging.path().join("source.json"), source)?;
        durable_write(&staging.path().join("compiled.json"), &compiled)?;
        durable_write(
            &staging.path().join("manifest.json"),
            &serde_json::to_vec_pretty(&manifest)?,
        )?;
        durable_write(
            &staging.path().join("attribution.txt"),
            manifest.attribution.as_bytes(),
        )?;
        fs::File::open(staging.path())?.sync_all()?;
        if let Err(e) = fs::rename(staging.path(), &path) {
            if path.exists() {
                return self.load(&id);
            }
            return Err(e.into());
        }
        fs::File::open(&self.root)?.sync_all()?;
        // The temporary path no longer exists; TempDir cleanup is harmless.
        self.load(&id)
    }
    pub fn load(&self, id: &str) -> Result<GeographicSnapshot, GeoError> {
        let path = self.path(id)?;
        let manifest: Manifest =
            serde_json::from_slice(&read_bounded(&path.join("manifest.json"), 64 * 1024)?)?;
        if manifest.schema != SCHEMA
            || manifest.compiler_version != COMPILER_VERSION
            || manifest.id != id
        {
            return Err(GeoError::Invalid(
                "unsupported or mismatched geography snapshot schema".into(),
            ));
        }
        manifest.region.validate_battle()?;
        let source = read_bounded(&path.join("source.json"), MAX_SOURCE_BYTES)?;
        let compiled = read_bounded(&path.join("compiled.json"), 64 * 1024 * 1024)?;
        if hash(&source) != manifest.source_hash
            || hash(&compiled) != manifest.compiled_hash
            || identity(
                &manifest.source_hash,
                manifest.region,
                &manifest.source_endpoint,
            )? != id
        {
            return Err(GeoError::Invalid(
                "geographic snapshot integrity check failed".into(),
            ));
        }
        let map: Map = serde_json::from_slice(&compiled)?;
        map.validate()?;
        let expected = MapSource {
            snapshot_id: id.into(),
            source_hash: manifest.source_hash.clone(),
            compiler_version: COMPILER_VERSION,
            acquired_unix_seconds: manifest.acquired_unix_seconds,
            region: manifest.region,
        };
        if map.source.as_ref() != Some(&expected) {
            return Err(GeoError::Invalid(
                "compiled map provenance does not match manifest".into(),
            ));
        }
        Ok(GeographicSnapshot { manifest, map })
    }
    /// Metadata listing is bounded and does not trust it as playable geometry. Load verifies hashes.
    pub fn list(&self) -> Result<Vec<Manifest>, GeoError> {
        if !self.root.exists() {
            return Ok(vec![]);
        }
        let mut out = Vec::new();
        for entry in fs::read_dir(&self.root)?.take(2048) {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if !valid_digest(&name) || !entry.file_type()?.is_dir() {
                continue;
            }
            if let Ok(bytes) = read_bounded(&entry.path().join("manifest.json"), 64 * 1024)
                && let Ok(m) = serde_json::from_slice::<Manifest>(&bytes)
                && m.id == name
                && m.schema == SCHEMA
            {
                out.push(m);
            }
        }
        out.sort_by(|a, b| {
            b.acquired_unix_seconds
                .cmp(&a.acquired_unix_seconds)
                .then(a.id.cmp(&b.id))
        });
        Ok(out)
    }
    pub fn find(
        &self,
        region: GeoRegion,
        endpoint: &str,
    ) -> Result<Option<GeographicSnapshot>, GeoError> {
        for m in self.list()? {
            if m.region == region && m.source_endpoint == endpoint {
                return self.load(&m.id).map(Some);
            }
        }
        Ok(None)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn region() -> GeoRegion {
        GeoRegion {
            south: 54.0,
            west: 10.0,
            north: 54.009,
            east: 10.015,
        }
    }
    fn source() -> Vec<u8> {
        br#"{"osm3s":{"timestamp_osm_base":"2026-01-01T00:00:00Z"},"elements":[{"type":"way","id":1,"tags":{"building":"yes"},"geometry":[{"lat":54.001,"lon":10.001},{"lat":54.001,"lon":10.002},{"lat":54.002,"lon":10.002},{"lat":54.002,"lon":10.001},{"lat":54.001,"lon":10.001}]},{"type":"way","id":2,"tags":{"highway":"residential"},"geometry":[{"lat":54.004,"lon":9.0},{"lat":54.004,"lon":11.0}]}]}"#.to_vec()
    }
    #[test]
    fn source_roundtrip_and_offline_reuse() {
        let d = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(d.path());
        let a = store.store(&source(), region(), "fixture", 100).unwrap();
        let b = store.load(&a.manifest.id).unwrap();
        assert_eq!(a.map, b.map);
        assert_eq!(store.list().unwrap().len(), 1);
        assert_eq!(
            store
                .store(&source(), region(), "fixture", 200)
                .unwrap()
                .manifest
                .acquired_unix_seconds,
            100
        );
        assert!(store.find(region(), "fixture").unwrap().is_some());
    }
    #[test]
    fn roads_are_clipped_not_used_to_expand_battle_bounds() {
        let map = compile(&source(), region()).unwrap();
        assert_eq!(map.bounds, region().local_bounds().unwrap());
        assert_eq!(map.roads.len(), 1);
        assert!(map.roads[0].points.iter().all(|&p| map.bounds.contains(p)));
    }
    #[test]
    fn corrupt_compiled_geometry_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(d.path());
        let a = store.store(&source(), region(), "fixture", 100).unwrap();
        fs::write(d.path().join(&a.manifest.id).join("compiled.json"), b"{}").unwrap();
        assert!(store.load(&a.manifest.id).is_err());
        assert!(store.store(&source(), region(), "fixture", 200).is_err());
    }
    #[test]
    fn error_responses_do_not_create_cache_entries() {
        let d = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(d.path());
        assert!(
            store
                .store(
                    br#"{"remark":"runtime error: timed out","elements":[]}"#,
                    region(),
                    "fixture",
                    100
                )
                .is_err()
        );
        assert!(store.list().unwrap().is_empty());
        assert!(store.load("../source").is_err());
    }
    #[test]
    fn region_changes_snapshot_identity() {
        let d = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(d.path());
        let a = store.store(&source(), region(), "fixture", 100).unwrap();
        let mut r = region();
        r.north -= 0.001;
        let b = store.store(&source(), r, "fixture", 100).unwrap();
        assert_ne!(a.manifest.id, b.manifest.id);
    }
}
