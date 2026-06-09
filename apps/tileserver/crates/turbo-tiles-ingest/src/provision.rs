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
    step!("isogbre", n50::upsert_isogbre(pool));
    step!("landcover", n50::upsert_landcover(pool));
    step!("stedsnavn", n50::upsert_stedsnavn(pool));
    step!("vegnett", n50::upsert_vegnett(pool));

    tracing::info!(area = %area.0, total_rows = total, "provision: N50 complete");
    Ok(JobOutcome {
        rows_in: total,
        rows_upserted: total,
    })
}
