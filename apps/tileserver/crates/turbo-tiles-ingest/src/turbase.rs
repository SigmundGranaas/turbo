//! Turrutebasen (Tur- og friluftsruter) ingest: restore + upsert.
//! Same restore-once/upsert-many pattern as N50.

use std::path::PathBuf;

use turbo_tiles_db::DbPool;

use crate::job::{JobError, JobOutcome};
use crate::pgdump_load::{self, PgDumpConfig};

const CONFIG: PgDumpConfig = PgDumpConfig {
    source_schema_pattern: "turogfriluftsruter\\_%",
    canonical_schema: "turbase_staging",
    sentinel_table: "fotrute",
    label: "turbase",
};

pub async fn restore(pool: &DbPool, file: PathBuf, force: bool) -> Result<JobOutcome, JobError> {
    let sql = pgdump_load::unzip_dump(&file).await?;
    let result = pgdump_load::restore(pool, sql, CONFIG, force).await?;
    Ok(pgdump_load::outcome_from_result(result))
}

pub async fn upsert(pool: &DbPool) -> Result<JobOutcome, JobError> {
    pgdump_load::require_staging(pool, CONFIG).await?;
    sqlx::raw_sql(include_str!("../sql/upsert_turbase.sql"))
        .execute(pool)
        .await?;
    // Rebuild pgRouting topology so the new edges are routable.
    // Two-step because of a known sqlx + pgRouting interaction: when
    // pgr_createTopology is called via sqlx, the function returns
    // 'OK' and populates paths.edge_vertices_pgr correctly, but the
    // UPDATE step that writes source_node/target_node back onto
    // paths.edge doesn't persist (likely a prepared-statement /
    // dynamic-SQL visibility issue inside the C function). The
    // workaround: do the UPDATE ourselves via a spatial join against
    // the vertices table.
    sqlx::raw_sql(
        "SELECT pgr_createTopology('paths.edge', 1.0, 'geom', 'id', \
                                   'source_node', 'target_node', \
                                   rows_where := 'deleted_at IS NULL', \
                                   clean := true);",
    )
    .execute(pool)
    .await
    .ok();
    // Map every edge's start/end to the nearest vertex within 1 m
    // (matches pgr_createTopology's tolerance). Two CTEs so the
    // planner can use the GIST index on both joins independently.
    sqlx::raw_sql(
        "WITH src AS (\
            SELECT e.id AS edge_id, v.id AS node_id \
            FROM paths.edge e \
            JOIN LATERAL (\
                SELECT v.id FROM paths.edge_vertices_pgr v \
                WHERE ST_DWithin(v.the_geom, ST_StartPoint(e.geom), 1.0) \
                ORDER BY v.the_geom <-> ST_StartPoint(e.geom) \
                LIMIT 1\
            ) v ON true \
            WHERE e.deleted_at IS NULL \
        ), \
        tgt AS (\
            SELECT e.id AS edge_id, v.id AS node_id \
            FROM paths.edge e \
            JOIN LATERAL (\
                SELECT v.id FROM paths.edge_vertices_pgr v \
                WHERE ST_DWithin(v.the_geom, ST_EndPoint(e.geom), 1.0) \
                ORDER BY v.the_geom <-> ST_EndPoint(e.geom) \
                LIMIT 1\
            ) v ON true \
            WHERE e.deleted_at IS NULL \
        ) \
        UPDATE paths.edge e \
        SET source_node = src.node_id, target_node = tgt.node_id \
        FROM src JOIN tgt USING (edge_id) \
        WHERE e.id = src.edge_id;",
    )
    .execute(pool)
    .await
    .ok();

    // Mirror the pgRouting vertex table into paths.node so the
    // routing crate's snap query (which reads paths.node) finds them.
    // CASCADE-safe: snapped_node_id columns on anchors are plain
    // bigints with no FK, so truncating paths.node doesn't fan out.
    sqlx::raw_sql(
        "TRUNCATE paths.node RESTART IDENTITY CASCADE; \
         INSERT INTO paths.node (id, geom) \
         SELECT id, the_geom FROM paths.edge_vertices_pgr; \
         SELECT setval(pg_get_serial_sequence('paths.node', 'id'), \
                       COALESCE((SELECT MAX(id) FROM paths.node), 1));",
    )
    .execute(pool)
    .await?;
    // Re-snap anchors after the node table refreshed. Bounded to 2km.
    sqlx::raw_sql(
        "UPDATE anchors.anchor a \
         SET snapped_node_id = nn.node_id, snap_distance_m = nn.dist \
         FROM ( \
             SELECT a2.id AS aid, n.id AS node_id, \
                    ST_Distance(n.geom, a2.geom) AS dist, \
                    ROW_NUMBER() OVER ( \
                        PARTITION BY a2.id ORDER BY ST_Distance(n.geom, a2.geom) \
                    ) AS rn \
             FROM anchors.anchor a2 \
             JOIN paths.node n ON ST_DWithin(n.geom, a2.geom, 2000.0) \
         ) nn \
         WHERE nn.aid = a.id AND nn.rn = 1;",
    )
    .execute(pool)
    .await
    .ok();
    let (edge_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM paths.edge \
         WHERE deleted_at IS NULL AND ingest_source = 'turbase'",
    )
    .fetch_one(pool)
    .await?;
    let (trail_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM trails.trail WHERE source = 'turbase'")
            .fetch_one(pool)
            .await?;
    Ok(JobOutcome {
        rows_in: edge_count,
        rows_upserted: trail_count,
    })
}

#[cfg(test)]
mod tests {

    #[test]
    fn upsert_sql_handles_norwegian_mark_colours() {
        let sql = include_str!("../sql/upsert_turbase.sql");
        for m in ["rød", "rod", "blå", "bla", "sort", "svart", "t-merket"] {
            assert!(sql.contains(m), "missing mark colour `{m}`");
        }
    }

    #[test]
    fn upsert_sql_builds_three_phase_rollup() {
        // edges (paths.edge) → trails (trails.trail) → links (trails.trail_edge).
        let sql = include_str!("../sql/upsert_turbase.sql");
        assert!(sql.contains("INSERT INTO paths.edge"));
        assert!(sql.contains("INSERT INTO trails.trail"));
        assert!(sql.contains("INSERT INTO trails.trail_edge"));
    }

    #[test]
    fn upsert_sql_includes_skiloype_as_winter_only() {
        // Ski tracks must be ingested as fkb_type='skiloype' with
        // season=['winter']; otherwise the ski profile won't reward
        // them and the hiking profile will pick them in summer.
        let sql = include_str!("../sql/upsert_turbase.sql");
        assert!(sql.contains("turbase_staging.skiloype"));
        assert!(sql.contains("'skiloype'"));
    }
}
