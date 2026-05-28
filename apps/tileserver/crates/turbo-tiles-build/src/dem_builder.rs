//! `norway.dem` builder, format v2.
//!
//! v1 forced a global tile grid and rejected any source raster whose
//! upperleft wasn't on it. With real `raster2pgsql -t 256x256` data
//! that's everything: each input GeoTIFF starts at its own origin.
//!
//! v2 stores one tile per source raster *at its own (ulx, uly)*.
//! Reader builds an rstar over tile bboxes at open() and answers
//! each lookup by finding the tile that contains the query point.
//!
//! Streaming pipeline:
//!   - Stream `paths.dem` rows ordered by rid.
//!   - For each row, decode the f32 array, compress to zstd, append.
//!   - Track per-tile (ulx, uly, offset, compressed_size).
//!   - At the end, rewrite the tile directory in place.

use std::fs::OpenOptions;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use byteorder::{LittleEndian, WriteBytesExt};
use chrono::Utc;
use futures::StreamExt;
use serde::Serialize;
use sqlx::Row;
use tracing::{info, warn};
use turbo_tiles_artifacts::{write_header, ArtifactKind, Header, HEADER_BYTES};
use turbo_tiles_db::DbPool;
use turbo_tiles_elev::{
    write_meta, DemMeta, TileEntry, COMPRESSION_ZSTD, DEFAULT_TILE_CELLS, DEM_FORMAT_VERSION,
    DEM_META_BYTES, NODATA_SENTINEL, TILE_ENTRY_BYTES,
};

use crate::BuildError;

#[derive(Debug, Clone, Serialize)]
pub struct DemBuildReport {
    pub out_path: PathBuf,
    pub tiles_written: u64,
    pub source_rows: u64,
    pub tiles_skipped_wrong_size: u64,
    pub tiles_skipped_null: u64,
    pub uncompressed_bytes: u64,
    pub compressed_bytes: u64,
    pub file_size_bytes: u64,
    pub seconds: f64,
    /// Coverage stats + sparseness warnings. See [`crate::health::audit_dem_coverage`].
    pub health: crate::health::HealthReport,
}

