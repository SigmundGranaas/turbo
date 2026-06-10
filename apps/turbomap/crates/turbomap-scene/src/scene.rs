//! The `Scene`: the whole map state as one immutable, diffable value.
//!
//! A host builds a `Scene` in its own language and hands it to a
//! [`crate::MapEngine`]; the engine diffs it against the previous scene
//! (see [`crate::diff`]) and updates minimally. Sources and the runtime
//! data they carry live in the *same* value — there is no static-style /
//! imperative-mutation split.

use serde::{Deserialize, Serialize};

use crate::style::{Color, Filter, Paint};

/// How a DEM raster encodes elevation into RGB.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DemEncoding {
    MapboxRgb,
    Terrarium,
}

/// Where a `Symbol` layer's text is anchored.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SymbolPlacement {
    /// At each feature's point (the default — place names, POIs).
    #[default]
    Point,
    /// Along a LineString feature's centerline, glyphs following the curve
    /// (road names, route labels).
    Line,
}

/// A named data source layers draw from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SourceDef {
    /// XYZ raster tiles (PNG/JPEG/WebP).
    RasterXyz {
        tiles: Vec<String>,
        #[serde(default = "default_tile_size")]
        tile_size: u32,
        #[serde(default)]
        min_zoom: u8,
        #[serde(default = "default_max_zoom")]
        max_zoom: u8,
        #[serde(default)]
        attribution: Option<String>,
    },
    /// XYZ vector tiles (MVT).
    VectorXyz {
        tiles: Vec<String>,
        #[serde(default)]
        min_zoom: u8,
        #[serde(default = "default_max_zoom")]
        max_zoom: u8,
    },
    /// Inline GeoJSON — the runtime-data path (route, track, measure, …).
    GeoJson { data: String },
    /// XYZ DEM tiles for terrain/hillshade.
    DemXyz {
        tiles: Vec<String>,
        encoding: DemEncoding,
        #[serde(default)]
        min_zoom: u8,
        #[serde(default = "default_max_zoom")]
        max_zoom: u8,
    },
}

fn default_tile_size() -> u32 {
    256
}
fn default_max_zoom() -> u8 {
    22
}

/// One entry in the ordered, bottom-to-top layer stack. Every variant
/// carries an `id` unique within the scene.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Layer {
    Raster {
        id: String,
        source: String,
        #[serde(default = "opaque")]
        opacity: Paint<f32>,
    },
    Fill {
        id: String,
        source: String,
        #[serde(default)]
        source_layer: Option<String>,
        #[serde(default)]
        filter: Filter,
        color: Paint<Color>,
        #[serde(default = "opaque")]
        opacity: Paint<f32>,
    },
    Line {
        id: String,
        source: String,
        #[serde(default)]
        source_layer: Option<String>,
        #[serde(default)]
        filter: Filter,
        color: Paint<Color>,
        width: Paint<f32>,
    },
    Circle {
        id: String,
        source: String,
        #[serde(default)]
        source_layer: Option<String>,
        #[serde(default)]
        filter: Filter,
        color: Paint<Color>,
        radius: Paint<f32>,
    },
    Symbol {
        id: String,
        source: String,
        #[serde(default)]
        source_layer: Option<String>,
        #[serde(default)]
        filter: Filter,
        text_field: String,
        text_size: Paint<f32>,
        color: Paint<Color>,
        /// Readability outline behind the label. Defaults to none
        /// (`halo_width` 0). `halo_width` is in glyph pixels.
        #[serde(default = "no_halo_color")]
        halo_color: Paint<Color>,
        #[serde(default = "no_halo_width")]
        halo_width: Paint<f32>,
        /// Feature property to rank label placement by (higher wins
        /// collisions). `None` falls back to font size.
        #[serde(default)]
        sort_key: Option<String>,
        /// Anchor the text at each point (default) or along a line.
        #[serde(default)]
        placement: SymbolPlacement,
    },
    Hillshade {
        id: String,
        source: String,
        #[serde(default = "default_exaggeration")]
        exaggeration: f32,
    },
    /// A host-supplied render pass, portable across platforms. The IR only
    /// names it; the renderer binds the actual pass by `kind`.
    Custom { id: String, kind: String },
}

fn no_halo_color() -> Paint<Color> {
    Paint::Const(Color::rgba(0, 0, 0, 0))
}
fn no_halo_width() -> Paint<f32> {
    Paint::Const(0.0)
}
fn opaque() -> Paint<f32> {
    Paint::Const(1.0)
}
fn default_exaggeration() -> f32 {
    1.5
}

impl Layer {
    /// The layer's scene-unique identity.
    pub fn id(&self) -> &str {
        match self {
            Layer::Raster { id, .. }
            | Layer::Fill { id, .. }
            | Layer::Line { id, .. }
            | Layer::Circle { id, .. }
            | Layer::Symbol { id, .. }
            | Layer::Hillshade { id, .. }
            | Layer::Custom { id, .. } => id,
        }
    }

    /// The source this layer draws from, if any (custom layers have none).
    pub fn source(&self) -> Option<&str> {
        match self {
            Layer::Raster { source, .. }
            | Layer::Fill { source, .. }
            | Layer::Line { source, .. }
            | Layer::Circle { source, .. }
            | Layer::Symbol { source, .. }
            | Layer::Hillshade { source, .. } => Some(source),
            Layer::Custom { .. } => None,
        }
    }
}

/// The complete, immutable description of a map. Sources are keyed for
/// stable diffing; layers are an ordered stack (index = draw order).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    #[serde(default)]
    pub sources: std::collections::BTreeMap<String, SourceDef>,
    #[serde(default)]
    pub layers: Vec<Layer>,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate scene-level invariants the diff and engines rely on:
    /// unique layer ids, and every non-custom layer pointing at a source
    /// that exists. Returns the offending id on failure.
    pub fn validate(&self) -> Result<(), SceneError> {
        let mut seen = std::collections::BTreeSet::new();
        for layer in &self.layers {
            if !seen.insert(layer.id()) {
                return Err(SceneError::DuplicateLayerId(layer.id().to_string()));
            }
            if let Some(src) = layer.source() {
                if !self.sources.contains_key(src) {
                    return Err(SceneError::UnknownSource {
                        layer: layer.id().to_string(),
                        source: src.to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// A scene that violates a structural invariant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneError {
    DuplicateLayerId(String),
    UnknownSource { layer: String, source: String },
}

impl std::fmt::Display for SceneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SceneError::DuplicateLayerId(id) => write!(f, "duplicate layer id '{id}'"),
            SceneError::UnknownSource { layer, source } => {
                write!(f, "layer '{layer}' references unknown source '{source}'")
            }
        }
    }
}

impl std::error::Error for SceneError {}
