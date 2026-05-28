//! Axis-aligned bounding box in WGS84 (EPSG:4326). Modules that need
//! EPSG:25833-projected bboxes do the transform inside their impls.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bbox {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

impl Bbox {
    pub fn new(west: f64, south: f64, east: f64, north: f64) -> Self {
        Bbox {
            west,
            south,
            east,
            north,
        }
    }

    pub fn contains(&self, lon: f64, lat: f64) -> bool {
        lon >= self.west && lon <= self.east && lat >= self.south && lat <= self.north
    }

    /// True iff `west < east` and `south < north`. Bboxes that wrap
    /// the antimeridian are not supported — at Norway's longitudes
    /// the issue doesn't arise.
    pub fn is_valid(&self) -> bool {
        self.west < self.east && self.south < self.north
    }

    /// Approximate area in square degrees. Use only for relative
    /// ordering — not a true geographic area.
    pub fn span_deg(&self) -> (f64, f64) {
        (self.east - self.west, self.north - self.south)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_inside() {
        let b = Bbox::new(10.0, 59.0, 11.0, 60.0);
        assert!(b.contains(10.5, 59.5));
    }

    #[test]
    fn contains_on_boundary() {
        // Boundaries are inclusive — a coordinate exactly on the
        // western edge still counts. This matters when an ingest
        // bbox is hand-typed to integer degrees and a feature lands
        // exactly on it.
        let b = Bbox::new(10.0, 59.0, 11.0, 60.0);
        assert!(b.contains(10.0, 59.0));
    }

    #[test]
    fn contains_outside() {
        let b = Bbox::new(10.0, 59.0, 11.0, 60.0);
        assert!(!b.contains(9.9, 59.5));
    }

    #[test]
    fn validity_rejects_inverted() {
        // Caller swapped west/east. We reject rather than silently
        // treating it as a degenerate bbox.
        let b = Bbox::new(11.0, 59.0, 10.0, 60.0);
        assert!(!b.is_valid());
    }

    #[test]
    fn validity_rejects_zero_height() {
        let b = Bbox::new(10.0, 59.0, 11.0, 59.0);
        assert!(!b.is_valid());
    }
}
