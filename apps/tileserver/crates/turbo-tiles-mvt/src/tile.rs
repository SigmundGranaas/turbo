use turbo_tiles_core::resource::Resource;
use turbo_tiles_core::tile::TileCoord;
use turbo_tiles_db::DbPool;

#[derive(Debug, thiserror::Error)]
pub enum MvtError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// Project the attribute set we expose at this zoom. At low zooms we
/// ship a minimal set so tiles stay small; from z=12 onward we expose
/// everything the GeoJSON endpoints also surface, so client tap-sheets
/// can read straight from the tile without a round trip in the common
/// case.
fn select_columns(z: u8) -> &'static str {
    if z >= 12 {
        "id::text AS id, name, marking, surface, difficulty, length_m, elevation_gain_m"
    } else {
        "id::text AS id, marking"
    }
}

/// Render one MVT tile for a resource. Empty tiles (no features in the
/// envelope) return an empty `bytea` — clients should treat this as a
/// well-formed, blank tile, not a 404.
pub async fn render_tile(
    pool: &DbPool,
    resource: Resource,
    coord: TileCoord,
) -> Result<Vec<u8>, MvtError> {
    let cols = select_columns(coord.z);
    let view = resource.view();
    let layer = resource.slug();

    // Tile envelope is in EPSG:3857; the view geometry is EPSG:25833,
    // so the `&&` index filter transforms the envelope back to 25833
    // for the GIST hit, then ST_AsMVTGeom projects the matched
    // geometries forward to 3857 for clipping into the tile.
    let sql = format!(
        r#"
        WITH bounds_3857 AS (
            SELECT ST_TileEnvelope($1::int, $2::int, $3::int) AS env
        ),
        bounds_25833 AS (
            SELECT ST_Transform(env, 25833) AS env FROM bounds_3857
        ),
        mvtgeom AS (
            SELECT
                ST_AsMVTGeom(
                    ST_Transform(v.geom, 3857),
                    (SELECT env FROM bounds_3857),
                    4096, 64, true
                ) AS geom,
                {cols}
            FROM {view} v
            WHERE v.geom && (SELECT env FROM bounds_25833)
        )
        SELECT COALESCE(ST_AsMVT(mvtgeom.*, '{layer}', 4096, 'geom', 'id'), ''::bytea)
        FROM mvtgeom
        WHERE geom IS NOT NULL
        "#,
    );

    let (bytes,): (Vec<u8>,) = sqlx::query_as(&sql)
        .bind(coord.z as i32)
        .bind(coord.x as i32)
        .bind(coord.y as i32)
        .fetch_one(pool)
        .await?;
    Ok(bytes)
}
