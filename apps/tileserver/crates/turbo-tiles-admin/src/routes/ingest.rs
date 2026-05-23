use std::path::PathBuf;

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use turbo_tiles_auth::{Curator, RequireRole};

use crate::error::AdminError;
use crate::state::AdminState;

/// Kick off an ingest job. Spawns a Tokio task so the admin call
/// returns immediately with the queued job id; clients poll
/// `/api/ingest/jobs` to watch progress.
pub async fn trigger(
    _: RequireRole<Curator>,
    State(state): State<AdminState>,
    Path(job): Path<String>,
) -> Result<Json<Value>, AdminError> {
    let job_name: turbo_tiles_ingest::JobName =
        job.parse().map_err(|e: String| AdminError::BadRequest(e))?;

    let pool = state.db.clone();
    let run_id = uuid::Uuid::new_v4();
    tracing::info!(job = job_name.as_str(), %run_id, "admin: triggering ingest");

    tokio::spawn(async move {
        if let Err(e) = turbo_tiles_ingest::run_job(&pool, job_name).await {
            tracing::error!(error = %e, job = job_name.as_str(), "ingest failed");
        }
    });

    Ok(Json(json!({
        "ok": true,
        "job": job_name.as_str(),
        "run_id": run_id,
    })))
}

#[derive(Debug, Deserialize)]
pub struct BulkBody {
    pub job: String,
    /// File name relative to the configured incoming dir, or an
    /// absolute path that resolves under it. Mutually exclusive with
    /// `upload_id` — pick one.
    #[serde(default)]
    pub file: Option<String>,
    /// UUID of a completed TUS upload. The server resolves it to the
    /// on-disk path under `<incoming>/.uploads/<id>/data`.
    #[serde(default)]
    pub upload_id: Option<uuid::Uuid>,
    #[serde(default)]
    pub source: Option<String>,
}

/// Trigger a bulk-file job (e.g. `dtm-load`) against a file that
/// already exists on the shared incoming volume. The path is
/// validated against `TILESERVER_INCOMING_DIR` before the job runs so
/// curators can't ask the server to read arbitrary files. Multi-GB
/// uploads bypass HTTPS entirely — drop the file on the volume
/// out-of-band (rsync, sftp, cloud-storage-fuse) then POST this.
pub async fn trigger_bulk(
    _: RequireRole<Curator>,
    State(state): State<AdminState>,
    Json(body): Json<BulkBody>,
) -> Result<Json<Value>, AdminError> {
    let job_name: turbo_tiles_ingest::JobName = body
        .job
        .parse()
        .map_err(|e: String| AdminError::BadRequest(e))?;

    let resolved = match (&body.file, body.upload_id) {
        (Some(file), None) => turbo_tiles_ingest::resolve_under_incoming(&PathBuf::from(file))
            .map_err(|e| AdminError::BadRequest(e.to_string()))?,
        (None, Some(id)) => super::tus::resolve_upload(id)?,
        (Some(_), Some(_)) => {
            return Err(AdminError::BadRequest(
                "specify either `file` or `upload_id`, not both".into(),
            ))
        }
        (None, None) => {
            return Err(AdminError::BadRequest(
                "either `file` or `upload_id` is required".into(),
            ))
        }
    };

    let pool = state.db.clone();
    let run_id = uuid::Uuid::new_v4();
    let opts = turbo_tiles_ingest::JobOptions {
        bbox: None,
        file: Some(resolved.clone()),
        source: body.source,
        // Thread the run_id through so the eventual paths.ingest_job
        // row carries this exact UUID — the SPA polls by run_id.
        run_id: Some(run_id),
        force: false,
    };
    tracing::info!(
        job = job_name.as_str(),
        %run_id,
        file = %resolved.display(),
        "admin: triggering bulk ingest"
    );
    tokio::spawn(async move {
        if let Err(e) = turbo_tiles_ingest::run_job_with_options(&pool, job_name, opts).await {
            tracing::error!(error = %e, job = job_name.as_str(), "bulk ingest failed");
        }
    });

    Ok(Json(json!({
        "ok": true,
        "job": job_name.as_str(),
        "run_id": run_id,
        "file": resolved.display().to_string(),
    })))
}

/// List files staged for ingest. Two sources are merged: rsync-style
/// drops at the top level of the incoming volume, and completed TUS
/// uploads under `.uploads/<id>/data`. The SPA shows both with
/// uploads tagged as such so the curator can tell where each file
/// came from.
pub async fn incoming(
    _: RequireRole<Curator>,
    State(_state): State<AdminState>,
) -> Result<Json<Value>, AdminError> {
    let dir = turbo_tiles_ingest::incoming_dir();
    let mut files: Vec<Value> = turbo_tiles_ingest::list_incoming()
        .into_iter()
        .map(|(name, size)| {
            json!({
                "source": "rsync",
                "name": name,
                "size_bytes": size,
            })
        })
        .collect();
    for upload in super::tus::list_completed() {
        files.push(json!({
            "source": "upload",
            "name": upload.filename,
            "size_bytes": upload.bytes_received,
            "total_bytes": upload.total_bytes,
            "complete": upload.complete,
            "upload_id": upload.upload_id,
            "created_at": upload.created_at,
        }));
    }
    Ok(Json(json!({
        "incoming_dir": dir,
        "files": files,
    })))
}

#[derive(Debug, Deserialize)]
pub struct JobsQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

fn default_limit() -> i64 {
    50
}

pub async fn jobs(
    _: RequireRole<Curator>,
    State(state): State<AdminState>,
    Query(q): Query<JobsQuery>,
) -> Result<Json<Value>, AdminError> {
    use sqlx::Row;
    let limit = q.limit.clamp(1, 200);
    let raw = sqlx::query(
        r#"
        SELECT id, run_id, name, status::text AS status,
               started_at, finished_at, rows_in, rows_upserted, error_text
        FROM paths.ingest_job
        WHERE ($1::text IS NULL OR status::text = $1)
          AND ($2::text IS NULL OR name = $2)
        ORDER BY id DESC
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(&q.status)
    .bind(&q.name)
    .bind(limit)
    .bind(q.offset)
    .fetch_all(&state.db)
    .await?;

    let rows: Vec<Value> = raw
        .into_iter()
        .map(|r| {
            json!({
                "id": r.try_get::<i64, _>("id").unwrap_or(0),
                "run_id": r.try_get::<uuid::Uuid, _>("run_id").ok(),
                "name": r.try_get::<String, _>("name").unwrap_or_default(),
                "status": r.try_get::<String, _>("status").unwrap_or_default(),
                "started_at": r.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("started_at").ok().flatten(),
                "finished_at": r.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("finished_at").ok().flatten(),
                "rows_in": r.try_get::<i64, _>("rows_in").unwrap_or(0),
                "rows_upserted": r.try_get::<i64, _>("rows_upserted").unwrap_or(0),
                "error_text": r.try_get::<Option<String>, _>("error_text").ok().flatten(),
            })
        })
        .collect();

    Ok(Json(json!({ "rows": rows })))
}
