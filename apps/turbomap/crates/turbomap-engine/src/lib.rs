//! `TurbomapEngine` — the wgpu renderer behind the renderer-agnostic
//! [`turbomap_scene::MapEngine`] contract.
//!
//! This is where Phase 1 (the `Scene`/`MapEngine` IR) meets the existing
//! `turbomap-core` pipelines: the host describes the map as a [`Scene`],
//! and the engine diffs and drives the GPU. It satisfies the same
//! [`conformance`](turbomap_scene::conformance) suite the reference
//! `ModelEngine` does, so a host can hold either behind the contract.
//!
//! Construction needs a wgpu device (the GPU plane is the host's, per the
//! architecture), so this crate is not headless like `turbomap-scene` —
//! but it stays driveable headless via a software adapter, which is how
//! the golden tests and the `inspect` dev tool exercise it.

pub mod engine;
pub mod resolver;

pub use engine::{DrainStats, TurbomapEngine};
pub use resolver::{ResolvedSource, SourceResolver};

// Re-export the contract surface so hosts depend on one crate.
pub use turbomap_scene::{
    Capabilities, CameraState, Hit, LatLng, MapEngine, Scene, SceneDelta, ScreenPoint,
};
