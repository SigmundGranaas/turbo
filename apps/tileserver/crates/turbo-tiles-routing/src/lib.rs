//! pgRouting-backed route, isochrone, and loop queries.
//!
//! Architecture: every algorithm builds an SQL CTE that pgRouting
//! reads as its "edges SQL" — we materialise per-profile cost on the
//! fly by selecting `cost_expression(profile)` from `paths.edge`,
//! bounded to a generous bbox around the query points so pgr_dijkstra
//! doesn't scan the whole national graph for short hikes.

pub mod profile;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use turbo_tiles_core::route::Profile;
use turbo_tiles_db::DbPool;

use crate::profile::cost_expression;

#[derive(Debug, thiserror::Error)]
pub enum RoutingError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("no route found between the requested points")]
    NoRouteFound,
    #[error("start or end point too far from the path network")]
    NoNearbyNode,
}

#[derive(Debug, Clone, Copy)]
pub struct LonLat {
    pub lon: f64,
    pub lat: f64,
}

#[derive(Debug, Clone, Default)]
pub struct RoutePreferences {
    pub avoid_unmarked: bool,
    pub prefer_marked: bool,
}

#[derive(Debug, Serialize)]
pub struct RouteResponse {
    pub geom: Value,
    pub distance_m: f64,
    pub duration_s: f64,
    pub elevation_gain_m: f64,
    pub edge_ids: Vec<i64>,
    pub warnings: Vec<String>,
}

/// Snap a lon/lat to the nearest node in the routing graph. Errors out
/// if no node is within `max_snap_m` (default 500 m) — better to tell
/// the user "you're too far from any path" than to silently route from
/// the wrong starting point.
async fn snap_to_node(pool: &DbPool, p: LonLat, max_snap_m: f64) -> Result<i64, RoutingError> {
    let row: Option<(i64,)> = sqlx::query_as(
        r#"
        SELECT id
        FROM paths.node
        WHERE ST_DWithin(
            geom,
            ST_Transform(ST_SetSRID(ST_Point($1, $2), 4326), 25833),
            $3
        )
        ORDER BY geom <-> ST_Transform(ST_SetSRID(ST_Point($1, $2), 4326), 25833)
        LIMIT 1
        "#,
    )
    .bind(p.lon)
    .bind(p.lat)
    .bind(max_snap_m)
    .fetch_optional(pool)
    .await?;
    row.map(|(id,)| id).ok_or(RoutingError::NoNearbyNode)
}

/// Average walking/cycling speed in m/s per profile. Used as a
/// fallback estimate when per-edge elevation isn't available; the
/// route handler prefers `hiking_speed_tobler` when both gain and
/// length are known.
fn nominal_speed_mps(profile: Profile) -> f64 {
    match profile {
        Profile::Hiking => 1.1,     // ~4 km/h
        Profile::Ski => 1.7,        // ~6 km/h
        Profile::BikeGravel => 4.2, // ~15 km/h
        Profile::BikeRoad => 6.1,   // ~22 km/h
    }
}

