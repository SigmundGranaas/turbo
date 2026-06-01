//! DNT cabins ingest.
//!
//! Source: DNT's open-data API (Nasjonal Turbase). Returns JSON
//! features for cabins (DNT-betjente, selvbetjente, ubetjente).
//! Each cabin becomes an `anchors.anchor` row with `kind = 'cabin'`.
//!
//! Two operating modes:
//!   * Network: fetch from a configurable URL (default the Nasjonal
//!     Turbase legacy endpoint). Best for production refresh.
//!   * File:    parse a JSON file supplied via `--file`. Used when
//!     the API is unavailable or for testing.
//!
//! The JSON shape we accept is intentionally tolerant — both the
//! "GeoJSON FeatureCollection" form and the older bare-array form
//! parse correctly.

use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;
use turbo_tiles_db::DbPool;

use crate::job::{JobError, JobOutcome};

/// One cabin record as we want it in memory. Tolerant — fields that
/// don't show up just default to None.
#[derive(Debug, Clone)]
pub struct CabinIn {
    pub name: String,
    pub lon: f64,
    pub lat: f64,
    pub elevation_m: Option<f64>,
    pub category: Option<String>,
    pub source_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CabinJson {
    FeatureCollection(GeoJsonFC),
    Array(Vec<DntCabinRecord>),
}

#[derive(Debug, Deserialize)]
struct GeoJsonFC {
    features: Vec<GeoJsonFeature>,
}

#[derive(Debug, Deserialize)]
struct GeoJsonFeature {
    geometry: Option<GeoJsonPoint>,
    properties: GeoJsonProps,
}

#[derive(Debug, Deserialize)]
struct GeoJsonPoint {
    coordinates: Vec<f64>,
}

#[derive(Debug, Deserialize)]
struct GeoJsonProps {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    navn: Option<String>,
    #[serde(default)]
    elevation: Option<f64>,
    #[serde(default)]
    hoyde: Option<f64>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    type_: Option<String>,
    #[serde(default)]
    id: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct DntCabinRecord {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    navn: Option<String>,
    #[serde(default)]
    lon: Option<f64>,
    #[serde(default)]
    lat: Option<f64>,
    #[serde(default)]
    longitude: Option<f64>,
    #[serde(default)]
    latitude: Option<f64>,
    #[serde(default)]
    elevation: Option<f64>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    id: Option<serde_json::Value>,
}

pub fn parse_json(bytes: &[u8]) -> Result<Vec<CabinIn>, JobError> {
    let v: CabinJson = serde_json::from_slice(bytes).map_err(|e| JobError::Parse(e.to_string()))?;
    let mut out = Vec::new();
    match v {
        CabinJson::FeatureCollection(fc) => {
            for f in fc.features {
                let Some(g) = f.geometry else { continue };
                if g.coordinates.len() < 2 {
                    continue;
                }
                let name = f.properties.name.or(f.properties.navn).unwrap_or_default();
                if name.is_empty() {
                    continue;
                }
                let source_id = match f.properties.id {
                    Some(serde_json::Value::String(s)) => s,
                    Some(serde_json::Value::Number(n)) => n.to_string(),
                    _ => format!("{}-{}", g.coordinates[0], g.coordinates[1]),
                };
                out.push(CabinIn {
                    name,
                    lon: g.coordinates[0],
                    lat: g.coordinates[1],
                    elevation_m: f.properties.elevation.or(f.properties.hoyde),
                    category: f.properties.category.or(f.properties.type_),
                    source_id,
                });
            }
        }
        CabinJson::Array(rows) => {
            for r in rows {
                let lon = r.lon.or(r.longitude);
                let lat = r.lat.or(r.latitude);
                let name = r.name.or(r.navn).unwrap_or_default();
                if name.is_empty() {
                    continue;
                }
                let (Some(lon), Some(lat)) = (lon, lat) else {
                    continue;
                };
                let source_id = match r.id {
                    Some(serde_json::Value::String(s)) => s,
                    Some(serde_json::Value::Number(n)) => n.to_string(),
                    _ => format!("{lon}-{lat}"),
                };
                out.push(CabinIn {
                    name,
                    lon,
                    lat,
                    elevation_m: r.elevation,
                    category: r.category,
                    source_id,
                });
            }
        }
    }
    Ok(out)
}

pub async fn run(
    pool: &DbPool,
    file: Option<PathBuf>,
    url: Option<String>,
) -> Result<JobOutcome, JobError> {
    let bytes = if let Some(p) = file {
        std::fs::read(&p).map_err(|e| JobError::Fetch(e.to_string()))?
    } else {
        let url = url.unwrap_or_else(|| "https://nasjonalturbase.no/api/legacy/cabins".to_string());
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent("turbo-tileserver/0.1 (+https://github.com/sigmundgranaas/turbo)")
            .build()
            .map_err(|e| JobError::Fetch(e.to_string()))?;
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| JobError::Fetch(e.to_string()))?
            .error_for_status()
            .map_err(|e| JobError::Fetch(e.to_string()))?;
        resp.bytes()
            .await
            .map_err(|e| JobError::Fetch(e.to_string()))?
            .to_vec()
    };

