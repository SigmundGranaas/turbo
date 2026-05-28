//! Generic cost-layer composers over vector feature collections.
//!
//! Three shapes cover every "feature integral" cost model the
//! curator has asked for so far:
//!
//! - [`PolygonIntegralLayer`] — additional cost is proportional to
//!   the metres of the candidate edge that lie inside polygons of
//!   the queried collection. Water, wetland, cliffs, cultivated
//!   areas, scree fields, snow patches, restricted zones.
//!
//! - [`LineCrossingLayer`] — additional cost is proportional to the
//!   *count* of crossings between the candidate edge and the
//!   collection's polylines, optionally weighted by per-feature
//!   attributes (river width, fence kind). Streams, rivers, fences.
//!
//! - [`PointProximityLayer`] — additional cost (often *negative*,
//!   i.e. a bonus) depends on distance from the cell to the nearest
//!   point feature within an influence radius. Cairns, viewpoints,
//!   marked trail anchors, water sources.
//!
//! Each composer is parameterised by a closure that maps
//! `(quantity, attrs, profile) → additional_effective_metres`. The
//! composer normalises the result against the edge length to
//! produce a multiplier that plugs into the existing `CostLayer`
//! trait. The "additional effective metres" framing is the same
//! unit the on-graph Naismith cost uses, so weights compose
//! sanely across the whole layer stack.
//!
//! ## Cell semantics
//!
//! Integral layers report `CellCost::default()` (neutral, multiplier
//! 1.0, never refused) — they don't participate in raster-style
//! cell rejection. All cost lives at the edge level via
//! [`CostLayer::edge_cost_modifier`]. This is the architectural
//! shift that fixes the "tiny tarn → 100 m halo" pathology: a
//! polygon's *shape* is preserved, and only the metres of route
//! that physically cross it cost anything.
//!
//! For features that genuinely *must* refuse traversal (ocean,
//! glaciers, private property) use [`PolygonRefusalLayer`] — same
//! collection, different semantics, callers pick the right tool.

use std::sync::Arc;

use turbo_tiles_geom::{
    polyline_length, segment_linestring_crossings, segment_polygon_intersection_length, Point,
};
use turbo_tiles_graph::{EdgeRecord, Profile};
use turbo_tiles_vector::{AttrView, GeomKind, VectorCollection};

use crate::cost::{CellCost, CostLayer};

