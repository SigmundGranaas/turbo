//! `ModelEngine` — a CPU-only reference implementation of [`MapEngine`].
//!
//! It implements the *renderer-agnostic* half of the contract — scene
//! diffing, camera, and a flat (pitch-0) Web Mercator projection — with
//! no GPU. Two uses:
//!
//! 1. it proves the [`crate::conformance`] suite is satisfiable, and acts
//!    as the worked example every real engine is measured against;
//! 2. it is a usable headless engine for scene-logic tests and for the
//!    eventual shadow-mode harness (compare a real engine's projection /
//!    delta against this ground truth).
//!
//! Projection ignores pitch and bearing — it is exact only for a
//! top-down camera, which is the regime the conformance projection check
//! exercises so that *every* engine agrees there.

use crate::diff::{diff, SceneDelta};
use crate::engine::{CameraState, Capabilities, Hit, MapEngine};
use crate::geo::{inverse_mercator, mercator_normalized, LatLng, ScreenPoint};
use crate::scene::Scene;

const TILE_SIZE_PX: f64 = 256.0;

/// A headless reference engine. See module docs.
pub struct ModelEngine {
    scene: Scene,
    camera: CameraState,
    width: f64,
    height: f64,
}

impl ModelEngine {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            scene: Scene::new(),
            camera: CameraState::new(LatLng::new(0.0, 0.0), 0.0),
            width: width as f64,
            height: height as f64,
        }
    }

    fn world_scale(&self) -> f64 {
        TILE_SIZE_PX * 2f64.powf(self.camera.zoom)
    }
}

impl MapEngine for ModelEngine {
    fn apply(&mut self, scene: Scene) -> SceneDelta {
        let delta = diff(&self.scene, &scene);
        self.scene = scene;
        delta
    }

    fn scene(&self) -> &Scene {
        &self.scene
    }

    fn camera(&self) -> CameraState {
        self.camera
    }

    fn set_camera(&mut self, camera: CameraState) {
        self.camera = camera;
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width as f64;
        self.height = height as f64;
    }

    fn project(&self, geo: LatLng) -> Option<ScreenPoint> {
        let scale = self.world_scale();
        let (cx, cy) = mercator_normalized(self.camera.center);
        let (px, py) = mercator_normalized(geo);
        Some(ScreenPoint {
            x: self.width / 2.0 + (px - cx) * scale,
            y: self.height / 2.0 + (py - cy) * scale,
        })
    }

    fn unproject(&self, screen: ScreenPoint) -> Option<LatLng> {
        let scale = self.world_scale();
        let (cx, cy) = mercator_normalized(self.camera.center);
        let nx = cx + (screen.x - self.width / 2.0) / scale;
        let ny = cy + (screen.y - self.height / 2.0) / scale;
        Some(inverse_mercator(nx, ny))
    }

    fn hit_test(&self, _screen: ScreenPoint, _tol_px: f64) -> Vec<Hit> {
        // The reference engine holds no decoded feature geometry (GeoJSON
        // sources are opaque strings here), so it reports no hits. A real
        // engine tessellates features and answers this for real.
        Vec::new()
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            // Honesty over aspiration (plan C3): NO engine renders custom
            // layers yet (they become real in plan D4), and the reference
            // model must not advertise more than the real engines deliver —
            // hosts read these flags to degrade, so a lie here becomes a
            // blank layer on screen.
            custom_layers: false,
            terrain: true,
            // The reference model doesn't rasterize anything, but the
            // CONTRACT it models compiles data-driven paint (`Paint::Match`),
            // and the wgpu engine renders it — mirror that.
            data_driven_paint: true,
            max_texture_size: 8192,
        }
    }
}
