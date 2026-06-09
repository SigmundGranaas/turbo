//! Self-contained geographic + screen primitives.
//!
//! The IR deliberately defines its own `LatLng`/`ScreenPoint` rather than
//! reaching into the renderer: this crate is the *shared schema* the
//! host languages bind to, so it must stand alone. Renderer crates can
//! provide `From` conversions on their side.

use serde::{Deserialize, Serialize};

/// WGS84 longitude/latitude in degrees.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LatLng {
    pub lat: f64,
    pub lng: f64,
}

impl LatLng {
    pub const fn new(lat: f64, lng: f64) -> Self {
        Self { lat, lng }
    }
}

/// A point in device-independent screen pixels, origin top-left.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScreenPoint {
    pub x: f64,
    pub y: f64,
}

impl ScreenPoint {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// The Web Mercator latitude limit (degrees). Beyond this the projection
/// diverges, so inputs are clamped here.
pub const MAX_LATITUDE_DEG: f64 = 85.051_128_779_806_59;

/// Project a coordinate to normalized Web Mercator space, where both
/// axes run `[0, 1]` over the whole world (x: -180°→180°, y: north→south).
pub fn mercator_normalized(ll: LatLng) -> (f64, f64) {
    let lat = ll.lat.clamp(-MAX_LATITUDE_DEG, MAX_LATITUDE_DEG);
    let x = (ll.lng + 180.0) / 360.0;
    let sin = lat.to_radians().sin();
    let y = 0.5 - (((1.0 + sin) / (1.0 - sin)).ln()) / (4.0 * std::f64::consts::PI);
    (x, y)
}

/// Inverse of [`mercator_normalized`].
pub fn inverse_mercator(x: f64, y: f64) -> LatLng {
    let lng = x * 360.0 - 180.0;
    let n = std::f64::consts::PI * (1.0 - 2.0 * y);
    let lat = n.sinh().atan().to_degrees();
    LatLng { lat, lng }
}
