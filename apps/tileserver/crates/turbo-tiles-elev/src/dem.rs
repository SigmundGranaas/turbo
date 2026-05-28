//! DEM artifact reader: mmap + LRU tile cache + rstar tile index.
//!
//! Format v2 — each tile stores its own (ulx, uly), so source
//! rasters from differently-aligned input GeoTIFFs all land in the
//! same artifact. Reader builds an rstar of tile bboxes at open()
//! and answers each sample by finding the tile that contains the
//! query point.

use std::fs::File;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use memmap2::Mmap;
use rstar::{RTree, RTreeObject, AABB};
use thiserror::Error;
use turbo_tiles_artifacts::{check_header, read_header, ArtifactKind, HEADER_BYTES};

use crate::cache::{CacheStats, TileCache, TileId};
use crate::format::{
    read_meta, read_tile_entry, DemMeta, TileEntry, DEM_FORMAT_VERSION, DEM_META_BYTES,
    TILE_ENTRY_BYTES,
};

/// Default tile cache size — bumped from 128 MB so an interactive
/// admin session over full-Norway DEM tiles doesn't thrash on
/// adjacent queries. Tunable via `Dem::open_with_cache` or the
/// `TILESERVER_DEM_CACHE_BYTES` env var read by the bin.
pub const DEFAULT_CACHE_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum DemError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("artifact: {0}")]
    Artifact(#[from] turbo_tiles_artifacts::ArtifactError),
    #[error("malformed DEM: {0}")]
    Malformed(&'static str),
    #[error("zstd: {0}")]
    Zstd(String),
    #[error("point out of coverage: x={x:.1} y={y:.1}")]
    OutOfCoverage { x: f64, y: f64 },
}

/// 2D point in EPSG:25833 metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointXY {
    pub x: f64,
    pub y: f64,
}

/// Slope (degrees from horizontal) + aspect (degrees, 0=N, clockwise).
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct SlopeAspect {
    pub slope_deg: f32,
    pub aspect_deg: f32,
}

/// Coverage / diagnostic info for the loaded DEM. Returned by
/// `/v1/debug/elev/coverage`. With per-tile origins there's no
/// single bbox; report the union of all tile bboxes plus per-tile
/// counts so curators can see how the data is distributed.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DemCoverage {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
    /// Approximated total cell count: tile_count × tile_cells².
    pub cells_x: u32,
    pub cells_y: u32,
    pub resolution_m: f32,
    pub nodata: f32,
    pub tile_cells: u32,
    /// Synthesised for UI compatibility: tile_count split across
    /// the union bbox at the tile_size grid. The artifact does NOT
    /// store a regular grid any more.
    pub tiles_x: u32,
    pub tiles_y: u32,
    pub tiles_present: u32,
    pub tiles_absent: u32,
    pub file_size_bytes: u64,
    pub build_timestamp_unix_sec: i64,
    pub cache: CacheStats,
}

#[derive(Debug, Clone, Copy)]
struct IndexedTile {
    /// Index into `Dem::tile_dir`.
    idx: u32,
    ulx: f64,
    uly: f64,
    tile_size_m: f64,
}

impl RTreeObject for IndexedTile {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        let min_y = self.uly - self.tile_size_m;
        let max_x = self.ulx + self.tile_size_m;
        AABB::from_corners([self.ulx, min_y], [max_x, self.uly])
    }
}

pub struct Dem {
    mmap: Mmap,
    meta: DemMeta,
    tile_dir: Vec<TileEntry>,
    /// rstar over tile bboxes — answers "which tile contains
    /// (x, y)?" in sub-50 µs even on 40 K tiles.
    tile_index: RTree<IndexedTile>,
    cache: TileCache,
    build_timestamp_unix_sec: i64,
    file_size_bytes: u64,
}

impl Dem {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, DemError> {
        Self::open_with_cache(path, DEFAULT_CACHE_BYTES)
    }

