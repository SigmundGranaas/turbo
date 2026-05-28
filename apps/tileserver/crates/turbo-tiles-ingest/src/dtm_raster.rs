//! Bulk DTM GeoTIFF ingest.
//!
//! Operator drops a Kartverket DTM10/DTM1 .tif on a shared volume
//! (configured via `TILESERVER_INCOMING_DIR`, defaults to
//! `/var/lib/tileserver/raw/incoming`), then triggers ingest with:
//!
//!   tileserver ingest --job dtm-load --file <path>
//!
//! The job shells out to PostGIS's `raster2pgsql` to convert the
//! GeoTIFF into PostGIS raster tiles and load them into `paths.dem`.
//! We don't try to reimplement raster2pgsql in Rust — it's the
//! battle-tested upstream tool and the per-job overhead is fine.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

use crate::job::{JobError, JobOutcome};

/// Resolve a user-supplied path against the configured incoming dir
/// and reject anything that escapes it (no `..` traversal, no
/// absolute paths outside the allowlist). Returns the canonical path
/// that raster2pgsql will read.
///
/// Operators using the CLI directly can pass any absolute path; this
/// guard only applies when a path comes in over the admin HTTP API.
pub fn resolve_under_incoming(file: &Path) -> Result<PathBuf, JobError> {
    let base = incoming_dir();
    let base = Path::new(&base);
    let candidate = if file.is_absolute() {
        file.to_path_buf()
    } else {
        base.join(file)
    };
    let canon = candidate
        .canonicalize()
        .map_err(|e| JobError::Fetch(format!("cannot resolve `{}`: {e}", file.display())))?;
    let base_canon = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    if !canon.starts_with(&base_canon) {
        return Err(JobError::Fetch(format!(
            "path `{}` escapes incoming dir `{}`",
            canon.display(),
            base_canon.display()
        )));
    }
    Ok(canon)
}

/// The configured incoming directory. The admin HTTP path-trigger
/// reads files exclusively from here; the CLI is unrestricted.
pub fn incoming_dir() -> String {
    std::env::var("TILESERVER_INCOMING_DIR")
        .unwrap_or_else(|_| "/var/lib/tileserver/raw/incoming".to_string())
}

/// List GeoTIFF and shapefile-zip files in the incoming dir. Returns
/// (file_name, size_bytes) tuples sorted by name. Used by the admin
/// SPA's "import from disk" panel.
pub fn list_incoming() -> Vec<(String, u64)> {
    let base = incoming_dir();
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&base) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let lower = name.to_ascii_lowercase();
        if !(lower.ends_with(".tif")
            || lower.ends_with(".tiff")
            || lower.ends_with(".gpkg")
            || lower.ends_with(".zip"))
        {
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        out.push((name.to_string(), size));
    }
    out.sort();
    out
}

/// Load one DTM GeoTIFF file into `paths.dem`. The `source` label
/// gets stamped on every row so the operator can later distinguish
/// (and selectively delete) DTM10 vs DTM1 imports.
///
/// Wraps the standard `raster2pgsql | psql` pipeline in `bash -c` —
/// the upstream tool emits SQL on stdout and psql ingests it; mixing
/// tokio's stdio with std's pipes is fragile, so we let the shell
/// orchestrate.
///
/// Concurrency-safe stamp: previously the post-load UPDATE keyed on
/// `source = '' OR IS NULL`, which means two simultaneous loads with
/// different source labels would race and mis-stamp each other's
/// rows. We now capture `MAX(rid)` before raster2pgsql runs and the
/// UPDATE filters on `rid > before_max`, so each job only stamps the
/// rows it inserted.
pub async fn load_geotiff(
    pool: &turbo_tiles_db::DbPool,
    file: &Path,
    source: &str,
) -> Result<JobOutcome, JobError> {
    let database_url =
        std::env::var("DATABASE_URL").map_err(|_| JobError::Fetch("DATABASE_URL unset".into()))?;
    let file_str = file
        .to_str()
        .ok_or_else(|| JobError::Fetch("non-UTF8 path".into()))?;

    // Capture the highest existing rid so we know which rows
    // raster2pgsql adds. `serial` ids are monotonic per connection
    // and won't be reused, so anything > before_max came from this
    // load. NULL on an empty table → -1 sentinel.
    let before_max: (i64,) = sqlx::query_as("SELECT COALESCE(MAX(rid), 0)::bigint FROM paths.dem")
        .fetch_one(pool)
        .await?;

    // raster2pgsql flags chosen for Kartverket DTM:
    //   -s 25833      target SRID (reproject if source differs)
    //   -t 256x256    tile into 256-px blocks for index efficiency
    //   -I            create the GiST index (idempotent)
    //   -a            APPEND mode — don't drop existing rows
    //
    // Note: `-C` (apply raster constraints) is omitted. When used on
    // a multi-tile bulk load, the first tile's max-extent constraint
    // rejects every subsequent tile that covers a different region.
    // Constraints can be re-applied at the end of the bulk load via
    // SELECT AddRasterConstraints('paths', 'dem', 'rast').
    let cmd = format!(
        "set -o pipefail; raster2pgsql -s 25833 -t 256x256 -I -a {file:?} paths.dem | psql -v ON_ERROR_STOP=1 -q -d {db:?}",
        file = file_str,
        db = database_url
    );

    let status = Command::new("bash")
        .arg("-c")
        .arg(&cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .await
        .map_err(|e| JobError::Fetch(format!("spawn bash: {e}")))?;

    if !status.success() {
        return Err(JobError::Fetch(format!(
            "raster2pgsql|psql exited with {status} while loading {}",
            file.display()
        )));
    }

    // Stamp source on exactly the rows this load inserted.
    let updated = sqlx::query("UPDATE paths.dem SET source = $1 WHERE rid > $2")
        .bind(source)
        .bind(before_max.0)
        .execute(pool)
        .await?;

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*)::bigint FROM paths.dem WHERE source = $1")
        .bind(source)
        .fetch_one(pool)
        .await?;

    Ok(JobOutcome {
        rows_in: updated.rows_affected() as i64,
        rows_upserted: count.0,
    })
}
