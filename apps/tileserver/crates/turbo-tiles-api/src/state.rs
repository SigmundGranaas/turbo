use std::sync::Arc;

use axum::extract::FromRef;
use turbo_tiles_auth::{AuthConfig, AuthState};
use turbo_tiles_db::DbPool;

/// Server state passed to every handler. Cheap-cloneable (pool clone is
/// an Arc internally, auth is already Arc-wrapped), so Axum can move it
/// freely and `FromRef` impls stay local to this crate.
#[derive(Clone)]
pub struct ApiState {
    pub db: DbPool,
    pub auth: AuthState,
    pub public_base_url: Arc<String>,
}

impl ApiState {
    pub fn new(db: DbPool, auth: AuthConfig, public_base_url: String) -> Self {
        Self {
            db,
            auth: AuthState(Arc::new(auth)),
            public_base_url: Arc::new(public_base_url),
        }
    }
}

impl FromRef<ApiState> for AuthState {
    fn from_ref(input: &ApiState) -> Self {
        input.auth.clone()
    }
}
