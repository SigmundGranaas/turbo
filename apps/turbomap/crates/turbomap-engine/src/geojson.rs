//! Rendering inline GeoJSON line overlays (route / track / measure) through
//! the core vector-tile pipeline.
//!
//! The core renderer is tile-based and has no per-tile scissor — vector
//! meshes are pre-projected to world space and drawn in full. So to draw a
//! world-spanning GeoJSON line without N× overdraw, [`GeoJsonVectorSource`]
//! *clips* the geometry to each requested tile (Liang–Barsky) and emits a
//! tile-local [`VectorTile`]. Empty tiles are returned for tiles the line
//! doesn't cross, so the layer's pull loop still drains.
//!
//! This slice handles `LineString`/`MultiLineString`. Points (→ markers)
//! and polygons are later slices.

use std::collections::HashMap;

use turbomap_core::{
    Feature, GeomType, Geometry, TileError, TileId, VectorTile, VectorTileLayer, VectorTileSource,
};
use turbomap_scene::geo::mercator_normalized;
use turbomap_scene::LatLng;

/// The MVT layer name geojson features are emitted under; the engine's
/// generated style rule matches on it.
pub const GEOJSON_LAYER: &str = "geojson";

const EXTENT: i32 = 4096;

/// A [`VectorTileSource`] over inline GeoJSON line geometry. Parsed once
/// into world space; each `request` clips to the tile.
pub struct GeoJsonVectorSource {
    /// Line strings in normalized world space (`[0,1]²`).
    lines: Vec<Vec<(f64, f64)>>,
    /// When false, geometry is emitted unclipped (the whole line per tile)
    /// — only used to profile the cost clipping saves.
    clip: bool,
}

impl GeoJsonVectorSource {
    /// Parse inline GeoJSON. Unrecognised geometry is ignored rather than
    /// erroring — a partial overlay beats a failed map.
    pub fn new(data: &str) -> Self {
        let lines = parse_lines(data);
        Self { lines, clip: true }
    }

    /// Disable per-tile clipping (profiling/comparison only).
    pub fn unclipped(mut self) -> Self {
        self.clip = false;
        self
    }

    /// Number of parsed line strings.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

impl VectorTileSource for GeoJsonVectorSource {
    fn request(&self, tile: TileId) -> Result<VectorTile, TileError> {
        let n = (1u64 << tile.z) as f64;
        let (x0, y0) = (tile.x as f64 / n, tile.y as f64 / n);
        let (x1, y1) = ((tile.x as f64 + 1.0) / n, (tile.y as f64 + 1.0) / n);
        let rect = Rect { x0, y0, x1, y1 };

        let mut features = Vec::new();
        for line in &self.lines {
            let subpaths = if self.clip {
                clip_polyline(line, rect)
            } else {
                vec![line.clone()]
            };
            for sub in subpaths {
                let local: Vec<(i32, i32)> = sub
                    .iter()
                    .map(|&(wx, wy)| {
                        let lx = ((wx - x0) / (x1 - x0) * EXTENT as f64).round() as i32;
                        let ly = ((wy - y0) / (y1 - y0) * EXTENT as f64).round() as i32;
                        (lx, ly)
                    })
                    .collect();
                if local.len() >= 2 {
                    features.push(Feature {
                        id: features.len() as u64,
                        geom_type: GeomType::LineString,
                        geometry: Geometry::LineString(vec![local]),
                        properties: HashMap::new(),
                    });
                }
            }
        }

        Ok(VectorTile {
            layers: vec![VectorTileLayer {
                name: GEOJSON_LAYER.to_string(),
                version: 2,
                extent: EXTENT as u32,
                features,
            }],
        })
    }

