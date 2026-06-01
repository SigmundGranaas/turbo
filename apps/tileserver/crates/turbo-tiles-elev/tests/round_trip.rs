//! End-to-end round trip: write a tiny synthetic DEM artifact by
//! hand, open it with `Dem::open`, and verify sampling + profile +
//! bilinear interpolation behave correctly.
//!
//! This is the core correctness test for Stage 1.

use std::fs::OpenOptions;
use std::io::Write;

use byteorder::{LittleEndian, WriteBytesExt};
use turbo_tiles_artifacts::{write_header, ArtifactKind, Header, HEADER_BYTES};
use turbo_tiles_elev::{
    write_meta, write_tile_entry, Dem, DemMeta, PointXY, TileEntry, COMPRESSION_ZSTD,
    DEM_FORMAT_VERSION, DEM_META_BYTES, NODATA_SENTINEL, TILE_ENTRY_BYTES,
};

const TILE_CELLS: u32 = 4;
const TILE_ULX: f64 = 100_000.0;
const TILE_ULY: f64 = 6_500_040.0;

fn write_tiny_artifact(path: &std::path::Path, cells: &[f32]) {
    // 4×4 cells = single tile at a fixed origin so column/row math
    // is clean. Sits inside the seeded `tiles_e2e` Oslo area.
    assert_eq!(cells.len(), (TILE_CELLS * TILE_CELLS) as usize);
    let meta = DemMeta {
        tile_count: 1,
        tile_cells: TILE_CELLS,
        pixel_size_m: 10.0,
        nodata: NODATA_SENTINEL,
        compression: COMPRESSION_ZSTD,
    };
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .unwrap();
    let hdr = Header {
        kind: ArtifactKind::Dem,
        format_version: DEM_FORMAT_VERSION,
        build_timestamp_unix_sec: 1_700_000_000,
    };
    write_header(&mut f, &hdr).unwrap();
    write_meta(&mut f, &meta).unwrap();

    let dir_offset = (HEADER_BYTES + DEM_META_BYTES) as u64;
    let payload_offset = dir_offset + TILE_ENTRY_BYTES as u64;
    let raw_bytes: &[u8] = bytemuck::cast_slice(cells);
    let compressed = zstd::encode_all(raw_bytes, 1).unwrap();
    let entry = TileEntry {
        ulx: TILE_ULX,
        uly: TILE_ULY,
        offset: payload_offset,
        compressed_size: compressed.len() as u32,
    };
    write_tile_entry(&mut f, &entry).unwrap();
    f.write_all(&compressed).unwrap();
    f.sync_all().unwrap();
}

#[test]
fn round_trip_bilinear_constant_grid() {
    // Constant elevation → every sample equals the constant, including
    // the bilinear interior.
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let cells = vec![123.0f32; 16];
    write_tiny_artifact(tmp.path(), &cells);
    let dem = Dem::open(tmp.path()).expect("open");
    // Centre of the (1,1) cell.
    let s = dem
        .sample(PointXY {
            x: 100_015.0,
            y: 6_500_025.0,
        })
        .unwrap();
    assert_eq!(s, Some(123.0));
}

#[test]
fn round_trip_bilinear_linear_gradient() {
    // Linear ramp west-to-east: cell (col, _) has elev=col*100. The
    // bilinear value half-way between col=1 and col=2 must be 150.
    let mut cells = vec![0.0f32; 16];
    for row in 0..TILE_CELLS {
        for col in 0..TILE_CELLS {
            cells[(row * TILE_CELLS + col) as usize] = col as f32 * 100.0;
        }
    }
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_tiny_artifact(tmp.path(), &cells);
    let dem = Dem::open(tmp.path()).expect("open");
    // Point at fractional col=1.5, row=1 (i.e. exact midpoint of the
    // cell edge between (1,1)=100 and (2,1)=200).
    let s = dem
        .sample(PointXY {
            x: 100_000.0 + 1.5 * 10.0,
            y: 6_500_040.0 - 1.0 * 10.0,
        })
        .unwrap()
        .unwrap();
    assert!((s - 150.0).abs() < 1e-3, "got {s}");
}

#[test]
fn round_trip_out_of_coverage_errors() {
    let cells = vec![5.0f32; 16];
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_tiny_artifact(tmp.path(), &cells);
    let dem = Dem::open(tmp.path()).expect("open");
    let err = dem
        .sample(PointXY {
            x: 999_999.0,
            y: 6_500_000.0,
        })
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("out of coverage"), "got: {msg}");
}

