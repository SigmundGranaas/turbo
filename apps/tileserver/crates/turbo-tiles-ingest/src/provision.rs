//! End-to-end N50 provisioning: download → restore → all canonical upserts,
//! as one logical job. This is the "no human touches a dump" path — given
//! only an area code it lands a fully-populated basemap.
//!
//! Each sub-step is independently idempotent (the restore skips when staging
//! exists unless `force`; upserts truncate-and-load), so a retry after a
//! mid-chain failure converges. `force` re-downloads + re-restores.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use turbo_tiles_db::DbPool;

use crate::geonorge::{self, Area, Dataset};
use crate::job::{JobError, JobOutcome};
use crate::pgdump_load;
use crate::{incoming_dir, n50};

/// Run the full N50 provisioning chain for an area (county code or
/// `national`). Downloads via the Geonorge Nedlasting API into the incoming
/// dir, restores, then runs every canonical upsert in order.
///
/// Freshness: the restored dump is content-hashed. When the same area is
/// re-provisioned (e.g. a scheduled refresh) and the hash matches the last
/// successful run, the restore + upserts + matview refresh are skipped — so
/// a cadence only does real work when Kartverket actually republished.
/// `force` bypasses the skip and re-restores from scratch.
pub async fn provision_n50(pool: &DbPool, area: &str, force: bool) -> Result<JobOutcome, JobError> {
    let area = Area::parse(area)?;
    let dest = PathBuf::from(incoming_dir());

    // 0. Coverage guard. Every n50 upsert is `DELETE WHERE source='n50'` +
    //    INSERT, so provisioning a county REPLACES whatever was there. Refuse
    //    (unless forced) when that would shrink national coverage down to one
    //    county — the easy way to accidentally wipe a prod dataset. Done
    //    before the expensive download so the failure is instant.
    let prev = existing_provision(pool).await?;
    guard_replacement(
        prev.as_ref().map(|p| (p.area.clone(), p.row_count)),
        &area,
        force,
    )?;

    // 0a. Pre-download freshness gate. A cheap Kartkatalog metadata GET tells
    //     us Geonorge's published data date without ordering/downloading. When
    //     it matches the last run's marker for this area, the whole 5-7 GiB
    //     download + ~20 GiB unzip is a no-op — so skip before touching disk.
    //     A metadata failure is non-fatal: we log and fall through to the
    //     download, which the content-hash skip below still guards.
    let meta_version = match geonorge::fetch_metadata_version(Dataset::N50).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "provision: metadata freshness check failed — proceeding to download");
            None
        }
    };
    if let Some(ref mv) = meta_version {
        if should_skip_metadata(prev.as_ref(), &area, mv, force) {
            let rows = prev.as_ref().map(|p| p.row_count).unwrap_or(0);
            tracing::info!(
                area = %area.0,
                meta_version = %mv,
                rows,
                "provision: source metadata unchanged since last run — skipping download entirely"
            );
            return Ok(JobOutcome {
                rows_in: rows,
                rows_upserted: rows,
            });
        }
    }

    // 1. Download the dump and unzip to the SQL file.
    tracing::info!(area = %area.0, force, "provision: fetching N50 dump");
    let zip = geonorge::fetch(Dataset::N50, &area, &dest).await?;
    let sql = pgdump_load::unzip_dump(&zip).await?;
    let version = hash_file(&sql)?;

    // 1a. Content-hash skip: same area, same content hash, not forced → the
    //     restore + upserts + refresh would be a no-op. Still stamp the fresh
    //     metadata marker so the next run can skip at the cheaper pre-download
    //     gate above (metadata moved but content didn't — a rare Kartverket
    //     re-publish of identical data).
    if should_skip(prev.as_ref(), &area, &version, force) {
        let rows = prev.as_ref().map(|p| p.row_count).unwrap_or(0);
        if meta_version.is_some() {
            record_provision(pool, &area.0, rows, &version, meta_version.as_deref()).await?;
        }
        tracing::info!(
            area = %area.0,
            version = %short(&version),
            rows,
            "provision: source unchanged since last run — skipping restore + upserts"
        );
        return Ok(JobOutcome {
            rows_in: rows,
            rows_upserted: rows,
        });
    }

    // 2. Restore into n50_staging (idempotent unless force). Passing the
    //    already-unzipped .sql avoids a second unzip (unzip_dump passes
    //    .sql through unchanged).
    tracing::info!(sql = %sql.display(), version = %short(&version), "provision: restoring dump");
    n50::restore(pool, sql, force).await?;

    // 3. Canonical upserts, in dependency-free order. Each writes one
    //    terrain.*/paths.*/anchors.* table and is independently re-runnable.
    let mut total: i64 = 0;
    macro_rules! step {
        ($label:literal, $call:expr) => {{
            let out = $call.await?;
            tracing::info!(
                step = $label,
                rows = out.rows_upserted,
                "provision: upsert done"
            );
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

    // Record area + row count + content hash + metadata marker so the next run
    // can guard against a shrinking replacement AND skip when the source is
    // unchanged — at the cheap pre-download gate when the marker is present.
    record_provision(pool, &area.0, total, &version, meta_version.as_deref()).await?;

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

/// The last successful provision's recorded state.
#[derive(Debug, Clone)]
pub struct ProvisionState {
    pub area: String,
    pub row_count: i64,
    /// SHA-256 of the last restored dump — guards the restore (post-download).
    pub source_version: Option<String>,
    /// Geonorge's published data date (`DateUpdated`) at the last run — the
    /// cheap pre-download freshness marker.
    pub source_metadata_version: Option<String>,
}

/// The last successful provision, if any.
async fn existing_provision(pool: &DbPool) -> Result<Option<ProvisionState>, JobError> {
    let row: Option<(String, i64, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT area, row_count, source_version, source_metadata_version \
         FROM paths.provision_state WHERE singleton",
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(
        |(area, row_count, source_version, source_metadata_version)| ProvisionState {
            area,
            row_count,
            source_version,
            source_metadata_version,
        },
    ))
}

/// The currently-provisioned area, if any — used by the boot refresh
/// scheduler to know what to re-check.
pub async fn provisioned_area(pool: &DbPool) -> Result<Option<String>, JobError> {
    Ok(existing_provision(pool).await?.map(|p| p.area))
}

/// Upsert the singleton provision-state row. `meta_version` is the Geonorge
/// `DateUpdated` marker (None when the metadata check was unavailable this run).
async fn record_provision(
    pool: &DbPool,
    area: &str,
    rows: i64,
    version: &str,
    meta_version: Option<&str>,
) -> Result<(), JobError> {
    sqlx::query(
        "INSERT INTO paths.provision_state \
             (singleton, area, row_count, source_version, source_metadata_version, provisioned_at) \
         VALUES (true, $1, $2, $3, $4, now()) \
         ON CONFLICT (singleton) DO UPDATE \
           SET area = EXCLUDED.area, row_count = EXCLUDED.row_count, \
               source_version = EXCLUDED.source_version, \
               source_metadata_version = EXCLUDED.source_metadata_version, \
               provisioned_at = EXCLUDED.provisioned_at",
    )
    .bind(area)
    .bind(rows)
    .bind(version)
    .bind(meta_version)
    .execute(pool)
    .await?;
    Ok(())
}

/// Skip the restore + upserts iff the prior run covered the *same* area with
/// the *same* content hash and we're not forcing. Pure for unit testing.
fn should_skip(
    prev: Option<&ProvisionState>,
    requested: &Area,
    version: &str,
    force: bool,
) -> bool {
    if force {
        return false;
    }
    matches!(
        prev,
        Some(ProvisionState { area, source_version: Some(v), .. })
            if *area == requested.0 && v == version
    )
}

/// Skip the *download itself* iff the prior run covered the same area and
/// Geonorge's published data date (`DateUpdated`) is unchanged, and we're not
/// forcing. This is the cheap pre-download gate — it avoids the 5-7 GiB
/// download + ~20 GiB unzip that the content-hash `should_skip` can only cut
/// short *after* paying that disk cost. Pure for unit testing.
fn should_skip_metadata(
    prev: Option<&ProvisionState>,
    requested: &Area,
    meta_version: &str,
    force: bool,
) -> bool {
    if force {
        return false;
    }
    matches!(
        prev,
        Some(ProvisionState { area, source_metadata_version: Some(v), .. })
            if *area == requested.0 && v == meta_version
    )
}

/// SHA-256 of a file, hex-encoded. Hashing the whole dump means any real
/// data change is detected; an incidental header change (e.g. a dump
/// timestamp) only causes a harmless full re-provision, never a stale skip.
fn hash_file(path: &Path) -> Result<String, JobError> {
    let mut f = std::fs::File::open(path)
        .map_err(|e| JobError::Fetch(format!("open {} for hashing: {e}", path.display())))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut f, &mut hasher)
        .map_err(|e| JobError::Fetch(format!("hash {}: {e}", path.display())))?;
    Ok(hex::encode(hasher.finalize()))
}

/// First 12 hex chars, for logs.
fn short(version: &str) -> &str {
    &version[..version.len().min(12)]
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
        assert!(
            guard_replacement(Some(("0000".into(), 7_000_000)), &area("national"), false).is_ok()
        );
        assert!(guard_replacement(Some(("03".into(), 28_000)), &area("03"), false).is_ok());
    }

    #[test]
    fn county_replacing_national_is_refused_without_force() {
        let err =
            guard_replacement(Some(("0000".into(), 7_000_000)), &area("03"), false).unwrap_err();
        assert!(matches!(err, JobError::WouldReplace(_)), "got {err:?}");
        // ...but force overrides it.
        assert!(guard_replacement(Some(("0000".into(), 7_000_000)), &area("03"), true).is_ok());
    }

    #[test]
    fn county_replacing_county_is_allowed_with_a_warning() {
        // Disjoint counties — a deliberate swap, not a shrink. Allowed.
        assert!(guard_replacement(Some(("34".into(), 750_000)), &area("03"), false).is_ok());
    }

    fn state(area: &str, ver: Option<&str>) -> ProvisionState {
        ProvisionState {
            area: area.into(),
            row_count: 100,
            source_version: ver.map(String::from),
            source_metadata_version: None,
        }
    }

    fn state_meta(area: &str, meta: Option<&str>) -> ProvisionState {
        ProvisionState {
            area: area.into(),
            row_count: 100,
            source_version: Some("hash".into()),
            source_metadata_version: meta.map(String::from),
        }
    }

    #[test]
    fn skips_when_same_area_and_hash_unchanged() {
        let prev = state("03", Some("abc123"));
        assert!(
            should_skip(Some(&prev), &area("03"), "abc123", false),
            "unchanged → skip"
        );
        // Different hash (Kartverket republished) → do the work.
        assert!(!should_skip(Some(&prev), &area("03"), "def456", false));
        // force always re-provisions.
        assert!(!should_skip(Some(&prev), &area("03"), "abc123", true));
    }

    #[test]
    fn never_skips_across_areas_or_fresh_db() {
        let nat = state("0000", Some("abc123"));
        assert!(
            !should_skip(Some(&nat), &area("03"), "abc123", false),
            "area change → never skip"
        );
        assert!(
            !should_skip(None, &area("03"), "abc123", false),
            "fresh db → never skip"
        );
    }

    #[test]
    fn never_skips_when_prior_version_unknown() {
        // Row written before source_version existed → always re-provision once.
        let prev = state("03", None);
        assert!(!should_skip(Some(&prev), &area("03"), "abc123", false));
    }

    #[test]
    fn national_replacing_a_county_is_allowed_an_upgrade() {
        assert!(guard_replacement(Some(("03".into(), 28_000)), &area("national"), false).is_ok());
    }

    #[test]
    fn pre_download_skips_when_metadata_date_unchanged() {
        let prev = state_meta("0000", Some("2026-06-15"));
        assert!(
            should_skip_metadata(Some(&prev), &area("national"), "2026-06-15", false),
            "same area + same DateUpdated → skip download"
        );
        // Kartverket republished (newer date) → download.
        assert!(!should_skip_metadata(
            Some(&prev),
            &area("national"),
            "2026-06-28",
            false
        ));
        // force always downloads.
        assert!(!should_skip_metadata(
            Some(&prev),
            &area("national"),
            "2026-06-15",
            true
        ));
    }

    #[test]
    fn pre_download_never_skips_across_areas_fresh_db_or_unknown_marker() {
        let nat = state_meta("0000", Some("2026-06-15"));
        assert!(
            !should_skip_metadata(Some(&nat), &area("03"), "2026-06-15", false),
            "area change → never skip"
        );
        assert!(
            !should_skip_metadata(None, &area("national"), "2026-06-15", false),
            "fresh db → never skip"
        );
        // Row written before the metadata marker existed → do one full run.
        let no_marker = state_meta("0000", None);
        assert!(!should_skip_metadata(
            Some(&no_marker),
            &area("national"),
            "2026-06-15",
            false
        ));
    }
}
