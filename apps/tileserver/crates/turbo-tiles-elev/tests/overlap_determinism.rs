//! D8 — `Dem::sample` must be a pure function of position.
//!
//! Tiles in a DEM artifact can overlap; the Sjunkhatten build has 76
//! overlapping pairs. `find_tile` used to take the first hit from an
//! r-tree iterator, so *which* tile answered in an overlap band
//! depended on traversal order — and therefore on the order tiles were
//! written. Three consequences, all bad:
//!
//!   - Slicing an artifact could change elevations, because dropping
//!     one of an overlapping pair changes who answers. That is what
//!     blocked region packs, and why a sliced CI pack still carries its
//!     own baseline.
//!   - The national artifact was **build-order dependent**: rebuild it
//!     from the same sources in a different order and get different
//!     routes.
//!   - Neither shows up as an error. Both artifacts are valid, both
//!     load, both solve, both hash stably. They simply disagree.
//!
//! The fix is a canonical rule — nearest tile centre, lowest index as
//! final tie-break — which depends only on the point and the *set* of
//! tiles present.
//!
//! These tests write the overlap by hand, because the property is
//! invisible on non-overlapping data: the CI pack showed zero
//! disagreement across 1239 corpus vertices *before* the fix, which
//! proves only that its routes avoid the bands.

use std::io::Write;

use turbo_tiles_artifacts::{write_header, ArtifactKind, Header, HEADER_BYTES};
use turbo_tiles_elev::{
    write_meta, write_tile_entry, Dem, DemMeta, PointXY, TileEntry, COMPRESSION_ZSTD,
    DEM_FORMAT_VERSION, DEM_META_BYTES, NODATA_SENTINEL, TILE_ENTRY_BYTES,
};

const CELLS: u32 = 8;
const PIXEL_M: f64 = 10.0;
/// 8 × 10 m — the side of one tile.
const SPAN_M: f64 = CELLS as f64 * PIXEL_M;

/// Write an artifact whose tiles are given as `(ulx, uly, fill)`, in the
/// order supplied — the directory order is the point of the test.
fn write_artifact(path: &std::path::Path, tiles: &[(f64, f64, f32)]) {
    let meta = DemMeta {
        tile_count: tiles.len() as u32,
        tile_cells: CELLS,
        pixel_size_m: PIXEL_M as f32,
        nodata: NODATA_SENTINEL,
        compression: COMPRESSION_ZSTD,
    };
    let mut f = std::fs::File::create(path).unwrap();
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

    let payloads: Vec<Vec<u8>> = tiles
        .iter()
        .map(|(_, _, fill)| {
            let data = vec![*fill; (CELLS * CELLS) as usize];
            zstd::encode_all(bytemuck::cast_slice::<f32, u8>(&data), 1).unwrap()
        })
        .collect();

    let mut at = (HEADER_BYTES + DEM_META_BYTES + tiles.len() * TILE_ENTRY_BYTES) as u64;
    for ((ulx, uly, _), payload) in tiles.iter().zip(&payloads) {
        write_tile_entry(
            &mut f,
            &TileEntry {
                ulx: *ulx,
                uly: *uly,
                offset: at,
                compressed_size: payload.len() as u32,
            },
        )
        .unwrap();
        at += payload.len() as u64;
    }
    for payload in &payloads {
        f.write_all(payload).unwrap();
    }
    f.sync_all().unwrap();
}

/// Two tiles overlapping by half a tile, carrying different elevations.
/// `A` sits west, `B` sits half a span east, so the eastern half of A
/// and the western half of B cover the same ground.
const AX: f64 = 500_000.0;
const AY: f64 = 7_000_000.0;
const BX: f64 = AX + SPAN_M * 0.5;
const A_FILL: f32 = 100.0;
const B_FILL: f32 = 200.0;

