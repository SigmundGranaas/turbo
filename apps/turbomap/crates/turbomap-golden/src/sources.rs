//! Deterministic, in-process synthetic tile sources.
//!
//! Golden tests must never touch the network: every pixel has to be a
//! pure function of the trace. These sources generate their tiles
//! analytically so a replay is byte-identical run to run.

use std::io::Cursor;

use image::{ImageEncoder, RgbaImage};
use turbomap_core::{RasterFormat, RasterTile, TileError, TileId, TileSource};

const TILE_PX: u32 = 256;

pub(crate) fn encode_png(img: &RgbaImage) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 * 1024);
    image::codecs::png::PngEncoder::new(Cursor::new(&mut out))
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .expect("png encode");
    out
}

/// Uniform parchment basemap — a single flat colour. Contributes no
/// gradient of its own, so it makes a near driver-independent baseline
/// and a neutral backdrop for the hillshade pipeline.
pub struct ParchmentBasemap;

impl TileSource for ParchmentBasemap {
    fn request(&self, _tile: TileId) -> Result<RasterTile, TileError> {
        let mut img = RgbaImage::new(TILE_PX, TILE_PX);
        for px in img.pixels_mut() {
            *px = image::Rgba([226, 218, 198, 255]);
        }
        Ok(RasterTile {
            bytes: encode_png(&img),
            format: RasterFormat::Png,
        })
    }

    fn min_zoom(&self) -> u8 {
        0
    }
    fn max_zoom(&self) -> u8 {
        20
    }
}

/// Uniform single-colour basemap with a chosen sRGB colour — the "land"
/// ground a vector basemap style paints on top of.
pub struct FlatBasemap(pub [u8; 3]);

impl TileSource for FlatBasemap {
    fn request(&self, _tile: TileId) -> Result<RasterTile, TileError> {
        let mut img = RgbaImage::new(TILE_PX, TILE_PX);
        for px in img.pixels_mut() {
            *px = image::Rgba([self.0[0], self.0[1], self.0[2], 255]);
        }
        Ok(RasterTile {
            bytes: encode_png(&img),
            format: RasterFormat::Png,
        })
    }

    fn min_zoom(&self) -> u8 {
        0
    }
    fn max_zoom(&self) -> u8 {
        20
    }
}

/// Synthetic Mapbox-Terrain-RGB DEM: a Gaussian peak (~1500 m) centred
/// on Bergen gives the hillshade pipeline a real gradient to shade.
pub struct GaussianTerrainSource {
    peak_lng: f64,
    peak_lat: f64,
    peak_height_m: f64,
    sigma_deg: f64,
}

impl GaussianTerrainSource {
    pub fn bergen() -> Self {
        Self {
            peak_lng: 5.32,
            peak_lat: 60.39,
            peak_height_m: 1500.0,
            sigma_deg: 0.6,
        }
    }
}

impl TileSource for GaussianTerrainSource {
    fn request(&self, tile: TileId) -> Result<RasterTile, TileError> {
        let n = (1u64 << tile.z) as f64;
        let mut img = RgbaImage::new(TILE_PX, TILE_PX);
        for py in 0..TILE_PX {
            for px in 0..TILE_PX {
                let fx = tile.x as f64 + (px as f64 + 0.5) / TILE_PX as f64;
                let fy = tile.y as f64 + (py as f64 + 0.5) / TILE_PX as f64;
                let lng = fx / n * 360.0 - 180.0;
                let lat = (std::f64::consts::PI * (1.0 - 2.0 * fy / n))
                    .sinh()
                    .atan()
                    .to_degrees();
                let dx = (lng - self.peak_lng) / self.sigma_deg;
                let dy = (lat - self.peak_lat) / self.sigma_deg;
                let h = self.peak_height_m * (-(dx * dx + dy * dy) * 0.5).exp();
                let scaled = ((h + 10000.0) * 10.0).round().clamp(0.0, 16_777_215.0) as u32;
                let r = ((scaled >> 16) & 0xFF) as u8;
                let g = ((scaled >> 8) & 0xFF) as u8;
                let b = (scaled & 0xFF) as u8;
                img.put_pixel(px, py, image::Rgba([r, g, b, 255]));
            }
        }
        Ok(RasterTile {
            bytes: encode_png(&img),
            format: RasterFormat::Png,
        })
    }

    fn min_zoom(&self) -> u8 {
        0
    }
    fn max_zoom(&self) -> u8 {
        20
    }
}
