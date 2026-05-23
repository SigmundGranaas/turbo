use axum::extract::FromRef;
use turbo_tiles_auth::AuthState;
use turbo_tiles_db::DbPool;

/// State injected into every admin handler. Cheap-cloneable so Axum
/// can move it freely (pool clone is an Arc internally, AuthState is
/// already an Arc).
#[derive(Clone)]
pub struct AdminState {
    pub db: DbPool,
    pub auth: AuthState,
}

impl FromRef<AdminState> for AuthState {
    fn from_ref(input: &AdminState) -> Self {
        input.auth.clone()
    }
}
