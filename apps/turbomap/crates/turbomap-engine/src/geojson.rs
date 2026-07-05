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
    VectorValue,
};
use turbomap_scene::geo::mercator_normalized;
use turbomap_scene::LatLng;

/// The MVT layer name geojson features are emitted under; the engine's
/// generated style rule matches on it.
pub const GEOJSON_LAYER: &str = "geojson";

const EXTENT: i32 = 4096;

/// A [`VectorTileSource`] over inline GeoJSON line + polygon geometry.
/// Parsed once into world space; each `request` clips to the tile.
/// A line string in world space with its feature properties (for
/// data-driven styling that filters on a property).
type LineFeature = (Vec<(f64, f64)>, HashMap<String, VectorValue>);

pub struct GeoJsonVectorSource {
    /// Line strings in normalized world space (`[0,1]²`) + properties.
    lines: Vec<LineFeature>,
    /// Polygons (each a list of rings: outer then holes) in world space.
    polygons: Vec<Vec<Vec<(f64, f64)>>>,
    /// Points in world space with their feature properties (for symbol
    /// labels — `Point` features whose `text_field` property is the label).
    points: Vec<(f64, f64, HashMap<String, VectorValue>)>,
    /// When false, geometry is emitted unclipped (the whole feature per
    /// tile) — only used to profile the cost clipping saves.
    clip: bool,
}

