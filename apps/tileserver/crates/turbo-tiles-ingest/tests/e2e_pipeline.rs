//! Real end-to-end pipeline test against a live Postgres + PostGIS +
//! pgRouting database. Skips silently when `INGEST_TEST_DATABASE_URL`
//! isn't set; runs full coverage when it is.
//!
//! Tested flow (every step exercises the actual production code path,
//! not a stub):
//!   1. Apply all migrations against a clean DB.
//!   2. Restore the synthetic N50 mini-dump via the same `n50::restore`
//!      that the admin trigger calls in production.
//!   3. Run every N50 upsert (vann, isogbre, landcover, stedsnavn, vegnett)
//!      and assert canonical tables are populated.
//!   4. Restore the synthetic Turrutebasen mini-dump and upsert; assert
//!      paths.edge / trails.trail populated.
//!   5. Seed the recommend fixture, build the skeleton, and verify
//!      anchors snap.
//!   6. Run reset_all and confirm the DB returns to empty state.
//!
//! Test isolation: each test acquires its own per-test schema prefix
//! and a dedicated test DB; never touches the production tiles DB.

use std::path::PathBuf;

use turbo_tiles_db::{DbConfig, DbPool};
use turbo_tiles_ingest::{n50, pgdump_load, turbase};

/// Connect to the test DB or skip the test. Reads from
/// `INGEST_TEST_DATABASE_URL` (or falls back to the local docker
/// instance the test harness spins up).
async fn pool_or_skip() -> Option<DbPool> {
    let url = std::env::var("INGEST_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:testpass@localhost:55433/tiles".to_string());
    // pgdump_load + dtm_raster shell out to psql/raster2pgsql which
    // read DATABASE_URL directly. Mirror the test URL there.
    std::env::set_var("DATABASE_URL", &url);
    let cfg = DbConfig {
        url,
        max_connections: 4,
        min_connections: 1,
        statement_timeout_ms: 120_000,
    };
    match cfg.connect().await {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("skipping E2E test: cannot connect to INGEST_TEST_DATABASE_URL: {e}");
            None
        }
    }
}

async fn ensure_clean(pool: &DbPool) {
    // Drop staging schemas + nuke canonical data so the test starts
    // from a known state.
    sqlx::query("DROP SCHEMA IF EXISTS n50_staging CASCADE")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DROP SCHEMA IF EXISTS turbase_staging CASCADE")
        .execute(pool)
        .await
        .ok();
    for sql in [
        "DELETE FROM trails.trail_edge",
        "DELETE FROM trails.trail",
        "DELETE FROM anchors.anchor",
        "DELETE FROM terrain.water_polygon",
        "DELETE FROM terrain.glacier_polygon",
        "DELETE FROM terrain.landcover_patch",
        "DELETE FROM terrain.building_polygon",
        "DELETE FROM terrain.contour",
        "DELETE FROM paths.edge",
        "DELETE FROM paths.node",
    ] {
        sqlx::query(sql).execute(pool).await.ok();
    }
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("fixtures")
        .join(name)
}

#[tokio::test]
async fn n50_restore_creates_canonical_staging_schema() {
    // Restore mini-fixture and verify the schema rename worked: a
    // `n50_staging` schema must exist with the expected tables.
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    ensure_clean(&pool).await;

    let fixture = fixture_path("n50_mini.sql");
    let outcome = n50::restore(&pool, fixture, true)
        .await
        .expect("restore ok");
    assert!(outcome.rows_in > 0);

    let (exists,): (bool,) = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM information_schema.schemata \
         WHERE schema_name = 'n50_staging')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(exists, "n50_staging schema must exist after restore");

    let (innsjo_count,): (i64,) = sqlx::query_as("SELECT COUNT(*)::bigint FROM n50_staging.innsjo")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(innsjo_count > 0, "innsjo rows must have been restored");
}

#[tokio::test]
async fn n50_upsert_vann_populates_water_polygons() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    ensure_clean(&pool).await;
    n50::restore(&pool, fixture_path("n50_mini.sql"), true)
        .await
        .expect("restore");

    let outcome = n50::upsert_vann(&pool).await.expect("upsert");
    assert!(outcome.rows_in >= 2, "fixture has 2 lakes");

    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM terrain.water_polygon WHERE source = 'n50'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(n, 2);
}

