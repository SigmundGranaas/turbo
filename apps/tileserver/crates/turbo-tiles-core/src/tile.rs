use serde::Deserialize;
use std::fmt;

/// Web-Mercator tile coordinates (z/x/y). Validated against
/// `0 <= x,y < 2^z` and `z <= 22` (over-zoom is meaningless past 22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub struct TileCoord {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum TileCoordError {
    #[error("zoom level {0} exceeds maximum 22")]
    ZoomTooHigh(u8),
    #[error("x={x} out of range for zoom {z}")]
    XOutOfRange { z: u8, x: u32 },
    #[error("y={y} out of range for zoom {z}")]
    YOutOfRange { z: u8, y: u32 },
}

impl TileCoord {
    pub fn new(z: u8, x: u32, y: u32) -> Result<Self, TileCoordError> {
        if z > 22 {
            return Err(TileCoordError::ZoomTooHigh(z));
        }
        let limit = 1u32 << z;
        if x >= limit {
            return Err(TileCoordError::XOutOfRange { z, x });
        }
        if y >= limit {
            return Err(TileCoordError::YOutOfRange { z, y });
        }
        Ok(Self { z, x, y })
    }
}

impl fmt::Display for TileCoord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}/{}", self.z, self.x, self.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_in_range_coords() {
        // At z=12, max x/y is 4095 (2^12 - 1). Off-by-one at the
        // upper bound matters for cache keys.
        assert!(TileCoord::new(12, 4095, 4095).is_ok());
        assert!(TileCoord::new(0, 0, 0).is_ok());
    }

    #[test]
    fn rejects_x_at_limit() {
        assert!(matches!(
            TileCoord::new(12, 4096, 0),
            Err(TileCoordError::XOutOfRange { .. })
        ));
    }

    #[test]
    fn rejects_y_at_limit() {
        assert!(matches!(
            TileCoord::new(12, 0, 4096),
            Err(TileCoordError::YOutOfRange { .. })
        ));
    }

    #[test]
    fn rejects_zoom_too_high() {
        // z > 22 is pathological for vector tiles.
        assert!(matches!(
            TileCoord::new(23, 0, 0),
            Err(TileCoordError::ZoomTooHigh(23))
        ));
    }

    #[test]
    fn display_matches_url_form() {
        let c = TileCoord::new(12, 2238, 1189).unwrap();
        assert_eq!(c.to_string(), "12/2238/1189");
    }
}