pub async fn build(pool: &DbPool, out_dir: &Path) -> Result<DemBuildReport, BuildError> {
    let started = Instant::now();
    std::fs::create_dir_all(out_dir)?;

    // ---- 1. Probe paths.dem for shape -----------------------------------------
    let count_row: (i64,) = sqlx::query_as("SELECT COUNT(*)::bigint FROM paths.dem")
        .fetch_one(pool)
        .await?;
    let source_rows = count_row.0 as u64;
    if source_rows == 0 {
        return Err(BuildError::Logic(
            "paths.dem is empty — run dtm-bulk-load first".into(),
        ));
    }

    let probe = sqlx::query(
        r#"
        SELECT
          ST_SRID(rast)            AS srid,
          ST_PixelWidth(rast)      AS px_w,
          ST_PixelHeight(rast)     AS px_h,
          ST_Width(rast)           AS w,
          ST_Height(rast)          AS h,
          ST_BandNoDataValue(rast, 1) AS nd
        FROM paths.dem
        LIMIT 1
        "#,
    )
    .fetch_one(pool)
    .await?;
    let srid: i32 = probe.try_get("srid")?;
    let px_w: f64 = probe.try_get("px_w")?;
    let px_h: f64 = probe.try_get("px_h")?;
    let src_w: i32 = probe.try_get("w")?;
    let src_h: i32 = probe.try_get("h")?;
    let src_nodata: Option<f64> = probe.try_get("nd").ok();
    let src_nodata = src_nodata.unwrap_or(NODATA_SENTINEL as f64) as f32;
    if srid != 25833 {
        return Err(BuildError::Logic(format!(
            "paths.dem rows must be EPSG:25833, got {srid}"
        )));
    }
    if (px_w - 10.0).abs() > 0.001 || (px_h.abs() - 10.0).abs() > 0.001 {
        return Err(BuildError::Logic(format!(
            "paths.dem must be 10 m pixels, got {px_w}×{px_h}"
        )));
    }
    if src_w != DEFAULT_TILE_CELLS as i32 || src_h != DEFAULT_TILE_CELLS as i32 {
        return Err(BuildError::Logic(format!(
            "paths.dem must be {tc}×{tc} tiles, got {src_w}×{src_h}",
            tc = DEFAULT_TILE_CELLS
        )));
    }
    let tile_cells: u32 = DEFAULT_TILE_CELLS;
    let pixel_size_m: f32 = px_w as f32;

    info!(
        source_rows,
        tile_cells, pixel_size_m, "DEM v2 build starting"
    );

    // ---- 2. Open output file --------------------------------------------------
    let out_path = out_dir.join(ArtifactKind::Dem.filename());
    let tmp_path = out_dir.join(format!("{}.tmp", ArtifactKind::Dem.filename()));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp_path)?;
    let mut w = BufWriter::with_capacity(8 * 1024 * 1024, file);

    let header = Header {
        kind: ArtifactKind::Dem,
        format_version: DEM_FORMAT_VERSION,
        build_timestamp_unix_sec: Utc::now().timestamp(),
    };
    write_header(&mut w, &header)?;

    // Write meta with the source_rows count up front, then the
    // placeholder directory, then payloads. The actual tile_count
    // may be less if any source rows get skipped — we rewrite the
    // meta at the end.
    let meta_placeholder = DemMeta {
        tile_count: source_rows as u32,
        tile_cells,
        pixel_size_m,
        nodata: NODATA_SENTINEL,
        compression: COMPRESSION_ZSTD,
    };
    write_meta(&mut w, &meta_placeholder)?;
    let dir_offset = (HEADER_BYTES + DEM_META_BYTES) as u64;
    let dir_bytes = source_rows as usize * TILE_ENTRY_BYTES;
    {
        let zeros = vec![0u8; dir_bytes];
        w.write_all(&zeros)?;
    }

    let mut dir: Vec<TileEntry> = Vec::with_capacity(source_rows as usize);
    let mut payload_offset: u64 = dir_offset + dir_bytes as u64;

    // ---- 3. Stream + append payloads ------------------------------------------
    let mut rows_q = sqlx::query(
        r#"
        SELECT
          ST_UpperLeftX(rast) AS ulx,
          ST_UpperLeftY(rast) AS uly,
          ST_Width(rast)::int4  AS w,
          ST_Height(rast)::int4 AS h,
          ST_BandNoDataValue(rast, 1) AS nd,
          COALESCE(ARRAY(SELECT unnest(ST_DumpValues(rast, 1))), '{}'::float8[]) AS vals
        FROM paths.dem
        ORDER BY rid
        "#,
    )
    .fetch(pool);

    let need = (tile_cells * tile_cells) as usize;
    let mut tiles_written = 0u64;
    let mut tiles_skipped_wrong_size = 0u64;
    let mut tiles_skipped_null = 0u64;
    let mut uncompressed_bytes = 0u64;
    let mut compressed_bytes = 0u64;
    while let Some(row) = rows_q.next().await {
        let row = row?;
        let ulx: f64 = row.try_get("ulx")?;
        let uly: f64 = row.try_get("uly")?;
        let w_int: i32 = row.try_get("w")?;
        let h_int: i32 = row.try_get("h")?;
        let nd_opt: Option<f64> = row.try_get("nd").ok();
        let nd = nd_opt.unwrap_or(src_nodata as f64) as f32;
        // PostGIS represents nodata cells as SQL NULL inside the
        // value array. sqlx needs `Vec<Option<f64>>` to decode that
        // — `Vec<f64>` chokes on any NULL element. We map NULL →
        // nodata sentinel in the loop below.
        let vals_opt: Option<Vec<Option<f64>>> = row.try_get("vals")?;
        let Some(vals_f64) = vals_opt else {
            tiles_skipped_null += 1;
            continue;
        };

        // Source rasters at GeoTIFF edges may be narrower than 256
        // cells — accept them but pad to 256×256 with nodata so the
        // on-disk format stays uniform. Tiles with height < 256 use
        // padding on the bottom rows.
        if w_int <= 0 || h_int <= 0 || w_int > tile_cells as i32 || h_int > tile_cells as i32 {
            tiles_skipped_wrong_size += 1;
            warn!(w_int, h_int, "skipping source raster: dims out of range");
            continue;
        }
        let src_cols = w_int as usize;
        let src_rows = h_int as usize;
        // Pad source array (which is src_rows × src_cols, row-major)
        // into our fixed 256 × 256 tile array.
        let mut floats: Vec<f32> = vec![NODATA_SENTINEL; need];
        let tc = tile_cells as usize;
        for r in 0..src_rows {
            for c in 0..src_cols {
                // NULL (None) → nodata; specific nodata sentinel from
                // the raster header → nodata; NaN → nodata; else use.
                let out = match vals_f64.get(r * src_cols + c).copied() {
                    Some(Some(v)) => {
                        let f = v as f32;
                        if f.is_nan() || f == nd {
                            NODATA_SENTINEL
                        } else {
                            f
                        }
                    }
                    _ => NODATA_SENTINEL,
                };
                floats[r * tc + c] = out;
            }
        }
        let raw_bytes: &[u8] = bytemuck::cast_slice(&floats);
        let compressed = zstd::encode_all(raw_bytes, /*level*/ 6)
            .map_err(|e| BuildError::Logic(format!("zstd encode: {e}")))?;
        let comp_len = compressed.len() as u32;
        w.write_all(&compressed)?;
        dir.push(TileEntry {
            ulx,
            uly,
            offset: payload_offset,
            compressed_size: comp_len,
        });
        payload_offset += comp_len as u64;
        uncompressed_bytes += raw_bytes.len() as u64;
        compressed_bytes += comp_len as u64;
        tiles_written += 1;
        if tiles_written % 1000 == 0 {
            info!(tiles_written, "DEM build progress");
        }
    }
    drop(rows_q);

    // ---- 4. Rewrite meta + tile directory in place --------------------------
    w.flush()?;
    let mut file = w.into_inner().map_err(|e| BuildError::Io(e.into_error()))?;

    // Re-stamp meta with the actual tile_count.
    file.seek(SeekFrom::Start(HEADER_BYTES as u64))?;
    let final_meta = DemMeta {
        tile_count: tiles_written as u32,
        tile_cells,
        pixel_size_m,
        nodata: NODATA_SENTINEL,
        compression: COMPRESSION_ZSTD,
    };
    {
        let mut buf = Vec::with_capacity(DEM_META_BYTES);
        write_meta(&mut buf, &final_meta)?;
        debug_assert_eq!(buf.len(), DEM_META_BYTES);
        file.write_all(&buf)?;
    }

    // Rewrite directory.
    file.seek(SeekFrom::Start(dir_offset))?;
    {
        let mut buf = Vec::with_capacity(dir.len() * TILE_ENTRY_BYTES);
        for e in &dir {
            buf.write_f64::<LittleEndian>(e.ulx)?;
            buf.write_f64::<LittleEndian>(e.uly)?;
            buf.write_u64::<LittleEndian>(e.offset)?;
            buf.write_u32::<LittleEndian>(e.compressed_size)?;
            buf.write_all(&[0u8; 4])?;
        }
        file.write_all(&buf)?;
    }
    file.sync_all()?;
    drop(file);

    std::fs::rename(&tmp_path, &out_path)?;
    let file_size_bytes = std::fs::metadata(&out_path)?.len();

    // Audit by re-opening the artifact we just wrote — that
    // exercises the same load path the runtime uses, so a
    // build/read mismatch surfaces immediately.
    let health = match turbo_tiles_elev::Dem::open(&out_path) {
        Ok(d) => crate::health::audit_dem_coverage(&d.coverage()),
        Err(e) => {
            let mut h = crate::health::HealthReport::default();
            h.error(
                "dem_open_failed",
                format!("audit could not open the just-written DEM: {e}"),
                Some("artifact format mismatch — check write/read code parity"),
            );
            h
        }
    };
    for w in &health.warnings {
        tracing::warn!(code = %w.code, "{}", w.message);
    }
    for e in &health.errors {
        tracing::error!(code = %e.code, "{}", e.message);
    }
    let health_path = out_dir.join("norway.dem.health.json");
    let body = serde_json::to_vec_pretty(&serde_json::json!({
        "written_at_unix_sec": chrono::Utc::now().timestamp(),
        "report": &health,
    }))
    .unwrap_or_default();
    let _ = std::fs::write(&health_path, body);

    Ok(DemBuildReport {
        out_path,
        tiles_written,
        source_rows,
        tiles_skipped_wrong_size,
        tiles_skipped_null,
        uncompressed_bytes,
        compressed_bytes,
        file_size_bytes,
        seconds: started.elapsed().as_secs_f64(),
        health,
    })
}
