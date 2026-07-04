//! Renderer-agnostic map IR + the `MapEngine` contract.
//!
//! This crate is the **shared schema** the whole system is built around:
//!
//! - [`Scene`] — the entire map state as one immutable, serialisable value
//!   (sources + an ordered layer stack). Unifies style and runtime data.
//! - [`diff`] — a pure `diff(old, new) -> SceneDelta`; the minimal change
//!   set is unit-testable with no renderer.
//! - [`style`] — `Paint<T>` (const + zoom curves today, data-driven later),
//!   colours, and feature filters.
//! - [`MapEngine`] — the contract every renderer (the wgpu engine and the
//!   MapLibre/MapKit/flutter_map adapters) implements; host code talks
//!   only to this.
//! - [`conformance`] — the behavioral suite every engine must pass.
//! - [`ModelEngine`] — a CPU-only reference engine that satisfies the
//!   suite and serves as ground truth for shadow comparison.
//!
//! It deliberately has **no renderer, GPU, or I/O dependency** — only
//! `serde` — so it can back uniffi codegen and the host-language bindings.

pub mod conformance;
pub mod diff;
pub mod engine;
pub mod geo;
pub mod model_engine;
pub mod scene;
pub mod style;

pub use diff::{diff, LayerChange, SceneDelta, SourceChange};
pub use engine::{Capabilities, CameraState, Hit, MapEngine};
pub use geo::{LatLng, ScreenPoint};
pub use model_engine::ModelEngine;
pub use scene::{
    CloudsDef, DemEncoding, EnvironmentDef, Layer, LightingDef, Scene, SceneError, SourceDef,
    SymbolPlacement, TextAnchor,
};
pub use style::{Color, Filter, FilterValue, Interpolate, MatchCase, Paint, ZoomStop};
