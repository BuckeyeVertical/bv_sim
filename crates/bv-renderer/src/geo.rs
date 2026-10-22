//! The SUAS field's geographic origin and local tangent-plane projection.

/// WGS84 latitude/longitude in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeoPoint {
    pub latitude: f64,
    pub longitude: f64,
}

impl GeoPoint {
    pub const fn new(latitude: f64, longitude: f64) -> Self {
        Self {
            latitude,
            longitude,
        }
    }
}

pub const SUAS_2026_FIELD: GeoPoint = GeoPoint::new(36.214_675, -96.006_555_56);
pub const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// Horizontal Bevy (X, Z) meters anchored at the geographic origin:
/// north = -X, east = -Z. This preserves the original SUAS layout projection.
#[derive(Clone, Copy, Debug)]
pub struct WorldGeography {
    pub home: GeoPoint,
}

impl WorldGeography {
    pub fn validate(self) -> Result<(), String> {
        if !self.home.latitude.is_finite()
            || self.home.latitude.abs() > 89.0
            || !self.home.longitude.is_finite()
            || self.home.longitude.abs() > 180.0
        {
            return Err(
                "Home requires finite latitude within ±89° and longitude within ±180°.".to_owned(),
            );
        }
        Ok(())
    }

    pub fn to_geo(self, position: bevy::math::DVec2) -> GeoPoint {
        let latitude = self.home.latitude.to_radians();
        GeoPoint::new(
            (latitude - position.x / EARTH_RADIUS_M).to_degrees(),
            self.home.longitude - (position.y / (EARTH_RADIUS_M * latitude.cos())).to_degrees(),
        )
    }

    pub fn to_world(self, point: GeoPoint) -> bevy::math::DVec2 {
        let north = (point.latitude - self.home.latitude).to_radians() * EARTH_RADIUS_M;
        let east = (point.longitude - self.home.longitude).to_radians()
            * EARTH_RADIUS_M
            * self.home.latitude.to_radians().cos();
        bevy::math::DVec2::new(-north, -east)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::DVec2;

    #[test]
    fn origin_axes_and_round_trips_preserve_local_coordinates() {
        let geo = WorldGeography {
            home: SUAS_2026_FIELD,
        };
        let origin = geo.to_geo(DVec2::ZERO);
        assert!((origin.latitude - geo.home.latitude).abs() < 1e-12);
        assert_eq!(origin.longitude, geo.home.longitude);
        assert!(geo.to_geo(DVec2::new(-100.0, 0.0)).latitude > geo.home.latitude);
        assert!(geo.to_geo(DVec2::new(0.0, -100.0)).longitude > geo.home.longitude);
        for position in [DVec2::new(-425.3, 321.7), DVec2::new(1800.0, -2500.0)] {
            assert!(geo.to_world(geo.to_geo(position)).distance(position) < 1e-7);
        }
    }

    #[test]
    fn known_suas_corner_matches_original_world_layout() {
        let geo = WorldGeography {
            home: SUAS_2026_FIELD,
        };
        let point = geo.to_world(GeoPoint::new(36.216341, -96.010424));
        assert!((point.x + 185.2507477899).abs() < 1e-6);
        assert!((point.y - 347.0496258167).abs() < 1e-6);
    }
}
