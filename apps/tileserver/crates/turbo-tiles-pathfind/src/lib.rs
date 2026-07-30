//! Off-trail pathfinding.
//!
//! - `Pathfinder` composes the routing graph with a stack of
//!   [`contributor::CostContributor`]s that price traversal of every mesh cell
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

pub(crate) mod avoid;
pub mod config;
pub mod contributor;
pub mod core;
pub(crate) mod cost_field;
pub mod fmm_adapter;
pub mod native_contributors;
pub mod pathfinder;
pub mod solver_trace;
pub mod tracer;
pub mod unified;

pub use config::{
    BaseConfig, ConfigError, CostConfig, CostConfigPatch, OffTrailConfig, Preset, PresetSet,
    ProfileSurface, SlopeConfig, SurfaceMultipliers, TotalGainConfig, TrailProximityConfig,
};
pub use contributor::{
    compose_edge_walk_seconds, ContributorKind, CostContributor, EdgeContext, EdgeElevProbe,
    EdgeKind, EdgeWalkCost, NamedContribution, BASE_PACE_S_PER_M,
};
// The engine's ports and vocabulary live in `turbo-route-model` (L1),
// which has zero dependencies — see that crate's docs for why. Re-exported
// here so callers of the engine need not name two crates to use one API.
pub use turbo_route_model::{Extent, Heightfield, ModeId, Point, Requirement, SlopeAspect};
pub use core::off_trail_mesh::{CostSample, MeshBbox, Point2, RefusedPolygon};
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
    CostMode, Inspect, InspectCell, InspectLayer, InspectPoint, LegKind, Path,
    PathLeg, PathStrategy, PathfindError, Pathfinder, Prefs, WaypointLeg,
};
pub use solver_trace::{PhaseFrame, Recorder, SolverEvent, SolverRecording};
pub use tracer::{LayerStats, MeshStats, PhaseTime, TraceSnapshot, Tracer};
