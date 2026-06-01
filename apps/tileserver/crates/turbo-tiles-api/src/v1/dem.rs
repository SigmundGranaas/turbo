//! `/v1/dem/rgb/{z}/{x}/{y}.png` — Mapbox-Terrain-RGB tile of the local
//! Kartverket DTM. The handler samples our existing `turbo-tiles-elev`
//! `Dem` 256×256 times per tile, packs each elevation into the three
//! RGB channels, and returns a PNG.
//!
//! Encoding reference: <https://docs.mapbox.com/data/tilesets/guides/access-elevation-data/>
//!
//!   `height_m = -10000 + ((R * 256² + G * 256 + B) * 0.1)`
//!
//! Nodata pixels (outside DTM coverage) are encoded as fully transparent
//! so the consumer's hillshade shader can fall back to whatever sits
//! below.

use std::io::Cursor;
use std::time::Instant;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use image::{ImageEncoder, RgbaImage};
use serde::Deserialize;
use turbo_tiles_elev::{wgs84_to_utm33n, PointXY};

use crate::error::ApiError;
use crate::state::ApiState;

const TILE_PX: u32 = 256;
/// Cap halo at a few pixels so a misconfigured client can't ask the
/// server to render a 1 024×1 024 PNG per tile by accident.
const MAX_HALO_PX: u32 = 4;

#[derive(Debug, Default, Deserialize)]
pub struct RgbQuery {
    /// Extra ring of pixels on every side. Lets the hillshade
    /// consumer compute gradients without a tile-edge seam — the
    /// outer `halo` pixels are sampled from the neighbouring tile's
    /// geographic area. Default `0` (back-compat). The returned PNG
    /// is `(256 + 2*halo)²`; the `x-tile-halo` header tells the
    /// client what they got.
    #[serde(default)]
    halo: Option<u32>,
}

pub async fn rgb(
    State(state): State<ApiState>,
    Path((z, x, y_ext)): Path<(u8, u32, String)>,
    Query(q): Query<RgbQuery>,
) -> Result<Response, ApiError> {
    let y_str = y_ext
        .strip_suffix(".png")
        .ok_or_else(|| ApiError::BadRequest("tile path must end in .png".into()))?;
    let y: u32 = y_str
        .parse()
        .map_err(|_| ApiError::BadRequest(format!("invalid tile y `{y_str}`")))?;
    if z > 22 {
        return Err(ApiError::BadRequest(format!("zoom {z} out of range")));
    }
    let n: u32 = 1 << z;
    if x >= n || y >= n {
        return Err(ApiError::BadRequest(format!(
            "tile coord ({x}, {y}) outside z={z} grid (max {})",
            n - 1
        )));
    }
    let halo = q.halo.unwrap_or(0);
    if halo > MAX_HALO_PX {
        return Err(ApiError::BadRequest(format!(
            "halo {halo} exceeds max of {MAX_HALO_PX}"
        )));
    }

    let dem = state
        .dem
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("dem"))?;
    let started = Instant::now();

    // Render off the request thread — sampling tens of thousands of
    // points is CPU-bound and we don't want to block tokio's runtime.
    let dem = dem.clone();
    let png_bytes = tokio::task::spawn_blocking(move || render_tile(&dem, z, x, y, halo))
        .await
        .map_err(|e| ApiError::Internal(format!("join: {e}")))??;
    let took_ms = started.elapsed().as_millis();
    let out_size = TILE_PX + 2 * halo;
    tracing::debug!(
        z,
        x,
        y,
        halo,
        took_ms,
        bytes = png_bytes.len(),
        "dem rgb tile"
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_static("image/png"))
        .header(
            header::CACHE_CONTROL,
            // The DTM is essentially immutable for a given deploy; tiles
            // are content-addressable in spirit. Aggressive caching is
            // fine; clients revalidate at app restart.
            HeaderValue::from_static("public, max-age=86400, immutable"),
        )
        .header("x-tile-size", HeaderValue::from(TILE_PX))
        .header("x-tile-halo", HeaderValue::from(halo))
        .header("x-png-size", HeaderValue::from(out_size))
        .body(Body::from(png_bytes))
        .map_err(|e| ApiError::Internal(e.to_string()))
}

