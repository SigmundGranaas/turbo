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

    fn hit_test(&self, screen: ScreenPoint, tol_px: f64) -> Vec<Hit> {
        // The reference engine answers hits for scene-declared POINT content
        // (circle layers over geo-json — plan P6.4): parse the points,
        // project with its own flat projection, compare screen distance.
        // Vector-tile features need tessellation and stay engine territory.
        let zoom = self.camera.zoom;
        let mut out = Vec::new();
        for layer in self.scene.layers.iter().rev() {
            let crate::scene::Layer::Circle {
                id, source, radius, ..
            } = layer
            else {
                continue;
            };
            let Some(crate::scene::SourceDef::GeoJson { data }) = self.scene.sources.get(source)
            else {
                continue;
            };
            let reach = tol_px + f64::from(radius.at(zoom));
            for (p, props) in geojson_points(data) {
                let Some(s) = self.project(p) else { continue };
                let (dx, dy) = (s.x - screen.x, s.y - screen.y);
                if dx * dx + dy * dy <= reach * reach {
                    let mut properties = props;
                    // Mirrors the wgpu engine: the owning layer rides along.
                    properties.insert("layer".to_string(), id.clone());
                    out.push(Hit {
                        layer_id: id.clone(),
                        // Engine-internal; the reference engine has none.
                        feature_id: None,
                        properties,
                    });
                }
            }
        }
        out
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            // Real since plan D4: the contract this model mirrors binds
            // `Layer::Custom` to registered, phase-bound render
            // contributions (same honesty rule as `terrain` — the model
            // doesn't rasterize, it models what the contract delivers).
            custom_layers: true,
            terrain: true,
            // The reference model doesn't rasterize anything, but the
            // CONTRACT it models compiles data-driven paint (`Paint::Match`),
            // and the wgpu engine renders it — mirror that.
            data_driven_paint: true,
            max_texture_size: 8192,
        }
    }
}

/// Minimal GeoJSON point extraction for the reference hit test: `Point` /
/// `MultiPoint` geometries (bare, in a `Feature`, or a `FeatureCollection`),
/// each with its feature's stringified properties. Mirrors the engine's
/// parser semantics without depending on it.
fn geojson_points(data: &str) -> Vec<(LatLng, std::collections::HashMap<String, String>)> {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(data) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect(&root, &std::collections::HashMap::new(), &mut out);
    return out;

    fn props_of(v: Option<&serde_json::Value>) -> std::collections::HashMap<String, String> {
        let mut out = std::collections::HashMap::new();
        if let Some(obj) = v.and_then(|v| v.as_object()) {
            for (k, v) in obj {
                let s = match v {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Number(n) => n.to_string(),
                    _ => continue,
                };
                out.insert(k.clone(), s);
            }
        }
        out
    }

    fn point(v: &serde_json::Value) -> Option<LatLng> {
        let arr = v.as_array()?;
        Some(LatLng::new(arr.get(1)?.as_f64()?, arr.first()?.as_f64()?))
    }

    fn collect(
        v: &serde_json::Value,
        props: &std::collections::HashMap<String, String>,
        out: &mut Vec<(LatLng, std::collections::HashMap<String, String>)>,
    ) {
        match v.get("type").and_then(|t| t.as_str()) {
            Some("FeatureCollection") => {
                if let Some(fs) = v.get("features").and_then(|f| f.as_array()) {
                    for f in fs {
                        collect(f, props, out);
                    }
                }
            }
            Some("Feature") => {
                let p = props_of(v.get("properties"));
                if let Some(g) = v.get("geometry") {
                    collect(g, &p, out);
                }
            }
            Some("Point") => {
                if let Some(p) = v.get("coordinates").and_then(point) {
                    out.push((p, props.clone()));
                }
            }
            Some("MultiPoint") => {
                if let Some(pts) = v.get("coordinates").and_then(|c| c.as_array()) {
                    for p in pts {
                        if let Some(p) = point(p) {
                            out.push((p, props.clone()));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
