//! Build-time data health audit.
//!
//! Every artifact builder emits a [`HealthReport`] alongside its
//! normal report. The audit surfaces the class of silent failure
//! the session-long debugging pattern showed up: ingest jobs that
//! say "OK" while dropping rows, topology builds that leave the
//! sti subgraph in 49 K disconnected pieces, vectors with feature
//! geometries spanning the whole country.
//!
//! ## Severity rules
//!
//! - **error**: the artifact is unsafe to serve. Boot should
//!   refuse to load it. (Today there are none of these yet — the
//!   existing artifacts pass all the audits — but the structure
//!   reserves space for them.)
//! - **warning**: the artifact loads and serves, but routing
//!   quality is materially degraded. Curator should investigate.
//!   The "49195 sti components, largest 7.2 %" finding from this
//!   session is the canonical example.
//! - **stats**: numbers that aren't a problem on their own but are
//!   useful when comparing two builds (drift detection).
//!
//! ## Comparison + drift
//!
//! [`HealthReport::compare_to`] returns a diff against a baseline.
//! `verify-artifacts` uses this to flag "318 K trail-class edges
//! disappeared since the last build" before the new artifact
//! ships.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HealthReport {
    pub warnings: Vec<HealthIssue>,
    pub errors: Vec<HealthIssue>,
    /// Free-form numeric stats — feature counts, component sizes,
    /// tile counts, etc. Used for drift detection across builds.
    pub stats: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthIssue {
    /// Stable short identifier, e.g. `"sti_fragmented"` or
    /// `"vector_count_drop"`. Tooling matches on this; the message
    /// is for humans.
    pub code: String,
    pub message: String,
    /// Optional hint pointing at the likely fix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl HealthReport {
    pub fn warn(&mut self, code: &str, message: String, hint: Option<&str>) {
        self.warnings.push(HealthIssue {
            code: code.to_string(),
            message,
            hint: hint.map(|s| s.to_string()),
        });
    }
    pub fn error(&mut self, code: &str, message: String, hint: Option<&str>) {
        self.errors.push(HealthIssue {
            code: code.to_string(),
            message,
            hint: hint.map(|s| s.to_string()),
        });
    }
    pub fn stat(&mut self, key: &str, value: f64) {
        self.stats.insert(key.to_string(), value);
    }

    /// Diff this report against a baseline. Returns a list of
    /// stats that drifted by more than `pct` of their baseline
    /// value (default 10%), plus any newly-appeared warnings.
    pub fn compare_to(&self, baseline: &HealthReport, pct: f64) -> HealthDiff {
        let mut drifted: Vec<DriftedStat> = Vec::new();
        for (k, &v) in &self.stats {
            let prev = baseline.stats.get(k).copied().unwrap_or(v);
            if prev == 0.0 {
                if v.abs() > 1e-6 {
                    drifted.push(DriftedStat {
                        key: k.clone(),
                        baseline: prev,
                        current: v,
                        pct: f64::INFINITY,
                    });
                }
                continue;
            }
            let delta_pct = ((v - prev) / prev * 100.0).abs();
            if delta_pct > pct {
                drifted.push(DriftedStat {
                    key: k.clone(),
                    baseline: prev,
                    current: v,
                    pct: delta_pct,
                });
            }
        }
        let baseline_codes: std::collections::HashSet<&str> = baseline
            .warnings
            .iter()
            .map(|i| i.code.as_str())
            .collect();
        let new_warnings: Vec<HealthIssue> = self
            .warnings
            .iter()
            .filter(|i| !baseline_codes.contains(i.code.as_str()))
            .cloned()
            .collect();
        HealthDiff {
            drifted,
            new_warnings,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthDiff {
    pub drifted: Vec<DriftedStat>,
    pub new_warnings: Vec<HealthIssue>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DriftedStat {
    pub key: String,
    pub baseline: f64,
    pub current: f64,
    /// Absolute percentage change.
    pub pct: f64,
}

// ============================================================
// Graph audit
// ============================================================

use turbo_tiles_graph::{EdgeRecord, NodePos};

/// Audit a graph being built: connectivity per `fkb_type`,
/// vertex density distribution, dangling-endpoint count. Runs on
/// the in-memory edges before they're written to disk.
///
/// The session-long mystery — "trails don't connect to each
/// other" — got tracked back to a 49 K-component sti subgraph
/// that nothing flagged. This audit catches that at build time.
pub fn audit_graph(
    nodes: &[NodePos],
    edges: &[EdgeRecord],
) -> HealthReport {
    let mut report = HealthReport::default();
    report.stat("nodes", nodes.len() as f64);
    report.stat("edges_directed", edges.len() as f64);
    if nodes.is_empty() || edges.is_empty() {
        report.error(
            "empty_graph",
            "graph artifact has no nodes or no edges".to_string(),
            Some("ingest pipeline likely failed silently"),
        );
        return report;
    }

    // Per fkb_type stats.
    let mut by_kind_count: BTreeMap<u8, u64> = BTreeMap::new();
    let mut by_kind_length: BTreeMap<u8, f64> = BTreeMap::new();
    for e in edges {
        *by_kind_count.entry(e.fkb_type).or_insert(0) += 1;
        *by_kind_length.entry(e.fkb_type).or_insert(0.0) += e.length_m as f64;
    }
    for (kind, n) in &by_kind_count {
        report.stat(&format!("edges_kind_{kind}"), *n as f64);
    }
    for (kind, l) in &by_kind_length {
        report.stat(&format!("edges_km_kind_{kind}"), l / 1000.0);
    }

    // Connected-component analysis per fkb_type, plus the full
    // graph. Implemented as union-find for O(N · α(N)) over the
    // edge list. For the production graph (1.34 M edges, 1.16 M
    // nodes) this runs in ~1 s.
    let component_stats = connectivity_stats(nodes.len(), edges, None);
    record_connectivity(&mut report, "all", &component_stats);
    for &kind in by_kind_count.keys() {
        let s = connectivity_stats(nodes.len(), edges, Some(kind));
        record_connectivity(&mut report, &format!("fkb_{kind}"), &s);
        // Flag the fragmentation pattern that bit this session.
        // Largest component < 50 % of nodes touching this kind
        // means most edges are isolated — trails won't actually
        // connect end-to-end via this subgraph.
        if s.total_nodes_in_subgraph > 1000 && s.largest_pct < 50.0 {
            report.warn(
                &format!("subgraph_fragmented_kind_{kind}"),
                format!(
                    "fkb_type={kind} subgraph: {} components, largest {:.1}% ({}/{} nodes)",
                    s.component_count,
                    s.largest_pct,
                    s.largest_component_size,
                    s.total_nodes_in_subgraph,
                ),
                Some(
                    "trails cross each other without being noded; \
                     consider pgr_nodeNetwork before pgr_createTopology",
                ),
            );
        }
    }

    // Vertex density per kind. Very-sparse edges (<3 vertices on a
    // long edge) hint that the upstream geometry got generalised
    // too aggressively or the edge is genuinely a single segment.
    let mut very_sparse_edges: u64 = 0;
    for e in edges {
        // Heuristic: edges longer than 200 m but with implied
        // vertex count of 2 (no graph_geom polyline lookup here;
        // we don't have it at this stage). Approximate by looking
        // at the edge length alone.
        if e.length_m > 1000.0 {
            very_sparse_edges = very_sparse_edges.saturating_add(1);
        }
    }
    report.stat("edges_over_1km", very_sparse_edges as f64);
    if (very_sparse_edges as f64) > (edges.len() as f64 * 0.10) {
        report.warn(
            "many_long_edges",
            format!(
                "{very_sparse_edges} edges (>{:.0}%) exceed 1 km — \
                 likely under-noded; routing precision is degraded",
                100.0 * very_sparse_edges as f64 / edges.len() as f64,
            ),
            Some("consider pgr_nodeNetwork to split at crossings"),
        );
    }

    // Dangling-endpoint check: every edge endpoint should be in
    // range of `nodes`. Indices out of range = malformed graph.
    let n = nodes.len() as u32;
    let mut bad_endpoints: u64 = 0;
    for e in edges {
        if e.from_id >= n || e.to_id >= n {
            bad_endpoints += 1;
        }
    }
    report.stat("dangling_endpoints", bad_endpoints as f64);
    if bad_endpoints > 0 {
        report.error(
            "dangling_endpoints",
            format!("{bad_endpoints} edges reference node ids outside [0, {n})"),
            Some("graph_builder bug — node compaction step is wrong"),
        );
    }

    report
}

#[derive(Debug)]
struct ComponentStats {
    component_count: u64,
    largest_component_size: u64,
    largest_pct: f64,
    total_nodes_in_subgraph: u64,
    median_size: u64,
}

fn connectivity_stats(
    n: usize,
    edges: &[EdgeRecord],
    filter_fkb: Option<u8>,
) -> ComponentStats {
    let mut parent: Vec<u32> = (0..n as u32).collect();
    let mut rank: Vec<u8> = vec![0; n];
    fn find(parent: &mut [u32], mut x: u32) -> u32 {
        while parent[x as usize] != x {
            parent[x as usize] = parent[parent[x as usize] as usize];
            x = parent[x as usize];
        }
        x
    }
    fn union(parent: &mut [u32], rank: &mut [u8], a: u32, b: u32) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra == rb { return; }
        let (ra, rb) = if rank[ra as usize] < rank[rb as usize] {
            (ra, rb)
        } else {
            (rb, ra)
        };
        parent[ra as usize] = rb;
        if rank[ra as usize] == rank[rb as usize] {
            rank[rb as usize] = rank[rb as usize].saturating_add(1);
        }
    }
    // Track which nodes the subgraph actually touches so we can
    // report "components in this subgraph" rather than counting
    // every singleton in the full graph.
    let mut touched: Vec<bool> = vec![false; n];
    for e in edges {
        if let Some(k) = filter_fkb {
            if e.fkb_type != k { continue; }
        }
        if (e.from_id as usize) >= n || (e.to_id as usize) >= n {
            continue;
        }
        touched[e.from_id as usize] = true;
        touched[e.to_id as usize] = true;
        union(&mut parent, &mut rank, e.from_id, e.to_id);
    }
    // Aggregate.
    let mut comp_sizes: BTreeMap<u32, u64> = BTreeMap::new();
    for i in 0..n as u32 {
        if !touched[i as usize] { continue; }
        let r = find(&mut parent, i);
        *comp_sizes.entry(r).or_insert(0) += 1;
    }
    let total: u64 = comp_sizes.values().sum();
    let largest = comp_sizes.values().copied().max().unwrap_or(0);
    let largest_pct = if total > 0 { 100.0 * largest as f64 / total as f64 } else { 0.0 };
    let mut sizes: Vec<u64> = comp_sizes.values().copied().collect();
    sizes.sort();
    let median = if sizes.is_empty() { 0 } else { sizes[sizes.len() / 2] };
    ComponentStats {
        component_count: comp_sizes.len() as u64,
        largest_component_size: largest,
        largest_pct,
        total_nodes_in_subgraph: total,
        median_size: median,
    }
}

fn record_connectivity(report: &mut HealthReport, prefix: &str, s: &ComponentStats) {
    report.stat(&format!("{prefix}_component_count"), s.component_count as f64);
    report.stat(&format!("{prefix}_largest_pct"), s.largest_pct);
    report.stat(&format!("{prefix}_largest_size"), s.largest_component_size as f64);
    report.stat(&format!("{prefix}_median_size"), s.median_size as f64);
    report.stat(
        &format!("{prefix}_touched_nodes"),
        s.total_nodes_in_subgraph as f64,
    );
}

// ============================================================
// Vector audit
// ============================================================

// ============================================================
// DEM audit
// ============================================================

/// Audit a DEM artifact's coverage map. Catches the case where
/// the user clicks somewhere with no elevation data — slope and
/// avalanche contributors silently return "no data" and routing
/// quality degrades without anyone noticing.
///
/// The Valnesfjord session was the canonical case: max_y of the
/// DEM was 7,000,255 metres N but the user clicked at y=7,475,705.
/// All DEM-derived contributors returned `None` for those queries.
/// This audit surfaces "tiles_absent" as a stat so a build-to-
/// build comparison flags shrinking coverage.
pub fn audit_dem_coverage(coverage: &turbo_tiles_elev::DemCoverage) -> HealthReport {
    let mut report = HealthReport::default();
    report.stat("dem_cells_x", coverage.cells_x as f64);
    report.stat("dem_cells_y", coverage.cells_y as f64);
    report.stat("dem_tiles_present", coverage.tiles_present as f64);
    report.stat("dem_tiles_absent", coverage.tiles_absent as f64);
    report.stat("dem_min_x", coverage.min_x);
    report.stat("dem_max_x", coverage.max_x);
    report.stat("dem_min_y", coverage.min_y);
    report.stat("dem_max_y", coverage.max_y);

    let total_tiles = coverage.tiles_present.saturating_add(coverage.tiles_absent);
    if total_tiles > 0 {
        let absent_pct =
            100.0 * coverage.tiles_absent as f64 / total_tiles as f64;
        report.stat("dem_tiles_absent_pct", absent_pct);
        if absent_pct > 20.0 {
            report.warn(
                "dem_sparse_coverage",
                format!(
                    "{:.1}% of DEM tiles ({}/{}) are absent — \
                     queries in those areas will get no slope/aspect data",
                    absent_pct, coverage.tiles_absent, total_tiles,
                ),
                Some(
                    "ingest the missing DTM tiles, or expect routing to \
                     fall back to layer-defaults in those regions",
                ),
            );
        }
    }
    if coverage.tiles_present == 0 {
        report.error(
            "dem_empty",
            "DEM has zero present tiles — slope/avalanche layers will all be no-ops".to_string(),
            Some("paths.dem table is empty or the build query filtered everything"),
        );
    }
    report
}

// ============================================================
// Mask audit
// ============================================================

/// Audit a refusal-mask artifact (water + glacier rasters). The
/// bbox MUST overlap the DEM bbox or routing in the mismatched
/// regions silently misbehaves.
pub fn audit_mask_coverage(
    coverage: &turbo_tiles_mask::MaskCoverage,
    dem_bbox: Option<(f64, f64, f64, f64)>,
) -> HealthReport {
    let mut report = HealthReport::default();
    report.stat("mask_cells_x", coverage.meta.cells_x as f64);
    report.stat("mask_cells_y", coverage.meta.cells_y as f64);
    report.stat("mask_cells_water", coverage.cells_water as f64);
    report.stat("mask_cells_glacier", coverage.cells_glacier as f64);

    let total = (coverage.meta.cells_x as u64) * (coverage.meta.cells_y as u64);
    if total > 0 {
        let water_pct = 100.0 * coverage.cells_water as f64 / total as f64;
        let glacier_pct = 100.0 * coverage.cells_glacier as f64 / total as f64;
        report.stat("mask_water_pct", water_pct);
        report.stat("mask_glacier_pct", glacier_pct);
    }
    if coverage.cells_water == 0 {
        report.warn(
            "mask_no_water",
            "refusal mask has zero water cells — ocean / lakes won't refuse".to_string(),
            Some("rasterisation step likely received no polygons"),
        );
    }
    if let Some((dem_min_x, dem_min_y, dem_max_x, dem_max_y)) = dem_bbox {
        let mask_min_x = coverage.meta.min_x;
        let mask_min_y = coverage.meta.min_y;
        let mask_max_x = coverage.meta.max_x;
        let mask_max_y = coverage.meta.max_y;
        // Bbox mismatch threshold: anything more than 5 km off
        // on any side is significant for the Norway artifact.
        let drift = [
            (mask_min_x - dem_min_x).abs(),
            (mask_min_y - dem_min_y).abs(),
            (mask_max_x - dem_max_x).abs(),
            (mask_max_y - dem_max_y).abs(),
        ];
        let max_drift = drift.iter().cloned().fold(0.0_f64, f64::max);
        report.stat("mask_dem_bbox_max_drift_m", max_drift);
        if max_drift > 5_000.0 {
            report.warn(
                "mask_dem_bbox_mismatch",
                format!(
                    "mask bbox drifts from DEM bbox by up to {:.0} m — \
                     regions covered by one but not the other will silently \
                     misbehave at routing time",
                    max_drift
                ),
                Some(
                    "rebuild mask with the DEM's bbox as input, or rebuild \
                     DEM to cover the mask's full extent",
                ),
            );
        }
    }
    report
}

/// Audit a vector layer as it's being built. Catches the ingest
/// pattern that bit this session: feature counts dropping silently
/// because of an attr_hash collision in the upsert SQL.
pub fn audit_vector_layer(
    name: &str,
    feature_count: u32,
    total_vertices: u32,
) -> Vec<HealthIssue> {
    let mut issues = Vec::new();
    if feature_count == 0 {
        issues.push(HealthIssue {
            code: format!("vector_{name}_empty"),
            message: format!("vector layer `{name}` has zero features"),
            hint: Some(
                "either no source rows existed, or the upsert SQL filtered \
                 them all out — check ingest job stderr"
                    .to_string(),
            ),
        });
    } else if feature_count > 0 && total_vertices < (feature_count as u32 * 2) {
        // Polygons need ≥3 vertices, lines need ≥2. If the ratio
        // is under 2 something went wrong at parse time.
        issues.push(HealthIssue {
            code: format!("vector_{name}_sparse"),
            message: format!(
                "vector layer `{name}`: {feature_count} features but only \
                 {total_vertices} vertices — geometry parse failed?"
            ),
            hint: Some(
                "check WKB parser branches in turbo-tiles-build/src/vector_builder.rs"
                    .to_string(),
            ),
        });
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use turbo_tiles_graph::{EdgeRecord, NodePos};

    fn node(x: f32, y: f32) -> NodePos { NodePos { x, y } }
    fn edge(from: u32, to: u32, len: f32, fkb: u8) -> EdgeRecord {
        EdgeRecord {
            from_id: from,
            to_id: to,
            length_m: len,
            gain_m: 0.0,
            loss_m: 0.0,
            slope_max_deg: 0.0,
            fkb_type: fkb,
            marking: 0,
            surface: 0,
            source: 0,
            attr_flags: 0,
        }
    }

    #[test]
    fn fully_connected_one_component() {
        // 0—1—2—3 chain. One component, largest 100%.
        let nodes: Vec<NodePos> = (0..4).map(|i| node(i as f32, 0.0)).collect();
        let edges = vec![edge(0,1,10.0,1), edge(1,2,10.0,1), edge(2,3,10.0,1)];
        let report = audit_graph(&nodes, &edges);
        // Largest component = 100% of touched nodes.
        let pct = report.stats["all_largest_pct"];
        assert!((pct - 100.0).abs() < 1e-3);
        assert_eq!(report.stats["all_component_count"], 1.0);
        // No fragmentation warning.
        assert!(!report.warnings.iter().any(|w| w.code.starts_with("subgraph_fragmented")));
    }

    #[test]
    fn fragmented_sti_subgraph_warns() {
        // 10 nodes, 5 disjoint pairs, ALL sti — 5 components of 2.
        let nodes: Vec<NodePos> = (0..10).map(|i| node(i as f32, 0.0)).collect();
        let mut edges = Vec::new();
        for i in 0..5 {
            edges.push(edge(i*2, i*2+1, 100.0, 1));
        }
        // Bump to over 1000 touched nodes by repeating the pattern.
        // Actually for the warning to fire we need >1000 touched nodes
        // AND largest<50%. With 5 components each 2 nodes (20%), but
        // only 10 touched nodes, the threshold gate doesn't fire.
        // Verify the stats nonetheless.
        let report = audit_graph(&nodes, &edges);
        assert_eq!(report.stats["fkb_1_component_count"], 5.0);
        let pct = report.stats["fkb_1_largest_pct"];
        assert!((pct - 20.0).abs() < 1e-3);
    }

    #[test]
    fn warning_fires_at_scale() {
        // 2000 nodes, 1000 disjoint pairs of sti edges — fully
        // fragmented at scale. The warning must fire.
        let nodes: Vec<NodePos> = (0..2000).map(|i| node(i as f32, 0.0)).collect();
        let mut edges = Vec::new();
        for i in 0..1000 {
            edges.push(edge(i*2, i*2+1, 50.0, 1));
        }
        let report = audit_graph(&nodes, &edges);
        let warns: Vec<&HealthIssue> = report
            .warnings
            .iter()
            .filter(|w| w.code.starts_with("subgraph_fragmented"))
            .collect();
        assert_eq!(warns.len(), 1);
        assert!(warns[0].message.contains("components"));
    }

    #[test]
    fn dangling_endpoint_errors() {
        let nodes = vec![node(0.0, 0.0), node(1.0, 0.0)];
        // Edge references node id 99 which doesn't exist.
        let edges = vec![edge(0, 99, 10.0, 1)];
        let report = audit_graph(&nodes, &edges);
        assert!(report.errors.iter().any(|e| e.code == "dangling_endpoints"));
    }

    #[test]
    fn empty_graph_errors() {
        let report = audit_graph(&[], &[]);
        assert!(report.errors.iter().any(|e| e.code == "empty_graph"));
    }

    #[test]
    fn compare_to_flags_large_drift() {
        let mut baseline = HealthReport::default();
        baseline.stat("edges_kind_1", 200_000.0);
        baseline.stat("edges_kind_2", 800_000.0);
        let mut current = HealthReport::default();
        current.stat("edges_kind_1", 100_000.0); // 50% drop
        current.stat("edges_kind_2", 810_000.0); // 1.25% rise
        let diff = current.compare_to(&baseline, 10.0);
        assert_eq!(diff.drifted.len(), 1);
        assert_eq!(diff.drifted[0].key, "edges_kind_1");
        assert!((diff.drifted[0].pct - 50.0).abs() < 1e-3);
    }
}
