//! MVT tile generation and per-resource GeoJSON list/detail queries.
//!
//! The MVT pipeline runs entirely in Postgres: `ST_AsMVTGeom` projects
//! the 25833-stored geometry to 3857 inside the tile envelope, and
//! `ST_AsMVT` aggregates the projected rows into a binary tile. SQLx
//! streams the resulting `bytea` straight into the HTTP response — no
//! Rust-side encoding.
//!
//! Zoom-dependent column projection lives in `select_columns()` — at
//! low zooms (z <= 11) we ship only `id` and `marking` to keep tile
//! size small; at z >= 12 the full attribute set is included.

pub mod basemap;
pub mod feature;
pub mod tile;

pub use basemap::{render_basemap_tile, BasemapConfig, BasemapLayer, GeomKind};
pub use feature::{feature_by_id, list_by_bbox, FeatureRow};
pub use tile::{render_tile, MvtError};