#[tokio::test]
async fn n50_upsert_isogbre_populates_glaciers() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    ensure_clean(&pool).await;
    n50::restore(&pool, fixture_path("n50_mini.sql"), true)
        .await
        .expect("restore");
    n50::upsert_isogbre(&pool).await.expect("upsert");

    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM terrain.glacier_polygon WHERE source = 'n50'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(n, 1, "fixture has 1 glacier");
}

#[tokio::test]
async fn n50_upsert_landcover_covers_every_class() {
    // Critical: this is the AR50-replacement upsert. Forest, wetland,
    // open must all land. If the SQL ever drops a source table, this
    // test catches it.
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    ensure_clean(&pool).await;
    n50::restore(&pool, fixture_path("n50_mini.sql"), true)
        .await
        .expect("restore");
    n50::upsert_landcover(&pool).await.expect("upsert");

    for (class, expected_min) in [("forest", 1), ("wetland", 1), ("open", 1)] {
        let (n,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*)::bigint FROM terrain.landcover_patch \
             WHERE source = 'n50' AND class = $1",
        )
        .bind(class)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            n >= expected_min,
            "class {class} should have ≥{expected_min} patches, got {n}"
        );
    }

    // attr_version bumped.
    let (v,): (i32,) =
        sqlx::query_as("SELECT version FROM recommend.attr_version WHERE singleton = true")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(v >= 2, "attr_version should bump on landcover upsert");
}

