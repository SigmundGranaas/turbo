use serde_json::Value;
use turbo_tiles_core::bbox::Bbox;
use turbo_tiles_core::resource::Resource;
use turbo_tiles_db::DbPool;

/// One row returned by the GeoJSON list endpoint. `geometry` is already
/// a GeoJSON object (`ST_AsGeoJSON(geom)::jsonb`) and `properties` is a
/// `jsonb_build_object(...)` aggregating the per-resource attribute
/// columns. The HTTP layer just wraps these into a FeatureCollection.
#[derive(Debug)]
pub struct FeatureRow {
    pub id: String,
    pub geometry: Value,
    pub properties: Value,
}

pub async fn list_by_bbox(
    pool: &DbPool,
    resource: Resource,
    bbox: Bbox,
    limit: i64,
) -> Result<Vec<FeatureRow>, sqlx::Error> {
    let view = resource.view();
    let sql = format!(
        r#"
        WITH env AS (
            SELECT ST_Transform(
                ST_MakeEnvelope($1::float8, $2::float8, $3::float8, $4::float8, 4326),
                25833
            ) AS g
        )
        SELECT
            v.id::text AS id,
            ST_AsGeoJSON(ST_Transform(v.geom, 4326))::jsonb AS geometry,
            jsonb_build_object(
                'name', v.name,
                'difficulty', v.difficulty,
                'length_m', v.length_m,
                'elevation_gain_m', v.elevation_gain_m,
                'marking', v.marking,
                'surface', v.surface,
                'season', v.season
            ) AS properties
        FROM {view} v, env
        WHERE v.geom && env.g
        ORDER BY v.length_m DESC NULLS LAST
        LIMIT $5
        "#,
    );

    let rows: Vec<(String, Value, Value)> = sqlx::query_as(&sql)
        .bind(bbox.west)
        .bind(bbox.south)
        .bind(bbox.east)
        .bind(bbox.north)
        .bind(limit)
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|(id, geometry, properties)| FeatureRow {
            id,
            geometry,
            properties,
        })
        .collect())
}

pub async fn feature_by_id(
    pool: &DbPool,
    resource: Resource,
    id: &str,
) -> Result<Option<FeatureRow>, sqlx::Error> {
    let view = resource.view();
    let sql = format!(
        r#"
        SELECT
            v.id::text AS id,
            ST_AsGeoJSON(ST_Transform(v.geom, 4326))::jsonb AS geometry,
            jsonb_build_object(
                'name', v.name,
                'description', v.description,
                'difficulty', v.difficulty,
                'length_m', v.length_m,
                'elevation_gain_m', v.elevation_gain_m,
                'elevation_loss_m', v.elevation_loss_m,
                'marking', v.marking,
                'surface', v.surface,
                'season', v.season,
                'source', v.source,
                'attribution', v.attribution
            ) AS properties
        FROM {view} v
        WHERE v.id::text = $1
        LIMIT 1
        "#,
    );

    let row: Option<(String, Value, Value)> =
        sqlx::query_as(&sql).bind(id).fetch_optional(pool).await?;

    Ok(row.map(|(id, geometry, properties)| FeatureRow {
        id,
        geometry,
        properties,
    }))
}
