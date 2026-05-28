//! Criterion bench enforcing the Stage 1/2 latency targets:
//!   - Dem::sample      P99 < 10 µs (after warm-up)
//!   - Dem::profile(100) P99 < 100 µs
//!   - Dem::slope_aspect P99 < 20 µs
//!
//! The synthetic DEM (16 × 16 cells = 1 tile) lives in a tempfile.
//! After the first sample faults-in the tile, subsequent samples hit
//! the LRU cache and the hot path is hash lookup + bilinear.

use std::io::Write;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use turbo_tiles_artifacts::{write_header, ArtifactKind, Header, HEADER_BYTES};
use turbo_tiles_elev::{
    write_meta, write_tile_entry, Dem, DemMeta, PointXY, TileEntry, COMPRESSION_ZSTD,
    DEM_FORMAT_VERSION, DEM_META_BYTES, NODATA_SENTINEL, TILE_ENTRY_BYTES,
};

const TILE_CELLS: u32 = 16;

fn make_dem() -> tempfile::NamedTempFile {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut cells = vec![0.0f32; (TILE_CELLS * TILE_CELLS) as usize];
    for row in 0..TILE_CELLS {
        for col in 0..TILE_CELLS {
            cells[(row * TILE_CELLS + col) as usize] =
                (col as f32 * 7.0) + (row as f32 * 3.0);
        }
    }
    let meta = DemMeta {
        min_x: 100_000.0,
        min_y: 6_500_000.0,
        max_x: 100_000.0 + TILE_CELLS as f64 * 10.0,
        max_y: 6_500_000.0 + TILE_CELLS as f64 * 10.0,
        cells_x: TILE_CELLS,
        cells_y: TILE_CELLS,
        resolution_m: 10.0,
        nodata: NODATA_SENTINEL,
        tile_cells: TILE_CELLS,
        compression: COMPRESSION_ZSTD,
    };
    let mut f = std::fs::File::create(tmp.path()).unwrap();
    write_header(
        &mut f,
        &Header {
            kind: ArtifactKind::Dem,
            format_version: DEM_FORMAT_VERSION,
            build_timestamp_unix_sec: 0,
        },
    )
    .unwrap();
    write_meta(&mut f, &meta).unwrap();
    let dir_offset = (HEADER_BYTES + DEM_META_BYTES) as u64;
    let payload_offset = dir_offset + TILE_ENTRY_BYTES as u64;
    let raw: &[u8] = bytemuck::cast_slice(&cells);
    let compressed = zstd::encode_all(raw, 1).unwrap();
    write_tile_entry(
        &mut f,
        &TileEntry {
            offset: payload_offset,
            compressed_size: compressed.len() as u32,
            present: 1,
        },
    )
    .unwrap();
    f.write_all(&compressed).unwrap();
    f.sync_all().unwrap();
    tmp
}

fn bench_sample(c: &mut Criterion) {
    let tmp = make_dem();
    let dem = Dem::open(tmp.path()).unwrap();
    // Warm the cache with one sample so we measure the steady-state
    // hot path, not the first-access tile fault.
    let _ = dem.sample(PointXY {
        x: 100_050.0,
        y: 6_500_050.0,
    });
    c.bench_function("dem_sample", |b| {
        b.iter(|| {
            let p = black_box(PointXY {
                x: 100_055.0,
                y: 6_500_055.0,
            });
            black_box(dem.sample(p).unwrap())
        });
    });
}

fn bench_profile_100(c: &mut Criterion) {
    let tmp = make_dem();
    let dem = Dem::open(tmp.path()).unwrap();
    let pts: Vec<PointXY> = (0..100)
        .map(|i| PointXY {
            x: 100_010.0 + (i as f64) * 1.4,
            y: 6_500_010.0 + (i as f64) * 1.0,
        })
        .collect();
    let _ = dem.profile(&pts);
    c.bench_function("dem_profile_100", |b| {
        b.iter(|| {
            black_box(dem.profile(black_box(&pts)).unwrap());
        });
    });
}

fn bench_slope_aspect(c: &mut Criterion) {
    let tmp = make_dem();
    let dem = Dem::open(tmp.path()).unwrap();
    let _ = dem.slope_aspect(PointXY {
        x: 100_050.0,
        y: 6_500_050.0,
    });
    c.bench_function("dem_slope_aspect", |b| {
        b.iter(|| {
            let p = black_box(PointXY {
                x: 100_055.0,
                y: 6_500_055.0,
            });
            black_box(dem.slope_aspect(p).unwrap())
        });
    });
}

criterion_group!(benches, bench_sample, bench_profile_100, bench_slope_aspect);
criterion_main!(benches);
