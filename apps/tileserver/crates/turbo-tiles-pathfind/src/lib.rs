//! Off-trail pathfinding.
//!
//! - `Pathfinder` composes the routing graph with a stack of
//!   [`cost::CostLayer`]s that score traversal of every mesh cell
//!   and every graph edge. Adding a new data source — marsh layer,
//!   ridge bonus, preferred-track set — means implementing the
//!   trait and registering an instance at boot.
//! - Theta\* + local mesh builder live in `core::off_trail*`. Pure;
//!   no I/O.

pub mod config;
pub mod contributor;
pub mod core;
pub mod cost;
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
    compose_edge_walk_seconds, ContributorKind, CostContributor, EdgeContext, EdgeKind,
    EdgeWalkCost, LegacyLayerAdapter, NamedContribution, BASE_PACE_S_PER_M,
};
pub use core::off_trail::{theta_star, Mesh, MeshNode, MeshNodeId, PathResult, Point2};
pub use core::off_trail_connector::{nearest_exit_via_mesh, OffTrailConnector};
pub use core::off_trail_mesh::{
    build_local_mesh, BuiltMesh, CostSample, ExitNode, MeshBbox, MeshBuildInput, RefusedPolygon,
};
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
