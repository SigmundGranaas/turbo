//! Admin reset endpoints. Lets the curator tear down per-source data
//! without dropping into psql. The SPA's Dashboard renders these as
//! confirm-then-reset buttons so the ingest pipeline can be exercised
//! repeatedly end-to-end from the UI.
//!
//! Scopes:
//!   recommend       — drop fixture seed (anchors, water, trail, skeleton)
//!   skeleton        — drop only synthetic off-trail edges
//!   n50_staging     — drop the heavy N50 staging schema
//!   turbase_staging — drop the Turrutebasen staging schema
//!   canonical       — drop derived canonical data
//!                     (terrain.*, anchors.*, trails.*, skeleton edges)
//!                     leaving paths.node, paths.edge real-source, paths.dem alone
//!   all             — every scope above plus paths.edge (all sources)
//!                     and paths.node. Brings the DB back to "just migrated".
//!
//! Every reset is a single transaction. Idempotent — re-running an
//! already-empty scope is a no-op.

use axum::extract::{Path, State};
use axum::Json;
use serde_json::{json, Value};
use turbo_tiles_auth::{Curator, RequireRole};

use crate::error::AdminError;
use crate::state::AdminState;

pub async fn reset(
    _: RequireRole<Curator>,
    State(state): State<AdminState>,
    Path(scope): Path<String>,
) -> Result<Json<Value>, AdminError> {
    let actions = match scope.as_str() {
        "recommend" => reset_recommend(&state.db).await?,
        "skeleton" => reset_skeleton(&state.db).await?,
        "n50_staging" => reset_n50_staging(&state.db).await?,
        "turbase_staging" => reset_turbase_staging(&state.db).await?,
        "canonical" => reset_canonical(&state.db).await?,
        "all" => reset_all(&state.db).await?,
        other => {
            return Err(AdminError::BadRequest(format!(
                "unknown reset scope `{other}` — try one of: \
                 recommend, skeleton, n50_staging, turbase_staging, canonical, all",
            )))
        }
    };
    tracing::info!(scope = %scope, ?actions, "admin: reset complete");
    Ok(Json(
        json!({ "ok": true, "scope": scope, "actions": actions }),
    ))
}

async fn reset_recommend(db: &turbo_tiles_db::DbPool) -> Result<Vec<String>, AdminError> {
    // Drop the fixture seed rows specifically — leaves any real-data
    // ingest untouched.
    let mut tx = db
        .begin()
        .await
        .map_err(|e| AdminError::Db(e.to_string()))?;
    let counts: Vec<(&str, i64)> = vec![
        run_count(&mut tx, "DELETE FROM anchors.anchor WHERE source_ref LIKE 'n50-fixture-%' OR source_ref LIKE 'seed-%' RETURNING 1", "anchors_fixture").await?,
        run_count(&mut tx, "DELETE FROM terrain.water_polygon WHERE source = 'seed' RETURNING 1", "water_seed").await?,
        run_count(&mut tx, "DELETE FROM trails.trail_edge WHERE trail_id IN (SELECT id FROM trails.trail WHERE source = 'manual' AND source_ref LIKE 'seed-%') RETURNING 1", "trail_edge_seed").await?,
        run_count(&mut tx, "DELETE FROM trails.trail WHERE source = 'manual' AND source_ref LIKE 'seed-%' RETURNING 1", "trails_seed").await?,
    ];
    tx.commit()
        .await
        .map_err(|e| AdminError::Db(e.to_string()))?;
    Ok(counts
        .into_iter()
        .map(|(k, n)| format!("{k}: {n}"))
        .collect())
}

async fn reset_skeleton(db: &turbo_tiles_db::DbPool) -> Result<Vec<String>, AdminError> {
    let r = sqlx::query("DELETE FROM paths.edge WHERE ingest_source = 'skeleton'")
        .execute(db)
        .await
        .map_err(|e| AdminError::Db(e.to_string()))?;
    Ok(vec![format!(
        "skeleton_edges_deleted: {}",
        r.rows_affected()
    )])
}

async fn reset_n50_staging(db: &turbo_tiles_db::DbPool) -> Result<Vec<String>, AdminError> {
    drop_schema_with_pattern(db, "n50_staging").await
}

async fn reset_turbase_staging(db: &turbo_tiles_db::DbPool) -> Result<Vec<String>, AdminError> {
    drop_schema_with_pattern(db, "turbase_staging").await
}

