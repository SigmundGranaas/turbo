//! Anchors ingest from N50 Stedsnavn (named places + summits).
//!
//! Source: Geonorge WFS for N50 — published as ATOM/WFS endpoints.
//! The feature types we read are:
//!   - `app:Hoydepunkt`   (height-tagged points) → AnchorKind::Summit
//!   - `app:Stedsnavn`    (named places)         → AnchorKind::NamedPlace
//!
//! When the WFS isn't reachable (CI, offline dev), we fall back to a
//! baked JSON fixture so the ingest pipeline can be exercised end-to-end.
//! The fixture is intentionally tiny — it's not a replacement dataset.
//!
//! Snap-to-graph happens inside this job: every inserted anchor gets
//! `snapped_node_id`/`snap_distance_m` filled from the nearest
//! `paths.node` row. This way `Anchors::nearest_to` and
//! `to_target` queries don't pay a runtime snap cost (Rule R3).

use std::path::PathBuf;

use serde::Deserialize;
use turbo_tiles_db::DbPool;

use crate::job::{JobError, JobOutcome};

/// Minimal anchor record used by both the WFS parser and the fixture.
/// Coordinates are in EPSG:25833 (UTM33N) — what N50 publishes natively.
#[derive(Debug, Clone, Deserialize)]
pub struct AnchorIn {
    pub kind: String,
    pub name: String,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub elevation_m: Option<f64>,
    #[serde(default)]
    pub source_ref: Option<String>,
}

/// Baked dev fixture — well-known anchors in Oslomarka. Used when
/// `TURBO_N50_FIXTURE=1` or when the WFS fetch fails. Coordinates are
/// approximate but real (Sognsvann grid centroid).
const FIXTURE: &str = include_str!("../data/n50_anchors_fixture.json");

pub async fn run(pool: &DbPool, opts_force_fixture: bool) -> Result<JobOutcome, JobError> {
    let anchors: Vec<AnchorIn> = if opts_force_fixture
        || std::env::var("TURBO_N50_FIXTURE").ok().as_deref() == Some("1")
    {
        parse_fixture()?
    } else {
        // WFS fetch — when available. Fall back to fixture on any
        // network/parse failure so dev environments aren't blocked.
        match fetch_wfs().await {
            Ok(v) if !v.is_empty() => v,
            Ok(_) => {
                tracing::warn!("N50 WFS returned no anchors — falling back to fixture");
                parse_fixture()?
            }
            Err(e) => {
                tracing::warn!(error = %e, "N50 WFS fetch failed — falling back to fixture");
                parse_fixture()?
            }
        }
    };

    upsert(pool, anchors).await
}

/// Same logic but reads anchors from a caller-supplied JSON file
/// (matches `AnchorIn[]` schema). Used when an operator wants to
/// load curated data from disk without going through the WFS or the
/// baked fixture.
pub async fn run_from_file(pool: &DbPool, file: PathBuf) -> Result<JobOutcome, JobError> {
    let bytes = std::fs::read(&file).map_err(|e| JobError::Fetch(e.to_string()))?;
    let anchors: Vec<AnchorIn> =
        serde_json::from_slice(&bytes).map_err(|e| JobError::Parse(e.to_string()))?;
    upsert(pool, anchors).await
}

fn parse_fixture() -> Result<Vec<AnchorIn>, JobError> {
    serde_json::from_str::<Vec<AnchorIn>>(FIXTURE).map_err(|e| JobError::Parse(e.to_string()))
}

/// WFS fetch stub: returns Err for now. A real impl will live in a
/// follow-up slice — until then the job uses the fixture so the rest
/// of the pipeline can be tested. Stable surface: signature won't
/// change when the WFS code lands.
async fn fetch_wfs() -> Result<Vec<AnchorIn>, JobError> {
    Err(JobError::Fetch("N50 WFS not yet wired".into()))
}

async fn upsert(pool: &DbPool, anchors: Vec<AnchorIn>) -> Result<JobOutcome, JobError> {
    let mut inserted = 0i64;
    let n = anchors.len();
    for a in anchors.into_iter() {
        let source_ref = a
            .source_ref
            .clone()
            .unwrap_or_else(|| format!("n50-{}-{}-{}", a.kind, a.x as i64, a.y as i64));
        let res = sqlx::query(
            r#"
            INSERT INTO anchors.anchor
                (kind, geom, name, elevation_m, sources, source_ref, attrs)
            VALUES
                ($1, ST_SetSRID(ST_Point($2, $3), 25833), $4, $5,
                 ARRAY['n50']::text[], $6, '{}'::jsonb)
            ON CONFLICT (source_ref) DO UPDATE
                SET geom = EXCLUDED.geom,
                    name = EXCLUDED.name,
                    elevation_m = EXCLUDED.elevation_m,
                    sources = ARRAY(SELECT DISTINCT unnest(
                        anchors.anchor.sources || EXCLUDED.sources
                    ))
            "#,
        )
        .bind(&a.kind)
        .bind(a.x)
        .bind(a.y)
        .bind(&a.name)
        .bind(a.elevation_m)
        .bind(&source_ref)
        .execute(pool)
        .await?;
        inserted += res.rows_affected() as i64;
    }

    // Snap newly inserted/updated rows to the nearest paths.node.
    // Bounded to 2000 m so anchors off the network don't grab arbitrary
    // distant nodes. Anchors that fall outside the bound get NULL —
    // composition queries handle that.
    sqlx::query(
        r#"
        UPDATE anchors.anchor a
        SET snapped_node_id = nn.node_id, snap_distance_m = nn.dist
        FROM (
            SELECT a2.id AS aid, n.id AS node_id,
                   ST_Distance(n.geom, a2.geom) AS dist,
                   ROW_NUMBER() OVER (
                       PARTITION BY a2.id ORDER BY ST_Distance(n.geom, a2.geom)
                   ) AS rn
            FROM anchors.anchor a2
            JOIN paths.node n ON ST_DWithin(n.geom, a2.geom, 2000.0)
            WHERE a2.snapped_node_id IS NULL OR a2.sources @> ARRAY['n50']::text[]
        ) nn
        WHERE nn.aid = a.id AND nn.rn = 1
        "#,
    )
    .execute(pool)
    .await?;

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*)::bigint FROM anchors.anchor")
        .fetch_one(pool)
        .await?;
    Ok(JobOutcome {
        rows_in: n as i64,
        rows_upserted: inserted.max(count),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_parses_and_is_non_empty() {
        // The fixture is baked at compile time — verifies the file
        // exists, parses, and provides a non-trivial number of
        // anchors so the dev pipeline produces something usable.
        let v = parse_fixture().expect("fixture parses");
        assert!(v.len() >= 5, "expected ≥5 anchors, got {}", v.len());
        // At least one summit so to_target queries can target a peak.
        assert!(v.iter().any(|a| a.kind == "summit"));
    }

    #[test]
    fn fixture_anchors_are_in_norway_utm33() {
        // Sanity-check: every anchor has coordinates that look like
        // EPSG:25833 metres, not lon/lat. Mainland Norway is roughly
        // x ∈ [0, 900_000], y ∈ [6_400_000, 7_900_000].
        for a in parse_fixture().unwrap() {
            assert!(
                (-50_000.0..=1_000_000.0).contains(&a.x),
                "{}: x={} looks like lon/lat",
                a.name,
                a.x
            );
            assert!(
                (6_000_000.0..=8_000_000.0).contains(&a.y),
                "{}: y={} looks like lon/lat",
                a.name,
                a.y
            );
        }
    }
}
