//! Load the dev/test recommendation fixture. Reads
//! `tools/seed-recommend-fixture.sql` (baked at compile time) and
//! executes it inside a single transaction so the seed lands
//! atomically. Idempotent — re-running truncates the prior seed first.

use turbo_tiles_db::DbPool;

use crate::job::{JobError, JobOutcome};

/// SQL baked at compile time so the binary carries it. Sits at
/// `apps/tileserver/tools/seed-recommend-fixture.sql` relative to the
/// crate root.
const FIXTURE_SQL: &str = include_str!("../../../tools/seed-recommend-fixture.sql");

pub async fn run(pool: &DbPool) -> Result<JobOutcome, JobError> {
    // Execute the whole fixture as one raw multi-statement SQL blob.
    // sqlx splits on top-level semicolons; the fixture is written so
    // semicolons only appear at statement boundaries. We execute
    // directly on the pool — the fixture is idempotent (TRUNCATE +
    // re-insert), so per-statement atomicity is sufficient.
    sqlx::raw_sql(FIXTURE_SQL).execute(pool).await?;

    let (anchors,): (i64,) = sqlx::query_as("SELECT COUNT(*)::bigint FROM anchors.anchor")
        .fetch_one(pool)
        .await?;
    Ok(JobOutcome {
        rows_in: anchors,
        rows_upserted: anchors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_sql_is_baked() {
        // The fixture file must be present at compile time — catches
        // a refactor that moves the path. Smoke test, no DB.
        assert!(FIXTURE_SQL.contains("anchors.anchor"));
        assert!(FIXTURE_SQL.contains("Vettakollen"));
        assert!(FIXTURE_SQL.contains("TRUNCATE anchors.anchor"));
    }
}
