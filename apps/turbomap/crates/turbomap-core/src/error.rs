//! Crate-level error types.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MapError {
    #[error("wgpu: {0}")]
    Wgpu(String),
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
