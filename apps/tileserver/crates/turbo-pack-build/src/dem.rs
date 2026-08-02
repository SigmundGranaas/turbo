//! Write `norway.dem` from WCS rasters.
//!
//! Same on-disk format as `turbo-tiles-build::dem_builder` — both go
//! through `turbo-tiles-elev` — but sourced from HTTP instead of a
//! PostGIS raster staging table.
//!
//! The region is never held in memory. Each fetched raster is cut into
//! 256-cell tiles, each tile is zstd'd and appended immediately, and
//! only the tile directory accumulates: 32 bytes per tile, so a 53 km
//! region costs about 18 KB of state for a 52 MB artifact. That is what
//! makes this runnable on a phone.

use std::fs::OpenOptions;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use turbo_tiles_artifacts::{write_header, ArtifactKind, Header, HEADER_BYTES};
use turbo_tiles_elev::{
    write_meta, write_tile_entry, DemMeta, TileEntry, COMPRESSION_ZSTD, DEFAULT_TILE_CELLS,
    DEM_META_BYTES, NODATA_SENTINEL, TILE_ENTRY_BYTES,
};

use crate::geotiff::Raster;
use crate::wcs::{BoxUtm, RESOLUTION_M};
use crate::BuildError;

/// Accumulates tiles into `norway.dem` as rasters arrive.
pub struct DemWriter {
    out_path: PathBuf,
    tmp_path: PathBuf,
    w: BufWriter<std::fs::File>,
    dir: Vec<TileEntry>,
    payload_offset: u64,
    dir_offset: u64,
    dir_capacity: usize,
    pub tiles_written: u64,
    pub tiles_all_nodata: u64,
    pub compressed_bytes: u64,
}

impl DemWriter {
    /// `expected_tiles` sizes the placeholder directory. Overshooting is
    /// free (the file is rewritten to the true count); undershooting is
    /// not, so callers should round up.
    pub fn create(out_dir: &Path, expected_tiles: usize) -> Result<Self, BuildError> {
        std::fs::create_dir_all(out_dir)?;
        let out_path = out_dir.join(ArtifactKind::Dem.filename());
        let tmp_path = out_dir.join(format!("{}.tmp", ArtifactKind::Dem.filename()));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)?;
        let mut w = BufWriter::with_capacity(8 << 20, file);

        write_header(
            &mut w,
            &Header {
                kind: ArtifactKind::Dem,
                format_version: turbo_tiles_elev::DEM_FORMAT_VERSION,
                build_timestamp_unix_sec: chrono::Utc::now().timestamp(),
            },
        )
        .map_err(|e| BuildError::Logic(format!("dem header: {e}")))?;
        write_meta(
            &mut w,
            &DemMeta {
                tile_count: expected_tiles as u32,
                tile_cells: DEFAULT_TILE_CELLS,
                pixel_size_m: RESOLUTION_M as f32,
                nodata: NODATA_SENTINEL,
                compression: COMPRESSION_ZSTD,
            },
        )?;
        let dir_offset = (HEADER_BYTES + DEM_META_BYTES) as u64;
        let dir_bytes = expected_tiles * TILE_ENTRY_BYTES;
        w.write_all(&vec![0u8; dir_bytes])?;

        Ok(Self {
            out_path,
            tmp_path,
            w,
            dir: Vec::with_capacity(expected_tiles),
            payload_offset: dir_offset + dir_bytes as u64,
            dir_offset,
            dir_capacity: expected_tiles,
            tiles_written: 0,
            tiles_all_nodata: 0,
            compressed_bytes: 0,
        })
    }

    /// Cut one fetched raster into 256-cell tiles and append them.
    ///
    /// The raster's own origin is used, not the request box's, so a
    /// server that honoured the box loosely still lands its samples in
    /// the right place — [`crate::wcs::fetch`] has already refused
    /// anything more than a pixel off.
    pub fn add(&mut self, r: &Raster) -> Result<(), BuildError> {
        let tc = DEFAULT_TILE_CELLS as usize;
        let across = r.width.div_ceil(tc);
        let down = r.height.div_ceil(tc);

        for ty in 0..down {
            for tx in 0..across {
                let mut floats = vec![NODATA_SENTINEL; tc * tc];
                let mut any = false;
                for row in 0..tc {
                    let sy = ty * tc + row;
                    if sy >= r.height {
                        break;
                    }
                    for col in 0..tc {
                        let sx = tx * tc + col;
                        if sx >= r.width {
                            break;
                        }
                        let v = r.values[sy * r.width + sx];
                        // The WCS marks absent ground with a large
                        // negative; NaN shows up at coverage edges. Both
                        // must become the sentinel the reader knows,
                        // because a raw -32767 would be read as terrain
                        // 32 km below sea level and route straight
                        // through it.
                        if v.is_finite() && v > -9000.0 {
                            floats[row * tc + col] = v;
                            any = true;
                        }
                    }
                }
                if !any {
                    // An all-nodata tile is real (sea, or outside the
                    // coverage). Storing it costs ~300 bytes zstd'd and
                    // saves the reader an rstar miss, but it is not
                    // terrain — count it so coverage stats stay honest.
                    self.tiles_all_nodata += 1;
                }
                let ulx = r.origin_x + (tx * tc) as f64 * r.pixel_size_x;
                let uly = r.origin_y - (ty * tc) as f64 * r.pixel_size_y;

                let raw: &[u8] = bytemuck::cast_slice(&floats);
                let compressed = zstd::encode_all(raw, 6)
                    .map_err(|e| BuildError::Logic(format!("zstd encode: {e}")))?;
                let len = compressed.len() as u32;
                self.w.write_all(&compressed)?;
                self.dir.push(TileEntry {
                    ulx,
                    uly,
                    offset: self.payload_offset,
                    compressed_size: len,
                });
                self.payload_offset += len as u64;
                self.compressed_bytes += len as u64;
                self.tiles_written += 1;
            }
        }
        Ok(())
    }

    /// Rewrite the directory and meta with the true tile count, then
    /// rename into place.
    pub fn finish(mut self) -> Result<PathBuf, BuildError> {
        if self.dir.len() > self.dir_capacity {
            return Err(BuildError::Logic(format!(
                "wrote {} tiles but reserved directory space for {} — the fetch plan and the \
                 tile count disagree",
                self.dir.len(),
                self.dir_capacity
            )));
        }
        self.w.flush()?;
        let mut file = self
            .w
            .into_inner()
            .map_err(|e| BuildError::Logic(format!("flushing {}: {e}", self.tmp_path.display())))?;

        file.seek(SeekFrom::Start(HEADER_BYTES as u64))?;
        {
            let mut bw = BufWriter::new(&mut file);
            write_meta(
                &mut bw,
                &DemMeta {
                    tile_count: self.dir.len() as u32,
                    tile_cells: DEFAULT_TILE_CELLS,
                    pixel_size_m: RESOLUTION_M as f32,
                    nodata: NODATA_SENTINEL,
                    compression: COMPRESSION_ZSTD,
                },
            )?;
            bw.flush()?;
        }
        file.seek(SeekFrom::Start(self.dir_offset))?;
        {
            let mut bw = BufWriter::new(&mut file);
            for e in &self.dir {
                write_tile_entry(&mut bw, e)?;
            }
            bw.flush()?;
        }
        file.sync_all()?;
        drop(file);

        std::fs::rename(&self.tmp_path, &self.out_path)?;
        Ok(self.out_path)
    }
}