async fn drop_schema_with_pattern(
    db: &turbo_tiles_db::DbPool,
    schema: &str,
) -> Result<Vec<String>, AdminError> {
    // Drop both the canonical name and any leftover hash-named
    // schema (in case a prior restore failed before the rename).
    let pattern = match schema {
        "n50_staging" => "n50kartdata\\_%",
        "turbase_staging" => "turogfriluftsruter\\_%",
        _ => return Err(AdminError::BadRequest("unknown staging".into())),
    };
    let mut actions: Vec<String> = Vec::new();
    // Canonical name.
    let r = sqlx::query(&format!("DROP SCHEMA IF EXISTS {} CASCADE", schema))
        .execute(db)
        .await
        .map_err(|e| AdminError::Db(e.to_string()))?;
    actions.push(format!("dropped_canonical: {}", schema));
    let _ = r;
    // Any hash-named leftovers.
    let leftovers: Vec<(String,)> = sqlx::query_as(
        "SELECT schema_name::text FROM information_schema.schemata WHERE schema_name LIKE $1",
    )
    .bind(pattern)
    .fetch_all(db)
    .await
    .map_err(|e| AdminError::Db(e.to_string()))?;
    for (s,) in leftovers {
        sqlx::query(&format!(
            "DROP SCHEMA IF EXISTS \"{}\" CASCADE",
            s.replace('"', "\"\"")
        ))
        .execute(db)
        .await
        .map_err(|e| AdminError::Db(e.to_string()))?;
        actions.push(format!("dropped_leftover: {}", s));
    }
    Ok(actions)
}

async fn reset_canonical(db: &turbo_tiles_db::DbPool) -> Result<Vec<String>, AdminError> {
    // Wipe derived canonical state, preserving paths.edge (the real
    // routing graph) and paths.dem (rasters). Use this when the
    // curator wants to re-run the upsert pipeline against a fresh
    // canonical schema without re-doing the restore.
    let mut tx = db
        .begin()
        .await
        .map_err(|e| AdminError::Db(e.to_string()))?;
    let counts = vec![
        run_count(
            &mut tx,
            "DELETE FROM trails.trail_edge RETURNING 1",
            "trail_edge",
        )
        .await?,
        run_count(&mut tx, "DELETE FROM trails.trail RETURNING 1", "trail").await?,
        run_count(&mut tx, "DELETE FROM anchors.anchor RETURNING 1", "anchor").await?,
        run_count(
            &mut tx,
            "DELETE FROM terrain.water_polygon RETURNING 1",
            "water_polygon",
        )
        .await?,
        run_count(
            &mut tx,
            "DELETE FROM terrain.glacier_polygon RETURNING 1",
            "glacier_polygon",
        )
        .await?,
        run_count(
            &mut tx,
            "DELETE FROM terrain.landcover_patch RETURNING 1",
            "landcover_patch",
        )
        .await?,
        run_count(
            &mut tx,
            "DELETE FROM terrain.ridgeline RETURNING 1",
            "ridgeline",
        )
        .await?,
        run_count(&mut tx, "DELETE FROM terrain.saddle RETURNING 1", "saddle").await?,
        run_count(
            &mut tx,
            "DELETE FROM terrain.drainage RETURNING 1",
            "drainage",
        )
        .await?,
        run_count(
            &mut tx,
            "DELETE FROM terrain.landform_patch RETURNING 1",
            "landform_patch",
        )
        .await?,
        run_count(
            &mut tx,
            "DELETE FROM terrain.treeline RETURNING 1",
            "treeline",
        )
        .await?,
        run_count(
            &mut tx,
            "DELETE FROM paths.edge WHERE ingest_source = 'skeleton' RETURNING 1",
            "skeleton_edges",
        )
        .await?,
    ];
    tx.commit()
        .await
        .map_err(|e| AdminError::Db(e.to_string()))?;
    Ok(counts
        .into_iter()
        .map(|(k, n)| format!("{k}: {n}"))
        .collect())
}

