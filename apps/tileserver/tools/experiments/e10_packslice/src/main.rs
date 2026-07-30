//! E10 — does a sliced region reproduce whole-region routes, and what
//! halo does it need?
//!
//! The offline plan assumes a region pack can be sliced out of the national
//! artifacts and still produce the same routes. Nothing tested that. The
//! open design question is the **boundary policy**: a corridor solve explores
//! a padded rectangle around the from→to line (`PAD_CAP_M` and
//! `HALF_WIDTH_CAP_M` are both 3000 m in `unified.rs`), so a pack clipped to
//! the route's own bounding box will starve the search of terrain it wants
//! to look at.
//!
//! This slices the **DEM** — the dominant artifact, 54% of the Sjunkhatten
//! pack — to a route's bbox expanded by a varying halo, keeps mask and graph
//! whole, and asks: at what halo does the route reproduce the whole-DEM
//! geometry bit-exactly?
//!
//! Isolating the DEM is deliberate. It is the artifact whose slicing is
//! format-trivial (v2 stores one entry per source tile with its own origin,
//! so a slice is a filter plus a verbatim payload copy — no resampling, no
//! recompression), and it is the one the corridor search is most sensitive
//! to. Mask and graph slicing have their own boundary effects, noted at the
//! end but not measured here.
//!
//! Usage: e10 <artifacts-dir> <scratch-dir>

use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::Arc;

use turbo_tiles_artifacts::{read_header, write_header, ArtifactKind, Header, HEADER_BYTES};
use turbo_tiles_elev::format::{read_meta, read_tile_entry};
use turbo_tiles_elev::{
    write_meta, write_tile_entry, wgs84_to_utm33n, Dem, DemMeta, TileEntry, DEM_FORMAT_VERSION,
    DEM_META_BYTES, TILE_ENTRY_BYTES,
};
use turbo_tiles_graph::Graph;
use turbo_tiles_mask::Mask;
use turbo_tiles_pathfind::{Pathfinder, Prefs};

/// Corpus routes inside the Sjunkhatten cell (same set as E1).
const ROUTES: &[([f64; 2], [f64; 2], &str)] = &[
    ([15.961509, 66.996683], [15.981, 67.004], "n-3496285"),
    ([15.001894, 66.866072], [15.019, 66.874], "n-3894666"),
    ([15.495658, 66.921925], [15.513, 66.930], "n-1821249"),
    ([15.287597, 66.741357], [15.340, 66.770], "n-1884462"),
    ([16.097354, 67.053586], [16.140, 67.075], "n-1895277"),
];

const HALOS_M: &[f64] = &[0.0, 500.0, 1000.0, 2000.0, 3000.0, 4000.0, 6000.0];

fn fnv1a(h: &mut u64, b: &[u8]) {
    for &x in b {
        *h ^= x as u64;
        *h = h.wrapping_mul(0x100_0000_01b3);
    }
}
const FNV: u64 = 0xcbf2_9ce4_8422_2325;

/// Slice a v2 DEM to the tiles intersecting `bbox`. Payloads are copied
/// byte-for-byte — no decode, no resample, no recompression — so any
/// geometry difference is attributable to *missing terrain*, never to
/// altered terrain.
fn slice_dem(
    src: &std::path::Path,
    dst: &std::path::Path,
    bbox: (f64, f64, f64, f64),
) -> std::io::Result<(u32, u32, u64)> {
    let mut f = std::fs::File::open(src)?;
    let _hdr = read_header(&mut f).expect("header");
    let meta = read_meta(&mut f)?;
    let cell = meta.pixel_size_m as f64;
    let span = meta.tile_cells as f64 * cell;

    let mut entries = Vec::with_capacity(meta.tile_count as usize);
    for _ in 0..meta.tile_count {
        entries.push(read_tile_entry(&mut f)?);
    }

    let (min_x, min_y, max_x, max_y) = bbox;
    let keep: Vec<&TileEntry> = entries
        .iter()
        .filter(|e| {
            // Tile covers [ulx, ulx+span] x [uly-span, uly].
            let (tx0, tx1) = (e.ulx, e.ulx + span);
            let (ty0, ty1) = (e.uly - span, e.uly);
            tx1 >= min_x && tx0 <= max_x && ty1 >= min_y && ty0 <= max_y
        })
        .collect();

    let mut out = std::io::BufWriter::new(std::fs::File::create(dst)?);
    write_header(
        &mut out,
        &Header {
            kind: ArtifactKind::Dem,
            format_version: DEM_FORMAT_VERSION,
            build_timestamp_unix_sec: 0,
        },
    )
    .expect("write header");
    let new_meta = DemMeta {
        tile_count: keep.len() as u32,
        ..meta
    };
    write_meta(&mut out, &new_meta)?;

    let dir_bytes = keep.len() * TILE_ENTRY_BYTES;
    let mut payload_at = (HEADER_BYTES + DEM_META_BYTES + dir_bytes) as u64;
    for e in &keep {
        write_tile_entry(
            &mut out,
            &TileEntry {
                ulx: e.ulx,
                uly: e.uly,
                offset: payload_at,
                compressed_size: e.compressed_size,
            },
        )?;
        payload_at += e.compressed_size as u64;
    }
    let mut total = 0u64;
    for e in &keep {
        f.seek(SeekFrom::Start(e.offset))?;
        let mut buf = vec![0u8; e.compressed_size as usize];
        f.read_exact(&mut buf)?;
        out.write_all(&buf)?;
        total += buf.len() as u64;
    }
    out.flush()?;
    Ok((meta.tile_count, keep.len() as u32, total + payload_at - total))
}

