//! `place-core` — the single source of truth for Turbo's search and
//! reverse-geocoding *decision* logic.
//!
//! This crate is pure: no I/O, no HTTP, no SQLite. Each platform does its own
//! data access (PostGIS KNN on the server, SQLite R*Tree/FTS on device, or the
//! live Kartverket backends during the migration) and funnels the resulting
//! [`ReverseInput`] into [`reverse_geocode`]. The behaviour is pinned by
//! `golden.json`, run from `tests/golden.rs` and (in later phases) mirrored by a
//! thin smoke test on each FFI binding.
//!
//! Tuning (distance bands, qualifiers, penalties) lives in the versioned
//! [`Ruleset`] (`ruleset.v1.json`), not in code — so the numbers can ship
//! without recompiling the native library. The *algorithm* (classification +
//! cascade + dedup) lives here and is shared by every runtime.

mod classify;
mod geo;
mod model;
mod rank;
mod ruleset;

pub use classify::classify;
pub use geo::haversine_m;
pub use model::{
    Address, Candidate, Kommune, LocationDescription, ProtectedArea, Qualifier, ReverseInput, Tier,
};
pub use rank::reverse_geocode;
pub use ruleset::{ClassifyRule, Ruleset};
