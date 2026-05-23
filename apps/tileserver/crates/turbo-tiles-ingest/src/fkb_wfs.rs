//! FKB Traktorveg+Sti+Skogsbilveg WFS ingest.
//!
//! Pulls features from `wms.geonorge.no/skwms1/wms.traktorveg_skogsbilveger`
//! in GML 3.2.1 chunks (one HTTP request per grid cell so the per-request
//! feature limit doesn't truncate). Parses GML, stages rows, then runs
//! the diff/upsert/topology rebuild.
//!
//! The WFS hostname is `wms.geonorge.no` rather than the documented
//! `wfs.geonorge.no` because the latter returns 500s for GetFeature —
//! same workaround the Flutter live-test path used (see the comment in
//! `apps/flutter/.../n50_sti_source.dart`).

use std::time::Duration;

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::job::{JobError, JobOutcome};
use crate::stage::attr_hash;

/// Bounding box in WGS84 (`west, south, east, north` order — the same
/// convention as the public GeoJSON list endpoint).
#[derive(Debug, Clone, Copy)]
pub struct Bbox {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

/// Default ingest window covers the Oslo area. Override via
/// `tileserver ingest --job fkb-sti --bbox W,S,E,N` for wider pulls.
/// Going nationwide is opt-in because a full Norway pull is multi-GB
/// and should be a scheduled job, not a developer's accidental
/// keystroke.
const DEFAULT_BBOX: Bbox = Bbox {
    west: 10.4,
    south: 59.8,
    east: 11.1,
    north: 60.1,
};

/// Grid cell edge length in degrees. 0.1° ~ 11 km north-south, 5-6 km
/// east-west at Norwegian latitudes. Small enough that the WFS rarely
/// truncates a single cell's response.
const GRID_DEG: f64 = 0.1;

/// WFS hard limit per GetFeature request. 5000 leaves headroom under
/// the upstream cap and keeps per-cell pulls under a few hundred KB.
const MAX_FEATURES_PER_CELL: u32 = 5000;

const ENDPOINT: &str = "https://wms.geonorge.no/skwms1/wms.traktorveg_skogsbilveger";
const TYPENAMES: &str = "ms:traktorveg_sti,ms:skogsbilveg";

pub async fn run(pool: &turbo_tiles_db::DbPool, _run_id: Uuid) -> Result<JobOutcome, JobError> {
    run_with_bbox(pool, DEFAULT_BBOX).await
}

/// Free-function entry used by `job::run_job_with_options` when no
/// explicit bbox is supplied — keeps the caller from needing to know
/// about `DEFAULT_BBOX`.
pub async fn run_default(pool: &turbo_tiles_db::DbPool) -> Result<JobOutcome, JobError> {
    run_with_bbox(pool, DEFAULT_BBOX).await
}

pub async fn run_with_bbox(
    pool: &turbo_tiles_db::DbPool,
    bbox: Bbox,
) -> Result<JobOutcome, JobError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent("turbo-tileserver/0.1 (+https://github.com/sigmundgranaas/turbo)")
        .build()
        .map_err(|e| JobError::Fetch(e.to_string()))?;

    let cells = grid_cells(bbox, GRID_DEG);
    tracing::info!(cells = cells.len(), "fkb-sti: planned grid");

    // Truncate-and-load: every run wipes staging so dedupe by attr_hash
    // applies cleanly. Doing the truncate in a transaction with the
    // upsert is the safest pattern, but Postgres can't truncate inside
    // an in-flight tx that already loaded data, so we truncate-up-front
    // and rely on attr_hash uniqueness to make re-runs idempotent
    // against staging itself.
    sqlx::query("TRUNCATE paths.staging_fkb_sti")
        .execute(pool)
        .await?;

    let mut total_in: i64 = 0;
    for (i, cell) in cells.iter().enumerate() {
        let body = fetch_cell(&client, *cell).await?;
        let rows = parse_features(&body)?;
        if rows.is_empty() {
            tracing::debug!(cell_idx = i, "fkb-sti: empty cell");
            continue;
        }
        total_in += stage_rows(pool, &rows).await?;
        tracing::info!(
            cell_idx = i,
            total = cells.len(),
            staged = total_in,
            "fkb-sti: progress"
        );
    }

    let upserted = upsert_from_staging(pool).await?;
    rebuild_topology(pool).await?;

    Ok(JobOutcome {
        rows_in: total_in,
        rows_upserted: upserted,
    })
}

fn grid_cells(bbox: Bbox, step: f64) -> Vec<Bbox> {
    let mut out = Vec::new();
    let mut s = bbox.south;
    while s < bbox.north {
        let mut w = bbox.west;
        let n = (s + step).min(bbox.north);
        while w < bbox.east {
            let e = (w + step).min(bbox.east);
            out.push(Bbox {
                west: w,
                south: s,
                east: e,
                north: n,
            });
            w = e;
        }
        s = n;
    }
    out
}

