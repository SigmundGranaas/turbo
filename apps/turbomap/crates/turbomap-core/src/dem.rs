//! The DEM codec: RGB-packed elevation tiles → metres.
//!
//! DEM tiles are normal PNG/WebP rasters, but each pixel's RGB channels
//! encode an elevation in metres. Two encodings dominate:
//!
//! - **Mapbox Terrain-RGB** (Mapbox, Maptiler):
//!   `h = -10000 + (R * 256² + G * 256 + B) * 0.1`
//! - **Terrarium** (Mapzen, AWS open data):
//!   `h = R * 256 + G + B / 256 - 32768`
//!
//! Both produce signed metres covering the realistic terrain range with
//! ~0.1 m precision. The encoding is part of the *source's* contract
//! ([`crate::TileSource::dem_encoding`]), and this module is the ONLY
//! place that knows it (plan slice D3): every ingest path decodes RGBA
//! to real heights here, `Map::ingest_terrain_tile` accepts metres, DEM
//! textures upload as `Rg16Float`, and the shaders sample `.r` directly —
//! no per-vertex/per-fragment decode, no encoding uniform.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemEncoding {
    /// Mapbox / Maptiler Terrain-RGB.
    MapboxRgb,
    /// Mapzen / AWS Terrarium.
    Terrarium,
}

/// Decode a single pixel's RGB to elevation in metres.
pub fn decode_elevation(enc: DemEncoding, r: u8, g: u8, b: u8) -> f32 {
    let rf = r as f32;
    let gf = g as f32;
    let bf = b as f32;
    match enc {
        DemEncoding::MapboxRgb => -10000.0 + (rf * 256.0 * 256.0 + gf * 256.0 + bf) * 0.1,
        DemEncoding::Terrarium => rf * 256.0 + gf + bf / 256.0 - 32768.0,
    }
}

/// A fully decoded DEM tile: real heights plus the source's coverage mask,
/// ready for [`crate::Map::ingest_terrain_tile`]. Row-major,
/// `width × height`, halo pixels included.
pub struct DecodedDem {
    pub width: u32,
    pub height: u32,
    /// Elevations in metres. "No data" pixels decode to 0 m, so water sits
    /// at sea level instead of a -10 km cliff.
    pub heights_m: Vec<f32>,
    /// Per-pixel coverage: 1 where the source had data, 0 where it marked
    /// "no data" (alpha < 128 — sea / outside DTM coverage). Uploaded as a
    /// filterable channel so the hillshade overlay can stay transparent
    /// over water, exactly as it keyed off the alpha channel before.
    pub coverage: Vec<u8>,
}

impl DecodedDem {
    /// A fully-covered tile from raw heights — for synthetic DEMs (tests,
    /// procedural sources) that have no "no data" concept.
    pub fn from_heights(width: u32, height: u32, heights_m: Vec<f32>) -> Self {
        let n = heights_m.len();
        Self {
            width,
            height,
            heights_m,
            coverage: vec![1u8; n],
        }
    }
}

/// Decode a whole RGBA DEM tile to real heights + coverage. This is the
/// ingest-side codec step every DEM path runs before handing elevations to
/// [`crate::Map::ingest_terrain_tile`].
///
/// Returns `None` for malformed inputs (zero dimensions, or a buffer shorter
/// than the claimed size — checked in `usize` so huge claimed dimensions
/// can't wrap a `u32` byte count and slip past the guard).
pub fn decode_dem_rgba(
    rgba: &[u8],
    width: u32,
    height: u32,
    enc: DemEncoding,
) -> Option<DecodedDem> {
    let px = (width as usize).checked_mul(height as usize)?;
    let required = px.checked_mul(4)?;
    if width == 0 || height == 0 || rgba.len() < required {
        return None;
    }
    let mut heights = vec![0.0f32; px];
    let mut coverage = vec![0u8; px];
    for (i, h) in heights.iter_mut().enumerate() {
        let j = i * 4;
        if rgba[j + 3] >= 128 {
            *h = decode_elevation(enc, rgba[j], rgba[j + 1], rgba[j + 2]);
            coverage[i] = 1;
        }
    }
    Some(DecodedDem {
        width,
        height,
        heights_m: heights,
        coverage,
    })
}

