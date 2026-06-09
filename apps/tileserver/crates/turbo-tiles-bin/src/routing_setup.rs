//! Reusable routing-engine setup — the ONE place that opens primitive
//! artifacts and assembles the production [`Pathfinder`] (default layer
//! stack + vector cost layers + landcover masks).
//!
//! Both the HTTP server (`serve`) and the headless `eval-terrain`
//! command call these functions, so an autonomous evaluation loop
//! routes through the *identical* production layer stack the server
//! uses — no drift between "what we measured" and "what we ship".
//!
//! This is also the seam the routing-engine architecture plan
//! (`docs/architecture/2026-06-routing-engine-unification-plan.md`)
//! calls `build_pathfinder` / the future `RoutingExecutor`: resource
//! ownership extracted out of the request/serve path.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use turbo_tiles_elev::Dem;
use turbo_tiles_graph::Graph;
use turbo_tiles_mask::Mask;
use turbo_tiles_pathfind::{CostConfig, Pathfinder};
use turbo_tiles_search::Index;

/// Primitive handles loaded once from an artifacts directory. Missing
/// artifacts leave the corresponding field `None` (degraded mode) —
/// the same tolerance `serve` has always had.
#[derive(Default, Clone)]
pub struct RoutingArtifacts {
    pub dem: Option<Arc<Dem>>,
    pub mask: Option<Arc<Mask>>,
    pub graph: Option<Arc<Graph>>,
    pub search: Option<Arc<Index>>,
}

/// Open the primitive artifacts (`norway.{dem,mask,graph,anchors}` plus
/// the `graph_geom` sibling) from `dir`. Behaviour-identical to the
/// open-and-log block that used to live inline in `serve`.
pub fn load_routing_artifacts(dir: Option<&Path>) -> RoutingArtifacts {
    let mut art = RoutingArtifacts::default();
    let Some(dir) = dir else {
        return art;
    };

    let dem_path = dir.join("norway.dem");
    if dem_path.exists() {
        match Dem::open(&dem_path) {
            Ok(d) => {
                let cov = d.coverage();
                tracing::info!(
                    path = %dem_path.display(),
                    cells_x = cov.cells_x,
                    cells_y = cov.cells_y,
                    tiles_present = cov.tiles_present,
                    tiles_absent = cov.tiles_absent,
                    file_size_bytes = cov.file_size_bytes,
                    "loaded DEM artifact"
                );
                art.dem = Some(Arc::new(d));
            }
            Err(e) => {
                tracing::error!(error = %e, path = %dem_path.display(), "failed to open DEM artifact; running in degraded mode")
            }
        }
    } else {
        tracing::warn!(path = %dem_path.display(), "DEM artifact not present; elev endpoints will return 503");
    }

    let mask_path = dir.join("norway.mask");
    if mask_path.exists() {
        match Mask::open(&mask_path) {
            Ok(m) => {
                let cov = m.coverage();
                tracing::info!(
                    path = %mask_path.display(),
                    cells_x = cov.meta.cells_x,
                    cells_y = cov.meta.cells_y,
                    file_size_bytes = cov.file_size_bytes,
                    cells_water = cov.cells_water,
                    cells_glacier = cov.cells_glacier,
                    "loaded refusal mask artifact"
                );
                art.mask = Some(Arc::new(m));
            }
            Err(e) => {
                tracing::error!(error = %e, path = %mask_path.display(), "failed to open mask artifact")
            }
        }
    }

    let graph_path = dir.join("norway.graph");
    if graph_path.exists() {
        match Graph::open(&graph_path) {
            Ok(mut g) => {
                // Attach the polyline sibling artifact if it sits next
                // to the graph. Missing/malformed is non-fatal — routes
                // degrade to straight-segment geometry rather than
                // failing.
                let geom_path = dir.join("norway.graph_geom");
                if geom_path.exists() {
                    match g.attach_geom(&geom_path) {
                        Ok(_) => tracing::info!(
                            path = %geom_path.display(),
                            "attached graph_geom artifact (high-fidelity polylines)"
                        ),
                        Err(e) => tracing::warn!(
                            error = %e,
                            path = %geom_path.display(),
                            "failed to attach graph_geom; routes will use endpoint segments"
                        ),
                    }
                }
                let s = g.stats();
                tracing::info!(
                    path = %graph_path.display(),
                    nodes = s.meta.node_count,
                    edges = s.meta.edge_count,
                    file_size_bytes = s.file_size_bytes,
                    has_polylines = g.has_geom(),
                    "loaded routing graph artifact"
                );
                art.graph = Some(Arc::new(g));
            }
            Err(e) => {
                tracing::error!(error = %e, path = %graph_path.display(), "failed to open graph artifact")
            }
        }
    }

    let anchors_path = dir.join("norway.anchors");
    if anchors_path.exists() {
        match Index::open(&anchors_path) {
            Ok(s) => {
                let st = s.stats();
                tracing::info!(
                    path = %anchors_path.display(),
                    count = st.meta.count,
                    file_size_bytes = st.file_size_bytes,
                    "loaded anchor search artifact"
                );
                art.search = Some(Arc::new(s));
            }
            Err(e) => {
                tracing::error!(error = %e, path = %anchors_path.display(), "failed to open search artifact")
            }
        }
    }

    art
}

