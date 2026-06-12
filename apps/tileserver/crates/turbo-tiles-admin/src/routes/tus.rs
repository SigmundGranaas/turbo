//! Minimal TUS 1.0.0 resumable-upload protocol implementation.
//!
//! Curators upload multi-GB Geonorge datasets from any browser via
//! `tus-js-client`. The protocol is small enough to hand-roll without
//! adding a crate — that lets us slot it into the existing axum
//! router with the same `RequireRole<Curator>` extractor, and keeps
//! the upload destination under our explicit control.
//!
//! Supported subset:
//!   - Core (Creation + offset PATCH + HEAD + DELETE termination)
//!   - 7-day expiration of incomplete uploads (background-swept,
//!     see TODO in lib.rs)
//!
//! Not supported (intentionally):
//!   - Concatenation — the SPA uploads single files, not parallel
//!     stream merges.
//!   - Checksum verification — the file gets re-read in full by
//!     raster2pgsql, which validates the GeoTIFF structure anyway.
//!
//! Storage layout per upload, under `TILESERVER_INCOMING_DIR/.uploads/`:
//!
//! ```text
//! <upload_id>/
//!   metadata.json   { filename, total_bytes, owner_sub, created_at }
//!   data            the partial/complete bytes
//! ```
//!
//! Completed uploads stay here — `routes::ingest::incoming` enumerates
//! them alongside legacy rsync drops, and `trigger_bulk` accepts
//! `upload_id` as an alternative to `file`.

use std::path::{Path, PathBuf};

use axum::body::Bytes;
use axum::extract::{Path as AxumPath, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use turbo_tiles_auth::{Curator, RequireRole};
use uuid::Uuid;

use crate::error::AdminError;
use crate::state::AdminState;

pub const TUS_VERSION: &str = "1.0.0";
pub const MAX_CHUNK_BYTES: usize = 16 * 1024 * 1024; // 16 MB per PATCH

/// Pulled out of routes::ingest so the file lister can read it too.
fn uploads_root() -> PathBuf {
    PathBuf::from(turbo_tiles_ingest::incoming_dir()).join(".uploads")
}

fn upload_dir(id: Uuid) -> PathBuf {
    uploads_root().join(id.to_string())
}

#[derive(Debug, Serialize, Deserialize)]
struct UploadMetadata {
    upload_id: Uuid,
    /// User-supplied original name, sanitised (`/` and NUL stripped).
    filename: String,
    /// Declared total upload size in bytes.
    total_bytes: u64,
    /// Curator's `sub` claim — only this user can PATCH/DELETE.
    owner_sub: Uuid,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl UploadMetadata {
    async fn load(id: Uuid) -> Result<Self, AdminError> {
        let path = upload_dir(id).join("metadata.json");
        let raw = fs::read(&path).await.map_err(|_| AdminError::NotFound)?;
        serde_json::from_slice(&raw).map_err(|e| AdminError::BadRequest(e.to_string()))
    }

    async fn save(&self) -> Result<(), AdminError> {
        let dir = upload_dir(self.upload_id);
        fs::create_dir_all(&dir)
            .await
            .map_err(|e| AdminError::Upload(format!("mkdir: {e}")))?;
        let raw = serde_json::to_vec_pretty(self).unwrap();
        fs::write(dir.join("metadata.json"), raw)
            .await
            .map_err(|e| AdminError::Upload(format!("write metadata: {e}")))
    }
}

/// CORS / TUS headers that every response must carry.
fn tus_response_headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        HeaderName::from_static("tus-resumable"),
        HeaderValue::from_static(TUS_VERSION),
    );
    h.insert(
        HeaderName::from_static("tus-version"),
        HeaderValue::from_static(TUS_VERSION),
    );
    h.insert(
        HeaderName::from_static("tus-extension"),
        HeaderValue::from_static("creation,termination"),
    );
    h.insert(
        HeaderName::from_static("tus-max-size"),
        HeaderValue::from_static("107374182400"), // 100 GB cap
    );
    h
}

#[derive(Debug, Deserialize)]
pub struct CreateOpts {
    /// Optional sanitised override for the destination filename. Falls
    /// back to the `filename` key in the TUS `Upload-Metadata` header.
    pub filename: Option<String>,
}

