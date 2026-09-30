//! Crate-level error types.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MapError {
    #[error("wgpu: {0}")]
    Wgpu(String),
}

/// Why a frame was not drawn. The renderer reports it; the host decides
/// what to do — a product's render loop may log and carry on, a test or an
/// embedding app that treats it as a bug fails. Never decided silently here.
#[derive(Debug, Error, Clone, Copy, PartialEq)]
pub enum RenderError {
    /// The camera produced a non-finite view: input guards let a NaN or an
    /// infinity through, or steep pitch/zoom maths degenerated. Nothing was
    /// encoded (a mobile driver hangs on a NaN matrix); the frame's metrics
    /// record it as dropped.
    #[error("non-finite camera: pitch {pitch_deg}°, zoom {zoom}, view-projection finite: {view_projection_finite} — nothing drawn")]
    NonFiniteCamera {
        pitch_deg: f64,
        zoom: f64,
        view_projection_finite: bool,
    },
    /// The host refused an essential upload (see `crate::upload`).
    #[error("{0}")]
    Upload(#[from] crate::upload::UploadRefused),
}

/// Why a [`crate::LatLngBounds`] could not be built or fitted.
#[derive(Debug, Error, Clone, Copy, PartialEq)]
pub enum FitError {
    /// A corner was NaN or infinite.
    #[error("bounds corner is not finite: lat {lat}, lng {lng}")]
    NonFiniteCorner { lat: f64, lng: f64 },
    /// South above north, or a latitude outside ±90°.
    #[error("bounds latitudes are not south ≤ north within ±90°: south {south}, north {north}")]
    Latitudes { south: f64, north: f64 },
    /// A longitude outside ±180°.
    #[error("bounds longitude {lng} is outside ±180°")]
    Longitude { lng: f64 },
    /// The visible viewport — the viewport minus the camera's insets and the
    /// padding on every side — has no width or no height to fit into.
    #[error("no room to fit bounds: viewport {viewport_w}×{viewport_h} px leaves {room_w}×{room_h} px after insets and {padding_px} px padding")]
    NoRoom {
        viewport_w: f64,
        viewport_h: f64,
        padding_px: f64,
        room_w: f64,
        room_h: f64,
    },
}

#[derive(Debug, Error)]
pub enum TileError {
    #[error("network: {0}")]
    Network(String),
    #[error("decode: {0}")]
    Decode(String),
    #[error("zoom {0} is outside the source's supported range")]
    ZoomOutOfRange(u8),
}