/// Tobler's hiking function: walking speed as a function of slope.
///   v = 6 * exp(-3.5 * |dh/dx + 0.05|)  km/h
/// Returns m/s. Used by the route handler when per-edge
/// elevation_gain_m is known so the duration estimate accounts for
/// climbs.
fn hiking_speed_tobler(elevation_gain_m: f64, horizontal_m: f64) -> f64 {
    if horizontal_m <= 0.0 {
        return 1.1;
    }
    let slope = elevation_gain_m / horizontal_m;
    // Tobler is symmetric around -0.05 (downhill at 5% grade is
    // peak speed ~6 km/h). For loss we approximate as the same
    // function evaluated at -slope; the per-edge sum below applies
    // it to gain-only segments, so callers should keep gain and
    // loss separate if they want full Tobler.
    let v_kmh = 6.0 * (-3.5 * (slope + 0.05).abs()).exp();
    v_kmh * 1000.0 / 3600.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, tol: f64) {
        assert!((a - b).abs() < tol, "expected {b}, got {a}");
    }

    #[test]
    fn tobler_peaks_at_minus_five_percent_grade() {
        // Tobler's classic result: max speed is on a -5% downhill,
        // ~6 km/h ≈ 1.67 m/s. -5% grade means dropping 5 m over
        // 100 m horizontal, so elevation_gain_m = -5.
        let v = hiking_speed_tobler(-5.0, 100.0);
        approx(v, 6.0 * 1000.0 / 3600.0, 0.01);
    }

    #[test]
    fn tobler_on_the_flat_is_about_5_kmh() {
        // Flat (gain=0) → v = 6 * exp(-3.5 * 0.05) ≈ 5.04 km/h.
        let v = hiking_speed_tobler(0.0, 100.0);
        approx(v, 5.036 * 1000.0 / 3600.0, 0.02);
    }

    #[test]
    fn tobler_uphill_is_slower_than_flat() {
        // 10% uphill (~3.55 km/h) is meaningfully slower than flat
        // ground (~5.04 km/h). Keeping the ratio loose since the
        // exact factor depends on the +0.05 bias in Tobler's
        // formula; what matters is the ordering.
        let flat = hiking_speed_tobler(0.0, 100.0);
        let uphill = hiking_speed_tobler(10.0, 100.0);
        assert!(uphill < flat * 0.8, "10% uphill should be noticeably slower");
        assert!(uphill > flat * 0.5, "10% uphill shouldn't be glacial");
    }

    #[test]
    fn tobler_zero_horizontal_returns_fallback() {
        // Edge case: a "vertical" segment with no horizontal extent
        // would divide-by-zero. Fall back to the nominal speed.
        let v = hiking_speed_tobler(10.0, 0.0);
        approx(v, 1.1, 1e-9);
    }

    #[test]
    fn nominal_speeds_are_in_expected_ranges() {
        assert!((nominal_speed_mps(Profile::Hiking) - 1.1).abs() < 0.01);
        assert!(nominal_speed_mps(Profile::Ski) > nominal_speed_mps(Profile::Hiking));
        assert!(nominal_speed_mps(Profile::BikeGravel) > nominal_speed_mps(Profile::Ski));
        assert!(nominal_speed_mps(Profile::BikeRoad) > nominal_speed_mps(Profile::BikeGravel));
    }

    #[test]
    fn haversine_oslo_to_bergen_is_about_300km() {
        let oslo = LonLat {
            lon: 10.7522,
            lat: 59.9139,
        };
        let bergen = LonLat {
            lon: 5.3221,
            lat: 60.3913,
        };
        let d = haversine_m(oslo, bergen);
        // Real great-circle distance is ~308 km.
        approx(d, 308_000.0, 5_000.0);
    }
}

/// Build an "edges SQL" subquery for pgRouting that exposes only edges
/// in the profile's traversable subset and selects the profile's cost
/// expression. The bbox restriction is critical — a national-graph
/// Dijkstra runs into the tens of seconds.
fn edges_sql(profile: Profile, prefs: &RoutePreferences, bbox_25833: &str) -> String {
    let cost = cost_expression(profile);
    let mut where_clauses = vec!["deleted_at IS NULL".to_string()];

    // Profile-specific edge exclusions. Stay conservative — when in
    // doubt include the edge with a high cost rather than filtering.
    match profile {
        Profile::Ski => {
            where_clauses.push("fkb_type IN ('skiloype','sti','traktorveg','skogsbilveg')".into())
        }
        Profile::BikeRoad => where_clauses.push("fkb_type IN ('sykkelvei','skogsbilveg')".into()),
        Profile::BikeGravel => {
            where_clauses.push("fkb_type IN ('sykkelvei','skogsbilveg','traktorveg','sti')".into())
        }
        Profile::Hiking => {} // all edge types acceptable
    }

    if prefs.avoid_unmarked {
        where_clauses.push("marking IS NOT NULL".into());
    }

    let where_sql = where_clauses.join(" AND ");

    format!(
        "SELECT id, source_node AS source, target_node AS target, \
         ({cost}) AS cost, ({cost}) AS reverse_cost \
         FROM paths.edge \
         WHERE {where_sql} \
         AND source_node IS NOT NULL AND target_node IS NOT NULL \
         AND geom && {bbox_25833}"
    )
}