    fn min_zoom(&self) -> u8 {
        0
    }
    fn max_zoom(&self) -> u8 {
        22
    }
}

#[derive(Clone, Copy)]
struct Rect {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

/// Parse GeoJSON into world-space line strings. Accepts a
/// FeatureCollection, a Feature, or a bare geometry; pulls LineString and
/// MultiLineString coordinates and projects `[lng, lat]` to world space.
fn parse_lines(data: &str) -> Vec<Vec<(f64, f64)>> {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(data) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_geometries(&root, &mut out);
    out
}

fn collect_geometries(value: &serde_json::Value, out: &mut Vec<Vec<(f64, f64)>>) {
    match value.get("type").and_then(|t| t.as_str()) {
        Some("FeatureCollection") => {
            if let Some(features) = value.get("features").and_then(|f| f.as_array()) {
                for f in features {
                    collect_geometries(f, out);
                }
            }
        }
        Some("Feature") => {
            if let Some(geom) = value.get("geometry") {
                collect_geometries(geom, out);
            }
        }
        Some("LineString") => {
            if let Some(coords) = value.get("coordinates").and_then(|c| c.as_array()) {
                if let Some(line) = parse_positions(coords) {
                    out.push(line);
                }
            }
        }
        Some("MultiLineString") => {
            if let Some(lines) = value.get("coordinates").and_then(|c| c.as_array()) {
                for line in lines {
                    if let Some(coords) = line.as_array() {
                        if let Some(line) = parse_positions(coords) {
                            out.push(line);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn parse_positions(coords: &[serde_json::Value]) -> Option<Vec<(f64, f64)>> {
    let mut line = Vec::with_capacity(coords.len());
    for pos in coords {
        let arr = pos.as_array()?;
        let lng = arr.first()?.as_f64()?;
        let lat = arr.get(1)?.as_f64()?;
        line.push(mercator_normalized(LatLng::new(lat, lng)));
    }
    (line.len() >= 2).then_some(line)
}

/// Parse GeoJSON `Point`/`MultiPoint` geometry into `(lng, lat)` pairs —
/// used to drive circle/marker layers, which are positioned in geographic
/// coordinates (not the world space lines use).
pub fn parse_points(data: &str) -> Vec<(f64, f64)> {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(data) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_points(&root, &mut out);
    out
}

fn collect_points(value: &serde_json::Value, out: &mut Vec<(f64, f64)>) {
    match value.get("type").and_then(|t| t.as_str()) {
        Some("FeatureCollection") => {
            if let Some(features) = value.get("features").and_then(|f| f.as_array()) {
                for f in features {
                    collect_points(f, out);
                }
            }
        }
        Some("Feature") => {
            if let Some(geom) = value.get("geometry") {
                collect_points(geom, out);
            }
        }
        Some("Point") => {
            if let Some(p) = value.get("coordinates").and_then(point_lng_lat) {
                out.push(p);
            }
        }
        Some("MultiPoint") => {
            if let Some(points) = value.get("coordinates").and_then(|c| c.as_array()) {
                for p in points {
                    if let Some(p) = point_lng_lat(p) {
                        out.push(p);
                    }
                }
            }
        }
        _ => {}
    }
}

fn point_lng_lat(value: &serde_json::Value) -> Option<(f64, f64)> {
    let arr = value.as_array()?;
    Some((arr.first()?.as_f64()?, arr.get(1)?.as_f64()?))
}

/// Clip a polyline to `rect`, returning the connected in-rect subpaths.
fn clip_polyline(line: &[(f64, f64)], rect: Rect) -> Vec<Vec<(f64, f64)>> {
    let mut out = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    for seg in line.windows(2) {
        match liang_barsky(seg[0], seg[1], rect) {
            Some((a, b)) => {
                if cur.is_empty() {
                    cur.push(a);
                    cur.push(b);
                } else if approx_eq(*cur.last().unwrap(), a) {
                    cur.push(b);
                } else {
                    // The line left and re-entered the rect: break the run.
                    if cur.len() >= 2 {
                        out.push(std::mem::take(&mut cur));
                    } else {
                        cur.clear();
                    }
                    cur.push(a);
                    cur.push(b);
                }
            }
            None => {
                if cur.len() >= 2 {
                    out.push(std::mem::take(&mut cur));
                } else {
                    cur.clear();
                }
            }
        }
    }
    if cur.len() >= 2 {
        out.push(cur);
    }
    out
}

fn approx_eq(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-12 && (a.1 - b.1).abs() < 1e-12
}

/// Liang–Barsky segment clip. Returns the clipped endpoints, or `None` if
/// the segment lies entirely outside the rectangle.
fn liang_barsky(p0: (f64, f64), p1: (f64, f64), r: Rect) -> Option<((f64, f64), (f64, f64))> {
    let dx = p1.0 - p0.0;
    let dy = p1.1 - p0.1;
    let p = [-dx, dx, -dy, dy];
    let q = [p0.0 - r.x0, r.x1 - p0.0, p0.1 - r.y0, r.y1 - p0.1];
    let mut t0 = 0.0_f64;
    let mut t1 = 1.0_f64;
    for i in 0..4 {
        if p[i].abs() < 1e-15 {
            // Parallel to this edge — reject if it starts outside it.
            if q[i] < 0.0 {
                return None;
            }
        } else {
            let r_t = q[i] / p[i];
            if p[i] < 0.0 {
                if r_t > t1 {
                    return None;
                }
                if r_t > t0 {
                    t0 = r_t;
                }
            } else {
                if r_t < t0 {
                    return None;
                }
                if r_t < t1 {
                    t1 = r_t;
                }
            }
        }
    }
    Some((
        (p0.0 + t0 * dx, p0.1 + t0 * dy),
        (p0.0 + t1 * dx, p0.1 + t1 * dy),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECT: Rect = Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 1.0,
        y1: 1.0,
    };

    #[test]
    fn fully_inside_segment_is_unchanged() {
        let c = liang_barsky((0.2, 0.2), (0.8, 0.8), RECT).unwrap();
        assert!(approx_eq(c.0, (0.2, 0.2)) && approx_eq(c.1, (0.8, 0.8)));
    }

    #[test]
    fn fully_outside_segment_is_rejected() {
        assert!(liang_barsky((1.2, 1.2), (1.8, 1.8), RECT).is_none());
    }

    #[test]
    fn crossing_segment_is_clipped_to_the_edge() {
        // From inside to well outside the right edge.
        let c = liang_barsky((0.5, 0.5), (1.5, 0.5), RECT).unwrap();
        assert!(approx_eq(c.0, (0.5, 0.5)));
        assert!((c.1 .0 - 1.0).abs() < 1e-12 && (c.1 .1 - 0.5).abs() < 1e-12);
    }

    #[test]
    fn polyline_entering_and_leaving_yields_one_subpath() {
        // Starts outside, dips through the rect, exits — one in-rect run.
        let line = vec![(-0.5, 0.5), (0.5, 0.5), (1.5, 0.5)];
        let subs = clip_polyline(&line, RECT);
        assert_eq!(subs.len(), 1, "{subs:?}");
        assert!(subs[0].len() >= 2);
    }

    #[test]
    fn polyline_leaving_and_reentering_yields_two_subpaths() {
        // In → out (above) → back in: two disconnected in-rect runs.
        let line = vec![
            (0.2, 0.5),
            (0.4, -0.5),
            (0.6, 0.5),
            (0.6, 0.9),
            (0.8, -0.5),
            (0.9, 0.5),
        ];
        let subs = clip_polyline(&line, RECT);
        assert!(subs.len() >= 2, "expected disconnected runs, got {subs:?}");
    }

    #[test]
    fn parses_linestring_feature_collection() {
        let data = r#"{
            "type": "FeatureCollection",
            "features": [
                { "type": "Feature", "geometry":
                    { "type": "LineString", "coordinates": [[5.2,60.3],[5.32,60.39],[5.45,60.45]] } }
            ]
        }"#;
        let src = GeoJsonVectorSource::new(data);
        assert_eq!(src.line_count(), 1);
    }

    #[test]
    fn parses_points_from_mixed_geojson() {
        let data = r#"{
            "type": "FeatureCollection",
            "features": [
                { "type": "Feature", "geometry": { "type": "Point", "coordinates": [5.32, 60.39] } },
                { "type": "Feature", "geometry":
                    { "type": "MultiPoint", "coordinates": [[5.1,60.3],[5.4,60.45]] } },
                { "type": "Feature", "geometry":
                    { "type": "LineString", "coordinates": [[5.0,60.0],[5.1,60.1]] } }
            ]
        }"#;
        let pts = parse_points(data);
        assert_eq!(pts.len(), 3, "two from points/multipoint + ignores the line");
        assert_eq!(pts[0], (5.32, 60.39));
    }

    #[test]
    fn request_emits_geojson_layer() {
        let data = r#"{"type":"LineString","coordinates":[[5.2,60.3],[5.45,60.45]]}"#;
        let src = GeoJsonVectorSource::new(data);
        let tile = src.request(TileId::new(0, 0, 0)).unwrap();
        assert_eq!(tile.layers.len(), 1);
        assert_eq!(tile.layers[0].name, GEOJSON_LAYER);
        assert_eq!(tile.layers[0].extent, EXTENT as u32);
    }
}
