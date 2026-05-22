//! pgRouting integration. Lands in M5. Cost functions per profile,
//! `pgr_dijkstraWithinBox` wrappers, isochrone via `pgr_drivingDistance`.

pub mod profile;

/// Placeholder so downstream crates can reference the module while M5
/// is unstarted. Replace with the real planner.
pub async fn route_placeholder() -> Result<(), &'static str> {
    Err("not yet implemented (M5)")
}