/// A point deep inside the overlap band, and closer to A's centre.
fn point_nearer_a() -> PointXY {
    PointXY {
        x: AX + SPAN_M * 0.55,
        y: AY - SPAN_M * 0.5,
    }
}

/// Also inside the band, but closer to B's centre.
fn point_nearer_b() -> PointXY {
    PointXY {
        x: AX + SPAN_M * 0.95,
        y: AY - SPAN_M * 0.5,
    }
}

#[test]
fn overlapping_tiles_answer_the_same_regardless_of_directory_order() {
    let dir = tempfile::tempdir().unwrap();

    let ab = dir.path().join("ab.dem");
    write_artifact(&ab, &[(AX, AY, A_FILL), (BX, AY, B_FILL)]);
    let ba = dir.path().join("ba.dem");
    write_artifact(&ba, &[(BX, AY, B_FILL), (AX, AY, A_FILL)]);

    let d_ab = Dem::open(&ab).unwrap();
    let d_ba = Dem::open(&ba).unwrap();

    for p in [point_nearer_a(), point_nearer_b()] {
        let a = d_ab.sample(p).unwrap();
        let b = d_ba.sample(p).unwrap();
        assert_eq!(
            a, b,
            "D8: the same point in an overlap band must sample identically \
             whatever order the tiles were written in. Got {a:?} vs {b:?} at \
             ({}, {}).",
            p.x, p.y
        );
    }
}

/// The rule is not merely *stable*, it is *nearest centre* — which is
/// also the geometrically better answer, since the point is most
/// interior to that tile and the bilinear stencil is least likely to
/// fall off an edge.
///
/// Without this, "deterministic" could be satisfied by always taking
/// the lowest index, which would keep answering from tile A at points
/// that are barely inside it and deep inside B.
#[test]
fn the_nearest_tile_centre_answers() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("ab.dem");
    write_artifact(&p, &[(AX, AY, A_FILL), (BX, AY, B_FILL)]);
    let dem = Dem::open(&p).unwrap();

    assert_eq!(
        dem.sample(point_nearer_a()).unwrap(),
        Some(A_FILL),
        "a point closer to A's centre must read A"
    );
    assert_eq!(
        dem.sample(point_nearer_b()).unwrap(),
        Some(B_FILL),
        "a point closer to B's centre must read B — a lowest-index rule \
         would wrongly answer {A_FILL} here"
    );
}

/// The property packs actually need: dropping a tile that does not
/// cover the point must not change the answer.
///
/// This is the direct statement of what D8 broke. Slicing keeps a
/// subset of tiles; if the answer at a point depends on tiles that do
/// not cover it, a slice cannot reproduce its source, and that is
/// exactly why `slice-pack` still emits a separate baseline.
///
/// Honest scope: unlike the two tests above, this one **passes against
/// the old `.next()` implementation too** — the far tile happens not to
/// perturb the r-tree traversal for these probe points. It documents
/// the property packs depend on; the regression guards are the other
/// two, both of which fail if the tie-break is removed (verified).
#[test]
fn dropping_an_uninvolved_tile_does_not_change_the_answer() {
    let dir = tempfile::tempdir().unwrap();

    // A third tile far to the east, covering neither probe point.
    let far = (AX + SPAN_M * 4.0, AY, 900.0);
    let with = dir.path().join("with.dem");
    write_artifact(&with, &[(AX, AY, A_FILL), (BX, AY, B_FILL), far]);
    let without = dir.path().join("without.dem");
    write_artifact(&without, &[(AX, AY, A_FILL), (BX, AY, B_FILL)]);

    let d_with = Dem::open(&with).unwrap();
    let d_without = Dem::open(&without).unwrap();

    for p in [point_nearer_a(), point_nearer_b()] {
        assert_eq!(
            d_with.sample(p).unwrap(),
            d_without.sample(p).unwrap(),
            "slicing away a tile that does not cover ({}, {}) must not change \
             the value there",
            p.x,
            p.y
        );
    }
}
