//! Write `norway.mask` from N50 surfaces.
//!
//! Same artifact as `turbo-tiles-build::mask_builder`, and the same fill
//! — both call `turbo_tiles_mask::scanline_fill`, which is why they can
//! be compared at all. What differs is only where the polygons come
//! from: PostGIS `terrain.water_polygon` there, N50 Arealdekke GML here.
//!
//! # Memory
//!
//! The national build allocates one byte per cell over Norway, which its
//! own comment puts at ~200 MB. That does not scale down by being
//! clever; it scales down by the region being small. At 100 m a 55 km
//! square is 550 × 550 cells — about 300 KB, packed to 75 KB. The
//! national figure is not a constraint this path ever meets.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use geo::Polygon;
use turbo_tiles_artifacts::{write_header, ArtifactKind, Header};
use turbo_tiles_mask::{
    packed_bytes, scanline_fill, write_meta, MaskMeta, RefusalKind, DEFAULT_RESOLUTION_M,
    MASK_FORMAT_VERSION,
};

use crate::wcs::BoxUtm;
use crate::BuildError;
use crate::IoAt;

/// The N50 feature types that become water.
///
/// Taken from `upsert_n50_vann.sql`, which is the pipeline's existing
/// answer to "what is water" — not re-derived, because a mask that
/// disagrees with the server's about a lake is a mask that refuses
/// ground the server walks over.
pub const WATER_TYPES: &[&str] = &["Innsjø", "InnsjøRegulert", "Elv", "Havflate"];

/// From `upsert_n50_isogbre.sql`.
pub const GLACIER_TYPES: &[&str] = &["SnøIsbre"];

/// Accumulates polygons into a refusal grid over a fixed region.
pub struct MaskWriter {
    cells: Vec<u8>,
    cells_x: u32,
    cells_y: u32,
    region: BoxUtm,
    res: f64,
    pub water_polygons: u32,
    pub glacier_polygons: u32,
}

impl MaskWriter {
    pub fn new(region: BoxUtm) -> Self {
        let res = DEFAULT_RESOLUTION_M as f64;
        // Ceil, so the grid covers the region rather than stopping short
        // of its east and south edges.
        let cells_x = (region.width() / res).ceil().max(1.0) as u32;
        let cells_y = (region.height() / res).ceil().max(1.0) as u32;
        Self {
            cells: vec![0u8; cells_x as usize * cells_y as usize],
            cells_x,
            cells_y,
            region,
            res,
            water_polygons: 0,
            glacier_polygons: 0,
        }
    }

    /// Rasterise one polygon as `kind`.
    ///
    /// Last write wins, so glacier must be added after water where the
    /// two overlap — the same property the PostGIS builder gets from the
    /// order it runs its layers in.
    pub fn add(&mut self, poly: &Polygon<f64>, kind: RefusalKind) {
        scanline_fill(
            poly,
            kind as u8,
            &mut self.cells,
            self.cells_x,
            self.cells_y,
            self.region.min_x,
            self.region.max_y,
            self.res,
        );
        match kind {
            RefusalKind::Glacier => self.glacier_polygons += 1,
            _ => self.water_polygons += 1,
        }
    }

    /// Cells marked refused, for a coverage sanity check.
    pub fn refused_cells(&self) -> u64 {
        self.cells.iter().filter(|&&c| c != 0).count() as u64
    }

    pub fn total_cells(&self) -> u64 {
        self.cells.len() as u64
    }

