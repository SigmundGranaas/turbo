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

/// Horizontal placement of a point label relative to its anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextAnchor {
    /// Centred on the point (the default — place names, route shields).
    #[default]
    Center,
    /// Left edge at the point (+ any icon width), so the text reads to the
    /// right of an icon — the POI-marker layout.
    Left,
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
        /// Per-tile halo (px) the server bakes in: each tile is `256 + 2·halo`
        /// and the outer ring is the neighbours' elevation, so adjacent terrain
        /// mesh edges agree and the surface doesn't crack at tile boundaries.
        /// 0 = no halo (visible seams under tilt).
        #[serde(default)]
        halo: u32,
    },
    /// A PMTiles v3 archive of raster tiles (decision D2/D7: one artifact is
    /// both the offline bundle and the serverless online source). `location`
    /// is a local filesystem path (the bundled-baseline case) or an http(s)
    /// URL (range requests against dumb static storage) — bundled-vs-remote
    /// is packaging, not architecture, so it is one variant. Zoom bounds and
    /// compression come from the archive header at resolve time.
    PmtilesRaster { location: String },
    /// A PMTiles v3 archive of MVT vector tiles.
    PmtilesVector { location: String },
    /// A PMTiles v3 archive of DEM tiles for terrain/hillshade.
    PmtilesDem {
        location: String,
        encoding: DemEncoding,
        /// Per-tile halo (px) baked into the archive's tiles; see
        /// [`SourceDef::DemXyz::halo`].
        #[serde(default)]
        halo: u32,
    },
    /// An ordered provider chain — the architecture's bundled-under-remote
    /// layering (D2/D7) stated in the IR: the engine serves a tile from the
    /// FIRST provider that covers it (e.g. a bundled coarse baseline), and
    /// only what no in-process provider can serve surfaces to the host as
    /// pending (e.g. detail zooms from a remote XYZ source). A cold start
    /// with no network renders the baseline; connectivity refines it — one
    /// source id, so layers and styles never know the difference.
    ///
    /// Providers must all be the same content kind (all raster, all vector,
    /// or all DEM — GeoJSON and nested chains are rejected by `validate`).
    Chain { providers: Vec<SourceDef> },
}

fn default_tile_size() -> u32 {
    256
}
fn default_max_zoom() -> u8 {
    22
}

