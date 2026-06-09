//! The `MapEngine` contract — the renderer-agnostic seam.
//!
//! Every renderer (the wgpu `turbomap` engine, and adapters wrapping
//! MapLibre / MapKit / flutter_map) implements this. Host feature code
//! talks only to this trait, never to a concrete renderer, which is what
//! makes swapping renderers — and shadow-comparing two of them — a
//! property of the system rather than a migration.
//!
//! Note the GPU/surface half of a real engine (attaching to a native
//! drawable, the vsync loop) is constructed by per-platform native glue,
//! *not* through this trait — you cannot pass a window handle across a
//! uniffi boundary. This trait is the control plane only.

use crate::diff::SceneDelta;
use crate::geo::{LatLng, ScreenPoint};
use crate::scene::Scene;

/// Camera pose. `pitch_deg`/`bearing_deg` are clamped/normalized by the
/// engine on use; the value itself is plain data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraState {
    pub center: LatLng,
    pub zoom: f64,
    pub pitch_deg: f64,
    pub bearing_deg: f64,
}

impl CameraState {
    pub fn new(center: LatLng, zoom: f64) -> Self {
        Self {
            center,
            zoom,
            pitch_deg: 0.0,
            bearing_deg: 0.0,
        }
    }
}

/// A feature struck by a hit test, top-most first.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub layer_id: String,
    pub feature_id: Option<String>,
}

/// What a backend can actually do. Hosts read this to degrade gracefully
/// and the conformance suite reads it to skip checks an engine opts out
/// of (e.g. a MapKit adapter that cannot host a custom layer).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub custom_layers: bool,
    pub terrain: bool,
    pub data_driven_paint: bool,
    pub max_texture_size: u32,
}

/// The renderer-agnostic control surface. A pure function of
/// `(scene, camera) -> projection/hit/pixels` from the host's point of
/// view; everything stateful (tiles, GPU resources) is the engine's own
/// business.
pub trait MapEngine {
    /// Replace the scene. Returns the [`SceneDelta`] the engine applied —
    /// surfacing it makes the contract observable and is what the
    /// conformance suite asserts on.
    fn apply(&mut self, scene: Scene) -> SceneDelta;

    /// The currently applied scene.
    fn scene(&self) -> &Scene;

    fn camera(&self) -> CameraState;
    fn set_camera(&mut self, camera: CameraState);

    /// Update the viewport (device pixels).
    fn resize(&mut self, width: u32, height: u32);

    /// Project a coordinate to screen pixels, or `None` if it is behind
    /// the camera / outside the projectable range.
    fn project(&self, geo: LatLng) -> Option<ScreenPoint>;

    /// Inverse of [`MapEngine::project`].
    fn unproject(&self, screen: ScreenPoint) -> Option<LatLng>;

    /// Features under a screen point within `tol_px`, top-most first.
    fn hit_test(&self, screen: ScreenPoint, tol_px: f64) -> Vec<Hit>;

    fn capabilities(&self) -> Capabilities;
}
