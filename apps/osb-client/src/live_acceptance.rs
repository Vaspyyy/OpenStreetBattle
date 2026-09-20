//! Explicit manual provider probe. Normal CI must never run this ignored test.
use crate::geographic;
use osb_geodata::SnapshotStore;
use osb_sim::Simulation;
use osb_world::GeoRegion;
use std::sync::{Arc, atomic::AtomicBool};

#[test]
#[ignore = "explicit live-provider acceptance only; requires OSB_LIVE_TEST_ENDPOINT"]
fn live_geography_roundtrip() {
    let endpoint =
        std::env::var("OSB_LIVE_TEST_ENDPOINT").expect("explicit endpoint opt-in required");
    let output = std::path::PathBuf::from(
        std::env::var("OSB_LIVE_TEST_OUTPUT").expect("explicit output directory required"),
    );
    std::fs::create_dir_all(&output).unwrap();
    // One fixed small public urban extract, not the user's location.
    let region = GeoRegion {
        south: 52.524,
        west: 13.405,
        north: 52.528,
        east: 13.411,
    };
    let store = SnapshotStore::new(output.join("geography"));
    let mut prepared = geographic::acquire(
        &store,
        region,
        &endpoint,
        42,
        &Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let map = &prepared.snapshot.map;
    assert!(
        !map.obstacles.is_empty(),
        "expected mapped buildings in the acceptance extract"
    );
    assert!(
        !map.roads.is_empty(),
        "expected mapped roads in the acceptance extract"
    );
    let summary = serde_json::json!({
        "snapshot_id": prepared.snapshot.manifest.id,
        "region": region,
        "source_timestamp": prepared.snapshot.manifest.osm_base_timestamp,
        "buildings": map.obstacles.len(),
        "road_parts": map.roads.len(),
        "warnings": map.warnings
    });
    std::fs::write(
        output.join("scenario.json"),
        serde_json::to_vec_pretty(prepared.simulation.scenario()).unwrap(),
    )
    .unwrap();
    for _ in 0..100 {
        prepared.simulation.step().unwrap();
    }
    let save = output.join("geographic.osb");
    osb_campaign::save(&save, &prepared.simulation.snapshot()).unwrap();
    let mut restored = Simulation::restore(osb_campaign::load(&save).unwrap()).unwrap();
    for _ in 0..100 {
        prepared.simulation.step().unwrap();
        restored.step().unwrap();
    }
    assert_eq!(
        prepared.simulation.fingerprint().unwrap(),
        restored.fingerprint().unwrap()
    );
    assert_eq!(restored.soldiers().len(), 32);
    std::fs::write(
        output.join("summary.json"),
        serde_json::to_vec_pretty(&summary).unwrap(),
    )
    .unwrap();
    println!("OSB_LIVE_GEOGRAPHY_OK {summary}");
}