/// Bounding box (EPSG:25833 envelope) containing both points expanded
/// by `padding_m`. Used as the `geom &&` filter to keep pgr_dijkstra's
/// candidate set small.
fn bbox_25833_for(from: LonLat, to: LonLat, padding_m: f64) -> String {
    // Build a literal SQL fragment that PostGIS evaluates at query
    // time. The padding is in metres because EPSG:25833 is metric.
    format!(
        "ST_Expand(\
            ST_Envelope(\
                ST_Collect(\
                    ST_Transform(ST_SetSRID(ST_Point({}, {}), 4326), 25833),\
                    ST_Transform(ST_SetSRID(ST_Point({}, {}), 4326), 25833)\
                )\
            ), {})",
        from.lon, from.lat, to.lon, to.lat, padding_m
    )
}

pub async fn find_route(
    pool: &DbPool,
    from: LonLat,
    to: LonLat,
    profile: Profile,
    prefs: RoutePreferences,
) -> Result<RouteResponse, RoutingError> {
    let start_vid = snap_to_node(pool, from, 500.0).await?;
    let end_vid = snap_to_node(pool, to, 500.0).await?;

    // 2× straight-line distance as the bbox padding. Generous enough
    // to allow non-trivial detours, tight enough that the graph stays
    // small. Capped to 20 km so very short queries still get a
    // reasonable search window.
    let straight_m = haversine_m(from, to);
    let padding_m = (straight_m * 2.0).max(20_000.0);
    let bbox = bbox_25833_for(from, to, padding_m);
    let edges = edges_sql(profile, &prefs, &bbox);

    // pgr_dijkstra returns one row per step. Joining each edge id back
    // to paths.edge gets us geometry, per-step length, and the
    // elevation_gain_m the dtm10-attach job stamped (NULL when not
    // yet attached — falls back to the nominal speed).
    #[allow(clippy::type_complexity)]
    let rows: Vec<(i64, f64, f64, Option<Value>, f64, Option<f64>)> = sqlx::query_as(
        r#"
        WITH path AS (
            SELECT * FROM pgr_dijkstra($1, $2::bigint, $3::bigint, directed := false)
        )
        SELECT
            COALESCE(path.edge, -1) AS edge,
            path.cost AS step_cost,
            path.agg_cost AS agg_cost,
            CASE WHEN path.edge = -1 THEN NULL
                 ELSE ST_AsGeoJSON(ST_Transform(e.geom, 4326))::jsonb
            END AS geom,
            COALESCE(e.length_m, 0) AS length_m,
            e.elevation_gain_m
        FROM path
        LEFT JOIN paths.edge e ON e.id = path.edge
        ORDER BY path.seq
        "#,
    )
    .bind(&edges)
    .bind(start_vid)
    .bind(end_vid)
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Err(RoutingError::NoRouteFound);
    }

    let mut edge_ids: Vec<i64> = Vec::new();
    let mut total_length_m: f64 = 0.0;
    let mut total_gain_m: f64 = 0.0;
    let mut duration_s: f64 = 0.0;
    let mut coords: Vec<Vec<f64>> = Vec::new();
    let nominal = nominal_speed_mps(profile);
    let mut elevation_unknown_warned = false;
    let mut warnings: Vec<String> = Vec::new();
    for (edge_id, _step_cost, _agg, geom, length_m, elev_gain) in rows {
        if edge_id < 0 {
            continue;
        }
        edge_ids.push(edge_id);
        total_length_m += length_m;
        total_gain_m += elev_gain.unwrap_or(0.0);
        // Per-edge duration: Tobler for hiking when elevation is
        // known; nominal speed everywhere else.
        let edge_seconds = match (profile, elev_gain) {
            (Profile::Hiking, Some(gain)) => {
                let v = hiking_speed_tobler(gain, length_m);
                if v > 0.0 {
                    length_m / v
                } else {
                    length_m / nominal
                }
            }
            (Profile::Hiking, None) => {
                if !elevation_unknown_warned {
                    warnings.push(
                        "elevation not attached to some edges; duration uses nominal speed".into(),
                    );
                    elevation_unknown_warned = true;
                }
                length_m / nominal
            }
            _ => length_m / nominal,
        };
        duration_s += edge_seconds;
        if let Some(geom) = geom {
            if let Some(arr) = geom.get("coordinates").and_then(|c| c.as_array()) {
                for pt in arr {
                    if let Some(p) = pt.as_array() {
                        if p.len() >= 2 {
                            coords.push(vec![
                                p[0].as_f64().unwrap_or(0.0),
                                p[1].as_f64().unwrap_or(0.0),
                            ]);
                        }
                    }
                }
            }
        }
    }

    let geom = serde_json::json!({
        "type": "LineString",
        "coordinates": coords,
    });

    Ok(RouteResponse {
        geom,
        distance_m: total_length_m,
        duration_s,
        elevation_gain_m: total_gain_m,
        edge_ids,
        warnings,
    })
}