    pub fn open_with_cache<P: AsRef<Path>>(
        path: P,
        cache_bytes: usize,
    ) -> Result<Self, DemError> {
        let file = File::open(path.as_ref())?;
        let file_size_bytes = file.metadata()?.len();
        let mmap = unsafe { Mmap::map(&file)? };

        if mmap.len() < HEADER_BYTES + DEM_META_BYTES {
            return Err(DemError::Malformed("file shorter than header+meta"));
        }
        let mut cursor = Cursor::new(&mmap[..]);
        let header = read_header(&mut cursor)?;
        check_header(&header, ArtifactKind::Dem, DEM_FORMAT_VERSION)?;
        let meta = read_meta(&mut cursor)?;

        let dir_bytes = meta.tile_count as usize * TILE_ENTRY_BYTES;
        if mmap.len() < HEADER_BYTES + DEM_META_BYTES + dir_bytes {
            return Err(DemError::Malformed("file shorter than tile directory"));
        }
        let mut tile_dir: Vec<TileEntry> = Vec::with_capacity(meta.tile_count as usize);
        for _ in 0..meta.tile_count {
            tile_dir.push(read_tile_entry(&mut cursor)?);
        }

        // Bulk-load the spatial index. ~80 ms for 40 K tiles.
        let tile_size_m = meta.tile_cells as f64 * meta.pixel_size_m as f64;
        let indexed: Vec<IndexedTile> = tile_dir
            .iter()
            .enumerate()
            .map(|(i, e)| IndexedTile {
                idx: i as u32,
                ulx: e.ulx,
                uly: e.uly,
                tile_size_m,
            })
            .collect();
        let tile_index = RTree::bulk_load(indexed);

        Ok(Self {
            mmap,
            meta,
            tile_dir,
            tile_index,
            cache: TileCache::new(cache_bytes),
            build_timestamp_unix_sec: header.build_timestamp_unix_sec,
            file_size_bytes,
        })
    }

    pub fn meta(&self) -> &DemMeta {
        &self.meta
    }

    /// Sample elevation at an EPSG:25833 point via bilinear
    /// interpolation. Behaviour:
    ///   - No tile contains `p` ⇒ `Err(OutOfCoverage)` — the system
    ///     has no idea what's at this point.
    ///   - Tile exists but the v00 cell is nodata ⇒ `Ok(None)`.
    ///   - Bilinear neighbours sit outside the tile (the point is
    ///     within a half-pixel of the eastern/southern edge) ⇒ fall
    ///     back to the available neighbours rather than reporting
    ///     "no data" — the v00 cell value is the honest answer.
    pub fn sample(&self, p: PointXY) -> Result<Option<f32>, DemError> {
        let tile_for_p = match self.find_tile(p.x, p.y) {
            Some(t) => t,
            None => return Err(DemError::OutOfCoverage { x: p.x, y: p.y }),
        };
        let pixel = self.meta.pixel_size_m as f64;
        let v00 = self.cell_value_at(p.x, p.y)?;
        let Some(v00) = v00 else { return Ok(None) };
        // Neighbours: missing entries fall back to v00 so the
        // bilinear collapses to nearest-neighbour at tile edges.
        let v10 = self.cell_value_at(p.x + pixel, p.y)?.unwrap_or(v00);
        let v01 = self.cell_value_at(p.x, p.y - pixel)?.unwrap_or(v00);
        let v11 = self.cell_value_at(p.x + pixel, p.y - pixel)?.unwrap_or(v00);
        let col_in_tile = ((p.x - tile_for_p.ulx) / pixel).floor() as i64;
        let row_in_tile = ((tile_for_p.uly - p.y) / pixel).floor() as i64;
        let cell_origin_x = tile_for_p.ulx + col_in_tile as f64 * pixel;
        let cell_origin_y = tile_for_p.uly - row_in_tile as f64 * pixel;
        let fx = ((p.x - cell_origin_x) / pixel).clamp(0.0, 1.0) as f32;
        let fy = ((cell_origin_y - p.y) / pixel).clamp(0.0, 1.0) as f32;
        let top = v00 * (1.0 - fx) + v10 * fx;
        let bot = v01 * (1.0 - fx) + v11 * fx;
        Ok(Some(top * (1.0 - fy) + bot * fy))
    }

