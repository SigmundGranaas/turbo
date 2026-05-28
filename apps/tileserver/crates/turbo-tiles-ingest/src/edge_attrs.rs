//! Per-edge terrain attribute derivation, chunked.
//!
//! Samples `paths.dem` at each edge's start + end vertex (NOT every
//! intermediate vertex — the per-vertex variant is correct but O(N×V)
//! and produces single-transaction UPDATEs that don't scale to the
//! 1M+ national edge set). Endpoint sampling captures the dominant
//! gradient for cost purposes; the off-trail mesh's local Theta\* gets
//! finer-grained slope from the local-mesh cost samples anyway.
//!
//! Per chunk:
//!   - Pick up to CHUNK_SIZE edges with max_slope_deg IS NULL.
//!   - For each: ST_Value at start + end via GiST-indexed paths.dem.
//!   - Compute slope = |dz / length|, aspect = ST_Azimuth(start, end).
//!   - Commit. Loop.
//!
//! Chunked commits mean progress is visible in the DB and the job can
//! be interrupted + resumed.

use turbo_tiles_db::DbPool;

use crate::job::{JobError, JobOutcome};

const CHUNK_SIZE: i64 = 50_000;

pub async fn run(pool: &DbPool, force: bool) -> Result<JobOutcome, JobError> {
    let (dem_rows,): (i64,) = sqlx::query_as("SELECT COUNT(*)::bigint FROM paths.dem")
        .fetch_one(pool)
        .await?;
    if dem_rows == 0 {
        return Err(JobError::Fetch(
            "paths.dem is empty — run dtm-load or dtm-bulk-load first".into(),
        ));
    }

    // Force-mode wipes prior slope values so every edge is re-sampled.
    if force {
        sqlx::query(
            "UPDATE paths.edge SET max_slope_deg = NULL, \
                                    mean_slope_deg = NULL, \
                                    mean_aspect_deg = NULL \
             WHERE deleted_at IS NULL",
        )
        .execute(pool)
        .await?;
    }

    let mut total_updated: i64 = 0;
    loop {
        // CTE picks the next CHUNK_SIZE candidate edges and updates
        // them in one transaction. Commits between chunks so the DB
        // shows progress and the job is interruptible.
        let chunk_sql = format!(
            r#"
            WITH cand AS (
                SELECT id, geom
                FROM paths.edge
                WHERE deleted_at IS NULL AND max_slope_deg IS NULL
                LIMIT {chunk}
            ),
            sampled AS (
                SELECT c.id,
                       ST_Length(c.geom) AS len,
                       degrees(ST_Azimuth(ST_StartPoint(c.geom),
                                          ST_EndPoint(c.geom))) AS azim,
                       -- The dem table has a GIST index on
                       -- ST_ConvexHull(rast). Filter against that
                       -- index expression first so candidate tiles
                       -- are picked from the index, not full scan.
                       (SELECT ST_Value(d.rast, 1, ST_StartPoint(c.geom))
                        FROM paths.dem d
                        WHERE ST_ConvexHull(d.rast) && ST_StartPoint(c.geom)
                        LIMIT 1) AS z_start,
                       (SELECT ST_Value(d.rast, 1, ST_EndPoint(c.geom))
                        FROM paths.dem d
                        WHERE ST_ConvexHull(d.rast) && ST_EndPoint(c.geom)
                        LIMIT 1) AS z_end
                FROM cand c
            )
            UPDATE paths.edge e
            SET
                max_slope_deg = CASE
                    WHEN s.len IS NULL OR s.len < 0.5 THEN 0.0
                    WHEN s.z_start IS NULL OR s.z_end IS NULL THEN 0.0
                    ELSE degrees(atan(abs(s.z_end - s.z_start) / GREATEST(s.len, 0.5)))
                END,
                mean_slope_deg = CASE
                    WHEN s.len IS NULL OR s.len < 0.5 THEN 0.0
                    WHEN s.z_start IS NULL OR s.z_end IS NULL THEN 0.0
                    ELSE degrees(atan(abs(s.z_end - s.z_start) / GREATEST(s.len, 0.5)))
                END,
                mean_aspect_deg = CASE
                    WHEN s.azim IS NULL THEN NULL
                    ELSE s.azim + 360.0 - 360.0 * floor((s.azim + 360.0) / 360.0)
                END,
                attr_version = (SELECT version FROM recommend.attr_version)
            FROM sampled s
            WHERE e.id = s.id
            "#,
            chunk = CHUNK_SIZE
        );

        let res = sqlx::query(&chunk_sql).execute(pool).await?;
        let updated = res.rows_affected() as i64;
        total_updated += updated;
        tracing::info!(
            chunk_updated = updated,
            total_updated,
            "edge-attrs chunk committed"
        );
        if updated == 0 {
            break;
        }
    }

    if total_updated > 0 {
        sqlx::query(
            "UPDATE recommend.attr_version \
             SET version = version + 1, notes = 'edge-attrs run', set_at = now() \
             WHERE singleton = true",
        )
        .execute(pool)
        .await?;
    }

    let (total,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM paths.edge \
         WHERE deleted_at IS NULL AND max_slope_deg IS NOT NULL",
    )
    .fetch_one(pool)
    .await?;

    Ok(JobOutcome {
        rows_in: total_updated,
        rows_upserted: total,
    })
}
