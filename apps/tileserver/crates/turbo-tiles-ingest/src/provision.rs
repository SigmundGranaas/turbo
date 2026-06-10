//! End-to-end N50 provisioning: download → restore → all canonical upserts,
//! as one logical job. This is the "no human touches a dump" path — given
//! only an area code it lands a fully-populated basemap.
//!
//! Each sub-step is independently idempotent (the restore skips when staging
//! exists unless `force`; upserts truncate-and-load), so a retry after a
//! mid-chain failure converges. `force` re-downloads + re-restores.

use std::path::PathBuf;

use turbo_tiles_db::DbPool;

use crate::geonorge::{self, Area, Dataset};
use crate::job::{JobError, JobOutcome};
use crate::{incoming_dir, n50};

/// Run the full N50 provisioning chain for an area (county code or
/// `national`). Downloads via the Geonorge Nedlasting API into the incoming
/// dir, restores, then runs every canonical upsert in order.
pub async fn provision_n50(pool: &DbPool, area: &str, force: bool) -> Result<JobOutcome, JobError> {
    let area = Area::parse(area)?;
    let dest = PathBuf::from(incoming_dir());

    // 0. Coverage guard. Every n50 upsert is `DELETE WHERE source='n50'` +
    //    INSERT, so provisioning a county REPLACES whatever was there. Refuse
    //    (unless forced) when that would shrink national coverage down to one
    //    county — the easy way to accidentally wipe a prod dataset. Done
    //    before the expensive download so the failure is instant.
    guard_replacement(existing_provision(pool).await?, &area, force)?;

    // 1. Download (or reuse) the dump. fetch() always re-orders today; a
    //    future source_version check makes a same-version fetch a no-op.
    tracing::info!(area = %area.0, force, "provision: fetching N50 dump");
    let zip = geonorge::fetch(Dataset::N50, &area, &dest).await?;

    // 2. Restore into n50_staging (idempotent unless force).
    tracing::info!(zip = %zip.display(), "provision: restoring dump");
    n50::restore(pool, zip, force).await?;

    // 3. Canonical upserts, in dependency-free order. Each writes one
    //    terrain.*/paths.*/anchors.* table and is independently re-runnable.
    let mut total: i64 = 0;
    macro_rules! step {
        ($label:literal, $call:expr) => {{
            let out = $call.await?;
            tracing::info!(step = $label, rows = out.rows_upserted, "provision: upsert done");
            total += out.rows_upserted;
        }};
    }
    step!("vann", n50::upsert_vann(pool));
    step!("hoydekurve", n50::upsert_hoydekurve(pool));
    step!("bygning", n50::upsert_bygning(pool));
    step!("kystkontur", n50::upsert_kystkontur(pool));
    step!("isogbre", n50::upsert_isogbre(pool));
    step!("landcover", n50::upsert_landcover(pool));
    step!("stedsnavn", n50::upsert_stedsnavn(pool));
    step!("vegnett", n50::upsert_vegnett(pool));

    // 4. Rebuild the low-zoom overview matviews from the fresh base tables.
    refresh_overviews(pool).await?;

    // Record what we just provisioned so the next run can guard against a
    // shrinking replacement.
    record_provision(pool, &area.0, total).await?;

    tracing::info!(area = %area.0, total_rows = total, "provision: N50 complete");
    Ok(JobOutcome {
        rows_in: total,
        rows_upserted: total,
    })
}

/// The basemap low-zoom overview matviews, kept in lockstep with
/// `migrations/20260603000006_basemap_overviews.sql` (and the
/// `overview_table` entries in `tools/basemap-layers.toml`).
const OVERVIEW_VIEWS: &[&str] = &[
    "basemap.water_overview",
    "basemap.landcover_overview",
    "basemap.coastline_overview",
    "basemap.transportation_overview",
    "basemap.contour_overview",
];

