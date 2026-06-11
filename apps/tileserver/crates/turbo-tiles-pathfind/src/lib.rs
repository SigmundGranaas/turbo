//! Off-trail pathfinding.
//!
//! - `Pathfinder` composes the routing graph with a stack of
//!   [`cost::CostLayer`]s that score traversal of every mesh cell
//!   and every graph edge. Adding a new data source — marsh layer,
//!   ridge bonus, preferred-track set — means implementing the
//!   trait and registering an instance at boot.
//! - The off-trail solver is the FMM grade-limited path (see
//!   `fmm_adapter`); `core::off_trail_mesh` keeps only the inspect-
//!   surface geometry types.

// Design-level clippy lints we deliberately accept crate-wide:
//  - type_complexity: the cost layers store boxed closures
//    (`Box<dyn Fn(..., &AttrView, Profile) -> f64 + Send + Sync>`) — naming
//    each via a type alias hurts more than it helps.
//  - arc_with_non_send_sync: the solver recorder/tracer Arcs are
//    thread-local plumbing; Arc keeps the API uniform.
//  - too_many_arguments: a couple of internal solve entry points thread
//    many tuning knobs; grouping them into a struct is a separate refactor.
#![allow(
    clippy::type_complexity,
    clippy::arc_with_non_send_sync,
    clippy::too_many_arguments
)]

pub mod config;
pub mod contributor;
pub mod core;
pub mod cost;
pub(crate) mod cost_field;
pub mod fmm_adapter;
pub mod layers;
pub mod native_contributors;
pub mod pathfinder;
pub mod solver_trace;
pub mod tracer;
pub mod unified;
pub mod vector_layers;

pub use config::{
    BaseConfig, ConfigError, CostConfig, CostConfigPatch, OffTrailConfig, Preset, PresetSet,
    ProfileSurface, SlopeConfig, SurfaceMultipliers, TotalGainConfig, TrailProximityConfig,
};
pub use contributor::{
    compose_edge_walk_seconds, ContributorKind, CostContributor, EdgeContext, EdgeElevProbe,
    EdgeKind, EdgeWalkCost, LegacyLayerAdapter, NamedContribution, BASE_PACE_S_PER_M,
};
pub use core::off_trail_mesh::{CostSample, MeshBbox, Point2, RefusedPolygon};
pub use cost::{compose_cell, compose_edge, CellCost, CostLayer};
pub use layers::{
    AvalancheTerrainLayer, DirectionalSlopeLayer, GraphSlopeLayer, LandcoverLayer, MarkingLayer,
    MaskRefusalLayer, PreferredEdgeLayer, SlopeLayer, TotalGainLayer, TrailProximityLayer,
};
pub use native_contributors::{
    has_native_replacement, AvalancheTerrainContributor, ContourCrossingContributor,
    DemCoveragePenaltyContributor, DirectionalSlopeContributor, GraphSlopeContributor,
    LandcoverContributor, LineCrossingContributor, MarkingBonusContributor, MaskRefusalContributor,
    NaismithGainContributor, PointProximityContributor, PolygonIntegralContributor,
    PolygonRefusalContributor, PreferredEdgeContributor, SurfacePaceContributor,
    ToblerSlopeContributor, TotalGainContributor, TrailProximityContributor,
    DISPLACED_LEGACY_LAYERS,
};
pub use pathfinder::{
    utm33n_to_wgs84, CostMode, Inspect, InspectCell, InspectLayer, InspectPoint, LegKind, Path,
    PathLeg, PathStrategy, PathfindError, Pathfinder, Prefs, WaypointLeg,
};
pub use solver_trace::{PhaseFrame, Recorder, SolverEvent, SolverRecording};
pub use tracer::{LayerStats, MeshStats, PhaseTime, TraceSnapshot, Tracer};
pub use vector_layers::{
    collection_polyline_length_in, LineCrossingLayer, PointProximityLayer, PolygonIntegralLayer,
    PolygonRefusalLayer,
};
