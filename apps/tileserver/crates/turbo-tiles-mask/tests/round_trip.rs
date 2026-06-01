//! Synthetic mask round-trip.

use std::io::Write;

use turbo_tiles_artifacts::{write_header, ArtifactKind, Header};
use turbo_tiles_mask::{
    packed_bytes, write_meta, Mask, MaskMeta, RefusalKind, DEFAULT_RESOLUTION_M,
    MASK_FORMAT_VERSION,
};

fn write_synthetic(path: &std::path::Path, cells_x: u32, cells_y: u32, cells: &[u8]) {
    assert_eq!(cells.len() as u64, cells_x as u64 * cells_y as u64);
    let meta = MaskMeta {
        min_x: 100_000.0,
        min_y: 6_500_000.0,
        max_x: 100_000.0 + cells_x as f64 * DEFAULT_RESOLUTION_M as f64,
        max_y: 6_500_000.0 + cells_y as f64 * DEFAULT_RESOLUTION_M as f64,
        cells_x,
        cells_y,
        resolution_m: DEFAULT_RESOLUTION_M,
    };
    let mut f = std::fs::File::create(path).unwrap();
    write_header(
        &mut f,
        &Header {
            kind: ArtifactKind::Mask,
            format_version: MASK_FORMAT_VERSION,
            build_timestamp_unix_sec: 1_700_000_000,
        },
    )
    .unwrap();
    write_meta(&mut f, &meta).unwrap();
    // Pack 2 bits per cell into bytes, LSB-first.
    let n_bytes = packed_bytes(cells.len() as u64) as usize;
    let mut packed = vec![0u8; n_bytes];
    for (i, &v) in cells.iter().enumerate() {
        let byte_i = i / 4;
        let bit_off = (i % 4) * 2;
        packed[byte_i] |= (v & 0b11) << bit_off;
    }
    f.write_all(&packed).unwrap();
    f.sync_all().unwrap();
}

#[test]
fn round_trip_water_cell_classifies_as_water() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    // 4×4 grid; cell (1,1) is water.
    let mut cells = vec![0u8; 16];
    cells[4 + 1] = 1;
    write_synthetic(tmp.path(), 4, 4, &cells);

    let mask = Mask::open(tmp.path()).expect("open");
    // Sample inside cell (1,1): col=1, row=1 → x in [100100,100200], y in [6500200,6500300].
    let k = mask.refused(100_150.0, 6_500_250.0).unwrap();
    assert_eq!(k, RefusalKind::Water);
    // Sample outside that cell → None.
    let k0 = mask.refused(100_050.0, 6_500_050.0).unwrap();
    assert_eq!(k0, RefusalKind::None);
}

#[test]
fn round_trip_glacier_classifies() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut cells = vec![0u8; 16];
    cells[2 * 4 + 2] = 2;
    write_synthetic(tmp.path(), 4, 4, &cells);
    let mask = Mask::open(tmp.path()).unwrap();
    let k = mask.refused(100_250.0, 6_500_150.0).unwrap();
    assert_eq!(k, RefusalKind::Glacier);
}

#[test]
fn coverage_counts_packed_cells() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut cells = vec![0u8; 16];
    cells[0] = 1;
    cells[1] = 1;
    cells[2] = 2;
    cells[3] = 0;
    write_synthetic(tmp.path(), 4, 4, &cells);
    let mask = Mask::open(tmp.path()).unwrap();
    let cov = mask.coverage();
    assert_eq!(cov.cells_water, 2);
    assert_eq!(cov.cells_glacier, 1);
    assert_eq!(cov.cells_total, 16);
}

#[test]
fn out_of_coverage_errors() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let cells = vec![0u8; 16];
    write_synthetic(tmp.path(), 4, 4, &cells);
    let mask = Mask::open(tmp.path()).unwrap();
    let err = mask.refused(999_999.0, 6_500_000.0).unwrap_err();
    assert!(err.to_string().contains("coverage"));
}
