//! Raster fallback tiles: the N50 basemap rasterised server-side with the
//! same house style the vector pipeline serves. See `render` for the
//! pipeline and `style` for the MapLibre-subset reader.

pub mod glyphs;
pub mod hillshade;
pub mod render;
pub mod slope;
pub mod sprite;
pub mod style;

pub use glyphs::{render_range, GlyphError, FONT_STACK};
pub use hillshade::HillshadeParams;
pub use render::{render_tile, tile_envelope_3857, RasterError};
pub use slope::render_slope_tile;
pub use sprite::{build as build_sprite, SpriteError};
pub use style::{RasterStyle, StyleError};