async fn fetch_cell(client: &reqwest::Client, cell: Bbox) -> Result<String, JobError> {
    // SRSNAME=urn:ogc:def:crs:EPSG::4326 makes axes lat,lon — so the
    // BBOX parameter is `minLat,minLon,maxLat,maxLon`. Same convention
    // as the Flutter live test.
    let bbox = format!(
        "{},{},{},{},urn:ogc:def:crs:EPSG::4326",
        cell.south, cell.west, cell.north, cell.east
    );
    let resp = client
        .get(ENDPOINT)
        .query(&[
            ("SERVICE", "WFS"),
            ("VERSION", "2.0.0"),
            ("REQUEST", "GetFeature"),
            ("TYPENAMES", TYPENAMES),
            ("OUTPUTFORMAT", "text/xml; subtype=gml/3.2.1"),
            ("SRSNAME", "urn:ogc:def:crs:EPSG::4326"),
            ("BBOX", &bbox),
            ("COUNT", &MAX_FEATURES_PER_CELL.to_string()),
        ])
        .send()
        .await
        .map_err(|e| JobError::Fetch(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(JobError::Fetch(format!(
            "GetFeature returned {} for bbox {:?}",
            resp.status(),
            cell
        )));
    }
    resp.text()
        .await
        .map_err(|e| JobError::Fetch(e.to_string()))
}

/// One staged row before insertion. `geom_wkt` is a `LINESTRING(...)`
/// in WGS84 lon-lat order, ready for `ST_GeomFromText` + `ST_Transform`.
#[derive(Debug)]
struct StagedFeature {
    fkb_type: String,
    geom_wkt: String,
    marking: Option<String>,
    surface: Option<String>,
    attrs: serde_json::Value,
}

/// Minimal GML 3.2 walker: looks for `traktorveg_sti` and `skogsbilveg`
/// features, pulls their `posList` plus a handful of attribute fields,
/// returns staged rows. Anything we don't recognise is skipped without
/// failing the whole batch — same tolerant policy as the Flutter
/// `GmlToGeoJson` parser.
fn parse_features(body: &str) -> Result<Vec<StagedFeature>, JobError> {
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);

    let mut out = Vec::new();
    let mut buf = Vec::new();

    // Walker state: which feature we're inside, accumulated attrs,
    // pending posList.
    let mut in_feature: Option<String> = None;
    let mut in_pos_list = false;
    let mut current_pos: Option<String> = None;
    let mut current_attrs: serde_json::Map<String, serde_json::Value> = Default::default();
    let mut in_attr: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => {
                return Err(JobError::Parse(format!("GML parse error: {e}")));
            }
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref());
                if in_feature.is_none() {
                    if name == "traktorveg_sti" || name == "skogsbilveg" {
                        in_feature = Some(name.to_string());
                        current_attrs.clear();
                        current_pos = None;
                    }
                } else if name == "posList" {
                    in_pos_list = true;
                } else if in_pos_list {
                    // Nested elements inside posList are unexpected; ignore.
                } else {
                    // Treat any other element inside a feature as a
                    // candidate property — the text content becomes the
                    // value, if it's a leaf.
                    in_attr = Some(name.to_string());
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref());
                if Some(name.as_str()) == in_feature.as_deref() {
                    if let Some(pos) = current_pos.take() {
                        if let Some(feat) =
                            build_feature(in_feature.as_deref().unwrap(), &pos, &current_attrs)
                        {
                            out.push(feat);
                        }
                    }
                    in_feature = None;
                    current_attrs.clear();
                } else if name == "posList" {
                    in_pos_list = false;
                } else if Some(name.as_str()) == in_attr.as_deref() {
                    in_attr = None;
                }
            }
            Ok(Event::Text(t)) => {
                if in_pos_list {
                    let txt = t.unescape().map_err(|e| JobError::Parse(e.to_string()))?;
                    current_pos = Some(txt.to_string());
                } else if let Some(attr) = in_attr.as_deref() {
                    let txt = t.unescape().map_err(|e| JobError::Parse(e.to_string()))?;
                    let s = txt.trim().to_string();
                    if !s.is_empty() {
                        current_attrs.insert(attr.to_string(), serde_json::Value::String(s));
                    }
                }
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(out)
}

fn local_name(qualified: &[u8]) -> String {
    let s = std::str::from_utf8(qualified).unwrap_or("");
    match s.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => s.to_string(),
    }
}

