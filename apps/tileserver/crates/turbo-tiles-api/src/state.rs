use std::sync::Arc;

use axum::extract::FromRef;
use turbo_tiles_auth::{AuthConfig, AuthState};
use turbo_tiles_db::DbPool;

/// Server state passed to every handler.
///
/// Stage 0 reset: the recommendation engine bundle (`TerrainServices`)
/// is gone. The new primitive handles land per stage:
///   - Stage 1: `dem: Option<Arc<Dem>>` (elevation primitive)
///   - Stage 3: `mask: Option<Arc<Mask>>`
///   - Stage 4: `graph: Option<Arc<Graph>>`
///   - Stage 5: `search: Option<Arc<Index>>`
///   - Stage 6: `pathfinder: Option<Arc<Pathfinder>>`
///
/// Each is an `Option` so the server boots in degraded mode when an
/// artifact isn't present; the affected endpoint returns 503 instead
/// of refusing to start.
///
/// `db` stays required for now; Stage 7 introduces `--no-db` mode
/// that drops the legacy catalog/resource/tiles endpoints.
#[derive(Clone)]
pub struct ApiState {
    pub db: DbPool,
    pub auth: AuthState,
    pub public_base_url: Arc<String>,
    pub dem: Option<Arc<turbo_tiles_elev::Dem>>,
    pub mask: Option<Arc<turbo_tiles_mask::Mask>>,
    pub graph: Option<Arc<turbo_tiles_graph::Graph>>,
    pub search: Option<Arc<turbo_tiles_search::Index>>,
    /// Auto-loaded landcover masks keyed by short class name
    /// (`wetland`, `forest`, …). Same `Mask` format as the
    /// water/glacier `mask` field; segregated by class so the SPA
    /// inspect endpoints can render them as separate overlays
    /// without scanning the pathfinder's `CostLayer` stack.
    pub landcover: std::collections::HashMap<&'static str, Arc<turbo_tiles_mask::Mask>>,
    /// Set at boot once the primitive artifacts are loaded. Holds
    /// the registered `CostLayer` stack — additions (custom layers
    /// for marsh, ridges, etc.) get plugged in at construction.
    pub pathfinder: Option<Arc<turbo_tiles_pathfind::Pathfinder>>,
}

impl ApiState {
    pub fn new(db: DbPool, auth: AuthConfig, public_base_url: String) -> Self {
        Self {
            db,
            auth: AuthState(Arc::new(auth)),
            public_base_url: Arc::new(public_base_url),
            dem: None,
            mask: None,
            graph: None,
            search: None,
            landcover: std::collections::HashMap::new(),
            pathfinder: None,
        }
    }
}

impl FromRef<ApiState> for AuthState {
    fn from_ref(input: &ApiState) -> Self {
        input.auth.clone()
    }
}
