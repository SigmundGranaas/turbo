//! The representation-agnostic world-data model (plan slice B1; decisions
//! D2/D9 in `docs/architecture/2026-07-turbomap-decima-inspired-engine-architecture.md`).
//!
//! Every streamable dataset is a **tree of chunks**: each chunk has a bounding
//! volume, a geometric error in meters (how wrong the world is if this chunk
//! renders instead of its children), and a refinement mode — and an *opaque*
//! payload. Nothing in this crate knows about PNG, MVT, terrain-RGB, or any
//! other wire format; that knowledge lives (and dies) in the engine's codec
//! registry. The Web-Mercator XYZ pyramid is **instance #1** of the tree
//! ([`TreeShape::ImplicitQuadtree`], nodes computed, never fetched); an
//! explicit fetched tree (3D Tiles `tileset.json` et al.) is instance #2 —
//! its lossless mapping onto these types is design-validated by the
//! `threedtiles_mapping` test before the types freeze.
//!
//! The [`lifecycle`] module is the single source of truth for "where is this
//! resource?" — the table that replaces the tile state smeared across six
//! collections in three layers (`Scene.ingested`, FFI `queued`, host
//! `inFlight`/`retryAt`, cache residency, fade bookkeeping). It is pure data +
//! transitions, deterministic by construction, and property-tested here with
//! no GPU, no IO, and no clock.

pub mod chunk;
pub mod lifecycle;
pub mod priority;
pub mod quadtree;
pub mod tree;

pub use chunk::{BoundingVolume, ChunkKey, ChunkMeta, NodeId, Refine, WorldLayerId};
pub use lifecycle::{Lifecycle, LifecycleError, Phase, PhaseHistogram, RequestId};
pub use priority::{Priority, Tier};
pub use quadtree::QuadKey;
pub use tree::{PyramidSpec, TreeShape};