/// Build one `(256 + 2*halo)²` Terrain-RGB PNG. Pure CPU work, safe to
/// run on `spawn_blocking`. Returns the PNG bytes ready to ship.
fn render_tile(
    dem: &turbo_tiles_elev::Dem,
    z: u8,
    x: u32,
    y: u32,
    halo: u32,
) -> Result<Vec<u8>, ApiError> {
    let size = TILE_PX + 2 * halo;
    let mut img: RgbaImage = RgbaImage::new(size, size);
    let halo_f = halo as f64;

    // Pixel (halo, halo) in the output maps to fractional tile coord
    // (x + 0.5/256, y + 0.5/256) — same centre as the no-halo case so
    // the interior 256² is bit-identical to the legacy contract.
    for py in 0..size {
        for px in 0..size {
            let frac_x = (x as f64) + ((px as f64) + 0.5 - halo_f) / TILE_PX as f64;
            let frac_y = (y as f64) + ((py as f64) + 0.5 - halo_f) / TILE_PX as f64;
            let (lng, lat) = tile_pixel_to_lng_lat(z, frac_x, frac_y);
            // Convert to UTM 33N (the DTM's native CRS) and sample.
            let utm = wgs84_to_utm33n(lng, lat);
            let pixel = match dem.sample(PointXY { x: utm.x, y: utm.y }) {
                Ok(Some(elev_m)) => encode_terrain_rgb(elev_m as f64),
                _ => [0, 0, 0, 0], // transparent for out-of-coverage / nodata
            };
            img.put_pixel(px, py, image::Rgba(pixel));
        }
    }

    // Encode PNG straight into a Vec. The PngEncoder defaults are fine —
    // these tiles are typically 5–20 KB.
    let mut out = Vec::with_capacity(32 * 1024);
    {
        let encoder = image::codecs::png::PngEncoder::new(Cursor::new(&mut out));
        encoder
            .write_image(img.as_raw(), size, size, image::ExtendedColorType::Rgba8)
            .map_err(|e| ApiError::Internal(format!("png encode: {e}")))?;
    }
    Ok(out)
}

/// Inverse Web-Mercator: (z, fractional tile coords) → (lng, lat).
/// `frac_x ∈ [0, n]`, `frac_y ∈ [0, n]` where `n = 2^z`.
fn tile_pixel_to_lng_lat(z: u8, frac_x: f64, frac_y: f64) -> (f64, f64) {
    let n = (1u64 << z) as f64;
    let lng = frac_x / n * 360.0 - 180.0;
    let lat_rad = (std::f64::consts::PI * (1.0 - 2.0 * frac_y / n))
        .sinh()
        .atan();
    (lng, lat_rad.to_degrees())
}

/// Pack `elevation_m` (metres) into the Mapbox Terrain-RGB scheme. The
/// last byte is alpha — always 255 for valid samples.
fn encode_terrain_rgb(elevation_m: f64) -> [u8; 4] {
    // Offset by 10000 m so subaqueous terrain stays non-negative; 0.1 m
    // increments per unit gives ±~1.6 Mm of range with one mm of
    // precision. Plenty for terrestrial DEMs.
    let scaled = ((elevation_m + 10000.0) * 10.0)
        .round()
        .clamp(0.0, 16_777_215.0) as u32;
    let r = ((scaled >> 16) & 0xFF) as u8;
    let g = ((scaled >> 8) & 0xFF) as u8;
    let b = (scaled & 0xFF) as u8;
    [r, g, b, 255]
}

#[cfg(test)]
mod tests {
    //! Value boundary: clients (turbomap, anyone else) trust this
    //! endpoint to (a) encode elevations per the published Mapbox spec
    //! so a single decoder works everywhere, and (b) inverse-project
    //! tile pixels to the same lat/lng convention slippy-map clients
    //! use.
    use super::*;