/// Assemble the production [`Pathfinder`]: default layer stack from
/// `cost_config`, then the vector cost layers and landcover masks
/// discovered under `dir`. Returns the Pathfinder plus the landcover
/// masks that were actually registered (the HTTP server hands these to
/// the inspect endpoints via `ApiState.landcover`; the eval command
/// ignores them).
///
/// Behaviour-identical to the assembly block that used to live inline
/// in `serve` — including the rule that a mask landcover layer is
/// SKIPPED (and therefore not returned in the map) when a vector
/// collection of the same name supersedes it.
pub fn build_pathfinder(
    dir: Option<&Path>,
    art: &RoutingArtifacts,
    cost_config: CostConfig,
) -> (Pathfinder, HashMap<&'static str, Arc<Mask>>) {
    let mut pf = Pathfinder::with_defaults_and_config(
        art.dem.clone(),
        art.mask.clone(),
        art.graph.clone(),
        cost_config,
    );

    let mut landcover: HashMap<&'static str, Arc<Mask>> = HashMap::new();
    let mut taken_layer_names: HashSet<&'static str> = HashSet::new();

    // ---- Vector cost layers -------------------------------------------
    //
    // If `norway.vectors` is present, register the per-feature-class cost
    // layers from it. These supersede the equivalent rasterised mask
    // layers — water, wetland, streams, cultivated, building — because
    // they preserve the original polygon shape and integrate cost along
    // each candidate edge instead of vetoing whole 25 m cells.
    if let Some(dir) = dir {
        let vec_path = dir.join("norway.vectors");
        if vec_path.exists() {
            match turbo_tiles_vector::VectorStore::open(&vec_path) {
                Ok(store) => {
                    tracing::info!(
                        path = %vec_path.display(),
                        collections = ?store.collection_names(),
                        "loaded vectors artifact"
                    );
                    if let Some(coll) = store.try_collection("water") {
                        // Width-proportional crossing cost: every metre of
                        // open water on an edge adds WATER_CROSS_PENALTY_PER_M
                        // "effective metres". Because it scales with crossing
                        // WIDTH, a small tarn stays cheap to ford while a real
                        // lake/river is effectively impassable — water is only
                        // ever chosen as an absolute last resort.
                        const WATER_CROSS_PENALTY_PER_M: f64 = 400.0;
                        let legacy = turbo_tiles_pathfind::PolygonIntegralLayer::new(
                            "water",
                            coll.clone(),
                            |len, _attrs, _p| len * WATER_CROSS_PENALTY_PER_M,
                        );
                        let native = turbo_tiles_pathfind::PolygonIntegralContributor::new(
                            "water",
                            coll,
                            |len, _attrs, _p| len * WATER_CROSS_PENALTY_PER_M,
                        );
                        pf.push_with_native(Arc::new(legacy), Arc::new(native));
                        taken_layer_names.insert("water");
                        // Tell the raster `mask_refusal` layer to stop
                        // vetoing water cells — the integral layer now
                        // handles them with proper edge-length cost.
                        if let Some(m) = art.mask.clone() {
                            pf.defer_mask_water_to_vector(m);
                            tracing::info!(
                                "deferred raster mask water refusal to vector water layer"
                            );
                        }
                    }
                    if let Some(coll) = store.try_collection("wetland") {
                        let legacy = turbo_tiles_pathfind::PolygonIntegralLayer::new(
                            "wetland",
                            coll.clone(),
                            |len, _attrs, _p| len * 1.5,
                        );
                        let native = turbo_tiles_pathfind::PolygonIntegralContributor::new(
                            "wetland",
                            coll,
                            |len, _attrs, _p| len * 1.5,
                        );
                        pf.push_with_native(Arc::new(legacy), Arc::new(native));
                        taken_layer_names.insert("wetland");
                    }
                    if let Some(coll) = store.try_collection("cultivated") {
                        // Innmark — soft penalty so the solver routes
                        // around farmyards but doesn't refuse them.
                        let legacy = turbo_tiles_pathfind::PolygonIntegralLayer::new(
                            "cultivated",
                            coll.clone(),
                            |len, _attrs, _p| len * 3.0,
                        );
                        let native = turbo_tiles_pathfind::PolygonIntegralContributor::new(
                            "cultivated",
                            coll,
                            |len, _attrs, _p| len * 3.0,
                        );
                        pf.push_with_native(Arc::new(legacy), Arc::new(native));
                        taken_layer_names.insert("cultivated");
                    }
                    if let Some(coll) = store.try_collection("ocean") {
                        // Saltwater is a hard veto — you cannot wade
                        // across a fjord.
                        let legacy = turbo_tiles_pathfind::PolygonRefusalLayer::new(
                            "ocean",
                            coll.clone(),
                            "ocean",
                        );
                        let native =
                            turbo_tiles_pathfind::PolygonRefusalContributor::new("ocean", coll, "ocean");
                        pf.push_with_native(Arc::new(legacy), Arc::new(native));
                        taken_layer_names.insert("ocean");
                    }
                    if let Some(coll) = store.try_collection("building") {
                        // Buildings are truly impassable — refusal layer,
                        // not integral.
                        let legacy = turbo_tiles_pathfind::PolygonRefusalLayer::new(
                            "building",
                            coll.clone(),
                            "building",
                        );
                        let native = turbo_tiles_pathfind::PolygonRefusalContributor::new(
                            "building", coll, "building",
                        );
                        pf.push_with_native(Arc::new(legacy), Arc::new(native));
                        taken_layer_names.insert("building");
                    }
                    if let Some(coll) = store.try_collection("streams") {
                        // Stream crossings — width-aware cost. Width is
                        // metres; crossing cost = 10 + 5×width metres per
                        // crossing.
                        let legacy = turbo_tiles_pathfind::LineCrossingLayer::new(
                            "streams",
                            coll.clone(),
                            |n, attrs, _p| {
                                let w = attrs.f32("width_m").unwrap_or(2.0) as f64;
                                (n as f64) * (10.0 + 5.0 * w)
                            },
                        );
                        let native = turbo_tiles_pathfind::LineCrossingContributor::new(
                            "streams",
                            coll,
                            |n, attrs, _p| {
                                let w = attrs.f32("width_m").unwrap_or(2.0) as f64;
                                (n as f64) * (10.0 + 5.0 * w)
                            },
                        );
                        pf.push_with_native(Arc::new(legacy), Arc::new(native));
                        taken_layer_names.insert("streams");
                        // Mask-based stream_barrier + bridge_zone are
                        // strictly weaker than this — skip both.
                        taken_layer_names.insert("stream_barrier");
                        taken_layer_names.insert("bridge_zone");
                    }
                }
                Err(e) => tracing::error!(
                    error = %e,
                    path = %vec_path.display(),
                    "failed to open vectors artifact"
                ),
            }
        }
    }

    // ---- Landcover mask layers ----------------------------------------
    if let Some(dir) = dir {
        // (file suffix, layer name, cost multiplier when class present)
        let landcover_specs: &[(&str, &'static str, f32)] = &[
            ("norway.wetland.mask", "wetland", 2.5),
            ("norway.forest.mask", "forest", 1.4),
            ("norway.open.mask", "open", 0.95),
            ("norway.cultivated.mask", "cultivated", 4.0),
            ("norway.developed.mask", "developed", 2.5),
            ("norway.building.mask", "building", f32::INFINITY),
            ("norway.stream_barrier.mask", "stream_barrier", 4.0),
            ("norway.bridge_zone.mask", "bridge_zone", 0.25),
        ];
        for (filename, layer_name, multiplier) in landcover_specs {
            // Skip mask layers superseded by vector collections of the
            // same name — they're strictly worse (cell-grid all-or-
            // nothing) and would double-count cost.
            if taken_layer_names.contains(layer_name) {
                tracing::info!(
                    layer = layer_name,
                    "skipping mask landcover; vector layer present"
                );
                continue;
            }
            let path = dir.join(filename);
            if !path.exists() {
                continue;
            }
            match Mask::open(&path) {
                Ok(m) => {
                    let cov = m.coverage();
                    tracing::info!(
                        path = %path.display(),
                        layer = layer_name,
                        present_cells = cov.cells_water,
                        multiplier,
                        "loaded landcover layer"
                    );
                    let arc = Arc::new(m);
                    // Two consumers: the pathfinder layer stack (cost
                    // composition) and the SPA inspect endpoints
                    // (visualisation). Share via Arc.
                    landcover.insert(*layer_name, arc.clone());
                    let legacy = turbo_tiles_pathfind::LandcoverLayer {
                        mask: arc.clone(),
                        layer_name,
                        multiplier: *multiplier,
                    };
                    // Translate the legacy multiplier into a walk-
                    // seconds-per-metre delta against the flat-trail
                    // baseline. Infinity multipliers (building) become a
                    // hard veto on the native side via
                    // `LandcoverContributor::veto`.
                    let delta_s_per_m = if multiplier.is_infinite() {
                        f64::INFINITY
                    } else {
                        ((*multiplier as f64) - 1.0) * turbo_tiles_pathfind::BASE_PACE_S_PER_M
                    };
                    let native = turbo_tiles_pathfind::LandcoverContributor::new(
                        arc,
                        layer_name,
                        delta_s_per_m,
                    );
                    pf.push_with_native(Arc::new(legacy), Arc::new(native));
                }
                Err(e) => {
                    tracing::error!(error = %e, path = %path.display(), "failed to open landcover artifact")
                }
            }
        }
    }

    tracing::info!(layers = ?pf.layer_names(), "pathfinder assembled");
    (pf, landcover)
}

/// Load the boot cost configuration with the same precedence `serve`
/// uses: explicit env var → `cost-config.toml` relative to CWD →
/// embedded defaults compiled into the binary.
pub fn load_cost_config() -> CostConfig {
    CostConfig::load_or_default(None).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to load cost-config; falling back to embedded defaults");
        CostConfig::from_embedded().expect("embedded cost-config defaults must parse")
    })
}
