use osb_geodata::compile;
use osb_world::GeoRegion;

fn region() -> GeoRegion {
    GeoRegion {
        south: 54.0,
        west: 10.0,
        north: 54.009,
        east: 10.015,
    }
}

#[test]
fn clipping_a_concave_building_keeps_disconnected_parts_separate() {
    // A U-shaped footprint joins below the selected region. The clipped result
    // must contain two legs, not a new wall across the space between them.
    let coordinates = [
        (9.999, 53.998),
        (10.016, 53.998),
        (10.016, 54.006),
        (10.012, 54.006),
        (10.012, 53.999),
        (10.003, 53.999),
        (10.003, 54.006),
        (9.999, 54.006),
        (9.999, 53.998),
    ];
    let geometry: Vec<_> = coordinates
        .into_iter()
        .map(|(lon, lat)| serde_json::json!({"lon": lon, "lat": lat}))
        .collect();
    let source = serde_json::to_vec(&serde_json::json!({
        "elements": [{"type": "way", "id": 9, "tags": {"building": "yes"}, "geometry": geometry}]
    }))
    .unwrap();
    let map = compile(&source, region()).unwrap();
    assert_eq!(map.obstacles.len(), 2);
    let middle = map.origin.project(54.004, 10.007).unwrap();
    assert!(map.walkable(middle), "clipping invented a connecting wall");
}

#[test]
fn missing_nodes_do_not_turn_into_direct_road_connections() {
    let source = br#"{"elements":[
        {"type":"node","id":1,"lat":54.004,"lon":10.001},
        {"type":"node","id":3,"lat":54.004,"lon":10.014},
        {"type":"way","id":1,"tags":{"highway":"residential"},"nodes":[1,2,3]},
        {"type":"way","id":2,"tags":{"highway":"residential"},"geometry":[{"lat":54.006,"lon":10.001},{"lat":54.006,"lon":10.014}]}
    ]}"#;
    let map = compile(source, region()).unwrap();
    assert_eq!(map.roads.len(), 1);
    assert!(map.roads[0].id.starts_with("osm-way-2-"));
    assert!(map.warnings.iter().any(|w| w.contains("skipped")));
}

#[test]
fn unsupported_water_does_not_become_verified_open_terrain() {
    let source = br#"{"elements":[{"type":"way","id":1,"tags":{"natural":"water"},"geometry":[{"lat":54.001,"lon":10.001},{"lat":54.001,"lon":10.014},{"lat":54.008,"lon":10.014},{"lat":54.001,"lon":10.001}]}]}"#;
    assert!(compile(source, region()).is_err());
}
