//! Attach DTM10 elevation to `paths.edge` rows.
//!
//! Per-edge: dump vertices via `ST_DumpPoints` on the 25833 geometry,
//! sample each through Kartverket's Hoydedata point service, then
//! sum positive deltas as `elevation_gain_m` and negative deltas as
//! `elevation_loss_m`. Writes are idempotent — only rows with NULL
//! `elevation_gain_m` are touched, so re-runs after a partial failure
//! resume.
//!
//! V1 uses the live Hoydedata WCS endpoint (one HTTP call per point,
//! ~50 ms each). For a national-scale backfill swap in the bulk
//! GeoTIFFs + a local raster sampler (postgis_raster or gdal-rs).
//! The trait below is the seam.

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use sqlx::Row;

use crate::job::{JobError, JobOutcome};

/// Geographic point used for elevation lookup. EPSG:4258 is what the
/// Kartverket Hoydedata service speaks — ETRS89 lat/lon, numerically
/// indistinguishable from WGS84 for our purposes.
#[derive(Debug, Clone, Copy)]
pub struct Point4258 {
    pub lat: f64,
    pub lon: f64,
}

#[async_trait]
pub trait ElevationProvider: Send + Sync {
    /// Return elevation in metres for each input point. Errors out
    /// hard on transport failure — the job-level retry/skip policy
    /// decides whether to fall back.
    async fn sample(&self, points: &[Point4258]) -> Result<Vec<Option<f64>>, JobError>;
}

/// Live Kartverket Hoydedata point lookup. Free, rate-limited but
/// generous; one HTTP call per point at ~50 ms. Used for the v1 demo
/// path; swap with a local GeoTIFF sampler for nationwide backfill.
pub struct HoydedataProvider {
    client: reqwest::Client,
}

impl HoydedataProvider {
    pub fn new() -> Result<Self, JobError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("turbo-tileserver/0.1 (+https://github.com/sigmundgranaas/turbo)")
            .build()
            .map_err(|e| JobError::Fetch(e.to_string()))?;
        Ok(Self { client })
    }
}

impl Default for HoydedataProvider {
    fn default() -> Self {
        Self::new().expect("reqwest client construction is infallible in practice")
    }
}

#[derive(Debug, Deserialize)]
struct HoydedataResponse {
    punkter: Vec<HoydedataPoint>,
}

#[derive(Debug, Deserialize)]
struct HoydedataPoint {
    z: Option<f64>,
}

#[async_trait]
impl ElevationProvider for HoydedataProvider {
    async fn sample(&self, points: &[Point4258]) -> Result<Vec<Option<f64>>, JobError> {
        let mut out = Vec::with_capacity(points.len());
        for p in points {
            let resp = self
                .client
                .get("https://ws.geonorge.no/hoydedata/v1/punkt")
                .query(&[
                    ("koordsys", "4258"),
                    ("nord", &p.lat.to_string()),
                    ("ost", &p.lon.to_string()),
                    ("geojson", "false"),
                ])
                .send()
                .await
                .map_err(|e| JobError::Fetch(e.to_string()))?;
            if !resp.status().is_success() {
                out.push(None);
                continue;
            }
            let body: HoydedataResponse = resp
                .json()
                .await
                .map_err(|e| JobError::Parse(e.to_string()))?;
            out.push(body.punkter.into_iter().next().and_then(|p| p.z));
        }
        Ok(out)
    }
}

/// Run the attach job over every edge missing elevation.
///
/// Two-tier sampling:
///   1. If `paths.dem` covers the edge (a DTM GeoTIFF was loaded via
///      `dtm-load`), use `ST_Value(rast, vertex)` per vertex — entirely
///      in-database, fast, no network.
///   2. Otherwise fall back to the live Kartverket Hoydedata point
///      service. Slow (one HTTP call per vertex) but works without
///      pre-loaded rasters and is the path the demo uses by default.
///
/// Capped at `max_edges` per run so a single invocation doesn't spend
/// hours hammering the upstream service.
pub async fn run(pool: &turbo_tiles_db::DbPool) -> Result<JobOutcome, JobError> {
    run_with(pool, &HoydedataProvider::default(), 500).await
}

