//! Global coordinates for selecting bounded tactical regions, not global simulation space.
use crate::{Bounds, GeoOrigin, WorldError};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeoCoordinate {
    pub latitude: f64,
    pub longitude: f64,
}
impl GeoCoordinate {
    pub fn validate(self) -> Result<(), WorldError> {
        if !self.latitude.is_finite()
            || !self.longitude.is_finite()
            || self.latitude.abs() > 90.0
            || self.longitude.abs() > 180.0
        {
            return Err(WorldError::Invalid("invalid geographic coordinate".into()));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeoRegion {
    pub south: f64,
    pub west: f64,
    pub north: f64,
    pub east: f64,
}
impl GeoRegion {
    pub fn validate(self) -> Result<(), WorldError> {
        GeoCoordinate {
            latitude: self.south,
            longitude: self.west,
        }
        .validate()?;
        GeoCoordinate {
            latitude: self.north,
            longitude: self.east,
        }
        .validate()?;
        if self.south >= self.north || self.west >= self.east || self.east - self.west > 180.0 {
            return Err(WorldError::Invalid(
                "select a nonempty region that does not cross the date line".into(),
            ));
        }
        Ok(())
    }
    pub fn center(self) -> GeoCoordinate {
        GeoCoordinate {
            latitude: (self.south + self.north) * 0.5,
            longitude: (self.west + self.east) * 0.5,
        }
    }
    pub fn origin(self) -> GeoOrigin {
        let c = self.center();
        GeoOrigin {
            latitude: c.latitude,
            longitude: c.longitude,
        }
    }
    pub fn dimensions_m(self) -> (f64, f64) {
        let r = 6_371_000.0;
        (
            (self.east - self.west).to_radians() * r * self.center().latitude.to_radians().cos(),
            (self.north - self.south).to_radians() * r,
        )
    }
    /// Initial real-world slice: 100 to 2000 metres per side, within the core projection envelope.
    pub fn validate_battle(self) -> Result<(), WorldError> {
        self.validate()?;
        self.origin().validate()?;
        let (w, h) = self.dimensions_m();
        if !(99.999..=2000.001).contains(&w) || !(99.999..=2000.001).contains(&h) {
            return Err(WorldError::Invalid(
                "battle area must be 100-2000 metres per side (at most 4 km²)".into(),
            ));
        }
        Ok(())
    }
    pub fn local_bounds(self) -> Result<Bounds, WorldError> {
        self.validate_battle()?;
        Ok(Bounds {
            min: self.origin().project(self.south, self.west)?,
            max: self.origin().project(self.north, self.east)?,
        })
    }
    pub fn around(center: GeoCoordinate, width_m: f64, height_m: f64) -> Result<Self, WorldError> {
        center.validate()?;
        let dy = (height_m * 0.5 / 6_371_000.0).to_degrees();
        let dx = (width_m * 0.5 / (6_371_000.0 * center.latitude.to_radians().cos())).to_degrees();
        let r = Self {
            south: center.latitude - dy,
            north: center.latitude + dy,
            west: center.longitude - dx,
            east: center.longitude + dx,
        };
        r.validate_battle()?;
        Ok(r)
    }
    pub fn overpass_query(self) -> Result<String, WorldError> {
        self.validate_battle()?;
        let b = format!("{},{},{},{}", self.south, self.west, self.north, self.east);
        Ok(format!(
            "[out:json][timeout:25][maxsize:33554432];(way[building]({b});way[highway]({b});relation[building]({b}););out geom;"
        ))
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapSource {
    pub snapshot_id: String,
    pub source_hash: String,
    pub compiler_version: u32,
    pub acquired_unix_seconds: u64,
    pub region: GeoRegion,
}
pub fn valid_digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
impl MapSource {
    pub fn validate(&self, origin: GeoOrigin, bounds: Bounds) -> Result<(), WorldError> {
        if !valid_digest(&self.snapshot_id)
            || !valid_digest(&self.source_hash)
            || self.compiler_version == 0
        {
            return Err(WorldError::Invalid("invalid geographic provenance".into()));
        }
        self.region.validate_battle()?;
        let expected = self.region.local_bounds()?;
        if origin != self.region.origin()
            || expected.min.distance(bounds.min) > 0.01
            || expected.max.distance(bounds.max) > 0.01
        {
            return Err(WorldError::Invalid(
                "map coordinates do not match its geographic snapshot".into(),
            ));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn region_roundtrip() {
        let r = GeoRegion::around(
            GeoCoordinate {
                latitude: 54.2,
                longitude: 10.8,
            },
            1000.0,
            800.0,
        )
        .unwrap();
        let (w, h) = r.dimensions_m();
        assert!((w - 1000.0).abs() < 1e-6 && (h - 800.0).abs() < 1e-6);
        let p = r.origin().project(r.south, r.west).unwrap();
        let (a, b) = r.origin().unproject(p);
        assert!((a - r.south).abs() < 1e-10 && (b - r.west).abs() < 1e-10);
    }
    #[test]
    fn unsafe_regions_rejected() {
        for r in [
            GeoRegion {
                south: 1.0,
                west: 179.9,
                north: 1.01,
                east: -179.9,
            },
            GeoRegion {
                south: f64::NAN,
                west: 0.0,
                north: 1.0,
                east: 1.0,
            },
            GeoRegion {
                south: 0.0,
                west: 0.0,
                north: 2.0,
                east: 2.0,
            },
        ] {
            assert!(r.validate_battle().is_err());
        }
        assert!(!valid_digest("../../bad"));
    }
    #[test]
    fn query_is_bounded_and_numeric() {
        let r = GeoRegion::around(
            GeoCoordinate {
                latitude: 50.0,
                longitude: 0.0,
            },
            500.0,
            500.0,
        )
        .unwrap();
        assert!(r.overpass_query().unwrap().contains("[timeout:25]"));
    }
}
