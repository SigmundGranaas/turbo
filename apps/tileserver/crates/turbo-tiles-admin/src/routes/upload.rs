use axum::extract::{Multipart, State};
use axum::Json;
use gpx::Gpx;
use serde_json::{json, Value};
use turbo_tiles_auth::{Curator, RequireRole};

use crate::error::AdminError;
use crate::state::AdminState;

/// Multipart form with a single `file` field carrying a GPX document.
/// Optional `resource` and `name` fields pre-fill the draft. The
/// returned id can be fed straight into the edit screen for the
/// curator to fill in difficulty/marking/etc.
pub async fn upload_gpx(
    RequireRole { claims, .. }: RequireRole<Curator>,
    State(state): State<AdminState>,
    mut form: Multipart,
) -> Result<Json<Value>, AdminError> {
    let mut bytes: Option<bytes::Bytes> = None;
    let mut resource = "hiking-trails".to_string();
    let mut name: Option<String> = None;

    while let Some(field) = form
        .next_field()
        .await
        .map_err(|e| AdminError::Upload(e.to_string()))?
    {
        let field_name = field.name().unwrap_or("").to_string();
        match field_name.as_str() {
            "file" => {
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| AdminError::Upload(e.to_string()))?;
                if data.len() > 25 * 1024 * 1024 {
                    return Err(AdminError::Upload("file exceeds 25 MB".into()));
                }
                bytes = Some(data);
            }
            "resource" => {
                let v = field
                    .text()
                    .await
                    .map_err(|e| AdminError::Upload(e.to_string()))?;
                resource = v;
            }
            "name" => {
                let v = field
                    .text()
                    .await
                    .map_err(|e| AdminError::Upload(e.to_string()))?;
                if !v.is_empty() {
                    name = Some(v);
                }
            }
            _ => {}
        }
    }

    let bytes = bytes.ok_or_else(|| AdminError::Upload("missing `file` part".into()))?;
    let gpx: Gpx = gpx::read(std::io::Cursor::new(&bytes))
        .map_err(|e| AdminError::Upload(format!("invalid GPX: {e}")))?;

    // Concatenate all track segments into one MultiLineString.
    let mut lines = Vec::new();
    for track in &gpx.tracks {
        for seg in &track.segments {
            if seg.points.len() < 2 {
                continue;
            }
            let coords: Vec<Vec<f64>> = seg
                .points
                .iter()
                .map(|wpt| {
                    let p = wpt.point();
                    vec![p.x(), p.y()]
                })
                .collect();
            lines.push(coords);
        }
    }
    if lines.is_empty() {
        return Err(AdminError::Upload(
            "GPX has no usable track segments".into(),
        ));
    }

    let derived_name = name
        .or_else(|| gpx.tracks.first().and_then(|t| t.name.clone()))
        .unwrap_or_else(|| "Imported GPX".to_string());
    let slug = slugify(&derived_name);

    let geom_geojson = serde_json::json!({
        "type": "MultiLineString",
        "coordinates": lines,
    });

    let row: (uuid::Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO paths.curated_route (
            resource, slug, name, geom, source, status, created_by
        )
        VALUES (
            $1, $2, $3,
            ST_Multi(ST_Transform(ST_GeomFromGeoJSON($4), 25833)),
            'gpx-import', 'draft', $5
        )
        RETURNING id
        "#,
    )
    .bind(&resource)
    .bind(&slug)
    .bind(&derived_name)
    .bind(geom_geojson.to_string())
    .bind(claims.sub)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(json!({
        "id": row.0,
        "resource": resource,
        "name": derived_name,
        "slug": slug,
    })))
}

fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = true;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("untitled");
    }
    out
}