fn haversine_m(a: LonLat, b: LonLat) -> f64 {
    const R: f64 = 6_371_000.0;
    let to_rad = |d: f64| d.to_radians();
    let dlat = to_rad(b.lat - a.lat);
    let dlon = to_rad(b.lon - a.lon);
    let s1 = (dlat / 2.0).sin();
    let s2 = (dlon / 2.0).sin();
    let aa = s1 * s1 + to_rad(a.lat).cos() * to_rad(b.lat).cos() * s2 * s2;
    let c = 2.0 * aa.sqrt().atan2((1.0 - aa).sqrt());
    R * c
}

/// Isochrone: reachable area within `minutes` of `from` under the
/// given profile. Returns one Polygon per minute threshold.
#[derive(Debug, Deserialize)]
pub struct IsochroneRequest {
    pub from: [f64; 2],
    pub minutes: Vec<u32>,
    pub profile: Profile,
}

pub async fn isochrone(pool: &DbPool, req: &IsochroneRequest) -> Result<Value, RoutingError> {
    let from = LonLat {
        lon: req.from[0],
        lat: req.from[1],
    };
    let start_vid = snap_to_node(pool, from, 500.0).await?;
    let speed = nominal_speed_mps(req.profile);
    let prefs = RoutePreferences::default();

    // Largest minute target determines the bbox we need.
    let max_minutes = req.minutes.iter().copied().max().unwrap_or(0);
    let max_dist_m = (max_minutes as f64) * 60.0 * speed;
    if max_dist_m <= 0.0 {
        return Ok(serde_json::json!({"type": "FeatureCollection", "features": []}));
    }
    // pgr_drivingDistance's cost limit is in the same units as the
    // edges SQL's `cost`. Our cost expression is in metres (+ small
    // adjustments), so the limit is also metres.
    let bbox = format!(
        "ST_Expand(ST_Transform(ST_SetSRID(ST_Point({}, {}), 4326), 25833), {})",
        from.lon,
        from.lat,
        max_dist_m * 1.5
    );
    let edges = edges_sql(req.profile, &prefs, &bbox);

    let mut features = Vec::new();
    for &m in &req.minutes {
        let limit_m = (m as f64) * 60.0 * speed;
        // ConcaveHull (target_percent=0.85) produces a tighter,
        // more realistic reachable-area shape than ConvexHull —
        // convex hulls of road networks balloon out across
        // unreachable land. Falls back to ConvexHull when PostGIS
        // can't build the concave hull (e.g. <3 points).
        let hull: Option<(Value,)> = sqlx::query_as(
            r#"
            WITH reach AS (
                SELECT node FROM pgr_drivingDistance($1, $2::bigint, $3::float8, directed := false)
            ),
            nodes AS (
                SELECT n.geom FROM reach r JOIN paths.node n ON n.id = r.node
            ),
            agg AS (SELECT ST_Collect(geom) AS g FROM nodes)
            SELECT ST_AsGeoJSON(
                ST_Transform(
                    COALESCE(
                        ST_ConcaveHull(g, 0.85),
                        ST_ConvexHull(g)
                    ),
                    4326
                )
            )::jsonb
            FROM agg
            WHERE g IS NOT NULL
            "#,
        )
        .bind(&edges)
        .bind(start_vid)
        .bind(limit_m)
        .fetch_optional(pool)
        .await?;
        if let Some((geom,)) = hull {
            features.push(serde_json::json!({
                "type": "Feature",
                "properties": {"minutes": m},
                "geometry": geom,
            }));
        }
    }

    Ok(serde_json::json!({
        "type": "FeatureCollection",
        "features": features,
    }))
}