    /// Slope + aspect via Horn (1981) central differences over the
    /// 3×3 neighbourhood. Tolerant of tile boundaries (cells span
    /// the lookup naturally because `cell_value_at` resolves per
    /// world coordinate).
    pub fn slope_aspect(&self, p: PointXY) -> Result<Option<SlopeAspect>, DemError> {
        let pixel = self.meta.pixel_size_m as f64;
        // 3×3 neighbours centred at p, stepping by pixel size.
        let mut n = [[0.0f32; 3]; 3];
        for dy in -1..=1i32 {
            for dx in -1..=1i32 {
                let x = p.x + dx as f64 * pixel;
                let y = p.y + dy as f64 * pixel;
                match self.cell_value_at(x, y)? {
                    Some(v) => n[(dy + 1) as usize][(dx + 1) as usize] = v,
                    None => return Ok(None),
                }
            }
        }
        let res_f = pixel as f32;
        let dzdx = ((n[0][2] + 2.0 * n[1][2] + n[2][2])
            - (n[0][0] + 2.0 * n[1][0] + n[2][0]))
            / (8.0 * res_f);
        let dzdy_grid = ((n[2][0] + 2.0 * n[2][1] + n[2][2])
            - (n[0][0] + 2.0 * n[0][1] + n[0][2]))
            / (8.0 * res_f);
        // World-space gradient is north-positive; n[2] is one row
        // *south* of n[0] in world terms, so grid-space dz/dy needs
        // flipping.
        let dzdy = -dzdy_grid;
        let slope_rad = (dzdx * dzdx + dzdy * dzdy).sqrt().atan();
        let aspect_rad = (-dzdx).atan2(dzdy);
        let mut aspect_deg = aspect_rad.to_degrees();
        if aspect_deg < 0.0 {
            aspect_deg += 360.0;
        }
        Ok(Some(SlopeAspect {
            slope_deg: slope_rad.to_degrees(),
            aspect_deg,
        }))
    }