/// Cost contribution evaluator for a polygon collection. The
/// returned value is "additional effective metres" added by *this
/// feature* given that `length_inside_m` of the candidate edge
/// lies within the feature. A simple "everything inside costs 80×"
/// becomes `|len, _, _| len * 80.0`.
///
/// Returning `f64::INFINITY` flags the edge as forbidden through
/// this layer; the composer surfaces that as
/// `CostLayer::edge_cost_modifier == INFINITY`.
pub type PolygonCostFn = dyn Fn(f64, &AttrView<'_>, Profile) -> f64 + Send + Sync + 'static;
pub type LineCostFn = dyn Fn(usize, &AttrView<'_>, Profile) -> f64 + Send + Sync + 'static;
pub type PointCostFn = dyn Fn(f64, &AttrView<'_>, Profile) -> f64 + Send + Sync + 'static;

/// Add cost to edges based on how many metres they spend inside
/// polygons of a vector collection.
pub struct PolygonIntegralLayer {
    name: &'static str,
    collection: Arc<VectorCollection>,
    cost_fn: Box<PolygonCostFn>,
    /// AABB padding for the rstar query (metres). Defaults to 0.
    aabb_pad_m: f32,
}

impl PolygonIntegralLayer {
    pub fn new<F>(
        name: &'static str,
        collection: Arc<VectorCollection>,
        cost_fn: F,
    ) -> Self
    where
        F: Fn(f64, &AttrView<'_>, Profile) -> f64 + Send + Sync + 'static,
    {
        assert_eq!(
            collection.kind(),
            GeomKind::Polygon,
            "PolygonIntegralLayer expects a Polygon collection, got {:?}",
            collection.kind()
        );
        Self { name, collection, cost_fn: Box::new(cost_fn), aabb_pad_m: 0.0 }
    }

    pub fn with_aabb_pad(mut self, m: f32) -> Self {
        self.aabb_pad_m = m;
        self
    }

    /// Total "extra effective metres" the polygon collection adds
    /// across segment AB. Public for testing + the per-edge debug
    /// endpoint.
    pub fn extra_metres(&self, fx: f64, fy: f64, tx: f64, ty: f64, profile: Profile) -> f64 {
        let a = Point::new(fx as f32, fy as f32);
        let b = Point::new(tx as f32, ty as f32);
        let mut extra: f64 = 0.0;
        for fid in self.collection.query_segment(a, b, self.aabb_pad_m) {
            let coords = self.collection.feature_coords(fid);
            let len_in = segment_polygon_intersection_length(a, b, coords);
            if len_in < 1e-6 {
                continue;
            }
            let attrs = self.collection.feature_attrs(fid);
            let contrib = (self.cost_fn)(len_in, &attrs, profile);
            if !contrib.is_finite() {
                return f64::INFINITY;
            }
            extra += contrib;
        }
        extra
    }
}

impl CostLayer for PolygonIntegralLayer {
    fn name(&self) -> &'static str { self.name }

    fn edge_cost_modifier(
        &self,
        fx: f64,
        fy: f64,
        tx: f64,
        ty: f64,
        profile: Profile,
    ) -> f32 {
        let dx = tx - fx;
        let dy = ty - fy;
        let edge_len = (dx * dx + dy * dy).sqrt();
        if edge_len < 1e-6 {
            return 1.0;
        }
        let extra = self.extra_metres(fx, fy, tx, ty, profile);
        if !extra.is_finite() {
            return f32::INFINITY;
        }
        (1.0 + extra / edge_len) as f32
    }

    fn edge_multiplier(&self, edge: &EdgeRecord, profile: Profile) -> f32 {
        // Approximate the graph edge as its straight node-to-node
        // segment for the integral query. Graph edges are typically
        // short enough that this is conservative — and the polyline
        // sibling artifact isn't visible here. A future refinement
        // could walk the polyline; today this gives the right ranking
        // without changing trait shape.
        //
        // We need node positions, which the `EdgeRecord` doesn't
        // carry directly. The CostLayer trait passes only the record
        // so this approximation falls back to "no edge contribution
        // from EdgeRecord-only context"; the off-trail solver routes
        // through `edge_cost_modifier` which DOES have coords.
        //
        // For the graph router we currently rely on Naismith + the
        // baked per-edge attrs (length_m, gain_m). When the graph
        // builder bakes per-edge "metres in water"/"crossings" attrs
        // (planned, see GraphSlopeLayer task), they'll appear here.
        let _ = (edge, profile);
        1.0
    }

    fn covers(&self, x: f64, y: f64) -> bool {
        // "Authoritative data at this point" is the union of every
        // feature's AABB — if any polygon's AABB contains the point,
        // we consider it covered. (Tight ring containment isn't
        // needed for the no-coverage pre-check; AABB is fine.)
        let p = Point::new(x as f32, y as f32);
        self.collection.query_point(p, 0.0).next().is_some()
    }
}

/// Add cost to edges based on how many polylines they cross.
pub struct LineCrossingLayer {
    name: &'static str,
    collection: Arc<VectorCollection>,
    cost_fn: Box<LineCostFn>,
    aabb_pad_m: f32,
}

impl LineCrossingLayer {
    pub fn new<F>(
        name: &'static str,
        collection: Arc<VectorCollection>,
        cost_fn: F,
    ) -> Self
    where
        F: Fn(usize, &AttrView<'_>, Profile) -> f64 + Send + Sync + 'static,
    {
        assert_eq!(
            collection.kind(),
            GeomKind::LineString,
            "LineCrossingLayer expects a LineString collection, got {:?}",
            collection.kind()
        );
        Self { name, collection, cost_fn: Box::new(cost_fn), aabb_pad_m: 0.0 }
    }

    pub fn with_aabb_pad(mut self, m: f32) -> Self {
        self.aabb_pad_m = m;
        self
    }

    pub fn extra_metres(&self, fx: f64, fy: f64, tx: f64, ty: f64, profile: Profile) -> f64 {
        let a = Point::new(fx as f32, fy as f32);
        let b = Point::new(tx as f32, ty as f32);
        let mut extra: f64 = 0.0;
        for fid in self.collection.query_segment(a, b, self.aabb_pad_m) {
            let coords = self.collection.feature_coords(fid);
            let crossings = segment_linestring_crossings(a, b, coords);
            if crossings.is_empty() {
                continue;
            }
            let attrs = self.collection.feature_attrs(fid);
            let contrib = (self.cost_fn)(crossings.len(), &attrs, profile);
            if !contrib.is_finite() {
                return f64::INFINITY;
            }
            extra += contrib;
        }
        extra
    }
}

impl CostLayer for LineCrossingLayer {
    fn name(&self) -> &'static str { self.name }

    fn edge_cost_modifier(
        &self,
        fx: f64,
        fy: f64,
        tx: f64,
        ty: f64,
        profile: Profile,
    ) -> f32 {
        let dx = tx - fx;
        let dy = ty - fy;
        let edge_len = (dx * dx + dy * dy).sqrt();
        if edge_len < 1e-6 {
            return 1.0;
        }
        let extra = self.extra_metres(fx, fy, tx, ty, profile);
        if !extra.is_finite() {
            return f32::INFINITY;
        }
        (1.0 + extra / edge_len) as f32
    }

    fn covers(&self, x: f64, y: f64) -> bool {
        let p = Point::new(x as f32, y as f32);
        self.collection.query_point(p, 0.0).next().is_some()
    }
}

/// Multiplier based on proximity of the cell to the nearest point
/// feature in the collection. `cost_fn(distance_m, attrs, profile)`
/// returns the *multiplier* directly (not "extra metres") because
/// proximity bonuses naturally compose multiplicatively (e.g.
/// "halve the local cost near a viewpoint").
///
/// Cells beyond `influence_radius_m` get the default `1.0`.
pub struct PointProximityLayer {
    name: &'static str,
    collection: Arc<VectorCollection>,
    cost_fn: Box<dyn Fn(f64, &AttrView<'_>, Profile) -> f64 + Send + Sync + 'static>,
    influence_radius_m: f32,
}

impl PointProximityLayer {
    pub fn new<F>(
        name: &'static str,
        collection: Arc<VectorCollection>,
        influence_radius_m: f32,
        cost_fn: F,
    ) -> Self
    where
        F: Fn(f64, &AttrView<'_>, Profile) -> f64 + Send + Sync + 'static,
    {
        assert_eq!(
            collection.kind(),
            GeomKind::Point,
            "PointProximityLayer expects a Point collection, got {:?}",
            collection.kind()
        );
        Self { name, collection, cost_fn: Box::new(cost_fn), influence_radius_m }
    }

    fn evaluate(&self, x: f64, y: f64, profile: Profile) -> f32 {
        let p = Point::new(x as f32, y as f32);
        let mut best: Option<(f64, u32)> = None;
        let r2 = (self.influence_radius_m as f64).powi(2);
        for fid in self.collection.query_point(p, self.influence_radius_m) {
            let pts = self.collection.feature_coords(fid);
            if pts.is_empty() {
                continue;
            }
            let q = pts[0];
            let dx = (q.x - p.x) as f64;
            let dy = (q.y - p.y) as f64;
            let d2 = dx * dx + dy * dy;
            if d2 > r2 {
                continue;
            }
            match best {
                Some((bd, _)) if bd <= d2 => {}
                _ => best = Some((d2, fid)),
            }
        }
        let Some((d2, fid)) = best else { return 1.0; };
        let attrs = self.collection.feature_attrs(fid);
        let dist = d2.sqrt();
        (self.cost_fn)(dist, &attrs, profile) as f32
    }
}

impl CostLayer for PointProximityLayer {
    fn name(&self) -> &'static str { self.name }

    fn cell_cost(&self, x: f64, y: f64, profile: Profile) -> CellCost {
        let m = self.evaluate(x, y, profile);
        if !m.is_finite() {
            CellCost::refused("point-refusal")
        } else {
            CellCost::multiplier(m)
        }
    }

    fn edge_cost_modifier(
        &self,
        fx: f64,
        fy: f64,
        tx: f64,
        ty: f64,
        profile: Profile,
    ) -> f32 {
        // Average the midpoint contribution. Point features are
        // small enough that finer integration isn't worthwhile;
        // the cell-level pass already samples the per-cell value.
        let mx = 0.5 * (fx + tx);
        let my = 0.5 * (fy + ty);
        self.evaluate(mx, my, profile)
    }

    fn covers(&self, _x: f64, _y: f64) -> bool {
        // Point-proximity layers don't establish "terrain coverage"
        // — they only bias what's already there. Leave the default.
        false
    }
}

/// Hard-refusal polygon layer. Use only when traversal is *truly*
/// impossible (ocean, building footprints, military exclusion).
/// For "expensive but crossable" features prefer
/// [`PolygonIntegralLayer`].
pub struct PolygonRefusalLayer {
    name: &'static str,
    collection: Arc<VectorCollection>,
    label: &'static str,
}

impl PolygonRefusalLayer {
    pub fn new(
        name: &'static str,
        collection: Arc<VectorCollection>,
        label: &'static str,
    ) -> Self {
        assert_eq!(
            collection.kind(),
            GeomKind::Polygon,
            "PolygonRefusalLayer expects a Polygon collection, got {:?}",
            collection.kind()
        );
        Self { name, collection, label }
    }

    fn point_inside(&self, x: f64, y: f64) -> bool {
        use turbo_tiles_geom::point_in_polygon;
        let p = Point::new(x as f32, y as f32);
        for fid in self.collection.query_point(p, 0.0) {
            let ring = self.collection.feature_coords(fid);
            if point_in_polygon(p, ring) {
                return true;
            }
        }
        false
    }
}

impl CostLayer for PolygonRefusalLayer {
    fn name(&self) -> &'static str { self.name }

    fn cell_cost(&self, x: f64, y: f64, _profile: Profile) -> CellCost {
        if self.point_inside(x, y) {
            CellCost::refused(self.label)
        } else {
            CellCost::default()
        }
    }

    fn edge_cost_modifier(
        &self,
        fx: f64,
        fy: f64,
        tx: f64,
        ty: f64,
        _profile: Profile,
    ) -> f32 {
        // If any metre of the edge intersects a refusal polygon
        // the edge is impassable. Use the integral helper at zero
        // cost just to compute "did we hit any polygon at all".
        let a = Point::new(fx as f32, fy as f32);
        let b = Point::new(tx as f32, ty as f32);
        for fid in self.collection.query_segment(a, b, 0.0) {
            let ring = self.collection.feature_coords(fid);
            let len = segment_polygon_intersection_length(a, b, ring);
            if len > 0.0 {
                return f32::INFINITY;
            }
        }
        1.0
    }

    fn covers(&self, x: f64, y: f64) -> bool {
        let p = Point::new(x as f32, y as f32);
        self.collection.query_point(p, 0.0).next().is_some()
    }
}

/// Total length of polylines in a collection that lie within an AABB.
/// Useful for debug diagnostics ("how much stream length is in this
/// 5 km bbox?"). Not part of the cost path.
pub fn collection_polyline_length_in(collection: &VectorCollection, bbox: turbo_tiles_geom::Aabb) -> f64 {
    let mut acc = 0.0;
    for fid in collection.query_aabb(bbox) {
        acc += polyline_length(collection.feature_coords(fid));
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use turbo_tiles_vector::{AttrField, AttrSchema, AttrType, CollectionBuilder, GeomKind};

    fn build_water_with_two_tarns() -> Arc<VectorCollection> {
        // Tarn A: tiny 5×5 m square at (100, 100).
        // Tarn B: bigger 50×50 m square at (200, 200).
        let schema = AttrSchema {
            fields: vec![AttrField {
                name: "area_m2".to_string(),
                ty: AttrType::F32,
                offset: 0,
            }],
            bytes_per_feature: 4,
        };
        let mut cb = CollectionBuilder::new("water", GeomKind::Polygon, schema);
        let a = vec![
            Point::new(100.0, 100.0),
            Point::new(105.0, 100.0),
            Point::new(105.0, 105.0),
            Point::new(100.0, 105.0),
        ];
        cb.push_feature(&a, &25.0_f32.to_le_bytes()).unwrap();
        let b = vec![
            Point::new(200.0, 200.0),
            Point::new(250.0, 200.0),
            Point::new(250.0, 250.0),
            Point::new(200.0, 250.0),
        ];
        cb.push_feature(&b, &2500.0_f32.to_le_bytes()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v");
        turbo_tiles_vector::write_store_to_path(&path, vec![cb], 0).unwrap();
        // Keep dir alive by leaking it. Tests are short-lived.
        let store = Box::leak(Box::new(
            turbo_tiles_vector::VectorStore::open(&path).unwrap(),
        ));
        std::mem::forget(dir);
        store.collection("water").unwrap()
    }

    #[test]
    fn small_tarn_adds_cost_proportional_to_crossing() {
        let coll = build_water_with_two_tarns();
        // 80 effective metres per metre in water.
        let layer = PolygonIntegralLayer::new(
            "water",
            coll,
            |len, _attrs, _p| len * 80.0,
        );
        // Edge cuts straight through the tiny tarn: 5 m inside.
        // Edge length 200 m. Expected multiplier = 1 + (5*80)/200 = 3.0.
        let mul = layer.edge_cost_modifier(0.0, 102.0, 200.0, 102.0, Profile::Foot);
        assert!((mul - 3.0).abs() < 1e-2, "got {mul}");
    }

    #[test]
    fn edge_that_misses_water_is_neutral() {
        let coll = build_water_with_two_tarns();
        let layer = PolygonIntegralLayer::new(
            "water",
            coll,
            |len, _, _| len * 80.0,
        );
        // Edge nowhere near either tarn.
        let mul = layer.edge_cost_modifier(0.0, 0.0, 50.0, 0.0, Profile::Foot);
        assert!((mul - 1.0).abs() < 1e-3);
    }

    #[test]
    fn infinite_cost_propagates_as_inf_multiplier() {
        let coll = build_water_with_two_tarns();
        let layer = PolygonIntegralLayer::new(
            "water",
            coll,
            |_, _, _| f64::INFINITY,
        );
        let mul = layer.edge_cost_modifier(0.0, 102.0, 200.0, 102.0, Profile::Foot);
        assert!(mul.is_infinite());
    }

    #[test]
    fn refusal_layer_vetoes_intersecting_edges() {
        let coll = build_water_with_two_tarns();
        let layer = PolygonRefusalLayer::new("ocean", coll, "ocean");
        // Edge cuts the bigger tarn.
        let mul = layer.edge_cost_modifier(0.0, 225.0, 400.0, 225.0, Profile::Foot);
        assert!(mul.is_infinite());
        // Edge misses everything.
        let mul = layer.edge_cost_modifier(0.0, 0.0, 50.0, 0.0, Profile::Foot);
        assert!((mul - 1.0).abs() < 1e-3);
    }

    #[test]
    fn larger_water_body_is_more_expensive() {
        let coll = build_water_with_two_tarns();
        let layer = PolygonIntegralLayer::new(
            "water",
            coll,
            |len, _, _| len * 80.0,
        );
        // Tiny tarn crossing: 5 m through tarn A, edge len 200 m.
        let tiny = layer.edge_cost_modifier(0.0, 102.0, 200.0, 102.0, Profile::Foot);
        // Big tarn crossing: 50 m through tarn B, edge len 400 m.
        // Expected = 1 + (50*80)/400 = 11.0.
        let big = layer.edge_cost_modifier(0.0, 225.0, 400.0, 225.0, Profile::Foot);
        assert!((tiny - 3.0).abs() < 1e-2);
        assert!((big - 11.0).abs() < 1e-2);
        assert!(big > tiny);
    }

    #[test]
    fn line_crossing_layer_counts_then_scores() {
        // Build a stream that's a horizontal polyline at y=50.
        let schema = AttrSchema {
            fields: vec![AttrField {
                name: "width_m".to_string(),
                ty: AttrType::F32,
                offset: 0,
            }],
            bytes_per_feature: 4,
        };
        let mut cb = CollectionBuilder::new("streams", GeomKind::LineString, schema);
        let stream = vec![Point::new(0.0, 50.0), Point::new(100.0, 50.0)];
        cb.push_feature(&stream, &3.0_f32.to_le_bytes()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v");
        turbo_tiles_vector::write_store_to_path(&path, vec![cb], 0).unwrap();
        let store = Box::leak(Box::new(
            turbo_tiles_vector::VectorStore::open(&path).unwrap(),
        ));
        std::mem::forget(dir);
        let coll = store.collection("streams").unwrap();
        let layer = LineCrossingLayer::new(
            "streams",
            coll,
            |n, attrs, _p| {
                let w = attrs.f32("width_m").unwrap_or(2.0) as f64;
                (n as f64) * (10.0 + 5.0 * w)
            },
        );
        // Vertical edge crosses the stream once.
        // expected extra = 1 * (10 + 5*3) = 25
        // edge length = 100
        // multiplier = 1 + 25/100 = 1.25
        let mul = layer.edge_cost_modifier(50.0, 0.0, 50.0, 100.0, Profile::Foot);
        assert!((mul - 1.25).abs() < 1e-2, "got {mul}");
    }
}
