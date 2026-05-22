use serde::Deserialize;
use std::str::FromStr;

/// A WGS84 bounding box in `west,south,east,north` order. Matches the
/// `?bbox=` query convention used by the GeoJSON list endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Bbox {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum BboxParseError {
    #[error("bbox must be four comma-separated floats (west,south,east,north)")]
    Shape,
    #[error("bbox component is not a finite number")]
    NonFinite,
    #[error("bbox west must be <= east and south <= north")]
    Inverted,
}

impl FromStr for Bbox {
    type Err = BboxParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != 4 {
            return Err(BboxParseError::Shape);
        }
        let parse = |p: &str| -> Result<f64, BboxParseError> {
            let v: f64 = p.trim().parse().map_err(|_| BboxParseError::Shape)?;
            if !v.is_finite() {
                return Err(BboxParseError::NonFinite);
            }
            Ok(v)
        };
        let west = parse(parts[0])?;
        let south = parse(parts[1])?;
        let east = parse(parts[2])?;
        let north = parse(parts[3])?;
        if west > east || south > north {
            return Err(BboxParseError::Inverted);
        }
        Ok(Self {
            west,
            south,
            east,
            north,
        })
    }
}