    pub fn profile(&self, points: &[PointXY]) -> Result<Vec<Option<f32>>, DemError> {
        let mut out = Vec::with_capacity(points.len());
        for p in points {
            match self.sample(*p) {
                Ok(v) => out.push(v),
                Err(DemError::OutOfCoverage { .. }) => out.push(None),
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    pub fn coverage(&self) -> DemCoverage {
        let tile_size_m = self.meta.tile_cells as f64 * self.meta.pixel_size_m as f64;
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for e in &self.tile_dir {
            min_x = min_x.min(e.ulx);
            max_x = max_x.max(e.ulx + tile_size_m);
            min_y = min_y.min(e.uly - tile_size_m);
            max_y = max_y.max(e.uly);
        }
        if !min_x.is_finite() {
            min_x = 0.0;
            min_y = 0.0;
            max_x = 0.0;
            max_y = 0.0;
        }
        let span_x = ((max_x - min_x) / self.meta.pixel_size_m as f64).round() as u32;
        let span_y = ((max_y - min_y) / self.meta.pixel_size_m as f64).round() as u32;
        let tiles_x = (span_x as f64 / self.meta.tile_cells as f64).ceil() as u32;
        let tiles_y = (span_y as f64 / self.meta.tile_cells as f64).ceil() as u32;
        let total = tiles_x as u64 * tiles_y as u64;
        let present = self.tile_dir.len() as u64;
        let absent = total.saturating_sub(present) as u32;
        DemCoverage {
            min_x,
            min_y,
            max_x,
            max_y,
            cells_x: span_x,
            cells_y: span_y,
            resolution_m: self.meta.pixel_size_m,
            nodata: self.meta.nodata,
            tile_cells: self.meta.tile_cells,
            tiles_x,
            tiles_y,
            tiles_present: present as u32,
            tiles_absent: absent,
            file_size_bytes: self.file_size_bytes,
            build_timestamp_unix_sec: self.build_timestamp_unix_sec,
            cache: self.cache.stats(),
        }
    }

    pub fn cache_stats(&self) -> CacheStats {
        self.cache.stats()
    }

    fn find_tile(&self, x: f64, y: f64) -> Option<&IndexedTile> {
        // rstar locate_in_envelope_intersecting: with point envelope,
        // returns tiles whose bbox covers the point. Most often
        // exactly one; we take the first (last-write-wins on overlap).
        let aabb = AABB::from_corners([x - 0.001, y - 0.001], [x + 0.001, y + 0.001]);
        self.tile_index
            .locate_in_envelope_intersecting(&aabb)
            .next()
    }

    fn cell_value_at(&self, x: f64, y: f64) -> Result<Option<f32>, DemError> {
        let tile = match self.find_tile(x, y) {
            Some(t) => t,
            None => return Ok(None),
        };
        let pixel = self.meta.pixel_size_m as f64;
        let col = ((x - tile.ulx) / pixel).floor() as i64;
        let row = ((tile.uly - y) / pixel).floor() as i64;
        let tc = self.meta.tile_cells as i64;
        if col < 0 || row < 0 || col >= tc || row >= tc {
            return Ok(None);
        }
        let payload = match self.cache.get(TileId(tile.idx, 0)) {
            Some(p) => p,
            None => self.load_tile(tile.idx)?,
        };
        let v = payload[(row as usize) * self.meta.tile_cells as usize + col as usize];
        if v == self.meta.nodata {
            Ok(None)
        } else {
            Ok(Some(v))
        }
    }

    fn load_tile(&self, idx: u32) -> Result<Arc<Vec<f32>>, DemError> {
        let tile_cells = self.meta.tile_cells as usize;
        let cells_per_tile = tile_cells * tile_cells;
        let entry = &self.tile_dir[idx as usize];
        let off = entry.offset as usize;
        let sz = entry.compressed_size as usize;
        if off + sz > self.mmap.len() {
            return Err(DemError::Malformed("tile payload past EOF"));
        }
        let compressed = &self.mmap[off..off + sz];
        let raw_bytes = zstd::decode_all(compressed)
            .map_err(|e| DemError::Zstd(e.to_string()))?;
        if raw_bytes.len() != cells_per_tile * std::mem::size_of::<f32>() {
            return Err(DemError::Malformed("decoded tile wrong size"));
        }
        let mut floats = vec![0f32; cells_per_tile];
        let bytes = bytemuck::cast_slice_mut::<f32, u8>(&mut floats);
        bytes.copy_from_slice(&raw_bytes);
        let payload = Arc::new(floats);
        self.cache.insert(TileId(idx, 0), payload.clone());
        Ok(payload)
    }
}

/// WGS84 (lon, lat) → EPSG:25833 (UTM33N) using the inverse of the
/// standard ellipsoidal UTM formulas (WGS84 ellipsoid). Accurate to
/// well under a metre for the entire UTM33N zone (Norway interior).
pub fn wgs84_to_utm33n(lon_deg: f64, lat_deg: f64) -> PointXY {
    const A: f64 = 6_378_137.0;
    const F: f64 = 1.0 / 298.257_223_563;
    let e2 = F * (2.0 - F);
    let ep2 = e2 / (1.0 - e2);
    let k0 = 0.9996;
    let lon0 = 15.0_f64.to_radians();
    let false_e = 500_000.0;
    let false_n = 0.0;

    let phi = lat_deg.to_radians();
    let lam = lon_deg.to_radians();
    let dlam = lam - lon0;

    let sin_phi = phi.sin();
    let cos_phi = phi.cos();
    let tan_phi = phi.tan();
    let n = A / (1.0 - e2 * sin_phi * sin_phi).sqrt();
    let t = tan_phi * tan_phi;
    let c = ep2 * cos_phi * cos_phi;
    let a_term = cos_phi * dlam;

    let m = A
        * ((1.0 - e2 / 4.0 - 3.0 * e2 * e2 / 64.0 - 5.0 * e2 * e2 * e2 / 256.0) * phi
            - (3.0 * e2 / 8.0 + 3.0 * e2 * e2 / 32.0 + 45.0 * e2 * e2 * e2 / 1024.0)
                * (2.0 * phi).sin()
            + (15.0 * e2 * e2 / 256.0 + 45.0 * e2 * e2 * e2 / 1024.0) * (4.0 * phi).sin()
            - (35.0 * e2 * e2 * e2 / 3072.0) * (6.0 * phi).sin());

    let x = k0
        * n
        * (a_term
            + (1.0 - t + c) * a_term.powi(3) / 6.0
            + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * ep2) * a_term.powi(5) / 120.0)
        + false_e;
    let y = k0
        * (m
            + n * tan_phi
                * (a_term * a_term / 2.0
                    + (5.0 - t + 9.0 * c + 4.0 * c * c) * a_term.powi(4) / 24.0
                    + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * ep2)
                        * a_term.powi(6)
                        / 720.0))
        + false_n;
    PointXY { x, y }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wgs84_to_utm33n_oslo_within_100m() {
        let p = wgs84_to_utm33n(10.7522, 59.9139);
        let dx = (p.x - 262_000.0).abs();
        let dy = (p.y - 6_649_000.0).abs();
        assert!(dx < 1000.0 && dy < 1000.0);
    }
}

#[allow(dead_code)]
pub use crate::format::DemMeta as DemMetaPub;
