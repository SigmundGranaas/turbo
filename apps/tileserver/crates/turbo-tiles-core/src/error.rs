use thiserror::Error;

/// Public error surface for the core crate. HTTP-layer errors live in
/// `turbo-tiles-api::error` and wrap these; never expose this enum
/// directly to clients.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("not found")]
    NotFound,
}