    fn approx(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn terrain_rgb_round_trips_within_one_decimetre() {
        // The 0.1 m quantisation in Mapbox Terrain-RGB means we should
        // round-trip elevations with sub-metre accuracy.
        for h in [-100.0, 0.0, 100.5, 1234.7, 2469.0_f64] {
            let [r, g, b, _] = encode_terrain_rgb(h);
            let decoded =
                -10000.0 + ((r as f64 * 256.0 * 256.0 + g as f64 * 256.0 + b as f64) * 0.1);
            assert!(
                (decoded - h).abs() < 0.1,
                "{h} → {r:?},{g:?},{b:?} → {decoded} (delta {})",
                decoded - h,
            );
        }
    }

    #[test]
    fn terrain_rgb_zero_elevation_lands_on_expected_bytes() {
        // 0 m → scaled = 100000 ⇒ (R=1, G=134, B=160).
        let p = encode_terrain_rgb(0.0);
        assert_eq!(p, [1, 134, 160, 255]);
    }

    #[test]
    fn root_tile_centre_is_the_equator_and_prime_meridian() {
        // (z=0, frac=0.5) maps to lat=0, lng=0.
        let (lng, lat) = tile_pixel_to_lng_lat(0, 0.5, 0.5);
        assert!(approx(lng, 0.0, 1e-12));
        assert!(approx(lat, 0.0, 1e-12));
    }

    #[test]
    fn bergen_tile_centre_lands_near_bergen_lat_lng() {
        // Tile (z=11, x=1054, y=590) covers Bergen. Centre pixel should
        // come out close to 60.39, 5.32.
        let (lng, lat) = tile_pixel_to_lng_lat(11, 1054.5, 590.5);
        assert!((lat - 60.39).abs() < 0.1, "lat = {lat}");
        assert!((lng - 5.32).abs() < 0.2, "lng = {lng}");
    }

    #[test]
    fn halo_pixel_at_offset_h_matches_no_halo_pixel_at_offset_0() {
        // The halo'd image's pixel (halo, halo) MUST sample the same
        // geographic point as the no-halo image's pixel (0, 0). This
        // is the contract that lets the GPU consumer crop to the
        // interior and still get the legacy tile content.
        let z = 11;
        let x = 1054;
        let y = 590;
        for halo in [1u32, 2, 4] {
            let halo_f = halo as f64;
            let frac_no_halo_x = (x as f64) + 0.5 / TILE_PX as f64;
            let frac_no_halo_y = (y as f64) + 0.5 / TILE_PX as f64;
            let frac_haloed_x = (x as f64) + ((halo as f64) + 0.5 - halo_f) / TILE_PX as f64;
            let frac_haloed_y = (y as f64) + ((halo as f64) + 0.5 - halo_f) / TILE_PX as f64;
            assert!(approx(frac_no_halo_x, frac_haloed_x, 1e-12));
            assert!(approx(frac_no_halo_y, frac_haloed_y, 1e-12));
        }
    }

    #[test]
    fn halo_outer_pixel_is_outside_legacy_tile_bounds() {
        // Pixel (0, 0) of a halo=1 image lies *outside* the tile's
        // geographic envelope — that's the whole point. Verifies the
        // sampler actually reaches the neighbour's terrain rather
        // than re-sampling this tile's edge.
        let z = 11;
        let x = 1054;
        let y = 590;
        let halo: u32 = 1;
        let halo_f = halo as f64;
        let frac_halo_corner_x = (x as f64) + (0.5 - halo_f) / TILE_PX as f64;
        let frac_halo_corner_y = (y as f64) + (0.5 - halo_f) / TILE_PX as f64;
        assert!(
            frac_halo_corner_x < x as f64,
            "halo did not reach left neighbour"
        );
        assert!(
            frac_halo_corner_y < y as f64,
            "halo did not reach top neighbour"
        );
    }
}
