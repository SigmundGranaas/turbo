//! FKB Traktorveg+Sti+Skogsbilveg WFS ingest.
//!
//! Pulls features from `wms.geonorge.no/skwms1/wms.traktorveg_skogsbilveger`
//! in GML chunks (paginated by bbox at fixed grid cells to stay under
//! the per-request feature limit), parses into staging rows, then runs
//! the generic upsert + topology rebuild.
//!
//! The actual WFS pull lives in M1. For the skeleton commit, this just
//! validates the staging table exists and returns a no-op outcome so
//! the binary's job runner can be exercised end-to-end.

use uuid::Uuid;

use crate::job::{JobError, JobOutcome};

pub async fn run(pool: &turbo_tiles_db::DbPool, _run_id: Uuid) -> Result<JobOutcome, JobError> {
    // Sanity check: the staging table exists. Migration 0002 creates
    // `paths.staging_fkb_sti` so this can't 404 unless the DB is unmigrated.
    sqlx::query("SELECT 1 FROM paths.staging_fkb_sti LIMIT 0")
        .execute(pool)
        .await?;

    tracing::warn!(
        "fkb-sti ingest is a skeleton: real WFS pull lands in the next slice. \
         Returning a no-op outcome so the runner can be tested."
    );

    Ok(JobOutcome {
        rows_in: 0,
        rows_upserted: 0,
    })
}