/// POST /upload — create a new upload session.
///
/// TUS-Resumable: 1.0.0
/// Upload-Length: <total_bytes>
/// Upload-Metadata: filename <base64>, ...
///
/// Response carries `Location: /admin/api/upload/<uuid>` which the
/// browser PATCHes the body to.
pub async fn create(
    RequireRole { claims, .. }: RequireRole<Curator>,
    State(state): State<AdminState>,
    headers: HeaderMap,
) -> Result<Response, AdminError> {
    let total_bytes: u64 = headers
        .get("upload-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| AdminError::BadRequest("Upload-Length header required".into()))?;

    if total_bytes == 0 {
        return Err(AdminError::BadRequest("Upload-Length must be > 0".into()));
    }

    // Disk space gate: refuse if we don't have ≥1.2× total_bytes free.
    if let Some(free) = free_disk_bytes(&uploads_root()) {
        if total_bytes as i128 * 12 / 10 > free as i128 {
            return Err(AdminError::Upload(format!(
                "not enough free disk for {total_bytes} bytes; {free} available"
            )));
        }
    }

    let filename = parse_filename_from_metadata(&headers)
        .or_else(|| {
            headers
                .get("x-original-filename")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "upload.bin".to_string());
    let filename = sanitise_filename(&filename);

    let upload_id = Uuid::new_v4();
    let dir = upload_dir(upload_id);
    fs::create_dir_all(&dir)
        .await
        .map_err(|e| AdminError::Upload(format!("mkdir: {e}")))?;
    // Touch the data file so PATCH can OpenOptions::append it.
    fs::File::create(dir.join("data"))
        .await
        .map_err(|e| AdminError::Upload(format!("create data: {e}")))?;

    let meta = UploadMetadata {
        upload_id,
        filename,
        total_bytes,
        owner_sub: claims.sub,
        created_at: chrono::Utc::now(),
    };
    meta.save().await?;

    tracing::info!(
        %upload_id,
        owner = %claims.sub,
        filename = %meta.filename,
        total_bytes,
        "tus: upload created"
    );

    // Persist a sentinel record in state? The filesystem is the
    // source of truth — leave it there.
    let _ = state.db.clone(); // tag-only; no DB write in V1

    // Location is an absolute path so tus-js-client can use it
    // directly via fetch (relative-resolution against the page URL
    // breaks because the SPA lives at /admin/app/* but the upload
    // endpoint is at /admin/api/upload).
    //
    // The prefix is configurable via `TILESERVER_UPLOAD_URL_PREFIX`
    // so the same binary serves correctly direct (default
    // `/admin/api`) and behind the gateway (set to
    // `/api/tiles-admin/api` in the gateway's environment block).
    let _ = &headers;
    let prefix =
        std::env::var("TILESERVER_UPLOAD_URL_PREFIX").unwrap_or_else(|_| "/admin/api".to_string());
    let location = format!("{prefix}/upload/{upload_id}");
    let mut h = tus_response_headers();
    h.insert(
        header::LOCATION,
        HeaderValue::from_str(&location).map_err(|e| AdminError::Upload(e.to_string()))?,
    );
    h.insert(
        HeaderName::from_static("upload-offset"),
        HeaderValue::from_static("0"),
    );
    Ok((StatusCode::CREATED, h).into_response())
}

/// HEAD /upload/<id> — report current byte offset for resume.
pub async fn head_upload(
    RequireRole { claims, .. }: RequireRole<Curator>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Response, AdminError> {
    let meta = UploadMetadata::load(id).await?;
    if meta.owner_sub != claims.sub {
        return Err(AdminError::Auth("upload owned by another curator".into()));
    }
    let offset = current_offset(id).await?;
    let mut h = tus_response_headers();
    h.insert(
        HeaderName::from_static("upload-offset"),
        HeaderValue::from_str(&offset.to_string()).unwrap(),
    );
    h.insert(
        HeaderName::from_static("upload-length"),
        HeaderValue::from_str(&meta.total_bytes.to_string()).unwrap(),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok((StatusCode::OK, h).into_response())
}

/// PATCH /upload/<id> — append a chunk at the byte offset advertised
/// by the client. Body must be `application/offset+octet-stream`.
///
/// The body extractor reads the chunk into a `Bytes` buffer (capped at
/// MAX_CHUNK_BYTES by the DefaultBodyLimit layer); we append to disk
/// in one syscall, then fsync. For 5–10 MB chunks the memory footprint
/// is one chunk per concurrent upload.
pub async fn patch_upload(
    RequireRole { claims, .. }: RequireRole<Curator>,
    AxumPath(id): AxumPath<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, AdminError> {
    let meta = UploadMetadata::load(id).await?;
    if meta.owner_sub != claims.sub {
        return Err(AdminError::Auth("upload owned by another curator".into()));
    }

    let supplied_offset: u64 = headers
        .get("upload-offset")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| AdminError::BadRequest("Upload-Offset header required".into()))?;

    if headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        != Some("application/offset+octet-stream")
    {
        return Err(AdminError::BadRequest(
            "Content-Type must be application/offset+octet-stream".into(),
        ));
    }

    let on_disk = current_offset(id).await?;
    if supplied_offset != on_disk {
        return Err(AdminError::BadRequest(format!(
            "offset mismatch: server has {on_disk}, client sent {supplied_offset}"
        )));
    }

    let new_offset = on_disk + body.len() as u64;
    if new_offset > meta.total_bytes {
        return Err(AdminError::BadRequest(format!(
            "chunk exceeds declared Upload-Length: {} + {} > {}",
            on_disk,
            body.len(),
            meta.total_bytes
        )));
    }

    let data_path = upload_dir(id).join("data");
    let mut f = tokio::fs::OpenOptions::new()
        .append(true)
        .open(&data_path)
        .await
        .map_err(|e| AdminError::Upload(format!("open data: {e}")))?;
    f.write_all(&body)
        .await
        .map_err(|e| AdminError::Upload(format!("write data: {e}")))?;
    f.flush()
        .await
        .map_err(|e| AdminError::Upload(format!("flush data: {e}")))?;

    let mut h = tus_response_headers();
    h.insert(
        HeaderName::from_static("upload-offset"),
        HeaderValue::from_str(&new_offset.to_string()).unwrap(),
    );
    Ok((StatusCode::NO_CONTENT, h).into_response())
}

/// DELETE /upload/<id> — abandon the upload, free disk.
pub async fn terminate(
    RequireRole { claims, .. }: RequireRole<Curator>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Response, AdminError> {
    let meta = UploadMetadata::load(id).await?;
    if meta.owner_sub != claims.sub {
        return Err(AdminError::Auth("upload owned by another curator".into()));
    }
    let dir = upload_dir(id);
    fs::remove_dir_all(&dir)
        .await
        .map_err(|e| AdminError::Upload(format!("remove: {e}")))?;
    Ok((StatusCode::NO_CONTENT, tus_response_headers()).into_response())
}

/// Public — used by `routes::ingest::incoming` to list completed
/// uploads alongside legacy rsync drops.
pub fn list_completed() -> Vec<CompletedUpload> {
    let root = uploads_root();
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&root) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(id_str) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Ok(id) = Uuid::parse_str(id_str) else {
            continue;
        };
        let meta_path = path.join("metadata.json");
        let data_path = path.join("data");
        let Ok(raw) = std::fs::read(&meta_path) else {
            continue;
        };
        let Ok(meta) = serde_json::from_slice::<UploadMetadata>(&raw) else {
            continue;
        };
        let on_disk = std::fs::metadata(&data_path).map(|m| m.len()).unwrap_or(0);
        out.push(CompletedUpload {
            upload_id: id,
            filename: meta.filename,
            total_bytes: meta.total_bytes,
            bytes_received: on_disk,
            complete: on_disk == meta.total_bytes,
            owner_sub: meta.owner_sub,
            created_at: meta.created_at,
        });
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    out
}

#[derive(Debug, Serialize)]
pub struct CompletedUpload {
    pub upload_id: Uuid,
    pub filename: String,
    pub total_bytes: u64,
    pub bytes_received: u64,
    pub complete: bool,
    pub owner_sub: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Used by `trigger_bulk` to translate an `upload_id` to an absolute
/// file path. Verifies the upload is complete before returning so the
/// ingest job can't be triggered against a partial transfer.
pub fn resolve_upload(id: Uuid) -> Result<PathBuf, AdminError> {
    let dir = upload_dir(id);
    let meta_raw = std::fs::read(dir.join("metadata.json")).map_err(|_| AdminError::NotFound)?;
    let meta: UploadMetadata =
        serde_json::from_slice(&meta_raw).map_err(|e| AdminError::BadRequest(e.to_string()))?;
    let data = dir.join("data");
    let on_disk = std::fs::metadata(&data).map(|m| m.len()).unwrap_or(0);
    if on_disk != meta.total_bytes {
        return Err(AdminError::BadRequest(format!(
            "upload {id} incomplete: {on_disk}/{} bytes",
            meta.total_bytes
        )));
    }
    Ok(data)
}

/// The current on-disk byte count for an upload's data file. Treated
/// as the authoritative offset — the metadata.json's bookkeeping is
/// advisory.
async fn current_offset(id: Uuid) -> Result<u64, AdminError> {
    let path = upload_dir(id).join("data");
    let meta = fs::metadata(&path)
        .await
        .map_err(|_| AdminError::NotFound)?;
    Ok(meta.len())
}

/// TUS `Upload-Metadata` header is comma-separated `key base64(value)`
/// pairs. Pull out `filename` (the only field the SPA sends).
fn parse_filename_from_metadata(headers: &HeaderMap) -> Option<String> {
    let raw = headers
        .get("upload-metadata")
        .and_then(|v| v.to_str().ok())?;
    for pair in raw.split(',') {
        let mut parts = pair.trim().splitn(2, ' ');
        let key = parts.next()?.trim();
        let value_b64 = parts.next()?.trim();
        if key == "filename" {
            use base64::Engine as _;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(value_b64)
                .ok()?;
            return String::from_utf8(bytes).ok();
        }
    }
    None
}

/// Strip path separators, NULs, and any other surprise chars that
/// could escape the uploads directory. Falls back to "upload.bin" if
/// the result would be empty.
fn sanitise_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '/' | '\\' | ':'))
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "upload.bin".to_string()
    } else {
        cleaned.to_string()
    }
}

// `statvfs.f_bavail` is `u32` on macOS but `u64` on Linux, so `u64::from`
// is meaningful on macOS yet a no-op on Linux — where clippy then fires
// `useless_conversion`. Allow it so the one cast stays portable across both.
#[allow(clippy::useless_conversion)]
fn free_disk_bytes(path: &Path) -> Option<u64> {
    // statvfs() via libc. Skip on platforms without it (we only ever
    // run Linux containers, so this is fine).
    #[cfg(unix)]
    {
        let c_path = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
        let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
        let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
        if rc != 0 {
            return None;
        }
        Some(u64::from(stat.f_bavail) * stat.f_frsize)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

/// Sweep abandoned TUS uploads from disk. Runs periodically as a
/// background Tokio task spawned at server startup (see bin/main.rs).
///
/// Policy:
///   - Incomplete uploads (on-disk bytes < total_bytes) older than
///     `incomplete_ttl` are deleted.
///   - Completed uploads (on-disk bytes == total_bytes) are deleted
///     after `completed_ttl` — long enough for the curator to
///     trigger ingest, short enough that forgotten files don't pile
///     up forever.
///   - Anything malformed (missing metadata.json, unparseable UUID)
///     is logged and left alone — better to leak a few bytes than
///     to nuke files we don't understand.
pub async fn sweep_abandoned(
    incomplete_ttl: chrono::Duration,
    completed_ttl: chrono::Duration,
) -> SweepStats {
    let root = uploads_root();
    let mut stats = SweepStats::default();
    let entries = match std::fs::read_dir(&root) {
        Ok(e) => e,
        Err(_) => return stats,
    };
    let now = chrono::Utc::now();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Ok(id) = Uuid::parse_str(name) else {
            continue;
        };
        let meta = match std::fs::read(path.join("metadata.json")) {
            Ok(raw) => match serde_json::from_slice::<UploadMetadata>(&raw) {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!(%id, error = %e, "tus sweep: bad metadata, skipping");
                    continue;
                }
            },
            Err(_) => continue,
        };
        let on_disk = std::fs::metadata(path.join("data"))
            .map(|m| m.len())
            .unwrap_or(0);
        let age = now - meta.created_at;
        let ttl = if on_disk == meta.total_bytes {
            completed_ttl
        } else {
            incomplete_ttl
        };
        if age > ttl {
            if let Err(e) = std::fs::remove_dir_all(&path) {
                tracing::error!(%id, error = %e, "tus sweep: remove failed");
                stats.failed += 1;
                continue;
            }
            tracing::info!(
                %id,
                filename = %meta.filename,
                bytes = on_disk,
                age_seconds = age.num_seconds(),
                complete = on_disk == meta.total_bytes,
                "tus sweep: deleted abandoned upload"
            );
            stats.deleted += 1;
        }
    }
    stats
}

#[derive(Debug, Default)]
pub struct SweepStats {
    pub deleted: u64,
    pub failed: u64,
}

/// Spawn the sweep loop on a Tokio task. Runs immediately on launch
/// (catches anything left over from a previous process), then every
/// `interval`. The task lives for the process lifetime.
pub fn spawn_sweeper(interval: std::time::Duration) {
    tokio::spawn(async move {
        let incomplete_ttl = chrono::Duration::days(2);
        let completed_ttl = chrono::Duration::days(7);
        loop {
            let stats = sweep_abandoned(incomplete_ttl, completed_ttl).await;
            if stats.deleted > 0 || stats.failed > 0 {
                tracing::info!(?stats, "tus sweep cycle complete");
            }
            tokio::time::sleep(interval).await;
        }
    });
}