/// Rebuild every basemap overview matview from its base table. Cheap relative
/// to a full provision; run at the end of provisioning and exposed as the
/// `refresh-basemap-overviews` job for ad-hoc refreshes.
pub async fn refresh_overviews(pool: &DbPool) -> Result<JobOutcome, JobError> {
    for v in OVERVIEW_VIEWS {
        tracing::info!(view = v, "refresh: basemap overview matview");
        sqlx::query(&format!("REFRESH MATERIALIZED VIEW {v}"))
            .execute(pool)
            .await?;
    }
    Ok(JobOutcome {
        rows_in: OVERVIEW_VIEWS.len() as i64,
        rows_upserted: OVERVIEW_VIEWS.len() as i64,
    })
}

/// Decide whether a provision is allowed, given the previously-provisioned
/// `(area, row_count)`. Pure so it's unit-testable without a DB or network.
///
/// Refuses (unless `force`) only the dangerous case: replacing NATIONAL
/// coverage with a single county. Any other area change is allowed but
/// warned, since counties are disjoint and replacing one with another is a
/// deliberate operator choice.
fn guard_replacement(
    prev: Option<(String, i64)>,
    requested: &Area,
    force: bool,
) -> Result<(), JobError> {
    let Some((prev_area, prev_rows)) = prev else {
        return Ok(());
    };
    if prev_area == requested.0 {
        return Ok(());
    }
    if prev_area == "0000" && !requested.is_national() && !force {
        return Err(JobError::WouldReplace(format!(
            "provisioning area `{}` would replace existing NATIONAL coverage \
             ({prev_rows} rows) — every N50 upsert deletes by source first. \
             Pass force=true to override, or use area=national to refresh.",
            requested.0
        )));
    }
    tracing::warn!(
        prev_area = %prev_area,
        prev_rows,
        new_area = %requested.0,
        "provision: replacing existing coverage with a different area"
    );
    Ok(())
}

/// The area + row count of the last successful provision, if any.
async fn existing_provision(pool: &DbPool) -> Result<Option<(String, i64)>, JobError> {
    let row: Option<(String, i64)> =
        sqlx::query_as("SELECT area, row_count FROM paths.provision_state WHERE singleton")
            .fetch_optional(pool)
            .await?;
    Ok(row)
}

/// Upsert the singleton provision-state row.
async fn record_provision(pool: &DbPool, area: &str, rows: i64) -> Result<(), JobError> {
    sqlx::query(
        "INSERT INTO paths.provision_state (singleton, area, row_count, provisioned_at) \
         VALUES (true, $1, $2, now()) \
         ON CONFLICT (singleton) DO UPDATE \
           SET area = EXCLUDED.area, row_count = EXCLUDED.row_count, \
               provisioned_at = EXCLUDED.provisioned_at",
    )
    .bind(area)
    .bind(rows)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(s: &str) -> Area {
        Area::parse(s).unwrap()
    }

    #[test]
    fn fresh_db_is_always_allowed() {
        assert!(guard_replacement(None, &area("03"), false).is_ok());
        assert!(guard_replacement(None, &area("national"), false).is_ok());
    }

    #[test]
    fn same_area_refresh_is_allowed() {
        assert!(guard_replacement(Some(("0000".into(), 7_000_000)), &area("national"), false).is_ok());
        assert!(guard_replacement(Some(("03".into(), 28_000)), &area("03"), false).is_ok());
    }

    #[test]
    fn county_replacing_national_is_refused_without_force() {
        let err = guard_replacement(Some(("0000".into(), 7_000_000)), &area("03"), false).unwrap_err();
        assert!(matches!(err, JobError::WouldReplace(_)), "got {err:?}");
        // ...but force overrides it.
        assert!(guard_replacement(Some(("0000".into(), 7_000_000)), &area("03"), true).is_ok());
    }

    #[test]
    fn county_replacing_county_is_allowed_with_a_warning() {
        // Disjoint counties — a deliberate swap, not a shrink. Allowed.
        assert!(guard_replacement(Some(("34".into(), 750_000)), &area("03"), false).is_ok());
    }

    #[test]
    fn national_replacing_a_county_is_allowed_an_upgrade() {
        assert!(guard_replacement(Some(("03".into(), 28_000)), &area("national"), false).is_ok());
    }
}
