//! HTTP acquisition belongs to the client, never to osb-sim or the headless runner.
use osb_geodata::{GeographicSnapshot, MAX_SOURCE_BYTES, SnapshotStore};
use osb_sim::{Scenario, Simulation};
use osb_world::GeoRegion;
use std::{
    io::Read,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
pub struct PreparedBattle {
    pub simulation: Simulation,
    pub snapshot: GeographicSnapshot,
}
pub fn cache_root() -> PathBuf {
    std::env::var_os("OSB_GEO_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let base = std::env::var_os("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".cache")))
                .unwrap_or_else(|| PathBuf::from(".cache"));
            base.join("openstreetbattle/geography")
        })
}
pub fn prepare(snapshot: GeographicSnapshot, seed: u64) -> Result<PreparedBattle, String> {
    let scenario = Scenario::on_map(snapshot.map.clone()).map_err(|e| e.to_string())?;
    let simulation = Simulation::new(scenario, seed).map_err(|e| e.to_string())?;
    Ok(PreparedBattle {
        simulation,
        snapshot,
    })
}
pub fn endpoint_url(endpoint: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(endpoint).map_err(|e| e.to_string())?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if (url.scheme() != "https" && !(url.scheme() == "http" && local))
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Use an HTTPS Overpass endpoint without embedded credentials or query parameters (loopback HTTP is allowed for local servers).".into());
    }
    Ok(url)
}
pub fn acquire(
    store: &SnapshotStore,
    region: GeoRegion,
    endpoint: &str,
    seed: u64,
    cancel: &Arc<AtomicBool>,
) -> Result<PreparedBattle, String> {
    region.validate_battle().map_err(|e| e.to_string())?;
    let url = endpoint_url(endpoint)?;
    if let Some(snapshot) = store.find(region, endpoint).map_err(|e| e.to_string())? {
        return prepare(snapshot, seed);
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled".into());
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent("OpenStreetBattle/0.2 (+https://github.com/Vaspyyy/OpenStreetBattle)")
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(45))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .post(url)
        .form(&[("data", region.overpass_query().map_err(|e| e.to_string())?)])
        .send()
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        let retry = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("not specified");
        return Err(format!(
            "Map data request returned HTTP {}. Retry-After: {retry}. No automatic retries or cache changes.",
            response.status()
        ));
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_SOURCE_BYTES as u64)
    {
        return Err("geography response exceeds 32 MiB".into());
    }
    let mut bytes = Vec::new();
    response
        .take((MAX_SOURCE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err("geography response exceeds 32 MiB".into());
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled; existing battlefield unchanged".into());
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let snapshot = store
        .store(&bytes, region, endpoint, stamp)
        .map_err(|e| e.to_string())?;
    prepare(snapshot, seed)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_validation() {
        assert!(endpoint_url("https://example.com/api/interpreter").is_ok());
        for url in [
            "file:///tmp/data",
            "http://example.com",
            "https://user:secret@example.com/api",
            "https://example.com/?key=secret",
        ] {
            assert!(endpoint_url(url).is_err());
        }
    }
    #[test]
    fn cached_acquisition_works_with_no_listening_server() {
        let d = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(d.path());
        let r = GeoRegion {
            south: 54.0,
            west: 10.0,
            north: 54.009,
            east: 10.015,
        };
        let src=br#"{"elements":[{"type":"way","id":2,"tags":{"highway":"residential"},"geometry":[{"lat":54.004,"lon":10.0},{"lat":54.004,"lon":10.015}]}]}"#;
        let endpoint = "http://127.0.0.1:1/api/interpreter";
        store.store(src, r, endpoint, 100).unwrap();
        let p = acquire(&store, r, endpoint, 42, &Arc::new(AtomicBool::new(false))).unwrap();
        assert_eq!(p.simulation.soldiers().len(), 32);
        assert_eq!(
            p.simulation.scenario().map.source.as_ref().unwrap().region,
            r
        );
    }
}