impl GeoJsonVectorSource {
    /// Parse inline GeoJSON. Unrecognised geometry is ignored rather than
    /// erroring — a partial overlay beats a failed map.
    pub fn new(data: &str) -> Self {
        let parsed = parse_geometry(data);
        Self {
            lines: parsed.lines,
            polygons: parsed.polygons,
            points: parsed.points,
            clip: true,
        }
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

    /// Number of parsed polygons.
    pub fn polygon_count(&self) -> usize {
        self.polygons.len()
    }

    /// Number of parsed points.
    pub fn point_count(&self) -> usize {
        self.points.len()
    }
}

impl VectorTileSource for GeoJsonVectorSource {
    fn request(&self, tile: TileId) -> Result<VectorTile, TileError> {
        let n = (1u64 << tile.z) as f64;
        let (x0, y0) = (tile.x as f64 / n, tile.y as f64 / n);
        let (x1, y1) = ((tile.x as f64 + 1.0) / n, (tile.y as f64 + 1.0) / n);
        let rect = Rect { x0, y0, x1, y1 };

        let to_local = |wx: f64, wy: f64| -> (i32, i32) {
            let lx = ((wx - x0) / (x1 - x0) * EXTENT as f64).round() as i32;
            let ly = ((wy - y0) / (y1 - y0) * EXTENT as f64).round() as i32;
            (lx, ly)
        };

        let mut features = Vec::new();
        for (line, props) in &self.lines {
            let subpaths = if self.clip {
                clip_polyline(line, rect)
            } else {
                vec![line.clone()]
            };
            for sub in subpaths {
                let local: Vec<(i32, i32)> = sub.iter().map(|&(wx, wy)| to_local(wx, wy)).collect();
                if local.len() >= 2 {
                    features.push(Feature {
                        id: features.len() as u64,
                        geom_type: GeomType::LineString,
                        geometry: Geometry::LineString(vec![local]),
                        properties: props.clone(),
                    });
                }
            }
        }
        for polygon in &self.polygons {
            let mut rings: Vec<Vec<(i32, i32)>> = Vec::new();
            for ring in polygon {
                let clipped = if self.clip {
                    clip_ring(ring, rect)
                } else {
                    ring.clone()
                };
                if clipped.len() >= 3 {
                    rings.push(clipped.iter().map(|&(wx, wy)| to_local(wx, wy)).collect());
                }
            }
            if !rings.is_empty() {
                features.push(Feature {
                    id: features.len() as u64,
                    geom_type: GeomType::Polygon,
                    geometry: Geometry::Polygon(rings),
                    properties: HashMap::new(),
                });
            }
        }
        for (wx, wy, props) in &self.points {
            // Points need no clipping — include those inside the tile
            // (half-open bounds so a point on a shared edge lands in one
            // tile only).
            if *wx >= x0 && *wx < x1 && *wy >= y0 && *wy < y1 {
                let local = to_local(*wx, *wy);
                features.push(Feature {
                    id: features.len() as u64,
                    geom_type: GeomType::Point,
                    geometry: Geometry::Point(vec![local]),
                    properties: props.clone(),
                });
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

#[derive(Default)]
struct ParsedGeometry {
    lines: Vec<LineFeature>,
    polygons: Vec<Vec<Vec<(f64, f64)>>>,
    points: Vec<(f64, f64, HashMap<String, VectorValue>)>,
}

/// Parse GeoJSON into world-space lines, polygons, and points-with-
/// properties. Accepts a FeatureCollection, a Feature, or a bare geometry;
/// projects every `[lng, lat]` to world space and carries each feature's
/// properties onto its points (for symbol labels).
fn parse_geometry(data: &str) -> ParsedGeometry {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(data) else {
        return ParsedGeometry::default();
    };
    let mut out = ParsedGeometry::default();
    collect_geometries(&root, &HashMap::new(), &mut out);
    out
}

fn collect_geometries(
    value: &serde_json::Value,
    props: &HashMap<String, VectorValue>,
    out: &mut ParsedGeometry,
) {
    match value.get("type").and_then(|t| t.as_str()) {
        Some("FeatureCollection") => {
            if let Some(features) = value.get("features").and_then(|f| f.as_array()) {
                for f in features {
                    collect_geometries(f, props, out);
                }
            }
        }
        Some("Feature") => {
            let feature_props = parse_props(value.get("properties"));
            if let Some(geom) = value.get("geometry") {
                collect_geometries(geom, &feature_props, out);
            }
        }
        Some("LineString") => {
            if let Some(coords) = value.get("coordinates").and_then(|c| c.as_array()) {
                if let Some(line) = parse_positions(coords) {
                    out.lines.push((line, props.clone()));
                }
            }
        }
        Some("MultiLineString") => {
            for_each_array(value.get("coordinates"), |line| {
                if let Some(coords) = line.as_array() {
                    if let Some(line) = parse_positions(coords) {
                        out.lines.push((line, props.clone()));
                    }
                }
            });
        }
        Some("Polygon") => {
            if let Some(rings) = parse_rings(value.get("coordinates")) {
                out.polygons.push(rings);
            }
        }
        Some("MultiPolygon") => {
            for_each_array(value.get("coordinates"), |poly| {
                if let Some(rings) = parse_rings(Some(poly)) {
                    out.polygons.push(rings);
                }
            });
        }
        Some("Point") => {
            if let Some((lng, lat)) = value.get("coordinates").and_then(point_lng_lat) {
                let (wx, wy) = mercator_normalized(LatLng::new(lat, lng));
                out.points.push((wx, wy, props.clone()));
            }
        }
        Some("MultiPoint") => {
            for_each_array(value.get("coordinates"), |p| {
                if let Some((lng, lat)) = point_lng_lat(p) {
                    let (wx, wy) = mercator_normalized(LatLng::new(lat, lng));
                    out.points.push((wx, wy, props.clone()));
                }
            });
        }
        _ => {}
    }
}

/// Convert a GeoJSON `properties` object into MVT property values.
fn parse_props(value: Option<&serde_json::Value>) -> HashMap<String, VectorValue> {
    let mut out = HashMap::new();
    if let Some(obj) = value.and_then(|v| v.as_object()) {
        for (k, v) in obj {
            let val = match v {
                serde_json::Value::String(s) => VectorValue::String(s.clone()),
                serde_json::Value::Bool(b) => VectorValue::Bool(*b),
                serde_json::Value::Number(n) if n.is_i64() => VectorValue::Int(n.as_i64().unwrap()),
                serde_json::Value::Number(n) => VectorValue::Float(n.as_f64().unwrap_or(0.0)),
                _ => continue,
            };
            out.insert(k.clone(), val);
        }
    }
    out
}

fn for_each_array(value: Option<&serde_json::Value>, mut f: impl FnMut(&serde_json::Value)) {
    if let Some(arr) = value.and_then(|v| v.as_array()) {
        for item in arr {
            f(item);
        }
    }
}

/// Parse a polygon's rings (`[[[lng,lat],...], ...]`) into world space.
fn parse_rings(value: Option<&serde_json::Value>) -> Option<Vec<Vec<(f64, f64)>>> {
    let rings = value?.as_array()?;
    let mut out = Vec::new();
    for ring in rings {
        if let Some(coords) = ring.as_array() {
            if let Some(r) = parse_positions(coords) {
                out.push(r);
            }
        }
    }
    (!out.is_empty()).then_some(out)
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

/// Parse GeoJSON `LineString`/`MultiLineString` geometry into an ordered
/// polyline of `(lng, lat)` pairs — the scene-declared route-tube layer's
/// geometry (plan P5.2). Multiple lines concatenate in document order (a
/// recorded track split into segments still reads as one route).
///
/// GEOGRAPHIC degrees, like [`parse_points`] — NOT the world space
/// [`ParsedGeometry`] holds internally. The P6.5 `interleave-content`
/// golden caught this returning world coordinates that the tube installer
/// then re-projected as if they were degrees, baking every scene-declared
/// tube at null island (the P5.2 tests only asserted install/remove
/// counts, never pixels).
pub fn parse_line(data: &str) -> Vec<(f64, f64)> {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(data) else {
        return Vec::new();
    };
    let mut geo = ParsedGeometry::default();
    collect_geometries(&root, &Default::default(), &mut geo);
    geo.lines
        .into_iter()
        .flat_map(|(line, _)| line)
        .map(|(x, y)| {
            let ll = turbomap_scene::geo::inverse_mercator(x, y);
            (ll.lng, ll.lat)
        })
        .collect()
}

/// Parse GeoJSON `Point`/`MultiPoint` geometry into `(lng, lat)` pairs —
/// used to drive circle/marker layers, which are positioned in geographic
/// coordinates (not the world space lines use).
pub fn parse_points(data: &str) -> Vec<(f64, f64)> {
    parse_points_with_props(data)
        .into_iter()
        .map(|(p, _)| p)
        .collect()
}

/// [`parse_points`] plus each point's stringified feature properties — the
/// attributes a hit test surfaces (plan P6.4: a tapped scene circle answers
/// with its domain id/name, not just a position). Points inside one
/// `MultiPoint` share their feature's properties.
pub fn parse_points_with_props(data: &str) -> Vec<PointWithProps> {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(data) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_points(&root, &HashMap::new(), &mut out);
    out
}

/// A `(lng, lat)` point plus its feature's stringified properties.
pub type PointWithProps = ((f64, f64), HashMap<String, String>);

/// Stringify a GeoJSON `properties` object for hit-test payloads.
fn parse_props_strings(value: Option<&serde_json::Value>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if let Some(obj) = value.and_then(|v| v.as_object()) {
        for (k, v) in obj {
            let s = match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Number(n) => n.to_string(),
                _ => continue,
            };
            out.insert(k.clone(), s);
        }
    }
    out
}

fn collect_points(
    value: &serde_json::Value,
    props: &HashMap<String, String>,
    out: &mut Vec<PointWithProps>,
) {
    match value.get("type").and_then(|t| t.as_str()) {
        Some("FeatureCollection") => {
            if let Some(features) = value.get("features").and_then(|f| f.as_array()) {
                for f in features {
                    collect_points(f, props, out);
                }
            }
        }
        Some("Feature") => {
            let feature_props = parse_props_strings(value.get("properties"));
            if let Some(geom) = value.get("geometry") {
                collect_points(geom, &feature_props, out);
            }
        }
        Some("Point") => {
            if let Some(p) = value.get("coordinates").and_then(point_lng_lat) {
                out.push((p, props.clone()));
            }
        }
        Some("MultiPoint") => {
            if let Some(points) = value.get("coordinates").and_then(|c| c.as_array()) {
                for p in points {
                    if let Some(p) = point_lng_lat(p) {
                        out.push((p, props.clone()));
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

/// Clip a polygon ring to `rect` (Sutherland–Hodgman). Returns the clipped
/// ring, or empty if nothing survives. Holes clip the same way; lyon's
/// fill rule recombines outer + hole rings.
fn clip_ring(ring: &[(f64, f64)], r: Rect) -> Vec<(f64, f64)> {
    // Clip against each rectangle edge in turn. `keep`/`x_at`/`y_at` encode
    // the half-plane and the intersection on that edge.
    let mut poly = ring.to_vec();
    // left: x >= x0
    poly = clip_edge(&poly, |p| p.0 >= r.x0, |a, b| intersect_x(a, b, r.x0));
    // right: x <= x1
    poly = clip_edge(&poly, |p| p.0 <= r.x1, |a, b| intersect_x(a, b, r.x1));
    // bottom: y >= y0
    poly = clip_edge(&poly, |p| p.1 >= r.y0, |a, b| intersect_y(a, b, r.y0));
    // top: y <= y1
    poly = clip_edge(&poly, |p| p.1 <= r.y1, |a, b| intersect_y(a, b, r.y1));
    poly
}

fn clip_edge(
    input: &[(f64, f64)],
    inside: impl Fn((f64, f64)) -> bool,
    intersect: impl Fn((f64, f64), (f64, f64)) -> (f64, f64),
) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    if input.is_empty() {
        return out;
    }
    let mut prev = *input.last().unwrap();
    for &cur in input {
        let cur_in = inside(cur);
        if cur_in {
            if !inside(prev) {
                out.push(intersect(prev, cur));
            }
            out.push(cur);
        } else if inside(prev) {
            out.push(intersect(prev, cur));
        }
        prev = cur;
    }
    out
}

fn intersect_x(a: (f64, f64), b: (f64, f64), xe: f64) -> (f64, f64) {
    let t = (xe - a.0) / (b.0 - a.0);
    (xe, a.1 + t * (b.1 - a.1))
}

fn intersect_y(a: (f64, f64), b: (f64, f64), ye: f64) -> (f64, f64) {
    let t = (ye - a.1) / (b.1 - a.1);
    (a.0 + t * (b.0 - a.0), ye)
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
    fn parse_line_returns_geographic_degrees() {
        // The tube installer feeds these to `LatLng` verbatim - a world-space
        // leak here bakes the whole route at null island (the P6.5 golden's
        // catch). Pin the round trip.
        let pts = parse_line(r#"{"type":"LineString","coordinates":[[5.06,60.32],[5.58,60.46]]}"#);
        assert_eq!(pts.len(), 2);
        assert!(
            (pts[0].0 - 5.06).abs() < 1e-9 && (pts[0].1 - 60.32).abs() < 1e-9,
            "expected (lng, lat) degrees, got {pts:?}"
        );
        assert!((pts[1].0 - 5.58).abs() < 1e-9 && (pts[1].1 - 60.46).abs() < 1e-9);
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
    fn ring_fully_inside_is_unchanged() {
        let ring = vec![(0.2, 0.2), (0.8, 0.2), (0.8, 0.8), (0.2, 0.8)];
        let clipped = clip_ring(&ring, RECT);
        assert_eq!(clipped.len(), 4);
    }

    #[test]
    fn ring_straddling_an_edge_is_clipped_inside() {
        // A square poking out the right edge → all x within [0,1].
        let ring = vec![(0.5, 0.2), (1.5, 0.2), (1.5, 0.8), (0.5, 0.8)];
        let clipped = clip_ring(&ring, RECT);
        assert!(!clipped.is_empty());
        assert!(
            clipped.iter().all(|p| p.0 <= 1.0 + 1e-9 && p.0 >= -1e-9),
            "{clipped:?}"
        );
    }

    #[test]
    fn ring_fully_outside_is_empty() {
        let ring = vec![(1.2, 1.2), (1.8, 1.2), (1.8, 1.8), (1.2, 1.8)];
        assert!(clip_ring(&ring, RECT).is_empty());
    }

    #[test]
    fn parses_polygon_geometry() {
        let data = r#"{"type":"Polygon","coordinates":[[[5.2,60.3],[5.4,60.3],[5.4,60.45],[5.2,60.45],[5.2,60.3]]]}"#;
        let src = GeoJsonVectorSource::new(data);
        assert_eq!(src.polygon_count(), 1);
        let tile = src.request(TileId::new(0, 0, 0)).unwrap();
        assert!(tile.layers[0]
            .features
            .iter()
            .any(|f| f.geom_type == GeomType::Polygon));
    }

    #[test]
    fn point_feature_carries_properties_for_labels() {
        let data = r#"{
            "type": "Feature",
            "properties": { "name": "Bergen" },
            "geometry": { "type": "Point", "coordinates": [5.32, 60.39] }
        }"#;
        let src = GeoJsonVectorSource::new(data);
        assert_eq!(src.point_count(), 1);
        // The tile containing Bergen should carry a Point feature with the
        // name property (the symbol label).
        let tile = src.request(TileId::new(0, 0, 0)).unwrap();
        let pt = tile.layers[0]
            .features
            .iter()
            .find(|f| f.geom_type == GeomType::Point)
            .expect("a point feature");
        assert_eq!(
            pt.properties.get("name"),
            Some(&VectorValue::String("Bergen".to_string()))
        );
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
        assert_eq!(
            pts.len(),
            3,
            "two from points/multipoint + ignores the line"
        );
        assert_eq!(pts[0], (5.32, 60.39));
    }

    #[test]
    fn points_carry_their_feature_properties() {
        let data = r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","properties":{"id":"mk-1","name":"Bergen","rank":3},
             "geometry":{"type":"Point","coordinates":[5.32,60.39]}},
            {"type":"Feature","properties":{"id":"mk-2"},
             "geometry":{"type":"MultiPoint","coordinates":[[6.0,61.0],[7.0,62.0]]}}
        ]}"#;
        let pts = parse_points_with_props(data);
        assert_eq!(pts.len(), 3);
        assert_eq!(pts[0].0, (5.32, 60.39));
        assert_eq!(pts[0].1.get("id").map(String::as_str), Some("mk-1"));
        assert_eq!(pts[0].1.get("name").map(String::as_str), Some("Bergen"));
        assert_eq!(pts[0].1.get("rank").map(String::as_str), Some("3"));
        // MultiPoint members share their feature's properties.
        assert_eq!(pts[1].1.get("id").map(String::as_str), Some("mk-2"));
        assert_eq!(pts[2].1.get("id").map(String::as_str), Some("mk-2"));
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