/// Ceiling on tiles produced by a plan, for sizing the directory.
pub fn expected_tiles(plan: &[BoxUtm]) -> usize {
    let tc = DEFAULT_TILE_CELLS as f64 * RESOLUTION_M;
    plan.iter()
        .map(|b| {
            let across = (b.width() / tc).ceil() as usize;
            let down = (b.height() / tc).ceil() as usize;
            across.max(1) * down.max(1)
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use turbo_tiles_elev::Dem;

    fn raster(
        origin_x: f64,
        origin_y: f64,
        w: usize,
        h: usize,
        f: impl Fn(usize, usize) -> f32,
    ) -> Raster {
        Raster {
            origin_x,
            origin_y,
            pixel_size_x: RESOLUTION_M,
            pixel_size_y: RESOLUTION_M,
            width: w,
            height: h,
            values: (0..w * h).map(|i| f(i % w, i / w)).collect(),
        }
    }

    /// The round trip that matters: what the writer stores is what the
    /// engine's own reader samples back, at the right ground position.
    #[test]
    fn a_written_tile_reads_back_through_the_real_dem_reader() {
        let dir = tempfile::tempdir().unwrap();
        let r = raster(480_000.0, 7_425_120.0, 512, 512, |x, y| {
            100.0 + x as f32 + y as f32 * 0.5
        });
        let mut w = DemWriter::create(dir.path(), 4).unwrap();
        w.add(&r).unwrap();
        let path = w.finish().unwrap();

        let dem = Dem::open(&path).expect("the elev crate must open what we wrote");
        // Sample exactly on the grid point of cell (300, 200). The
        // reader anchors a sample at the cell's upper-left corner, not
        // its centre, so a half-cell offset here would interpolate
        // against the neighbours and hide a placement error behind a
        // plausible number.
        let x = 480_000.0 + 300.0 * RESOLUTION_M;
        let y = 7_425_120.0 - 200.0 * RESOLUTION_M;
        let got = dem
            .sample(turbo_tiles_elev::PointXY { x, y })
            .expect("sample must not error")
            .expect("terrain here");
        assert!(
            (got - 500.0).abs() < 1e-3,
            "sampled {got}, expected exactly 500 at the grid point"
        );
    }

    #[test]
    fn nodata_stays_nodata_rather_than_becoming_deep_terrain() {
        let dir = tempfile::tempdir().unwrap();
        // -32767 is what the WCS uses off-coverage.
        let r = raster(480_000.0, 7_425_120.0, 256, 256, |_, _| -32767.0);
        let mut w = DemWriter::create(dir.path(), 1).unwrap();
        w.add(&r).unwrap();
        assert_eq!(w.tiles_all_nodata, 1);
        let path = w.finish().unwrap();
        let dem = Dem::open(&path).unwrap();
        let s = dem
            .sample(turbo_tiles_elev::PointXY {
                x: 480_100.0,
                y: 7_425_000.0,
            })
            .expect("sample must not error");
        assert!(s.is_none(), "expected no terrain, got {s:?}");
    }

    #[test]
    fn the_directory_reservation_matches_what_the_plan_produces() {
        let plan = crate::wcs::plan(BoxUtm {
            min_x: 480_000.0,
            min_y: 7_400_000.0,
            max_x: 505_000.0,
            max_y: 7_420_000.0,
        });
        let reserved = expected_tiles(&plan);
        let dir = tempfile::tempdir().unwrap();
        let mut w = DemWriter::create(dir.path(), reserved).unwrap();
        for b in &plan {
            let cols = (b.width() / RESOLUTION_M).round() as usize;
            let rows = (b.height() / RESOLUTION_M).round() as usize;
            w.add(&raster(b.min_x, b.max_y, cols, rows, |_, _| 1.0))
                .unwrap();
        }
        assert!(
            w.tiles_written as usize <= reserved,
            "wrote {} tiles, reserved {reserved}",
            w.tiles_written
        );
        w.finish()
            .expect("must not overflow the reserved directory");
    }
}
