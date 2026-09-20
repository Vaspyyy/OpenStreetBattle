//! Import an existing small OSM/Overpass JSON export. Never queries community servers.
use crate::{Bounds, GeoOrigin, Map, Obstacle, Point, Road, WorldError};
use serde::Deserialize;
use std::collections::BTreeMap;
#[derive(Deserialize)]
struct Document {
    elements: Vec<Element>,
}
#[derive(Deserialize)]
struct Element {
    #[serde(rename = "type")]
    kind: String,
    id: i64,
    lat: Option<f64>,
    lon: Option<f64>,
    #[serde(default)]
    nodes: Vec<i64>,
    #[serde(default)]
    geometry: Vec<Coordinate>,
    #[serde(default)]
    tags: BTreeMap<String, String>,
}
#[derive(Deserialize)]
struct Coordinate {
    lat: f64,
    lon: f64,
}

pub fn import_osm_json(input: &str) -> Result<Map, WorldError> {
    if input.len() > 32 * 1024 * 1024 {
        return Err(WorldError::Invalid(
            "OSM input exceeds 32 MiB; import a smaller extract".into(),
        ));
    }
    let doc: Document = serde_json::from_str(input)?;
    if doc.elements.len() > 500_000 {
        return Err(WorldError::Invalid("too many OSM elements".into()));
    }
    let mut nodes = BTreeMap::new();
    let mut all = Vec::new();
    for e in &doc.elements {
        if e.kind == "node"
            && let (Some(lat), Some(lon)) = (e.lat, e.lon)
        {
            nodes.insert(e.id, (lat, lon));
            all.push((lat, lon));
        }
        all.extend(e.geometry.iter().map(|c| (c.lat, c.lon)));
    }
    if all.is_empty() {
        return Err(WorldError::Invalid(
            "no coordinates found; export nodes or use Overpass out geom".into(),
        ));
    }
    if all
        .iter()
        .any(|(a, b)| !a.is_finite() || !b.is_finite() || a.abs() > 90.0 || b.abs() > 180.0)
    {
        return Err(WorldError::Invalid("invalid OSM coordinate".into()));
    }
    let mut low = (90f64, 180f64);
    let mut high = (-90f64, -180f64);
    for &(lat, lon) in &all {
        low.0 = low.0.min(lat);
        low.1 = low.1.min(lon);
        high.0 = high.0.max(lat);
        high.1 = high.1.max(lon);
    }
    let origin = GeoOrigin {
        latitude: (low.0 + high.0) * 0.5,
        longitude: (low.1 + high.1) * 0.5,
    };
    origin.validate()?;
    let a = origin.project(low.0, low.1)?;
    let b = origin.project(high.0, high.1)?;
    let mut map=Map{source:None,name:"Imported OSM extract".into(),origin,bounds:Bounds{min:Point::new(a.x-40.0,a.y-40.0),max:Point::new(b.x+40.0,b.y+40.0)},obstacles:Vec::new(),roads:Vec::new(),attribution:"© OpenStreetMap contributors, ODbL 1.0. Source: user-supplied OSM JSON extract.".into(),warnings:vec!["Only simple closed building ways and highway polylines are imported. Missing features are UNKNOWN, not verified open terrain. No interiors, height/elevation, walls, rivers, vegetation, bridges or multipolygon relations yet.".into()]};
    let mut unsupported_relations = 0;
    let mut incomplete = 0;
    for e in &doc.elements {
        if e.kind == "relation" {
            unsupported_relations += 1;
            continue;
        }
        if e.kind != "way" {
            continue;
        }
        let building = e.tags.get("building").is_some_and(|s| s != "no");
        let road = e.tags.contains_key("highway");
        if !building && !road {
            continue;
        }
        let raw: Option<Vec<(f64, f64)>> = if !e.geometry.is_empty() {
            Some(e.geometry.iter().map(|c| (c.lat, c.lon)).collect())
        } else {
            e.nodes.iter().map(|id| nodes.get(id).copied()).collect()
        };
        let Some(raw) = raw else {
            incomplete += 1;
            continue;
        };
        let mut points = raw
            .into_iter()
            .map(|(a, b)| origin.project(a, b))
            .collect::<Result<Vec<_>, _>>()?;
        if building {
            if points.len() < 4 || points.first() != points.last() {
                incomplete += 1;
                continue;
            }
            points.pop();
            map.obstacles.push(Obstacle {
                id: format!("osm-way-{}", e.id),
                vertices: points,
            });
        } else if points.len() >= 2 {
            map.roads.push(Road {
                id: format!("osm-way-{}", e.id),
                name: e.tags.get("name").cloned().unwrap_or_default(),
                points,
            });
        }
    }
    if unsupported_relations > 0 {
        map.warnings.push(format!("Skipped {unsupported_relations} unsupported relations; this extract is not a complete terrain model."));
    }
    if incomplete > 0 {
        map.warnings
            .push(format!("Skipped {incomplete} incomplete or unclosed ways."));
    }
    if map.obstacles.is_empty() && map.roads.is_empty() {
        return Err(WorldError::Invalid(
            "no supported complete building or road geometry".into(),
        ));
    }
    map.validate()?;
    Ok(map)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imports_geometry_export() {
        let m=import_osm_json(r#"{"elements":[{"type":"way","id":1,"tags":{"building":"yes"},"geometry":[{"lat":54.0,"lon":10.0},{"lat":54.0,"lon":10.001},{"lat":54.001,"lon":10.001},{"lat":54.001,"lon":10.0},{"lat":54.0,"lon":10.0}]}]}"#).unwrap();
        assert_eq!(m.obstacles.len(), 1);
        assert!(m.attribution.contains("OpenStreetMap"));
    }
    #[test]
    fn missing_coordinates_rejected() {
        assert!(import_osm_json(r#"{"elements":[]}"#).is_err());
    }
    #[test]
    fn incomplete_ways_do_not_become_invisible_walls() {
        assert!(import_osm_json(r#"{"elements":[{"type":"node","id":1,"lat":54.0,"lon":10.0},{"type":"way","id":2,"tags":{"building":"yes"},"nodes":[1,2,3,1]}]}"#).is_err());
    }
}
