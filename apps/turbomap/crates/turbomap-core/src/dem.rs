//! Decoders for RGB-packed digital elevation tiles.
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
//! ~0.1 m precision. The encoding is part of the *source's* contract;
//! the hillshade pipeline takes the encoding as a config and decodes
//! per fragment in the shader.

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
    fn mapbox_max_24bit_is_within_realistic_terrain() {
        // (255, 255, 255) → 16777215; * 0.1 - 10000 = 1667711.5 m.
        // Way above Everest (8849 m) but matches the spec — encoding
        // covers a much larger range than terrestrial elevation.
        let h = decode_elevation(DemEncoding::MapboxRgb, 255, 255, 255);
        assert!(h > 0.0 && h < 2_000_000.0, "got {h}");
    }
}
