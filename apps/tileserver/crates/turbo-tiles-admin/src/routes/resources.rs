use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use turbo_tiles_auth::{Curator, RequireRole};
use uuid::Uuid;

use crate::error::AdminError;
use crate::state::AdminState;

/// One row in the admin resource list. Carries enough for the table
/// row plus filtering — geometry is fetched on demand in detail/edit.
#[derive(Debug, Serialize)]
pub struct RouteRow {
    pub id: Uuid,
    pub resource: String,
    pub slug: String,
    pub name: Option<String>,
    pub difficulty: Option<String>,
    pub length_m: Option<f64>,
    pub status: String,
    pub source: String,
    pub needs_review: bool,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

pub async fn summary(
    _: RequireRole<Curator>,
    State(state): State<AdminState>,
) -> Result<Json<Value>, AdminError> {
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        r#"
        SELECT resource, status::text, COUNT(*) AS n
        FROM paths.curated_route
        GROUP BY resource, status
        ORDER BY resource, status
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    let mut by_resource: std::collections::BTreeMap<String, serde_json::Map<String, Value>> =
        Default::default();
    for (resource, status, n) in rows {
        let entry = by_resource.entry(resource).or_default();
        entry.insert(status, Value::from(n));
    }
    let body: Value = by_resource
        .into_iter()
        .map(|(k, v)| (k, Value::Object(v)))
        .collect::<serde_json::Map<_, _>>()
        .into();

    Ok(Json(serde_json::json!({ "resources": body })))
}

pub async fn list(
    _: RequireRole<Curator>,
    State(state): State<AdminState>,
    Path(resource): Path<String>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, AdminError> {
    validate_resource(&resource)?;

    let limit = q.limit.clamp(1, 200);
    let rows: Vec<RouteRow> = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<f64>,
            String,
            String,
            bool,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        r#"
        SELECT id, resource, slug, name, difficulty, length_m,
               status::text, source, needs_review, updated_at
        FROM paths.curated_route
        WHERE resource = $1
          AND ($2::text IS NULL OR status::text = $2)
          AND ($3::text IS NULL OR source = $3)
          AND ($4::text IS NULL OR name ILIKE '%' || $4 || '%')
        ORDER BY updated_at DESC
        LIMIT $5 OFFSET $6
        "#,
    )
    .bind(&resource)
    .bind(&q.status)
    .bind(&q.source)
    .bind(&q.q)
    .bind(limit)
    .bind(q.offset)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|r| RouteRow {
        id: r.0,
        resource: r.1,
        slug: r.2,
        name: r.3,
        difficulty: r.4,
        length_m: r.5,
        status: r.6,
        source: r.7,
        needs_review: r.8,
        updated_at: r.9,
    })
    .collect();

    let total: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM paths.curated_route WHERE resource = $1")
            .bind(&resource)
            .fetch_one(&state.db)
            .await?;

    Ok(Json(serde_json::json!({
        "rows": rows,
        "total": total.0,
        "limit": limit,
        "offset": q.offset,
    })))
}

pub async fn detail(
    _: RequireRole<Curator>,
    State(state): State<AdminState>,
    Path((resource, id)): Path<(String, Uuid)>,
) -> Result<Json<Value>, AdminError> {
    validate_resource(&resource)?;
    // sqlx tuple FromRow caps at 16, so we fetch as an untyped row and
    // pull each column individually. Less concise but avoids inventing
    // a struct just for one read.
    use sqlx::Row;
    let row = sqlx::query(
        r#"
        SELECT id, resource, slug, name, description, difficulty,
               marking, season, surface, attribution,
               length_m, elevation_gain_m, elevation_loss_m,
               status::text AS status, source, external_id, needs_review,
               created_at, updated_at,
               ST_AsGeoJSON(ST_Transform(geom, 4326))::jsonb AS geometry
        FROM paths.curated_route
        WHERE resource = $1 AND id = $2
        "#,
    )
    .bind(&resource)
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AdminError::NotFound)?;

    let id: Uuid = row.try_get("id")?;
    let resource: String = row.try_get("resource")?;
    let slug: String = row.try_get("slug")?;
    let name: Option<String> = row.try_get("name")?;
    let description: Option<String> = row.try_get("description")?;
    let difficulty: Option<String> = row.try_get("difficulty")?;
    let marking: Option<String> = row.try_get("marking")?;
    let season: Vec<String> = row.try_get("season")?;
    let surface: Option<String> = row.try_get("surface")?;
    let attribution: Option<String> = row.try_get("attribution")?;
    let length_m: Option<f64> = row.try_get("length_m")?;
    let elevation_gain_m: Option<f64> = row.try_get("elevation_gain_m")?;
    let elevation_loss_m: Option<f64> = row.try_get("elevation_loss_m")?;
    let status: String = row.try_get("status")?;
    let source: String = row.try_get("source")?;
    let external_id: Option<String> = row.try_get("external_id")?;
    let needs_review: bool = row.try_get("needs_review")?;
    let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;
    let updated_at: chrono::DateTime<chrono::Utc> = row.try_get("updated_at")?;
    let geometry: Value = row.try_get("geometry")?;

    Ok(Json(serde_json::json!({
        "id": id, "resource": resource, "slug": slug, "name": name,
        "description": description, "difficulty": difficulty, "marking": marking,
        "season": season, "surface": surface, "attribution": attribution,
        "length_m": length_m, "elevation_gain_m": elevation_gain_m,
        "elevation_loss_m": elevation_loss_m, "status": status, "source": source,
        "external_id": external_id, "needs_review": needs_review,
        "created_at": created_at, "updated_at": updated_at, "geometry": geometry,
    })))
}