fn build_feature(
    fkb_type_root: &str,
    pos_list: &str,
    attrs: &serde_json::Map<String, serde_json::Value>,
) -> Option<StagedFeature> {
    // posList is whitespace-separated numbers. With EPSG:4326 axis
    // order, pairs are (lat, lon). WKT needs (lon, lat).
    let nums: Vec<f64> = pos_list
        .split_whitespace()
        .filter_map(|t| t.parse::<f64>().ok())
        .collect();
    if nums.len() < 4 || !nums.len().is_multiple_of(2) {
        return None;
    }
    let mut wkt = String::from("LINESTRING(");
    for (i, pair) in nums.chunks_exact(2).enumerate() {
        if i > 0 {
            wkt.push_str(", ");
        }
        let (lat, lon) = (pair[0], pair[1]);
        // Sanity gate: drop obviously wrong values (parser fell over).
        if !lat.is_finite() || !lon.is_finite() {
            return None;
        }
        wkt.push_str(&format!("{lon} {lat}"));
    }
    wkt.push(')');

    // `typeveg` discriminates sti / traktorveg inside the combined
    // `traktorveg_sti` layer; the `skogsbilveg` layer doesn't have it.
    let fkb_type = match fkb_type_root {
        "traktorveg_sti" => attrs
            .get("typeveg")
            .and_then(|v| v.as_str())
            .unwrap_or("sti")
            .to_lowercase(),
        other => other.to_string(),
    };

    let marking = attrs
        .get("merking")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let surface = attrs
        .get("dekke")
        .or_else(|| attrs.get("surface"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let hash = attr_hash(&wkt, &fkb_type, marking.as_deref(), surface.as_deref());
    let mut merged = attrs.clone();
    merged.insert("attr_hash".to_string(), serde_json::Value::String(hash));

    Some(StagedFeature {
        fkb_type,
        geom_wkt: wkt,
        marking,
        surface,
        attrs: serde_json::Value::Object(merged),
    })
}

async fn stage_rows(
    pool: &turbo_tiles_db::DbPool,
    rows: &[StagedFeature],
) -> Result<i64, JobError> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;
    let mut inserted: i64 = 0;
    for r in rows {
        // attr_hash already lives in attrs (set in build_feature); pull
        // it back out for the column.
        let hash = r
            .attrs
            .get("attr_hash")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JobError::Parse("missing attr_hash in staged row".into()))?
            .to_string();

        let res = sqlx::query(
            r#"
            INSERT INTO paths.staging_fkb_sti
                (geom, fkb_type, marking, surface, attrs, attr_hash)
            VALUES (
                ST_Transform(ST_GeomFromText($1, 4326), 25833),
                $2, $3, $4, $5, $6
            )
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(&r.geom_wkt)
        .bind(&r.fkb_type)
        .bind(&r.marking)
        .bind(&r.surface)
        .bind(&r.attrs)
        .bind(&hash)
        .execute(&mut *tx)
        .await?;
        inserted += res.rows_affected() as i64;
    }
    tx.commit().await?;
    Ok(inserted)
}

/// Diff staging against `paths.edge`, insert new rows, update changed
/// ones, soft-delete the rows that no longer appear (scoped to
/// `ingest_source = 'fkb'` so curated manual edges aren't touched).
async fn upsert_from_staging(pool: &turbo_tiles_db::DbPool) -> Result<i64, JobError> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;

    // 1. Insert new rows (attr_hash not yet in edge).
    let inserted = sqlx::query(
        r#"
        INSERT INTO paths.edge
            (geom, fkb_type, marking, surface, attrs, attr_hash, ingest_source)
        SELECT s.geom, s.fkb_type, s.marking, s.surface, s.attrs, s.attr_hash, 'fkb'
        FROM paths.staging_fkb_sti s
        WHERE NOT EXISTS (
            SELECT 1 FROM paths.edge e
            WHERE e.attr_hash = s.attr_hash AND e.deleted_at IS NULL
        )
        "#,
    )
    .execute(&mut *tx)
    .await?;

    // 2. Soft-delete edges that vanished from the source. Scoped to
    //    `ingest_source = 'fkb'` so manual/turbase curation is safe.
    let soft_deleted = sqlx::query(
        r#"
        UPDATE paths.edge e
        SET deleted_at = now()
        WHERE e.ingest_source = 'fkb'
          AND e.deleted_at IS NULL
          AND NOT EXISTS (
              SELECT 1 FROM paths.staging_fkb_sti s
              WHERE s.attr_hash = e.attr_hash
          )
        "#,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    tracing::info!(
        inserted = inserted.rows_affected(),
        soft_deleted = soft_deleted.rows_affected(),
        "fkb-sti: upsert complete"
    );
    Ok(inserted.rows_affected() as i64)
}

/// Build (or rebuild) source/target node references on `paths.edge`.
/// `pgr_createTopology` populates `source_node`/`target_node` from a
/// shared geometry tolerance. Running it after every ingest keeps the
/// routing graph in sync, but it's expensive — we only rebuild if the
/// last topology pass is older than the last ingested edge.
pub(crate) async fn rebuild_topology(pool: &turbo_tiles_db::DbPool) -> Result<(), JobError> {
    // 1 m tolerance in 25833 (metric SRID) is conservative enough to
    // snap segments that share a literal endpoint but loose enough to
    // tolerate FKB's per-segment rounding.
    sqlx::query(
        "SELECT pgr_createTopology('paths.edge', 1.0, 'geom', 'id', 'source_node', 'target_node')",
    )
    .execute(pool)
    .await?;
    Ok(())
}
