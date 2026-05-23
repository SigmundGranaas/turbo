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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_oslo_bbox() {
        let b: Bbox = "10.6,59.8,11.0,60.0".parse().unwrap();
        assert_eq!(b.west, 10.6);
        assert_eq!(b.south, 59.8);
        assert_eq!(b.east, 11.0);
        assert_eq!(b.north, 60.0);
    }

    #[test]
    fn rejects_wrong_arity() {
        assert!(matches!(
            "1,2,3".parse::<Bbox>(),
            Err(BboxParseError::Shape)
        ));
        assert!(matches!(
            "1,2,3,4,5".parse::<Bbox>(),
            Err(BboxParseError::Shape)
        ));
    }

    #[test]
    fn rejects_non_numeric() {
        assert!(matches!(
            "a,b,c,d".parse::<Bbox>(),
            Err(BboxParseError::Shape)
        ));
    }

    #[test]
    fn rejects_inverted_bbox() {
        // west > east — common copy-paste error worth catching at
        // the boundary.
        assert!(matches!(
            "11,59,10,60".parse::<Bbox>(),
            Err(BboxParseError::Inverted)
        ));
        assert!(matches!(
            "10,60,11,59".parse::<Bbox>(),
            Err(BboxParseError::Inverted)
        ));
    }

    #[test]
    fn rejects_non_finite() {
        // NaN/Inf would propagate through ST_MakeEnvelope and either
        // crash the query or return nothing. Bail at the boundary.
        assert!(matches!(
            "NaN,0,1,1".parse::<Bbox>(),
            Err(BboxParseError::NonFinite)
        ));
    }

    #[test]
    fn tolerates_whitespace() {
        let b: Bbox = " 10 , 59 , 11 , 60 ".parse().unwrap();
        assert_eq!(b.west, 10.0);
    }

    #[test]
    fn point_bbox_is_valid() {
        // Zero-area envelopes are degenerate but acceptable; PostGIS
        // ST_MakeEnvelope handles them fine.
        let b: Bbox = "10,60,10,60".parse().unwrap();
        assert_eq!(b.west, b.east);
    }
}