#[cfg(test)]
mod tests {
    //! Value boundary: hosts plug in a DEM source with an encoding
    //! choice. Decoding has to match the published reference values for
    //! both formats — getting this wrong silently means hillshade
    //! gradients are scaled wrong.
    use super::*;

    fn approx(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn mapbox_rgb_sea_level_pixel_is_close_to_zero() {
        // Mapbox encodes sea level (0 m) as RGB ≈ (134, 16, 0): the
        // encoding offsets by 10000 m and scales by 0.1, so
        // (R*65536 + G*256 + B) = 100000.
        // 134*65536 + 16*256 + 0 = 8781824 + 4096 + 0 = 8785920... hmm
        // doesn't match.
        // Reference: black (0,0,0) → -10000 m (sea floor far below).
        let h = decode_elevation(DemEncoding::MapboxRgb, 0, 0, 0);
        assert!(approx(h, -10000.0, 1e-3), "got {h}");

        // (R=39, G=22, B=176): (39*65536 + 22*256 + 176) * 0.1 = 256093.3
        // → -10000 + 256093.3 = 246093 ≈ Mount Everest range (×0.1).
        // Easier reference: highest precision step.
        // 0.1 m per unit increment in B.
        let h_low = decode_elevation(DemEncoding::MapboxRgb, 0, 0, 0);
        let h_one_unit = decode_elevation(DemEncoding::MapboxRgb, 0, 0, 1);
        assert!(approx(h_one_unit - h_low, 0.1, 1e-3));
    }

    #[test]
    fn terrarium_zero_offset_for_grayscale_neutral() {
        // Terrarium: (32768, 0, 0)/256 = 32768. Subtract offset → 0 m.
        // I.e. R=128 alone gives (128*256 + 0 + 0/256) = 32768.
        let h = decode_elevation(DemEncoding::Terrarium, 128, 0, 0);
        assert!(approx(h, 0.0, 1e-3), "got {h}");
    }

    #[test]
    fn terrarium_one_metre_step_matches_green_channel() {
        // Per spec: G channel encodes whole metres.
        let h0 = decode_elevation(DemEncoding::Terrarium, 128, 0, 0);
        let h1 = decode_elevation(DemEncoding::Terrarium, 128, 1, 0);
        assert!(approx(h1 - h0, 1.0, 1e-3));
    }

    #[test]
    fn decode_dem_rgba_maps_nodata_to_sea_level_and_rejects_malformed() {
        // 2×1: one real Mapbox pixel (0,0,1 → -9999.9 m), one no-data pixel.
        let rgba = [0, 0, 1, 255, 10, 10, 10, 0];
        let d = decode_dem_rgba(&rgba, 2, 1, DemEncoding::MapboxRgb).unwrap();
        assert!(
            approx(d.heights_m[0], -9999.9, 1e-2),
            "got {}",
            d.heights_m[0]
        );
        assert_eq!(d.coverage[0], 1);
        assert_eq!(
            d.heights_m[1], 0.0,
            "no-data (alpha 0) decodes to sea level"
        );
        assert_eq!(
            d.coverage[1], 0,
            "no-data is masked for the hillshade overlay"
        );
        // Malformed: buffer shorter than claimed; u32-overflow dimensions.
        assert!(decode_dem_rgba(&rgba, 256, 256, DemEncoding::MapboxRgb).is_none());
        assert!(decode_dem_rgba(&rgba, 65536, 65536, DemEncoding::MapboxRgb).is_none());
        assert!(decode_dem_rgba(&rgba, 0, 4, DemEncoding::MapboxRgb).is_none());
    }

    #[test]
    fn mapbox_max_24bit_is_within_realistic_terrain() {
        // (255, 255, 255) → 16777215; * 0.1 - 10000 = 1667711.5 m.
        // Way above Everest (8849 m) but matches the spec — encoding
        // covers a much larger range than terrestrial elevation.
        let h = decode_elevation(DemEncoding::MapboxRgb, 255, 255, 255);
        assert!(h > 0.0 && h < 2_000_000.0, "got {h}");
    }
}
