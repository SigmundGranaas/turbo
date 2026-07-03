//! N50 Kartdata ingest. One heavy restore + several cheap per-target
//! upserts that all read from the shared `n50_staging` schema.
//!
//! The operator's workflow:
//!   1. Drop `Basisdata_*_N50Kartdata_PostGIS.zip` on the incoming volume.
//!   2. Trigger `n50-restore` (10–20 min for nationwide). One call,
//!      then n50_staging is populated.
//!   3. Trigger any combination of:
//!      n50-vann-upsert, n50-isogbre-upsert, n50-landcover-upsert,
//!      n50-stedsnavn-upsert, n50-vegnett-upsert.
//!   4. After upserts, trigger `edge-attrs` and `skeleton-build` to
//!      finish wiring the off-trail mesh.
//!
//! Re-running an upsert is fast (seconds) because the heavy restore is
//! amortised across all of them.

use std::path::PathBuf;

use turbo_tiles_db::DbPool;

use crate::job::{JobError, JobOutcome};
use crate::pgdump_load::{self, PgDumpConfig};

const CONFIG: PgDumpConfig = PgDumpConfig {
    source_schema_pattern: "n50kartdata\\_%",
    canonical_schema: "n50_staging",
    sentinel_table: "innsjo",
    label: "n50",
};

pub async fn restore(pool: &DbPool, file: PathBuf, force: bool) -> Result<JobOutcome, JobError> {
    // Pass the raw archive straight through — restore() streams `.zip` via
    // `unzip -p | psql` so the ~30 GiB uncompressed dump is never written.
    let result = pgdump_load::restore(pool, file, CONFIG, force).await?;
    Ok(pgdump_load::outcome_from_result(result))
}

pub async fn upsert_vann(pool: &DbPool) -> Result<JobOutcome, JobError> {
    pgdump_load::require_staging(pool, CONFIG).await?;
    sqlx::raw_sql(include_str!("../sql/upsert_n50_vann.sql"))
        .execute(pool)
        .await?;
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM terrain.water_polygon WHERE source = 'n50'")
            .fetch_one(pool)
            .await?;
    Ok(JobOutcome {
        rows_in: count,
        rows_upserted: count,
    })
}

pub async fn upsert_hoydekurve(pool: &DbPool) -> Result<JobOutcome, JobError> {
    pgdump_load::require_staging(pool, CONFIG).await?;
    sqlx::raw_sql(include_str!("../sql/upsert_n50_hoydekurve.sql"))
        .execute(pool)
        .await?;
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM terrain.contour WHERE source = 'n50'")
            .fetch_one(pool)
            .await?;
    Ok(JobOutcome {
        rows_in: count,
        rows_upserted: count,
    })
}

pub async fn upsert_kystkontur(pool: &DbPool) -> Result<JobOutcome, JobError> {
    pgdump_load::require_staging(pool, CONFIG).await?;
    sqlx::raw_sql(include_str!("../sql/upsert_n50_kystkontur.sql"))
        .execute(pool)
        .await?;
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM terrain.coastline WHERE source = 'n50'")
            .fetch_one(pool)
            .await?;
    Ok(JobOutcome {
        rows_in: count,
        rows_upserted: count,
    })
}

pub async fn upsert_bygning(pool: &DbPool) -> Result<JobOutcome, JobError> {
    pgdump_load::require_staging(pool, CONFIG).await?;
    sqlx::raw_sql(include_str!("../sql/upsert_n50_bygning.sql"))
        .execute(pool)
        .await?;
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM terrain.building_polygon WHERE source = 'n50'",
    )
    .fetch_one(pool)
    .await?;
    Ok(JobOutcome {
        rows_in: count,
        rows_upserted: count,
    })
}

pub async fn upsert_isogbre(pool: &DbPool) -> Result<JobOutcome, JobError> {
    pgdump_load::require_staging(pool, CONFIG).await?;
    sqlx::raw_sql(include_str!("../sql/upsert_n50_isogbre.sql"))
        .execute(pool)
        .await?;
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM terrain.glacier_polygon WHERE source = 'n50'")
            .fetch_one(pool)
            .await?;
    Ok(JobOutcome {
        rows_in: count,
        rows_upserted: count,
    })
}

pub async fn upsert_landcover(pool: &DbPool) -> Result<JobOutcome, JobError> {
    pgdump_load::require_staging(pool, CONFIG).await?;
    sqlx::raw_sql(include_str!("../sql/upsert_n50_landcover.sql"))
        .execute(pool)
        .await?;
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM terrain.landcover_patch WHERE source = 'n50'")
            .fetch_one(pool)
            .await?;
    // Landcover changed → bump attr_version so cached CandidateIds invalidate.
    if count > 0 {
        sqlx::query(
            "UPDATE recommend.attr_version \
             SET version = version + 1, notes = 'n50-landcover-upsert', set_at = now() \
             WHERE singleton = true",
        )
        .execute(pool)
        .await?;
    }
    Ok(JobOutcome {
        rows_in: count,
        rows_upserted: count,
    })
}

pub async fn upsert_stedsnavn(pool: &DbPool) -> Result<JobOutcome, JobError> {
    pgdump_load::require_staging(pool, CONFIG).await?;
    sqlx::raw_sql(include_str!("../sql/upsert_n50_stedsnavn.sql"))
        .execute(pool)
        .await?;
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM anchors.anchor \
         WHERE 'n50' = ANY(sources)",
    )
    .fetch_one(pool)
    .await?;
    Ok(JobOutcome {
        rows_in: count,
        rows_upserted: count,
    })
}

pub async fn upsert_vegnett(pool: &DbPool) -> Result<JobOutcome, JobError> {
    pgdump_load::require_staging(pool, CONFIG).await?;
    sqlx::raw_sql(include_str!("../sql/upsert_n50_vegnett.sql"))
        .execute(pool)
        .await?;
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM paths.edge \
         WHERE deleted_at IS NULL AND attrs->>'source' = 'n50_vegnett'",
    )
    .fetch_one(pool)
    .await?;
    Ok(JobOutcome {
        rows_in: count,
        rows_upserted: count,
    })
}

#[cfg(test)]
mod tests {

    #[test]
    fn upsert_sql_files_all_baked() {
        // Compile-time include_str! check — every SQL file is on disk
        // and contains the canonical 'n50' source label so the
        // DELETE-by-source idempotency works.
        for sql in [
            include_str!("../sql/upsert_n50_vann.sql"),
            include_str!("../sql/upsert_n50_isogbre.sql"),
            include_str!("../sql/upsert_n50_landcover.sql"),
            include_str!("../sql/upsert_n50_stedsnavn.sql"),
            include_str!("../sql/upsert_n50_vegnett.sql"),
        ] {
            assert!(
                sql.contains("n50_staging"),
                "upsert references canonical n50_staging schema"
            );
        }
    }

    #[test]
    fn landcover_covers_all_n50_source_tables() {
        // skog/myr/apentomrade/dyrketmark — if any of these vanish
        // from the SQL, landcover coverage gets worse silently.
        let sql = include_str!("../sql/upsert_n50_landcover.sql");
        for t in ["skog", "myr", "apentomrade", "dyrketmark"] {
            assert!(
                sql.contains(&format!("n50_staging.{t}")),
                "landcover upsert missing {t}"
            );
        }
        for c in ["'forest'", "'wetland'", "'open'"] {
            assert!(sql.contains(c), "landcover class {c} missing");
        }
    }
}
