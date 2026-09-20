//! North-up Web Mercator browsing camera. Never used to compute simulation distances.
use osb_world::{GeoCoordinate, Point};
pub const MERCATOR_LIMIT: f64 = 85.0511287798066;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobeCamera {
    pub center: GeoCoordinate,
    pub zoom: f64,
}
fn unit(c: GeoCoordinate) -> Point {
    let lat = c
        .latitude
        .clamp(-MERCATOR_LIMIT, MERCATOR_LIMIT)
        .to_radians();
    Point::new(
        (c.longitude + 180.0) / 360.0,
        (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / std::f64::consts::PI) * 0.5,
    )
}
fn geo(p: Point) -> GeoCoordinate {
    GeoCoordinate {
        longitude: (p.x * 360.0).rem_euclid(360.0) - 180.0,
        latitude: (std::f64::consts::PI * (1.0 - 2.0 * p.y.clamp(0.0, 1.0)))
            .sinh()
            .atan()
            .to_degrees(),
    }
}
impl Default for GlobeCamera {
    fn default() -> Self {
        Self {
            center: GeoCoordinate {
                latitude: 52.5,
                longitude: 10.0,
            },
            zoom: 6.0,
        }
    }
}
impl GlobeCamera {
    pub fn scale(self) -> f64 {
        512.0 * 2f64.powf(self.zoom)
    }
    pub fn screen_to_geo(self, p: Point, viewport_center: Point) -> GeoCoordinate {
        let c = unit(self.center);
        let k = self.scale();
        geo(Point::new(
            c.x + (p.x - viewport_center.x) / k,
            c.y + (p.y - viewport_center.y) / k,
        ))
    }
    pub fn geo_to_screen(self, c: GeoCoordinate, viewport_center: Point) -> Point {
        let p = unit(c);
        let origin = unit(self.center);
        let k = self.scale();
        let dx = (p.x - origin.x + 0.5).rem_euclid(1.0) - 0.5;
        Point::new(
            viewport_center.x + dx * k,
            viewport_center.y + (p.y - origin.y) * k,
        )
    }
    pub fn pan(&mut self, delta: Point) {
        if delta.finite() {
            self.center = self.screen_to_geo(Point::new(-delta.x, -delta.y), Point::new(0.0, 0.0));
        }
    }
    pub fn zoom_at(&mut self, delta: f64, cursor: Point, center: Point) {
        if !delta.is_finite() || !cursor.finite() || !center.finite() {
            return;
        }
        let anchor = self.screen_to_geo(cursor, center);
        self.zoom = (self.zoom + delta).clamp(1.0, 19.0);
        let shifted = self.geo_to_screen(anchor, center);
        self.pan(Point::new(cursor.x - shifted.x, cursor.y - shifted.y));
    }
    pub fn jump(&mut self, c: GeoCoordinate, zoom: f64) -> bool {
        if c.validate().is_err() || !zoom.is_finite() {
            return false;
        }
        self.center = GeoCoordinate {
            latitude: c.latitude.clamp(-MERCATOR_LIMIT, MERCATOR_LIMIT),
            ..c
        };
        self.zoom = zoom.clamp(1.0, 19.0);
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn screen_roundtrip() {
        let camera = GlobeCamera {
            center: GeoCoordinate {
                latitude: 54.2,
                longitude: 10.8,
            },
            zoom: 16.0,
        };
        let center = Point::new(500.0, 300.0);
        let p = Point::new(340.0, 220.0);
        assert!(
            camera
                .geo_to_screen(camera.screen_to_geo(p, center), center)
                .distance(p)
                < 1e-6
        );
    }
    #[test]
    fn cursor_zoom_anchor() {
        let mut c = GlobeCamera::default();
        let p = Point::new(120.0, 75.0);
        let center = Point::new(300.0, 200.0);
        let before = c.screen_to_geo(p, center);
        c.zoom_at(1.0, p, center);
        assert!(c.geo_to_screen(before, center).distance(p) < 1e-6);
    }
    #[test]
    fn mercator_scale_and_wrap() {
        let c = GlobeCamera {
            center: GeoCoordinate {
                latitude: 0.0,
                longitude: 0.0,
            },
            zoom: 2.0,
        };
        assert!(
            (c.geo_to_screen(
                GeoCoordinate {
                    latitude: 0.0,
                    longitude: 90.0
                },
                Point::new(0.0, 0.0)
            )
            .x - 512.0)
                .abs()
                < 1e-8
        );
        let mut c = GlobeCamera::default();
        assert!(!c.jump(
            GeoCoordinate {
                latitude: f64::NAN,
                longitude: 0.0
            },
            2.0
        ));
    }
}
