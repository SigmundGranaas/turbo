//! Skeleton-edge synthesis. Builds straight-line off-trail connectors
//! between anchors via PostGIS Delaunay triangulation, filtered by
//! sensible length bounds and refused-region intersection.
//!
//! What this does NOT do (per the user's rule "raster cannot make
//! real data"):
//!   * Derive ridgelines, drainages, or saddles from DTM raster.
//!   * Infer landform features.
//!
//! What this does:
//!   * Connect KNOWN VECTOR ENTITIES (anchors) with straight-line
//!     graph edges.
//!   * Use authoritative vector polygons (water, glacier) to refuse
//!     crossings.
//!   * Sample DTM10 along the new edge geometry to populate
//!     `elevation_gain_m` / `elevation_loss_m` — this is sampling, not
//!     feature derivation.
//!
//! Cost penalty: handled in `turbo-tiles-routing::profile::cost_expression`
//! via the `ingest_source = 'skeleton'` branch — see that file.

use turbo_tiles_db::DbPool;

use crate::job::{JobError, JobOutcome};

/// Length bounds for off-trail connectors. Short floor avoids
/// near-duplicate edges where two anchors snap to almost the same
/// trail node; ceiling rejects implausibly long off-trail traverses
/// that would let pgRouting bypass huge swathes of the network.
const MIN_LENGTH_M: f64 = 200.0;
const MAX_LENGTH_M: f64 = 4_000.0;

