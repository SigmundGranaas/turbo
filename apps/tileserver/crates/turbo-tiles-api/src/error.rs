use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("not found")]
    NotFound,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("database error: {0}")]
    Db(String),
    /// The requested primitive has no artifact loaded. Server is up
    /// but this endpoint is degraded.
    #[error("primitive unavailable: {0}")]
    PrimitiveUnavailable(&'static str),
    /// The query lies outside loaded primitives' coverage. Carries
    /// a JSON payload with `available_coverage` so the SPA can
    /// fly the user to where data does exist.
    #[error("{message}")]
    NoCoverage {
        message: String,
        details: serde_json::Value,
    },
    #[error("internal: {0}")]
    Internal(String),
    #[error(transparent)]
    Auth(#[from] turbo_tiles_auth::AuthError),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            ApiError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "database error".into()),
            ApiError::PrimitiveUnavailable(p) => (
                StatusCode::SERVICE_UNAVAILABLE,
                format!("primitive {p} unavailable — artifact not loaded"),
            ),
            ApiError::NoCoverage { message, details } => {
                // 422 Unprocessable Entity — request is well-formed
                // but the system has no data to answer it. Payload
                // includes the coverage hint so the SPA can guide
                // the user to where data does exist.
                let body = json!({"error": message, "details": details});
                return (StatusCode::UNPROCESSABLE_ENTITY, Json(body)).into_response();
            }
            ApiError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m.clone()),
            ApiError::Auth(e) => return e.clone_into_response(),
        };
        if matches!(self, ApiError::Db(_)) {
            tracing::error!(error = %self, "api error");
        }
        (status, Json(json!({"error": message}))).into_response()
    }
}

// Workaround: AuthError is not Clone, so propagate by re-rendering.
trait CloneIntoResponse {
    fn clone_into_response(&self) -> Response;
}

impl CloneIntoResponse for turbo_tiles_auth::AuthError {
    fn clone_into_response(&self) -> Response {
        let status = match self {
            turbo_tiles_auth::AuthError::MissingToken | turbo_tiles_auth::AuthError::Invalid(_) => {
                StatusCode::UNAUTHORIZED
            }
            turbo_tiles_auth::AuthError::MissingRole => StatusCode::FORBIDDEN,
        };
        let body = json!({"error": self.to_string()});
        (status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        ApiError::Db(e.to_string())
    }
}
