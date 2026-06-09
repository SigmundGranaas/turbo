//! Hands-off data provisioning from the admin panel.
//!
//! `POST /admin/api/provision` kicks off the full N50 chain (Geonorge
//! download → restore → all canonical upserts) for an area, returning a
//! `run_id` the SPA polls via `/api/ingest/jobs`. No file upload, no rsync.
//!
//! `GET /admin/api/geonorge/areas` proxies the Nedlasting area codelist so
//! the SPA can render a county dropdown without hardcoding fylke codes.

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use turbo_tiles_auth::{Curator, RequireRole};

use crate::error::AdminError;
use crate::state::AdminState;

const N50_UUID: &str = "ea192681-d039-42ec-b1bc-f3ce04c189ac";

#[derive(Debug, Deserialize)]
pub struct ProvisionBody {
    /// Two-digit county code (e.g. `"03"`) or `"national"`.
    pub area: String,
    /// Re-download + re-restore from scratch even if staging exists.
    #[serde(default)]
    pub force: bool,
}

/// Fire-and-forget the `provision-n50` job. Returns immediately with the
/// `run_id`; the SPA watches progress on `/api/ingest/jobs?name=provision-n50`.
pub async fn provision(
    _: RequireRole<Curator>,
    State(state): State<AdminState>,
    Json(body): Json<ProvisionBody>,
) -> Result<Json<Value>, AdminError> {
    // Validate the area up front so the curator gets an immediate 400 on a
    // typo rather than a job row that fails seconds later.
    turbo_tiles_ingest::geonorge::Area::parse(&body.area)
        .map_err(|e| AdminError::BadRequest(e.to_string()))?;

    let run_id = uuid::Uuid::new_v4();
    let opts = turbo_tiles_ingest::JobOptions {
        area: Some(body.area.clone()),
        run_id: Some(run_id),
        force: body.force,
        ..Default::default()
    };
    tracing::info!(area = %body.area, force = body.force, %run_id, "admin: provisioning N50");

    tokio::spawn(async move {
        // Provisioning is a minutes-long batch job — restore + upserts +
        // topology rebuilds. Run it on a dedicated pool with the statement
        // timeout disabled rather than the serving pool (whose short timeout
        // would kill the cold-cache contour/vegnett upserts).
        let pool = match turbo_tiles_db::DbConfig::from_env() {
            Ok(mut cfg) => {
                cfg.statement_timeout_ms = 0;
                cfg.max_connections = 2;
                match cfg.connect().await {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!(error = %e, "provision-n50: batch pool connect failed");
                        return;
                    }
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "provision-n50: DATABASE_URL missing");
                return;
            }
        };
        if let Err(e) = turbo_tiles_ingest::run_job_with_options(
            pool,
            turbo_tiles_ingest::JobName::ProvisionN50,
            opts,
        )
        .await
        {
            tracing::error!(error = %e, "provision-n50 failed");
        }
    });

    Ok(Json(json!({
        "ok": true,
        "job": "provision-n50",
        "area": body.area,
        "run_id": run_id,
    })))
}

/// Proxy the Geonorge area codelist for N50 so the SPA can populate a
/// county picker. Cached client-side; this is a thin pass-through that
/// normalises to `{code, name, type}` and prepends a "Whole country" entry.
pub async fn geonorge_areas(
    _: RequireRole<Curator>,
    State(_state): State<AdminState>,
) -> Result<Json<Value>, AdminError> {
    let url = format!("https://nedlasting.geonorge.no/api/codelists/area/{N50_UUID}");
    let client = reqwest::Client::builder()
        // Trust the OS trust store too (TLS-intercepting dev/CI proxies),
        // not just bundled webpki roots — same posture as the fetch client.
        .tls_built_in_native_certs(true)
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| AdminError::Upstream(format!("build client: {e}")))?;
    let resp: Value = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AdminError::Upstream(format!("geonorge areas: {e}")))?
        .error_for_status()
        .map_err(|e| AdminError::Upstream(format!("geonorge areas: {e}")))?
        .json()
        .await
        .map_err(|e| AdminError::Upstream(format!("geonorge areas not JSON: {e}")))?;

    let mut areas = vec![json!({ "code": "national", "name": "Hele landet", "type": "landsdekkende" })];
    if let Some(arr) = resp.as_array() {
        for a in arr {
            if a.get("type").and_then(|t| t.as_str()) == Some("fylke") {
                areas.push(json!({
                    "code": a.get("code").and_then(|c| c.as_str()).unwrap_or_default(),
                    "name": a.get("name").and_then(|n| n.as_str()).unwrap_or_default(),
                    "type": "fylke",
                }));
            }
        }
    }
    Ok(Json(json!({ "areas": areas })))
}