/// One entry in the ordered, bottom-to-top layer stack. Every variant
/// carries an `id` unique within the scene.
///
/// `Symbol` is the widest variant (text + halo + icon styling), so the enum
/// carries some padding on the smaller variants. A scene holds only a
/// handful of layers and is rebuilt rarely, so the few spare bytes per
/// layer aren't worth boxing every variant's fields for.
#[allow(clippy::large_enum_variant)]
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
        // The style-spec key is `source-layer` (kebab). The enum's
        // `rename_all = "kebab-case"` renames variants, NOT struct-variant
        // fields — so without this the app's `"source-layer"` silently dropped to
        // None, the rule's source-layer became "" and matched no MVT layer
        // (water never rendered). Alias keeps any snake_case callers working.
        #[serde(default, rename = "source-layer", alias = "source_layer")]
        source_layer: Option<String>,
        #[serde(default)]
        filter: Filter,
        color: Paint<Color>,
        #[serde(default = "opaque")]
        opacity: Paint<f32>,
    },
    /// Extrude matching polygons to 3D prisms `height_m` metres tall — the
    /// building-extrusion layer. Reads as flat from straight down; the form
    /// appears as the camera pitches.
    FillExtrusion {
        id: String,
        source: String,
        // The style-spec key is `source-layer` (kebab). The enum's
        // `rename_all = "kebab-case"` renames variants, NOT struct-variant
        // fields — so without this the app's `"source-layer"` silently dropped to
        // None, the rule's source-layer became "" and matched no MVT layer
        // (water never rendered). Alias keeps any snake_case callers working.
        #[serde(default, rename = "source-layer", alias = "source_layer")]
        source_layer: Option<String>,
        #[serde(default)]
        filter: Filter,
        color: Paint<Color>,
        /// Default / fallback height in metres.
        height_m: Paint<f32>,
        /// Numeric feature property giving each polygon's own height (e.g.
        /// OMT `render_height`). `None` ⇒ every feature uses `height_m`.
        #[serde(default)]
        height_property: Option<String>,
        /// Numeric feature property giving each polygon's *base* height (e.g.
        /// OMT `render_min_height`), so rooftop structures float. `None` ⇒
        /// extrude from the ground.
        #[serde(default)]
        min_height_property: Option<String>,
    },
    Line {
        id: String,
        source: String,
        // The style-spec key is `source-layer` (kebab). The enum's
        // `rename_all = "kebab-case"` renames variants, NOT struct-variant
        // fields — so without this the app's `"source-layer"` silently dropped to
        // None, the rule's source-layer became "" and matched no MVT layer
        // (water never rendered). Alias keeps any snake_case callers working.
        #[serde(default, rename = "source-layer", alias = "source_layer")]
        source_layer: Option<String>,
        #[serde(default)]
        filter: Filter,
        color: Paint<Color>,
        width: Paint<f32>,
        /// Dash pattern in screen pixels as `[dash, gap]` (further entries
        /// are reserved). `None`/empty ⇒ a solid line. Like MapLibre's
        /// `line-dasharray`, but in pixels rather than line-width units.
        #[serde(default)]
        dash_array: Option<Vec<f32>>,
    },
    Circle {
        id: String,
        source: String,
        // The style-spec key is `source-layer` (kebab). The enum's
        // `rename_all = "kebab-case"` renames variants, NOT struct-variant
        // fields — so without this the app's `"source-layer"` silently dropped to
        // None, the rule's source-layer became "" and matched no MVT layer
        // (water never rendered). Alias keeps any snake_case callers working.
        #[serde(default, rename = "source-layer", alias = "source_layer")]
        source_layer: Option<String>,
        #[serde(default)]
        filter: Filter,
        color: Paint<Color>,
        radius: Paint<f32>,
    },
    Symbol {
        id: String,
        source: String,
        // The style-spec key is `source-layer` (kebab). The enum's
        // `rename_all = "kebab-case"` renames variants, NOT struct-variant
        // fields — so without this the app's `"source-layer"` silently dropped to
        // None, the rule's source-layer became "" and matched no MVT layer
        // (water never rendered). Alias keeps any snake_case callers working.
        #[serde(default, rename = "source-layer", alias = "source_layer")]
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
        /// Optional sprite drawn at each point feature, behind the label:
        /// a POI icon, or — with text — a route shield. Names a sprite in
        /// the renderer's built-in atlas. `None` ⇒ text only.
        #[serde(default)]
        icon_image: Option<String>,
        /// On-screen height of `icon_image` in pixels.
        #[serde(default = "default_icon_size")]
        icon_size: Paint<f32>,
        /// Tint for the monochrome SDF icon (and shield background).
        #[serde(default = "default_icon_color")]
        icon_color: Paint<Color>,
        /// Horizontal placement of the text. `Center` (default) keeps the
        /// current behaviour (place names, shields); `Left` puts the text
        /// to the right of the icon (POI markers).
        #[serde(default)]
        text_anchor: TextAnchor,
        /// Extra tracking between glyphs in em (0 = none) — area labels
        /// (water bodies, districts) are spaced out, the convention that
        /// signals a place's spatial extent. Point labels only.
        #[serde(default)]
        letter_spacing: f32,
        /// Faux-bold weight in glyph raster pixels (0 = the font's natural
        /// weight). Lets the style express a weight hierarchy — heavy city
        /// names, medium area labels, light street names.
        #[serde(default)]
        font_weight: f32,
    },
    Hillshade {
        id: String,
        source: String,
        #[serde(default = "default_exaggeration")]
        exaggeration: f32,
        /// When true the DEM serves *only* as the heightmap that
        /// displaces the ground — no relief-shading overlay is drawn.
        /// The basemap raster lights itself from the sun instead (one
        /// lit 3D surface), which is the "DEM is height, not a tile"
        /// look. Default false = also draw the classic hillshade.
        #[serde(default)]
        height_only: bool,
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
fn default_icon_size() -> Paint<f32> {
    Paint::Const(24.0)
}
fn default_icon_color() -> Paint<Color> {
    // A neutral slate so an icon with no explicit colour is still visible.
    Paint::Const(Color::rgb(70, 78, 92))
}

impl Layer {
    /// The layer's scene-unique identity.
    pub fn id(&self) -> &str {
        match self {
            Layer::Raster { id, .. }
            | Layer::Fill { id, .. }
            | Layer::FillExtrusion { id, .. }
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
            | Layer::FillExtrusion { source, .. }
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
    /// unique layer ids, every non-custom layer pointing at a source that
    /// exists, and well-formed provider chains (non-empty, un-nested, one
    /// content kind). Returns the offending id on failure.
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
        for (id, def) in &self.sources {
            if let SourceDef::Chain { providers } = def {
                if providers.is_empty() {
                    return Err(SceneError::InvalidChain {
                        source: id.clone(),
                        reason: "a chain needs at least one provider".to_string(),
                    });
                }
                if providers.iter().any(|p| matches!(p, SourceDef::Chain { .. })) {
                    return Err(SceneError::InvalidChain {
                        source: id.clone(),
                        reason: "chains cannot nest".to_string(),
                    });
                }
                let kind = chain_kind(&providers[0]);
                if kind.is_none() || providers.iter().any(|p| chain_kind(p) != kind) {
                    return Err(SceneError::InvalidChain {
                        source: id.clone(),
                        reason: "providers must all be raster, all vector, or all DEM"
                            .to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// The content kind a chain provider serves — `None` for kinds a chain
/// cannot contain (inline GeoJSON needs no fallback; nesting is rejected).
fn chain_kind(def: &SourceDef) -> Option<u8> {
    match def {
        SourceDef::RasterXyz { .. } | SourceDef::PmtilesRaster { .. } => Some(0),
        SourceDef::VectorXyz { .. } | SourceDef::PmtilesVector { .. } => Some(1),
        SourceDef::DemXyz { .. } | SourceDef::PmtilesDem { .. } => Some(2),
        SourceDef::GeoJson { .. } | SourceDef::Chain { .. } => None,
    }
}

/// A scene that violates a structural invariant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneError {
    DuplicateLayerId(String),
    UnknownSource { layer: String, source: String },
    InvalidChain { source: String, reason: String },
}

impl std::fmt::Display for SceneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SceneError::DuplicateLayerId(id) => write!(f, "duplicate layer id '{id}'"),
            SceneError::UnknownSource { layer, source } => {
                write!(f, "layer '{layer}' references unknown source '{source}'")
            }
            SceneError::InvalidChain { source, reason } => {
                write!(f, "source '{source}' has an invalid provider chain: {reason}")
            }
        }
    }
}

impl std::error::Error for SceneError {}