pub async fn run(pool: &DbPool) -> Result<JobOutcome, JobError> {
    // 0. Pre-flight: we need at least 3 snapped anchors to triangulate.
    let (snapped_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM anchors.anchor WHERE snapped_node_id IS NOT NULL",
    )
    .fetch_one(pool)
    .await?;
    if snapped_count < 3 {
        return Err(JobError::Fetch(format!(
            "need at least 3 snapped anchors to triangulate; found {snapped_count}",
        )));
    }

    // 1. Wipe previously-synthesised skeleton edges so re-running is
    // idempotent. Hard delete (not soft) — there's no curator-edited
    // data on skeleton edges.
    sqlx::query("DELETE FROM paths.edge WHERE ingest_source = 'skeleton'")
        .execute(pool)
        .await?;

    // 2. Build a temp table of candidate Delaunay edges. PostGIS
    // `ST_DelaunayTriangles(..., 0.0, 1)` returns a MULTILINESTRING of
    // the triangulation's edges directly, which is exactly what we
    // need — no triangle-to-edge extraction required.
    //
    // 3. For each candidate, snap each endpoint back to the source
    // anchor (lookup by exact geom match) and read its snapped_node_id.
    // Drop edges where either endpoint isn't an anchor or where the
    // two anchors share a snapped_node (degenerate).
    //
    // 4. Filter:
    //    - by length [MIN_LENGTH_M, MAX_LENGTH_M]
    //    - by NOT intersecting any water polygon
    //    - by NOT intersecting any glacier polygon
    //
    // 5. Sample DTM10 at the two endpoints to compute net
    // elevation_gain_m / elevation_loss_m. We use endpoints only (not
    // along-line vertices) because a straight off-trail line typically
    // has no intermediate vertices to sample; the next slice can
    // densify the line if more granularity is needed.
    let insert_sql = r#"
        WITH snapped_anchors AS (
            SELECT id, geom, snapped_node_id
            FROM anchors.anchor
            WHERE snapped_node_id IS NOT NULL
        ),
        pts AS (
            SELECT ST_Collect(geom) AS g FROM snapped_anchors
        ),
        edges AS (
            SELECT (ST_Dump(ST_DelaunayTriangles((SELECT g FROM pts), 0.0, 1))).geom AS edge_geom
        ),
        endpoints AS (
            SELECT edge_geom,
                   ST_StartPoint(edge_geom) AS a_pt,
                   ST_EndPoint(edge_geom)   AS b_pt,
                   ST_Length(edge_geom)     AS len
            FROM edges
        ),
        candidate AS (
            SELECT e.edge_geom AS geom,
                   e.len,
                   a.snapped_node_id AS source_node,
                   b.snapped_node_id AS target_node
            FROM endpoints e
            JOIN snapped_anchors a ON ST_Equals(e.a_pt, a.geom)
            JOIN snapped_anchors b ON ST_Equals(e.b_pt, b.geom)
            WHERE a.snapped_node_id <> b.snapped_node_id
              AND e.len BETWEEN $1 AND $2
        ),
        sane AS (
            SELECT c.geom, c.len, c.source_node, c.target_node
            FROM candidate c
            WHERE NOT EXISTS (
                SELECT 1 FROM terrain.water_polygon w
                WHERE ST_Intersects(c.geom, w.geom)
            )
            AND NOT EXISTS (
                SELECT 1 FROM terrain.glacier_polygon g
                WHERE ST_Intersects(c.geom, g.geom)
            )
        ),
        with_elev AS (
            SELECT s.geom, s.len, s.source_node, s.target_node,
                   (SELECT ST_Value(d.rast, 1, ST_StartPoint(s.geom))
                    FROM paths.dem d
                    WHERE ST_Intersects(d.rast, ST_StartPoint(s.geom))
                    LIMIT 1) AS z_a,
                   (SELECT ST_Value(d.rast, 1, ST_EndPoint(s.geom))
                    FROM paths.dem d
                    WHERE ST_Intersects(d.rast, ST_EndPoint(s.geom))
                    LIMIT 1) AS z_b
            FROM sane s
        )
        INSERT INTO paths.edge
            (source_node, target_node, geom,
             elevation_gain_m, elevation_loss_m,
             fkb_type, marking, surface, season,
             attrs, attr_hash, ingest_source)
        SELECT
            we.source_node,
            we.target_node,
            we.geom,
            GREATEST(0.0, COALESCE(we.z_b, 0) - COALESCE(we.z_a, 0)) AS gain,
            GREATEST(0.0, COALESCE(we.z_a, 0) - COALESCE(we.z_b, 0)) AS loss,
            'off_trail'::text,
            NULL::text,
            NULL::text,
            ARRAY['summer','winter']::text[],
            '{}'::jsonb,
            encode(
                sha256(('skeleton-' || we.source_node::text || '-' ||
                                       we.target_node::text)::bytea),
                'hex'
            ) AS attr_hash,
            'skeleton'::paths.ingest_source
        FROM with_elev we
        ON CONFLICT (attr_hash) WHERE deleted_at IS NULL DO NOTHING
        RETURNING id
    "#;

    let inserted: Vec<(i64,)> = sqlx::query_as(insert_sql)
        .bind(MIN_LENGTH_M)
        .bind(MAX_LENGTH_M)
        .fetch_all(pool)
        .await?;
    let count = inserted.len() as i64;

    // 6. Bump attr_version since the routing graph changed.
    if count > 0 {
        sqlx::query(
            r#"
            UPDATE recommend.attr_version
            SET version = version + 1, notes = 'skeleton-build', set_at = now()
            WHERE singleton = true
            "#,
        )
        .execute(pool)
        .await?;
    }

    let (total,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM paths.edge \
         WHERE ingest_source = 'skeleton' AND deleted_at IS NULL",
    )
    .fetch_one(pool)
    .await?;

    Ok(JobOutcome {
        rows_in: count,
        rows_upserted: total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_bounds_are_reasonable() {
        // The min must exclude near-zero pairs (sub-200m) and the max
        // must keep off-trail routing local (<4km). If these drift
        // out of range, the skeleton becomes either degenerate
        // (everything connects to everything) or vacuous.
        assert!(MIN_LENGTH_M >= 50.0);
        assert!(MAX_LENGTH_M <= 10_000.0);
        assert!(MAX_LENGTH_M > MIN_LENGTH_M);
    }
}
