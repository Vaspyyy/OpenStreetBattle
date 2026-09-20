//! Compile raw OSM geometry into the *selected* bounds. Basemap tiles are never used here.
use crate::{GeoError, MAX_SOURCE_BYTES};
use geo::{BooleanOps, LineString, MultiLineString, Polygon, Rect, Validation};
use osb_world::{GeoRegion, Map, Obstacle, Point, Road};
use serde::Deserialize;
use std::collections::BTreeMap;
#[derive(Deserialize)]
struct Document {
    elements: Vec<Element>,
    remark: Option<String>,
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
    geometry: Vec<Option<Coordinate>>,
    #[serde(default)]
    tags: BTreeMap<String, String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
struct Coordinate {
    lat: f64,
    lon: f64,
}
fn valid(c: Coordinate) -> bool {
    c.lat.is_finite() && c.lon.is_finite() && c.lat.abs() <= 90.0 && c.lon.abs() <= 180.0
}

pub fn compile(source: &[u8], region: GeoRegion) -> Result<Map, GeoError> {
    region.validate_battle()?;
    if source.len() > MAX_SOURCE_BYTES {
        return Err(GeoError::Invalid(
            "geography response exceeds 32 MiB".into(),
        ));
    }
    let doc: Document = serde_json::from_slice(source)?;
    if doc.remark.as_ref().is_some_and(|s| !s.trim().is_empty()) {
        return Err(GeoError::Invalid(
            "OSM service reported an incomplete/error response; nothing was cached".into(),
        ));
    }
    if doc.elements.len() > 100_000 {
        return Err(GeoError::Invalid(
            "too many source elements; select a smaller area".into(),
        ));
    }
    let origin = region.origin();
    let bounds = region.local_bounds()?;
    let clip = Rect::new((region.west, region.south), (region.east, region.north)).to_polygon();
    let convert = |line: &LineString<f64>| -> Result<Vec<Point>, GeoError> {
        line.0
            .iter()
            .map(|p| Ok(bounds.clamp(origin.project(p.y, p.x)?)))
            .collect()
    };
    let nodes: BTreeMap<_, _> = doc
        .elements
        .iter()
        .filter(|e| e.kind == "node")
        .filter_map(|e| {
            Some((
                e.id,
                Coordinate {
                    lat: e.lat?,
                    lon: e.lon?,
                },
            ))
        })
        .collect();
    if nodes.values().any(|&c| !valid(c)) {
        return Err(GeoError::Invalid("invalid source coordinate".into()));
    }
    let mut map=Map {name:format!("Geographic battle {:.5}, {:.5}",origin.latitude,origin.longitude),origin,bounds,
        roads:Vec::new(),obstacles:Vec::new(),source:None,
        attribution:"© OpenStreetMap contributors, ODbL 1.0. Authoritative geometry: selected OSM snapshot.".into(),
        warnings:vec!["Partial terrain model: simple building ways and roads only. No water barriers, elevation, forests, building interiors or bridge topology. Unmapped and unsupported geography is UNKNOWN, not verified open ground.".into()]};
    let mut skipped = 0usize;
    let mut relations = 0usize;
    let mut vertices = 0usize;
    for e in doc.elements {
        if e.kind == "relation" {
            relations += 1;
            continue;
        }
        if e.kind != "way" {
            continue;
        }
        let building = e.tags.get("building").is_some_and(|s| s != "no");
        if !building && !e.tags.contains_key("highway") {
            continue;
        }
        let points: Option<Vec<_>> = if !e.geometry.is_empty() {
            e.geometry.into_iter().collect()
        } else {
            e.nodes.iter().map(|id| nodes.get(id).copied()).collect()
        };
        let Some(points) = points else {
            skipped += 1;
            continue;
        };
        vertices = vertices.saturating_add(points.len());
        if vertices > 500_000 || points.len() > 10_000 {
            return Err(GeoError::Invalid("source geometry budget exceeded".into()));
        }
        if points.iter().any(|&c| !valid(c)) {
            return Err(GeoError::Invalid("invalid source coordinate".into()));
        }
        if points.len() < 2 {
            skipped += 1;
            continue;
        }
        // Avoid passing a date-line-spanning segment to planar clipping.
        if points
            .windows(2)
            .any(|p| (p[0].lon - p[1].lon).abs() > 180.0)
        {
            skipped += 1;
            continue;
        }
        let line = LineString::from(points.iter().map(|c| (c.lon, c.lat)).collect::<Vec<_>>());
        if building {
            if points.len() < 4 || points.first() != points.last() {
                skipped += 1;
                continue;
            }
            let polygon = Polygon::new(line, vec![]);
            if !polygon.is_valid() {
                skipped += 1;
                continue;
            }
            for (part, p) in polygon.intersection(&clip).0.into_iter().enumerate() {
                if !p.interiors().is_empty() {
                    skipped += 1;
                    continue;
                }
                let mut v = convert(p.exterior())?;
                if v.first() == v.last() {
                    v.pop();
                }
                v.dedup();
                if v.len() < 3 {
                    continue;
                }
                map.obstacles.push(Obstacle {
                    id: format!("osm-way-{}-part-{part}", e.id),
                    vertices: v,
                });
            }
        } else {
            for (part, line) in clip
                .clip(&MultiLineString(vec![line]), false)
                .0
                .into_iter()
                .enumerate()
            {
                let mut p = convert(&line)?;
                p.dedup();
                if p.len() < 2 {
                    continue;
                }
                map.roads.push(Road {
                    id: format!("osm-way-{}-part-{part}", e.id),
                    name: e.tags.get("name").cloned().unwrap_or_default(),
                    points: p,
                });
            }
        }
        if map.obstacles.len() > 10_000 || map.roads.len() > 40_000 {
            return Err(GeoError::Invalid(
                "compiled feature budget exceeded; select a smaller area".into(),
            ));
        }
    }
    if relations > 0 {
        map.warnings.push(format!("{relations} multipolygon/building relations not simulated. Their visible map outlines are not collision geometry."));
    }
    if skipped > 0 {
        map.warnings.push(format!("{skipped} incomplete, invalid or unsupported ways/parts skipped. Check the collision overlay."));
    }
    if map.obstacles.is_empty() && map.roads.is_empty() {
        return Err(GeoError::Invalid(
            "no supported geometry in this region; refusing to invent an empty battlefield".into(),
        ));
    }
    map.obstacles.sort_by(|a, b| a.id.cmp(&b.id));
    map.roads.sort_by(|a, b| a.id.cmp(&b.id));
    map.validate()?;
    Ok(map)
}