#[derive(Debug, Deserialize)]
pub struct CreateBody {
    pub slug: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub difficulty: Option<String>,
    pub marking: Option<String>,
    #[serde(default)]
    pub season: Vec<String>,
    pub surface: Option<String>,
    /// GeoJSON `MultiLineString` or `LineString`, WGS84.
    pub geometry: Value,
    #[serde(default)]
    pub attribution: Option<String>,
}

pub async fn create(
    RequireRole { claims, .. }: RequireRole<Curator>,
    State(state): State<AdminState>,
    Path(resource): Path<String>,
    Json(body): Json<CreateBody>,
) -> Result<Json<Value>, AdminError> {
    validate_resource(&resource)?;
    if body.slug.is_empty() {
        return Err(AdminError::BadRequest("slug must not be empty".into()));
    }
    let geom_json = body.geometry.to_string();
    let row: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO paths.curated_route (
            resource, slug, name, description, difficulty, marking,
            season, surface, geom, source, status, attribution, created_by
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8,
            ST_Multi(ST_Transform(ST_GeomFromGeoJSON($9), 25833)),
            'manual', 'draft', $10, $11
        )
        RETURNING id
        "#,
    )
    .bind(&resource)
    .bind(&body.slug)
    .bind(&body.name)
    .bind(&body.description)
    .bind(&body.difficulty)
    .bind(&body.marking)
    .bind(&body.season)
    .bind(&body.surface)
    .bind(&geom_json)
    .bind(&body.attribution)
    .bind(claims.sub)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(serde_json::json!({ "id": row.0 })))
}

#[derive(Debug, Deserialize)]
pub struct UpdateBody {
    pub name: Option<String>,
    pub description: Option<String>,
    pub difficulty: Option<String>,
    pub marking: Option<String>,
    pub season: Option<Vec<String>>,
    pub surface: Option<String>,
    pub status: Option<String>,
    pub attribution: Option<String>,
}

pub async fn update(
    _: RequireRole<Curator>,
    State(state): State<AdminState>,
    Path((resource, id)): Path<(String, Uuid)>,
    Json(body): Json<UpdateBody>,
) -> Result<Json<Value>, AdminError> {
    validate_resource(&resource)?;
    if let Some(s) = &body.status {
        if !matches!(s.as_str(), "draft" | "published" | "archived") {
            return Err(AdminError::BadRequest(format!("invalid status `{s}`")));
        }
    }
    let res = sqlx::query(
        r#"
        UPDATE paths.curated_route
        SET name = COALESCE($3, name),
            description = COALESCE($4, description),
            difficulty = COALESCE($5, difficulty),
            marking = COALESCE($6, marking),
            season = COALESCE($7, season),
            surface = COALESCE($8, surface),
            status = COALESCE($9::paths.route_status, status),
            attribution = COALESCE($10, attribution)
        WHERE resource = $1 AND id = $2
        "#,
    )
    .bind(&resource)
    .bind(id)
    .bind(&body.name)
    .bind(&body.description)
    .bind(&body.difficulty)
    .bind(&body.marking)
    .bind(&body.season)
    .bind(&body.surface)
    .bind(&body.status)
    .bind(&body.attribution)
    .execute(&state.db)
    .await?;

    if res.rows_affected() == 0 {
        return Err(AdminError::NotFound);
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn archive(
    _: RequireRole<Curator>,
    State(state): State<AdminState>,
    Path((resource, id)): Path<(String, Uuid)>,
) -> Result<Json<Value>, AdminError> {
    validate_resource(&resource)?;
    let res = sqlx::query(
        "UPDATE paths.curated_route SET status = 'archived' WHERE resource = $1 AND id = $2",
    )
    .bind(&resource)
    .bind(id)
    .execute(&state.db)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AdminError::NotFound);
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

fn validate_resource(s: &str) -> Result<(), AdminError> {
    match s {
        "hiking-trails" | "ski-tracks" | "forest-roads" | "cycling-routes" => Ok(()),
        other => Err(AdminError::BadRequest(format!(
            "unknown resource `{other}`"
        ))),
    }
}
