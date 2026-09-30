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

#[derive(Debug, Error)]
pub enum TileError {
    #[error("network: {0}")]
    Network(String),
    #[error("decode: {0}")]
    Decode(String),
    #[error("zoom {0} is outside the source's supported range")]
    ZoomOutOfRange(u8),
}
