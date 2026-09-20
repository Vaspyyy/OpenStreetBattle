//! Authoritative map geometry. No renderer, network access or screen coordinates.
mod geography;
pub use geography::{GeoCoordinate, GeoRegion, MapSource, valid_digest};
mod nav;
mod osm;
pub use nav::Navigation;
pub use osm::import_osm_json;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorldError {
    #[error("invalid map: {0}")]
    Invalid(String),
    #[error("invalid OSM JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Local east/north position in metres. Only presentation converts this to pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
    pub fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
    pub fn toward(self, other: Self, distance: f64) -> Self {
        let d = self.distance(other);
        if d <= distance || d < 1e-9 {
            other
        } else {
            Self::new(
                self.x + (other.x - self.x) * distance / d,
                self.y + (other.y - self.y) * distance / d,
            )
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeoOrigin {
    pub latitude: f64,
    pub longitude: f64,
}
impl GeoOrigin {
    pub fn validate(self) -> Result<(), WorldError> {
        if !self.latitude.is_finite()
            || !self.longitude.is_finite()
            || self.latitude.abs() > 80.0
            || self.longitude.abs() > 180.0
        {
            return Err(WorldError::Invalid(
                "local projection requires latitude within +/-80 and longitude within +/-180"
                    .into(),
            ));
        }
        Ok(())
    }
    /// Bounded local equirectangular projection, not a global routing coordinate system.
    pub fn project(self, latitude: f64, longitude: f64) -> Result<Point, WorldError> {
        self.validate()?;
        if !latitude.is_finite()
            || !longitude.is_finite()
            || latitude.abs() > 90.0
            || longitude.abs() > 180.0
        {
            return Err(WorldError::Invalid(
                "non-finite or out-of-range geographic coordinate".into(),
            ));
        }
        let p = Point::new(
            (longitude - self.longitude).to_radians()
                * 6_371_000.0
                * self.latitude.to_radians().cos(),
            (latitude - self.latitude).to_radians() * 6_371_000.0,
        );
        if p.x.abs() > 20_000.0 || p.y.abs() > 20_000.0 {
            return Err(WorldError::Invalid(
                "extract exceeds the local projection envelope; split it into regional chunks"
                    .into(),
            ));
        }
        Ok(p)
    }
    pub fn unproject(self, p: Point) -> (f64, f64) {
        (
            self.latitude + (p.y / 6_371_000.0).to_degrees(),
            self.longitude + (p.x / (6_371_000.0 * self.latitude.to_radians().cos())).to_degrees(),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}
impl Bounds {
    pub fn contains(self, p: Point) -> bool {
        p.finite()
            && p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
    }
    pub fn center(self) -> Point {
        Point::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
        )
    }
    pub fn clamp(self, p: Point) -> Point {
        Point::new(
            p.x.clamp(self.min.x, self.max.x),
            p.y.clamp(self.min.y, self.max.y),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Obstacle {
    pub id: String,
    pub vertices: Vec<Point>,
}
impl Obstacle {
    pub fn rectangle(id: impl Into<String>, min: Point, max: Point) -> Self {
        Self {
            id: id.into(),
            vertices: vec![min, Point::new(max.x, min.y), max, Point::new(min.x, max.y)],
        }
    }
    pub fn contains(&self, p: Point) -> bool {
        let mut inside = false;
        for (a, b) in self.edges() {
            if point_on_segment(p, a, b) {
                return true;
            }
            if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
                inside = !inside;
            }
        }
        inside
    }
    pub fn edges(&self) -> impl Iterator<Item = (Point, Point)> + '_ {
        self.vertices
            .iter()
            .copied()
            .zip(self.vertices.iter().copied().cycle().skip(1))
            .take(self.vertices.len())
    }
    pub fn blocks(&self, a: Point, b: Point) -> bool {
        self.contains(a)
            || self.contains(b)
            || self.edges().any(|(c, d)| segments_intersect(a, b, c, d))
    }
}
fn cross(a: Point, b: Point, c: Point) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}
fn point_on_segment(p: Point, a: Point, b: Point) -> bool {
    cross(a, b, p).abs() < 1e-7
        && p.x >= a.x.min(b.x) - 1e-7
        && p.x <= a.x.max(b.x) + 1e-7
        && p.y >= a.y.min(b.y) - 1e-7
        && p.y <= a.y.max(b.y) + 1e-7
}
fn segments_intersect(a: Point, b: Point, c: Point, d: Point) -> bool {
    let (x, y, u, v) = (
        cross(a, b, c),
        cross(a, b, d),
        cross(c, d, a),
        cross(c, d, b),
    );
    ((x > 0.0 && y < 0.0 || x < 0.0 && y > 0.0) && (u > 0.0 && v < 0.0 || u < 0.0 && v > 0.0))
        || point_on_segment(c, a, b)
        || point_on_segment(d, a, b)
        || point_on_segment(a, c, d)
        || point_on_segment(b, c, d)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Road {
    pub id: String,
    pub name: String,
    pub points: Vec<Point>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Map {
    pub name: String,
    pub origin: GeoOrigin,
    pub bounds: Bounds,
    pub obstacles: Vec<Obstacle>,
    pub roads: Vec<Road>,
    pub attribution: String,
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<MapSource>,
}
impl Map {
    pub fn validate(&self) -> Result<(), WorldError> {
        self.origin.validate()?;
        if let Some(source) = &self.source {
            source.validate(self.origin, self.bounds)?;
        }
        let b = self.bounds;
        if !b.min.finite()
            || !b.max.finite()
            || b.max.x <= b.min.x
            || b.max.y <= b.min.y
            || b.max.x - b.min.x > 20_000.0
            || b.max.y - b.min.y > 20_000.0
        {
            return Err(WorldError::Invalid(
                "bounds must be finite, positive and no wider than 20 km per axis".into(),
            ));
        }
        if self.obstacles.len() > 100_000 || self.roads.len() > 100_000 {
            return Err(WorldError::Invalid("too many geographic features".into()));
        }
        for o in &self.obstacles {
            if o.vertices.len() < 3
                || o.vertices.len() > 100_000
                || o.vertices.iter().any(|&p| !b.contains(p))
            {
                return Err(WorldError::Invalid(format!("invalid polygon {}", o.id)));
            }
            let area: f64 = o.edges().map(|(a, b)| a.x * b.y - b.x * a.y).sum();
            if area.abs() < 1e-6 {
                return Err(WorldError::Invalid(format!("zero-area polygon {}", o.id)));
            }
        }
        for r in &self.roads {
            if r.points.len() < 2 || r.points.iter().any(|&p| !b.contains(p)) {
                return Err(WorldError::Invalid(format!("invalid road {}", r.id)));
            }
        }
        Ok(())
    }
    pub fn walkable(&self, p: Point) -> bool {
        self.bounds.contains(p) && !self.obstacles.iter().any(|o| o.contains(p))
    }
    pub fn clear_segment(&self, a: Point, b: Point) -> bool {
        self.bounds.contains(a)
            && self.bounds.contains(b)
            && !self.obstacles.iter().any(|o| o.blocks(a, b))
    }
    pub fn visible(&self, a: Point, b: Point, range: f64) -> bool {
        a.distance(b) <= range && self.clear_segment(a, b)
    }
    pub fn demo() -> Self {
        let rect =
            |id, x, y, w, h| Obstacle::rectangle(id, Point::new(x, y), Point::new(x + w, y + h));
        Self {
            name:"First Contact: fictional test village".into(),
            source: None,
            origin:GeoOrigin {latitude:0.0,longitude:0.0},
            bounds:Bounds {min:Point::new(0.0,0.0),max:Point::new(900.0,600.0)},
            obstacles:vec![rect("west-north",180.0,355.0,130.0,85.0),rect("west-south",180.0,150.0,130.0,85.0),rect("center",405.0,225.0,90.0,150.0),rect("east-north",590.0,355.0,130.0,85.0),rect("east-south",590.0,150.0,130.0,85.0)],
            roads:vec![Road {id:"east-west".into(),name:"Crossing Street".into(),points:vec![Point::new(0.0,300.0),Point::new(350.0,300.0),Point::new(350.0,200.0),Point::new(550.0,200.0),Point::new(550.0,300.0),Point::new(900.0,300.0)]},Road {id:"north".into(),name:"North Street".into(),points:vec![Point::new(350.0,0.0),Point::new(350.0,600.0)]}],
            attribution:"Hand-authored synthetic fixture. NOT a real-world map.".into(),
            warnings:vec!["2D opaque building footprints only. No elevation, interiors, vegetation or bridge semantics yet.".into()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn demo_is_valid() {
        Map::demo().validate().unwrap();
    }
    #[test]
    fn buildings_block_sight() {
        let m = Map::demo();
        assert!(!m.visible(Point::new(350.0, 300.0), Point::new(550.0, 300.0), 500.0));
    }
    #[test]
    fn open_street_has_sight() {
        let m = Map::demo();
        assert!(m.visible(Point::new(100.0, 100.0), Point::new(400.0, 100.0), 500.0));
    }
    #[test]
    fn range_is_not_omniscience() {
        let m = Map::demo();
        assert!(!m.visible(Point::new(0.0, 0.0), Point::new(800.0, 0.0), 100.0));
    }
    #[test]
    fn boundary_is_blocking() {
        let o = Obstacle::rectangle("x", Point::new(1.0, 1.0), Point::new(2.0, 2.0));
        assert!(o.blocks(Point::new(0.0, 1.0), Point::new(3.0, 1.0)));
    }
    #[test]
    fn inside_is_blocked() {
        let o = Obstacle::rectangle("x", Point::new(1.0, 1.0), Point::new(2.0, 2.0));
        assert!(o.blocks(Point::new(1.5, 1.5), Point::new(4.0, 4.0)));
    }
    #[test]
    fn projection_roundtrip() {
        let g = GeoOrigin {
            latitude: 54.0,
            longitude: 10.0,
        };
        let p = g.project(54.005, 10.009).unwrap();
        let (lat, lon) = g.unproject(p);
        assert!((lat - 54.005).abs() < 1e-10 && (lon - 10.009).abs() < 1e-10);
    }
    #[test]
    fn rejects_global_projection() {
        assert!(
            GeoOrigin {
                latitude: 0.0,
                longitude: 0.0
            }
            .project(40.0, 40.0)
            .is_err()
        );
    }
    #[test]
    fn rejects_nan_geometry() {
        let mut m = Map::demo();
        m.bounds.max.x = f64::NAN;
        assert!(m.validate().is_err());
    }
}