    /// Pack to 2 bits per cell and write the artifact.
    pub fn finish(self, out_dir: &Path) -> Result<PathBuf, BuildError> {
        std::fs::create_dir_all(out_dir).at(out_dir)?;
        let n_packed = packed_bytes(self.cells.len() as u64) as usize;
        let mut packed = vec![0u8; n_packed];
        for (i, &v) in self.cells.iter().enumerate() {
            packed[i / 4] |= (v & 0b11) << ((i % 4) * 2);
        }

        let out_path = out_dir.join(ArtifactKind::Mask.filename());
        let tmp_path = out_dir.join(format!("{}.tmp", ArtifactKind::Mask.filename()));
        let f = File::create(&tmp_path).at(&tmp_path)?;
        let mut w = BufWriter::new(f);
        write_header(
            &mut w,
            &Header {
                kind: ArtifactKind::Mask,
                format_version: MASK_FORMAT_VERSION,
                build_timestamp_unix_sec: chrono::Utc::now().timestamp(),
            },
        )
        .map_err(|e| BuildError::Logic(format!("mask header: {e}")))?;
        write_meta(
            &mut w,
            &MaskMeta {
                min_x: self.region.min_x,
                min_y: self.region.min_y,
                // The grid is cells_x × cells_y from the north-west
                // corner, which may extend past the requested east and
                // south edges after the ceil above. Report the grid, not
                // the request, or the reader maps cells to the wrong
                // ground.
                max_x: self.region.min_x + self.cells_x as f64 * self.res,
                max_y: self.region.max_y,
                cells_x: self.cells_x,
                cells_y: self.cells_y,
                resolution_m: DEFAULT_RESOLUTION_M,
            },
        )?;
        w.write_all(&packed)?;
        w.flush()?;
        drop(w);
        std::fs::rename(&tmp_path, &out_path).at(&tmp_path)?;
        Ok(out_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::LineString;
    use turbo_tiles_mask::Mask;

    fn region() -> BoxUtm {
        BoxUtm {
            min_x: 500_000.0,
            min_y: 7_400_000.0,
            max_x: 501_000.0,
            max_y: 7_401_000.0,
        }
    }

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon<f64> {
        Polygon::new(
            LineString::from(vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)]),
            vec![],
        )
    }

    /// The round trip: what the writer marks is what the engine's own
    /// reader refuses, at the right ground position.
    #[test]
    fn a_lake_reads_back_as_refused_through_the_real_mask_reader() {
        let dir = tempfile::tempdir().unwrap();
        let mut m = MaskWriter::new(region());
        m.add(
            &square(500_200.0, 7_400_200.0, 500_500.0, 7_400_500.0),
            RefusalKind::Water,
        );
        let path = m.finish(dir.path()).unwrap();

        let mask = Mask::open(&path).expect("the mask crate must open what we wrote");
        assert_eq!(
            mask.refused(500_350.0, 7_400_350.0).unwrap(),
            RefusalKind::Water
        );
        assert_eq!(
            mask.refused(500_900.0, 7_400_900.0).unwrap(),
            RefusalKind::None
        );
    }

    /// Glacier over water must win, because that is the order the
    /// PostGIS builder produces and the two must agree.
    #[test]
    fn glacier_added_after_water_wins_the_overlap() {
        let dir = tempfile::tempdir().unwrap();
        let mut m = MaskWriter::new(region());
        let s = square(500_200.0, 7_400_200.0, 500_600.0, 7_400_600.0);
        m.add(&s, RefusalKind::Water);
        m.add(&s, RefusalKind::Glacier);
        let path = m.finish(dir.path()).unwrap();
        let mask = Mask::open(&path).unwrap();
        assert_eq!(
            mask.refused(500_400.0, 7_400_400.0).unwrap(),
            RefusalKind::Glacier
        );
    }

    /// North-up: a lake in the northern half must not read back in the
    /// southern one. A flipped Y still produces a plausible mask.
    #[test]
    fn the_grid_is_north_up() {
        let dir = tempfile::tempdir().unwrap();
        let mut m = MaskWriter::new(region());
        // Northern quarter of the region.
        m.add(
            &square(500_000.0, 7_400_800.0, 501_000.0, 7_401_000.0),
            RefusalKind::Water,
        );
        let path = m.finish(dir.path()).unwrap();
        let mask = Mask::open(&path).unwrap();
        assert_eq!(
            mask.refused(500_500.0, 7_400_900.0).unwrap(),
            RefusalKind::Water
        );
        assert_eq!(
            mask.refused(500_500.0, 7_400_100.0).unwrap(),
            RefusalKind::None
        );
    }

    #[test]
    fn an_empty_region_refuses_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let m = MaskWriter::new(region());
        assert_eq!(m.refused_cells(), 0);
        let path = m.finish(dir.path()).unwrap();
        let mask = Mask::open(&path).unwrap();
        assert_eq!(
            mask.refused(500_500.0, 7_400_500.0).unwrap(),
            RefusalKind::None
        );
    }
}
