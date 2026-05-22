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
