//! `norway.anchors` builder. Reads `anchors.anchor` and emits the
//! search artifact consumed by `turbo-tiles-search`.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::Utc;
use futures::TryStreamExt;
use serde::Serialize;
use sqlx::Row;
use tracing::info;
use turbo_tiles_artifacts::{write_header, ArtifactKind, Header};
use turbo_tiles_db::DbPool;
use turbo_tiles_search::{
    write_meta, AnchorKind, AnchorRecord, AnchorsMeta, ANCHOR_RECORD_BYTES, SEARCH_FORMAT_VERSION,
};

use crate::BuildError;

#[derive(Debug, Clone, Serialize)]
pub struct SearchBuildReport {
    pub out_path: PathBuf,
    pub anchors: u32,
    pub names_bytes: u64,
    pub file_size_bytes: u64,
    pub seconds: f64,
}

pub async fn build(pool: &DbPool, out_dir: &Path) -> Result<SearchBuildReport, BuildError> {
    let started = Instant::now();
    std::fs::create_dir_all(out_dir)?;

    let mut rows = sqlx::query(
        r#"
        SELECT
          id::bigint                            AS id,
          kind                                  AS kind_text,
          ST_X(geom)::float8                    AS x,
          ST_Y(geom)::float8                    AS y,
          COALESCE(elevation_m, 0.0)            AS elev,
          name                                  AS name
        FROM anchors.anchor
        ORDER BY id
        "#,
    )
    .fetch(pool);

    let mut records: Vec<AnchorRecord> = Vec::new();
    let mut names_blob: Vec<u8> = Vec::new();
    while let Some(row) = rows.try_next().await? {
        let id: i64 = row.try_get("id")?;
        let kind_text: String = row.try_get::<String, _>("kind_text").unwrap_or_default();
        let x: f64 = row.try_get("x")?;
        let y: f64 = row.try_get("y")?;
        let elev: f64 = row.try_get("elev")?;
        let name: Option<String> = row.try_get("name").ok();
        let (name_off, name_len) = match name.as_deref() {
            Some(n) if !n.is_empty() => {
                let off = names_blob.len() as u32;
                names_blob.extend_from_slice(n.as_bytes());
                (off, n.len() as u32)
            }
            _ => (0, 0),
        };
        records.push(AnchorRecord {
            id: id as u64,
            kind: AnchorKind::from_text(&kind_text) as u32,
            x: x as f32,
            y: y as f32,
            elev_m: elev as f32,
            name_off,
            name_len,
        });
    }
    drop(rows);
    info!(
        anchors = records.len(),
        names_bytes = names_blob.len(),
        "loaded anchors"
    );

    let out_path = out_dir.join(ArtifactKind::Anchors.filename());
    let tmp_path = out_dir.join(format!("{}.tmp", ArtifactKind::Anchors.filename()));
    let f = File::create(&tmp_path)?;
    let mut w = BufWriter::with_capacity(8 * 1024 * 1024, f);
    write_header(
        &mut w,
        &Header {
            kind: ArtifactKind::Anchors,
            format_version: SEARCH_FORMAT_VERSION,
            build_timestamp_unix_sec: Utc::now().timestamp(),
        },
    )?;
    write_meta(
        &mut w,
        &AnchorsMeta {
            count: records.len() as u32,
            names_size: names_blob.len() as u64,
        },
    )?;
    debug_assert_eq!(std::mem::size_of::<AnchorRecord>(), ANCHOR_RECORD_BYTES);
    w.write_all(bytemuck::cast_slice(&records))?;
    w.write_all(&names_blob)?;
    w.flush()?;
    drop(w);
    std::fs::rename(&tmp_path, &out_path)?;
    let file_size_bytes = std::fs::metadata(&out_path)?.len();

    Ok(SearchBuildReport {
        out_path,
        anchors: records.len() as u32,
        names_bytes: names_blob.len() as u64,
        file_size_bytes,
        seconds: started.elapsed().as_secs_f64(),
    })
}