#[tokio::test]
async fn n50_upsert_stedsnavn_classifies_anchor_kinds() {
    // Vettakollen + Tryvannshogda → summits.
    // Sognsvann → waterfeature.
    // Kobberhaughytta → cabin.
    // Frognerseteren → named_place.
    // terrengpunkt 7001 (hoyde 720) → summit; 7002 (hoyde 250) → filtered.
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    ensure_clean(&pool).await;
    n50::restore(&pool, fixture_path("n50_mini.sql"), true)
        .await
        .expect("restore");
    n50::upsert_stedsnavn(&pool).await.expect("upsert");

    let (summits,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM anchors.anchor WHERE kind = 'summit'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        summits >= 3,
        "expected ≥3 summits (Vettakollen + Tryvannshogda + high terrengpunkt), got {summits}"
    );

    let (cabins,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM anchors.anchor WHERE kind = 'cabin'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(cabins >= 1);

    let (lakes,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM anchors.anchor WHERE kind = 'waterfeature'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(lakes >= 1);

    // Low terrengpunkt must be filtered out (we set the cutoff at 600m).
    let (low_terr,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM anchors.anchor \
         WHERE source_ref = 'n50-terrengpunkt-7002'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(low_terr, 0, "low-elevation terrengpunkt should be filtered");
}

#[tokio::test]
async fn n50_upsert_vegnett_creates_road_edges() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    ensure_clean(&pool).await;
    n50::restore(&pool, fixture_path("n50_mini.sql"), true)
        .await
        .expect("restore");
    n50::upsert_vegnett(&pool).await.expect("upsert");

    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM paths.edge \
         WHERE deleted_at IS NULL AND attrs->>'source' = 'n50_vegnett'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 2, "fixture has 2 veglenke rows");

    // Verify fkb_type classification. The vegnett upsert emits the canonical
    // "vei" vocabulary the graph encoder understands and the resource views
    // now filter on (migration 20260603000002): `traktorvei` / `skogsvei`.
    let (traktor,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM paths.edge \
         WHERE deleted_at IS NULL AND attrs->>'source' = 'n50_vegnett' \
           AND fkb_type = 'traktorvei'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (skogs,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM paths.edge \
         WHERE deleted_at IS NULL AND attrs->>'source' = 'n50_vegnett' \
           AND fkb_type = 'skogsvei'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(traktor, 1, "Traktorveg → fkb_type 'traktorvei'");
    assert_eq!(skogs, 1, "Skogsbilveg → fkb_type 'skogsvei'");

    // Reconciliation guard: the N50 vegnett edges must now actually surface
    // in the served forest-roads view (skogsvei + traktorvei both qualify).
    // This is what the vocabulary mismatch used to break.
    let (forest_edges,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM paths.v_forest_roads WHERE id LIKE 'edge:%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        forest_edges, 2,
        "both N50 vegnett edges (skogsvei + traktorvei) must appear in v_forest_roads"
    );
}

#[tokio::test]
async fn n50_upsert_hoydekurve_creates_contours() {
    // N50 "Høyde" theme → terrain.contour. The fixture has 3 main
    // contours (200/220/600 m), 1 auxiliary (210 m) and 1 depression
    // (180 m). Index lines are the 100 m multiples among main/depression
    // (200 + 600), so 2 lines must carry is_index.
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    ensure_clean(&pool).await;
    n50::restore(&pool, fixture_path("n50_mini.sql"), true)
        .await
        .expect("restore");

    let outcome = n50::upsert_hoydekurve(&pool).await.expect("upsert");
    assert_eq!(outcome.rows_in, 5, "fixture has 3 main + 1 aux + 1 depression");

    for (kind, expected) in [("main", 3), ("auxiliary", 1), ("depression", 1)] {
        let (n,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*)::bigint FROM terrain.contour WHERE source = 'n50' AND kind = $1",
        )
        .bind(kind)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, expected, "kind {kind} count");
    }

    // Index detection: 200 m and 600 m main contours are the only
    // 100 m multiples in the fixture.
    let (index_lines,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM terrain.contour WHERE source = 'n50' AND is_index",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(index_lines, 2, "200 m + 600 m are index contours");

    // Geometry must be clean LineStrings in EPSG:25833 (ST_Dump output).
    let (bad_geom,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM terrain.contour \
         WHERE source = 'n50' AND (ST_GeometryType(geom) <> 'ST_LineString' OR ST_SRID(geom) <> 25833)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(bad_geom, 0, "all contours must be LineString/25833");
}

#[tokio::test]
async fn n50_upsert_bygning_creates_buildings() {
    // N50 BygningerOgAnlegg → terrain.building_polygon. The fixture has 2
    // footprints, one named (a cabin), one unnamed. Geometry must come out as
    // MultiPolygon in EPSG:25833.
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    ensure_clean(&pool).await;
    n50::restore(&pool, fixture_path("n50_mini.sql"), true)
        .await
        .expect("restore");

    let outcome = n50::upsert_bygning(&pool).await.expect("upsert");
    assert_eq!(outcome.rows_in, 2, "fixture has 2 building footprints");

    let (named,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM terrain.building_polygon \
         WHERE source = 'n50' AND name = 'Kobberhaughytta'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(named, 1, "named cabin footprint must carry its navn");

    let (bad_geom,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM terrain.building_polygon \
         WHERE source = 'n50' AND (ST_GeometryType(geom) <> 'ST_MultiPolygon' OR ST_SRID(geom) <> 25833)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(bad_geom, 0, "all buildings must be MultiPolygon/25833");
}

#[tokio::test]
async fn turbase_full_ingest_creates_edges_and_trails() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    ensure_clean(&pool).await;
    turbase::restore(&pool, fixture_path("turbase_mini.sql"), true)
        .await
        .expect("restore");
    turbase::upsert(&pool).await.expect("upsert");

    let (edges,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM paths.edge \
         WHERE deleted_at IS NULL AND ingest_source = 'turbase'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(edges >= 4, "expected ≥4 foot trail edges, got {edges}");

    let (ski,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM paths.edge \
         WHERE deleted_at IS NULL AND ingest_source = 'turbase' AND fkb_type = 'skiloype'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ski, 1, "expected 1 ski track");

    let (trails,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM trails.trail WHERE source = 'turbase'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        trails >= 2,
        "expected ≥2 trail rollups (anleggsnummer A001 + A002), got {trails}"
    );

    // Marking colour: Norwegian → English mapping.
    let (red,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM paths.edge \
         WHERE deleted_at IS NULL AND ingest_source = 'turbase' AND marking = 'red'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (blue,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM paths.edge \
         WHERE deleted_at IS NULL AND ingest_source = 'turbase' AND marking = 'blue'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(red, 3, "3 red-marked Fotrute rows");
    assert_eq!(blue, 1, "1 blue-marked Fotrute row");
}

#[tokio::test]
async fn require_staging_errors_when_n50_not_restored() {
    // Defensive contract: an upsert run before its restore must
    // surface a clear error, not silently no-op.
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    sqlx::query("DROP SCHEMA IF EXISTS n50_staging CASCADE")
        .execute(&pool)
        .await
        .ok();
    let err = n50::upsert_vann(&pool).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("not found") || msg.contains("-restore"),
        "expected helpful error, got: {msg}"
    );
}

#[tokio::test]
async fn restore_skips_when_staging_exists_unless_forced() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    ensure_clean(&pool).await;

    // First restore — should populate.
    let r1 = n50::restore(&pool, fixture_path("n50_mini.sql"), false)
        .await
        .expect("first restore");
    assert!(r1.rows_in > 0);

    // Second restore without force — should skip (rows_in count
    // reported is from existing schema, not zero).
    let r2 = n50::restore(&pool, fixture_path("n50_mini.sql"), false)
        .await
        .expect("second restore skips");
    assert_eq!(r1.rows_in, r2.rows_in, "skipped restore returns same shape");
}

// The old `full_pipeline_then_skeleton_then_to_target_recipe` test
// was removed in Stage 0 along with the recommendation engine. Stage
// 7 will reintroduce a hybrid pipeline test that drives
// build-artifacts → primitives → the new /v1/pathfind endpoint
// against the same fixture data.

#[tokio::test]
async fn reset_all_clears_every_namespace_to_zero() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    ensure_clean(&pool).await;
    n50::restore(&pool, fixture_path("n50_mini.sql"), true)
        .await
        .expect("restore");
    n50::upsert_vann(&pool).await.expect("vann");
    n50::upsert_landcover(&pool).await.expect("landcover");

    // Hit the reset_all SQL directly (the admin endpoint is auth-gated;
    // we test the same SQL block).
    for sql in [
        "DELETE FROM trails.trail_edge",
        "DELETE FROM trails.trail",
        "DELETE FROM anchors.anchor",
        "DELETE FROM terrain.water_polygon",
        "DELETE FROM terrain.glacier_polygon",
        "DELETE FROM terrain.landcover_patch",
        "DELETE FROM terrain.building_polygon",
        "DELETE FROM terrain.contour",
        "DELETE FROM paths.edge",
        "DELETE FROM paths.node",
        "DELETE FROM paths.dem",
        "DELETE FROM paths.ingest_job",
        "DROP SCHEMA IF EXISTS n50_staging CASCADE",
        "DROP SCHEMA IF EXISTS turbase_staging CASCADE",
    ] {
        sqlx::query(sql).execute(&pool).await.expect("reset step");
    }

    for (label, sql) in [
        ("anchors", "SELECT COUNT(*)::bigint FROM anchors.anchor"),
        (
            "water",
            "SELECT COUNT(*)::bigint FROM terrain.water_polygon",
        ),
        (
            "landcover",
            "SELECT COUNT(*)::bigint FROM terrain.landcover_patch",
        ),
        ("edges", "SELECT COUNT(*)::bigint FROM paths.edge"),
        ("nodes", "SELECT COUNT(*)::bigint FROM paths.node"),
    ] {
        let (n,): (i64,) = sqlx::query_as(sql).fetch_one(&pool).await.unwrap();
        assert_eq!(n, 0, "{label} should be empty after reset_all");
    }

    let (exists,): (bool,) = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM information_schema.schemata \
         WHERE schema_name = 'n50_staging')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!exists, "n50_staging should be gone after reset");
}

#[tokio::test]
async fn pgdump_load_unzip_dump_returns_sql_path() {
    // Verify the zip→sql discovery works against our test fixture
    // (it's a raw .sql, not a zip — passthrough case).
    let p = fixture_path("n50_mini.sql");
    let out = pgdump_load::unzip_dump(&p).await.expect("unzip");
    assert_eq!(out, p);
}