#[test]
fn round_trip_nodata_returns_none() {
    let mut cells = vec![10.0f32; 16];
    // Make the (0,0) neighbourhood nodata so a sample near it
    // surfaces None rather than a polluted interpolation.
    cells[0] = NODATA_SENTINEL;
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_tiny_artifact(tmp.path(), &cells);
    let dem = Dem::open(tmp.path()).expect("open");
    let s = dem
        .sample(PointXY {
            x: 100_005.0,
            y: 6_500_035.0,
        })
        .unwrap();
    assert_eq!(s, None, "any nodata neighbour must invalidate the sample");
}

#[test]
fn round_trip_profile_traverses_grid() {
    let mut cells = vec![0.0f32; 16];
    for row in 0..TILE_CELLS {
        for col in 0..TILE_CELLS {
            cells[(row * TILE_CELLS + col) as usize] = (col + row * TILE_CELLS) as f32;
        }
    }
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_tiny_artifact(tmp.path(), &cells);
    let dem = Dem::open(tmp.path()).expect("open");
    let pts = (0..4)
        .map(|i| PointXY {
            x: 100_005.0 + i as f64 * 10.0,
            y: 6_500_035.0,
        })
        .collect::<Vec<_>>();
    let prof = dem.profile(&pts).unwrap();
    assert_eq!(prof.len(), 4);
    assert!(prof.iter().all(|p| p.is_some()));
}

#[test]
fn slope_aspect_linear_west_to_east_ramp() {
    // West-to-east ramp: elev = col * 5.0 → ∂z/∂x = 5/10 = 0.5
    // (5 m per 10 m east). Slope = atan(0.5) ≈ 26.57°, aspect = 270°
    // (downhill faces west when uphill goes east).
    let mut cells = vec![0.0f32; 16];
    for row in 0..TILE_CELLS {
        for col in 0..TILE_CELLS {
            cells[(row * TILE_CELLS + col) as usize] = col as f32 * 5.0;
        }
    }
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_tiny_artifact(tmp.path(), &cells);
    let dem = Dem::open(tmp.path()).expect("open");
    // Centre cell (col=2, row=2) — far enough from edges for the 3×3.
    let sa = dem
        .slope_aspect(PointXY {
            x: 100_020.0,
            y: 6_500_020.0,
        })
        .unwrap()
        .expect("interior cell, no nodata");
    let expected_slope = (0.5_f32).atan().to_degrees();
    assert!(
        (sa.slope_deg - expected_slope).abs() < 0.5,
        "got slope {}, expected ~{}",
        sa.slope_deg,
        expected_slope
    );
    // Aspect for east-up gradient is west (270°).
    assert!(
        (sa.aspect_deg - 270.0).abs() < 5.0,
        "got aspect {}, expected ~270",
        sa.aspect_deg
    );
}

#[test]
fn slope_aspect_flat_returns_zero() {
    let cells = vec![100.0f32; 16];
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_tiny_artifact(tmp.path(), &cells);
    let dem = Dem::open(tmp.path()).expect("open");
    let sa = dem
        .slope_aspect(PointXY {
            x: 100_020.0,
            y: 6_500_020.0,
        })
        .unwrap()
        .expect("interior cell");
    assert!(
        sa.slope_deg.abs() < 1e-3,
        "flat ground must yield 0 slope, got {}",
        sa.slope_deg
    );
}

#[test]
fn slope_aspect_edge_returns_none() {
    let cells = vec![5.0f32; 16];
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_tiny_artifact(tmp.path(), &cells);
    let dem = Dem::open(tmp.path()).expect("open");
    // Top-left cell — 3×3 doesn't fit.
    let sa = dem
        .slope_aspect(PointXY {
            x: 100_001.0,
            y: 6_500_039.0,
        })
        .unwrap();
    assert!(
        sa.is_none(),
        "edge cell should not have a 3×3 neighbourhood"
    );
}

#[test]
fn round_trip_rejects_wrong_magic() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut f = std::fs::File::create(tmp.path()).unwrap();
    f.write_u32::<LittleEndian>(0xDEAD_BEEF).unwrap();
    f.write_all(&[0u8; 256]).unwrap();
    f.sync_all().unwrap();
    let err = match Dem::open(tmp.path()) {
        Ok(_) => panic!("expected open() to fail on bad magic"),
        Err(e) => e,
    };
    let msg = err.to_string();
    assert!(
        msg.contains("magic") || msg.contains("artifact"),
        "got: {msg}"
    );
}