async fn reset_all(db: &turbo_tiles_db::DbPool) -> Result<Vec<String>, AdminError> {
    let mut actions: Vec<String> = Vec::new();
    actions.extend(reset_canonical(db).await?);
    actions.extend(reset_n50_staging(db).await?);
    actions.extend(reset_turbase_staging(db).await?);

    // Now wipe paths.edge (all sources), paths.node, paths.dem,
    // paths.ingest_job. The DB ends up in the same state as a fresh
    // `tileserver migrate` — schemas + extensions intact, no data.
    let mut tx = db
        .begin()
        .await
        .map_err(|e| AdminError::Db(e.to_string()))?;
    let counts = vec![
        run_count(&mut tx, "DELETE FROM paths.edge RETURNING 1", "paths_edge").await?,
        run_count(&mut tx, "DELETE FROM paths.node RETURNING 1", "paths_node").await?,
        run_count(&mut tx, "DELETE FROM paths.dem RETURNING 1", "paths_dem").await?,
        run_count(
            &mut tx,
            "DELETE FROM paths.ingest_job RETURNING 1",
            "ingest_job",
        )
        .await?,
    ];
    sqlx::query(
        "UPDATE recommend.attr_version SET version = 1, notes = 'reset all', set_at = now() WHERE singleton = true",
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AdminError::Db(e.to_string()))?;
    tx.commit()
        .await
        .map_err(|e| AdminError::Db(e.to_string()))?;
    actions.extend(counts.into_iter().map(|(k, n)| format!("{k}: {n}")));
    actions.push("attr_version_reset: 1".into());
    Ok(actions)
}

async fn run_count(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    sql: &str,
    label: &'static str,
) -> Result<(&'static str, i64), AdminError> {
    let rows: Vec<(i32,)> = sqlx::query_as(sql)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| AdminError::Db(e.to_string()))?;
    Ok((label, rows.len() as i64))
}

/// State summary — lets the SPA show "DB is empty" vs "data loaded".
pub async fn state(
    _: RequireRole<Curator>,
    State(state): State<AdminState>,
) -> Result<Json<Value>, AdminError> {
    use sqlx::Row;
    let row = sqlx::query(
        r#"
        SELECT
            (SELECT COUNT(*)::bigint FROM paths.node) AS nodes,
            (SELECT COUNT(*)::bigint FROM paths.edge WHERE deleted_at IS NULL) AS edges,
            (SELECT COUNT(*)::bigint FROM paths.edge WHERE deleted_at IS NULL AND ingest_source = 'skeleton') AS skeleton_edges,
            (SELECT COUNT(*)::bigint FROM paths.dem) AS dem_tiles,
            (SELECT COUNT(*)::bigint FROM anchors.anchor) AS anchors,
            (SELECT COUNT(*)::bigint FROM anchors.anchor WHERE snapped_node_id IS NOT NULL) AS anchors_snapped,
            (SELECT COUNT(*)::bigint FROM terrain.water_polygon) AS water_polygons,
            (SELECT COUNT(*)::bigint FROM terrain.glacier_polygon) AS glacier_polygons,
            (SELECT COUNT(*)::bigint FROM terrain.landcover_patch) AS landcover_patches,
            (SELECT COUNT(*)::bigint FROM trails.trail) AS trails,
            (SELECT EXISTS(SELECT 1 FROM information_schema.schemata WHERE schema_name = 'n50_staging')) AS n50_staged,
            (SELECT EXISTS(SELECT 1 FROM information_schema.schemata WHERE schema_name = 'turbase_staging')) AS turbase_staged,
            (SELECT version FROM recommend.attr_version WHERE singleton = true) AS attr_version
        "#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| AdminError::Db(e.to_string()))?;
    Ok(Json(json!({
        "paths": {
            "nodes": row.try_get::<i64, _>("nodes").unwrap_or(0),
            "edges": row.try_get::<i64, _>("edges").unwrap_or(0),
            "skeleton_edges": row.try_get::<i64, _>("skeleton_edges").unwrap_or(0),
            "dem_tiles": row.try_get::<i64, _>("dem_tiles").unwrap_or(0),
        },
        "anchors": {
            "total": row.try_get::<i64, _>("anchors").unwrap_or(0),
            "snapped": row.try_get::<i64, _>("anchors_snapped").unwrap_or(0),
        },
        "terrain": {
            "water_polygons": row.try_get::<i64, _>("water_polygons").unwrap_or(0),
            "glacier_polygons": row.try_get::<i64, _>("glacier_polygons").unwrap_or(0),
            "landcover_patches": row.try_get::<i64, _>("landcover_patches").unwrap_or(0),
        },
        "trails": { "total": row.try_get::<i64, _>("trails").unwrap_or(0) },
        "staging": {
            "n50_present": row.try_get::<bool, _>("n50_staged").unwrap_or(false),
            "turbase_present": row.try_get::<bool, _>("turbase_staged").unwrap_or(false),
        },
        "recommend": { "attr_version": row.try_get::<i32, _>("attr_version").unwrap_or(0) },
    })))
}