pub async fn run_with(
    pool: &turbo_tiles_db::DbPool,
    provider: &dyn ElevationProvider,
    max_edges: i64,
) -> Result<JobOutcome, JobError> {
    // Pull each candidate edge twice: the 25833 geometry for the
    // raster sampler and the 4258 lon/lat for the Hoydedata fallback.
    let edges = sqlx::query(
        r#"
        SELECT id,
               ST_AsGeoJSON(ST_Transform(geom, 4258))::jsonb AS geom_4258,
               EXISTS (
                   SELECT 1 FROM paths.dem d
                   WHERE ST_Intersects(ST_ConvexHull(d.rast), e.geom)
               ) AS dem_covers
        FROM paths.edge e
        WHERE deleted_at IS NULL
          AND elevation_gain_m IS NULL
        ORDER BY id
        LIMIT $1
        "#,
    )
    .bind(max_edges)
    .fetch_all(pool)
    .await?;

    let mut updated: i64 = 0;
    let mut from_raster: i64 = 0;
    for row in edges {
        let id: i64 = row.try_get("id")?;
        let dem_covers: bool = row.try_get("dem_covers")?;

        let elevations = if dem_covers {
            // Sample directly from the local DEM. Returns one row per
            // vertex in path order so the gain/loss windowing below
            // works the same way as the HTTPS fallback.
            let samples: Vec<(Option<f64>,)> = sqlx::query_as(
                r#"
                WITH pts AS (
                    SELECT (dp).path[1] AS ord, (dp).geom AS g
                    FROM (
                        SELECT ST_DumpPoints(geom) AS dp
                        FROM paths.edge WHERE id = $1
                    ) sub
                )
                SELECT ST_Value(d.rast, pts.g)
                FROM pts
                LEFT JOIN paths.dem d
                       ON ST_Intersects(d.rast, pts.g)
                ORDER BY pts.ord
                "#,
            )
            .bind(id)
            .fetch_all(pool)
            .await?;
            from_raster += 1;
            samples.into_iter().map(|(v,)| v).collect()
        } else {
            let geom: serde_json::Value = row.try_get("geom_4258")?;
            let coords = match geom.get("coordinates").and_then(|c| c.as_array()) {
                Some(arr) => arr.clone(),
                None => continue,
            };
            let points: Vec<Point4258> = coords
                .iter()
                .filter_map(|p| {
                    let arr = p.as_array()?;
                    Some(Point4258 {
                        lon: arr.first()?.as_f64()?,
                        lat: arr.get(1)?.as_f64()?,
                    })
                })
                .collect();
            if points.len() < 2 {
                continue;
            }
            match provider.sample(&points).await {
                Ok(es) => es,
                Err(e) => {
                    tracing::warn!(error = %e, edge_id = id, "elevation sample failed; skipping");
                    continue;
                }
            }
        };

        let (gain, loss) = gain_loss(&elevations);
        sqlx::query(
            r#"
            UPDATE paths.edge
            SET elevation_gain_m = $2,
                elevation_loss_m = $3
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(gain)
        .bind(loss)
        .execute(pool)
        .await?;
        updated += 1;
    }

    tracing::info!(
        updated,
        from_raster,
        from_hoydedata = updated - from_raster,
        "dtm10-attach complete"
    );
    Ok(JobOutcome {
        rows_in: updated,
        rows_upserted: updated,
    })
}

/// Sum signed deltas between consecutive elevations. None values
/// break the run (a missing sample means we can't compute either
/// half of the adjacent delta).
fn gain_loss(elevations: &[Option<f64>]) -> (f64, f64) {
    let mut gain = 0.0;
    let mut loss = 0.0;
    for w in elevations.windows(2) {
        if let (Some(a), Some(b)) = (w[0], w[1]) {
            let d = b - a;
            if d > 0.0 {
                gain += d;
            } else {
                loss += -d;
            }
        }
    }
    (gain, loss)
}
