//! Raster fallback tiles: the N50 basemap rasterised server-side with the
//! same house style the vector pipeline serves. See `render` for the
//! pipeline and `style` for the MapLibre-subset reader.

pub mod render;
pub mod style;

pub use render::{render_tile, tile_envelope_3857, RasterError};
pub use style::{RasterStyle, StyleError};
