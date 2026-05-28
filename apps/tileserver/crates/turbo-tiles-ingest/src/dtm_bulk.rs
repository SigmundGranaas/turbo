//! Bulk DTM10 GeoTIFF loader.
//!
//! Kartverket ships DTM10 as a single zip containing hundreds of
//! tiled GeoTIFFs (`<tile>_10m_z33.tif` + `.tfw` + `.aux.xml`). This
//! job:
//!
//!   1. Resolves the zip under the incoming dir,
//!   2. Extracts it to a sibling directory (idempotent — skips if
//!      already extracted),
//!   3. Loops over `.tif` files, calling the per-file `load_geotiff`
//!      for each that hasn't been imported yet (keyed on source label
//!      + base filename),
//!   4. Stamps `source = 'dtm10'` on every row inserted.
//!
//! Idempotency: we keep a single side table `paths.dem_tile_log` that
//! records which `<filename>` has been ingested. Re-running skips
//! tiles already in the log.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

use turbo_tiles_db::DbPool;

use crate::job::{JobError, JobOutcome};

pub async fn run(pool: &DbPool, zip_path: PathBuf, source: String) -> Result<JobOutcome, JobError> {
    if !zip_path.exists() {
        return Err(JobError::Fetch(format!(
            "zip not found: {}",
            zip_path.display()
        )));
    }

    // Extract to <zip-dir>/<zip-stem>/. unzip is idempotent with -n
    // (never overwrite); on second-run we just confirm the directory
    // is there and proceed to the per-file loop.
    let stem = zip_path
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| JobError::Fetch("zip has no stem".into()))?;
    let extract_dir = zip_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(stem);
    std::fs::create_dir_all(&extract_dir)
        .map_err(|e| JobError::Fetch(format!("create extract dir: {e}")))?;

    let status = Command::new("unzip")
        .arg("-n") // never overwrite — makes the job idempotent
        .arg("-q") // quiet
        .arg(zip_path)
        .arg("-d")
        .arg(&extract_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .await
        .map_err(|e| JobError::Fetch(format!("spawn unzip: {e}")))?;
    if !status.success() {
        return Err(JobError::Fetch(format!("unzip exited with {status}")));
    }

    // Ensure the side log table exists so we can skip already-loaded
    // tiles cheaply. Lives in the paths schema next to dem.
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS paths.dem_tile_log (
            filename     text PRIMARY KEY,
            source       text NOT NULL,
            loaded_at    timestamptz NOT NULL DEFAULT now(),
            row_count    bigint NOT NULL DEFAULT 0
        )
        "#,
    )
    .execute(pool)
    .await?;

    let mut tiles: Vec<PathBuf> = Vec::new();
    collect_tifs(&extract_dir, &mut tiles)
        .map_err(|e| JobError::Fetch(format!("scan extract dir: {e}")))?;
    tiles.sort();

    let mut total_loaded = 0i64;
    let mut total_skipped = 0i64;
    // Iterate by owned `PathBuf` so no slice borrow crosses awaits —
    // keeps the future Send-for-all-lifetimes for tokio::spawn. Each
    // tile load is wrapped in a `Box::pin(async move { ... })` for
    // the same reason: it bounds the inner future's lifetime to the
    // owned values so the outer future remains Send for all
    // lifetimes (which tokio::spawn requires).
    for tile in tiles.into_iter() {
        let fname = tile
            .file_name()
            .and_then(|o| o.to_str())
            .unwrap_or("")
            .to_string();
        let already: Option<(String,)> = sqlx::query_as(
            "SELECT filename FROM paths.dem_tile_log WHERE filename = $1",
        )
        .bind(&fname)
        .fetch_optional(pool)
        .await?;
        if already.is_some() {
            total_skipped += 1;
            continue;
        }
        tracing::info!(file = %fname, "dtm-bulk-load: ingesting tile");
        let source_clone = source.clone();
        let pool_ref = pool;
        let outcome: JobOutcome = Box::pin(async move {
            crate::dtm_raster::load_geotiff(pool_ref, &tile, &source_clone).await
        })
        .await?;
        sqlx::query(
            r#"
            INSERT INTO paths.dem_tile_log (filename, source, row_count)
            VALUES ($1, $2, $3)
            ON CONFLICT (filename) DO UPDATE
                SET source = EXCLUDED.source, row_count = EXCLUDED.row_count, loaded_at = now()
            "#,
        )
        .bind(&fname)
        .bind(&source)
        .bind(outcome.rows_in)
        .execute(pool)
        .await?;
        total_loaded += 1;
    }

    Ok(JobOutcome {
        rows_in: total_loaded + total_skipped,
        rows_upserted: total_loaded,
    })
}

fn collect_tifs(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            collect_tifs(&p, out)?;
        } else if let Some(ext) = p.extension().and_then(OsStr::to_str) {
            let lower = ext.to_ascii_lowercase();
            if lower == "tif" || lower == "tiff" {
                out.push(p);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_tifs_finds_tifs_recursively() {
        // Build a small in-tmp directory layout and verify the
        // collector picks up both top-level and nested .tif files
        // and ignores non-tif extensions. Regression bait for
        // changes that accidentally filter case-sensitively or
        // skip subdirectories.
        let root = std::env::temp_dir().join("dtm-bulk-collect-test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::write(root.join("a.tif"), b"x").unwrap();
        std::fs::write(root.join("nested/b.TIFF"), b"x").unwrap();
        std::fs::write(root.join("nested/c.txt"), b"x").unwrap();
        let mut out = Vec::new();
        collect_tifs(&root, &mut out).unwrap();
        out.sort();
        assert_eq!(out.len(), 2);
        assert!(out.iter().any(|p| p.ends_with("a.tif")));
        assert!(out.iter().any(|p| p.ends_with("b.TIFF")));
        let _ = std::fs::remove_dir_all(&root);
    }
}