    let cabins = parse_json(&bytes)?;
    upsert(pool, cabins).await
}

async fn upsert(pool: &DbPool, cabins: Vec<CabinIn>) -> Result<JobOutcome, JobError> {
    let n = cabins.len();
    for c in cabins.into_iter() {
        let source_ref = format!("dnt-{}", c.source_id);
        sqlx::query(
            r#"
            INSERT INTO anchors.anchor
                (kind, geom, name, elevation_m, sources, source_ref, attrs)
            VALUES
                ('cabin',
                 ST_Transform(ST_SetSRID(ST_Point($1, $2), 4326), 25833),
                 $3, $4, ARRAY['dnt']::text[], $5,
                 jsonb_build_object('category', $6::text))
            ON CONFLICT (source_ref) DO UPDATE
                SET geom = EXCLUDED.geom,
                    name = EXCLUDED.name,
                    elevation_m = EXCLUDED.elevation_m,
                    attrs = anchors.anchor.attrs ||
                            jsonb_build_object('category', $6::text)
            "#,
        )
        .bind(c.lon)
        .bind(c.lat)
        .bind(&c.name)
        .bind(c.elevation_m)
        .bind(&source_ref)
        .bind(c.category.as_deref())
        .execute(pool)
        .await?;
    }

    // Snap freshly-imported cabins to the graph. Bounded to 2000 m
    // — cabins in deep wilderness without nearby trails just won't be
    // routing targets, which is the right behaviour.
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
            WHERE a2.source_ref LIKE 'dnt-%'
        ) nn
        WHERE nn.aid = a.id AND nn.rn = 1
        "#,
    )
    .execute(pool)
    .await?;

    let (total,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM anchors.anchor WHERE 'dnt' = ANY(sources)")
            .fetch_one(pool)
            .await?;
    Ok(JobOutcome {
        rows_in: n as i64,
        rows_upserted: total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_geojson_feature_collection() {
        // GeoJSON form — the modern shape Nasjonal Turbase emits.
        let json = br#"{
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "geometry": {"type": "Point", "coordinates": [10.5, 60.5]},
                    "properties": {"name": "Glitterheim", "elevation": 1384, "id": "glitterheim"}
                },
                {
                    "type": "Feature",
                    "geometry": {"type": "Point", "coordinates": [9.8, 61.4]},
                    "properties": {"navn": "Spiterstulen", "hoyde": 1106, "id": 42}
                }
            ]
        }"#;
        let cabins = parse_json(json).unwrap();
        assert_eq!(cabins.len(), 2);
        assert_eq!(cabins[0].name, "Glitterheim");
        assert_eq!(cabins[0].source_id, "glitterheim");
        assert!((cabins[1].elevation_m.unwrap() - 1106.0).abs() < 1.0);
        // Numeric id stringifies cleanly.
        assert_eq!(cabins[1].source_id, "42");
    }

    #[test]
    fn parse_array_form() {
        // Older legacy form — bare array of cabin objects.
        let json = br#"[
            {"name": "Krokan", "lon": 8.2, "lat": 59.8, "elevation": 1100, "id": "krokan"}
        ]"#;
        let cabins = parse_json(json).unwrap();
        assert_eq!(cabins.len(), 1);
        assert_eq!(cabins[0].name, "Krokan");
        assert!((cabins[0].lon - 8.2).abs() < 1e-9);
    }

    #[test]
    fn parse_skips_records_without_name() {
        // A record with no name (and no navn) isn't useful as an
        // anchor — drop it silently rather than insert "" rows.
        let json = br#"[
            {"lon": 10.0, "lat": 60.0}
        ]"#;
        let cabins = parse_json(json).unwrap();
        assert!(cabins.is_empty());
    }

    #[test]
    fn parse_skips_records_without_coordinates() {
        // Defensive: a Feature with no geometry block, or an Array
        // record missing lon/lat, should be dropped.
        let json = br#"[
            {"name": "Ghost cabin", "id": "no-coords"}
        ]"#;
        let cabins = parse_json(json).unwrap();
        assert!(cabins.is_empty());
    }

    #[test]
    fn parse_fallbacks_to_coord_based_source_id() {
        // When the upstream record lacks an `id`, we synthesise one
        // from the coordinates so the upsert's source_ref stays
        // populated and idempotent re-runs work.
        let json = br#"[
            {"name": "Unnamed cabin", "lon": 10.123, "lat": 60.456}
        ]"#;
        let cabins = parse_json(json).unwrap();
        assert_eq!(cabins.len(), 1);
        assert!(cabins[0].source_id.contains("10.123"));
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        // Bad JSON must surface as JobError::Parse, not panic.
        let result = parse_json(b"{not json}");
        assert!(matches!(result, Err(JobError::Parse(_))));
    }
}