fn solve_hash(pf: &Pathfinder, from: [f64; 2], to: [f64; 2], off_trail: bool) -> (String, u64) {
    let mut prefs = Prefs {
        max_off_trail_km: 20.0,
        ..Default::default()
    };
    if off_trail {
        prefs.force_off_trail = true;
        prefs.snap_radius_m = 0.0;
        prefs.bridge_radius_m = 0.0;
    }
    match pf.solve(from, to, prefs) {
        Ok(p) => {
            let mut h = FNV;
            for c in &p.geometry {
                fnv1a(&mut h, &c[0].to_bits().to_le_bytes());
                fnv1a(&mut h, &c[1].to_bits().to_le_bytes());
            }
            (format!("{h:016x}"), p.geometry.len() as u64)
        }
        Err(e) => (format!("ERR:{e:?}"), 0),
    }
}

fn main() {
    let art = std::env::args().nth(1).expect("artifacts dir");
    let scratch = std::env::args().nth(2).expect("scratch dir");
    let art = std::path::Path::new(&art);
    let scratch = std::path::Path::new(&scratch);
    std::fs::create_dir_all(scratch).unwrap();

    let mask = Arc::new(Mask::open(art.join("norway.mask")).expect("mask"));
    let graph = Arc::new(Graph::open(art.join("norway.graph")).expect("graph"));
    let src_dem = art.join("norway.dem");
    let full_len = std::fs::metadata(&src_dem).unwrap().len();

    // ---- reference: the whole DEM --------------------------------------
    let full = Arc::new(Dem::open(&src_dem).expect("dem"));
    let pf_full = Pathfinder::with_defaults(Some(full), Some(mask.clone()), Some(graph.clone()));
    println!("# E10 — pack slice fidelity and halo requirement");
    println!("full DEM: {:.1} MB\n", full_len as f64 / 1048576.0);

    for lane_off_trail in [true, false] {
        let lane = if lane_off_trail { "off-trail" } else { "unified" };
        println!("## lane: {lane}");
        let refs: Vec<(String, u64)> = ROUTES
            .iter()
            .map(|(f, t, _)| solve_hash(&pf_full, *f, *t, lane_off_trail))
            .collect();

        print!("{:<12}", "halo_m");
        for (_, _, id) in ROUTES {
            print!(" {id:>12}");
        }
        println!("  {:>9} {:>8}", "slice_MB", "tiles");

        for &halo in HALOS_M {
            // PER-ROUTE bbox. The first version used the union of all
            // endpoints, which handed every route a ~48 km effective halo and
            // made the sweep meaningless. Each route now gets a slice sized to
            // ITS OWN endpoints plus the halo under test.
            let mut sz_sum = 0f64;
            let mut kept_sum = 0u32;
            print!("{halo:<12.0}");
            for (i, (f, t, _)) in ROUTES.iter().enumerate() {
                let (uf, ut) = (wgs84_to_utm33n(f[0], f[1]), wgs84_to_utm33n(t[0], t[1]));
                let bbox = (
                    uf.x.min(ut.x) - halo,
                    uf.y.min(ut.y) - halo,
                    uf.x.max(ut.x) + halo,
                    uf.y.max(ut.y) + halo,
                );
                let dst = scratch.join(format!("h{halo:.0}_r{i}.dem"));
                let (_all, kept, _sz) = slice_dem(&src_dem, &dst, bbox).expect("slice");
                sz_sum += std::fs::metadata(&dst).unwrap().len() as f64;
                kept_sum += kept;

                let sliced = Arc::new(Dem::open(&dst).expect("sliced dem"));
                let pf = Pathfinder::with_defaults(
                    Some(sliced),
                    Some(mask.clone()),
                    Some(graph.clone()),
                );
                let got = solve_hash(&pf, *f, *t, lane_off_trail);
                let _ = std::fs::remove_file(&dst);
                let mark = if got == refs[i] {
                    "MATCH".to_string()
                } else if got.0.starts_with("ERR") {
                    "refuse".to_string()
                } else {
                    format!("DIFF{:+}", got.1 as i64 - refs[i].1 as i64)
                };
                print!(" {mark:>12}");
            }
            println!("  {:>9.1} {:>8}", sz_sum / 1048576.0, kept_sum);
        }
        println!();
    }

    println!("Reference hashes (whole DEM, off-trail lane):");
    let pf = &pf_full;
    for (f, t, id) in ROUTES {
        let (h, n) = solve_hash(pf, *f, *t, true);
        println!("  {id:<12} pts={n:<6} {h}");
    }
}
