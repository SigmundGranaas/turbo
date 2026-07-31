//! **M2** — how long does cutting a pack take when the source is the
//! whole country?
//!
//! The endpoint `GET /v1/packs/:key/:file` cuts a region on demand and
//! answers `202 Retry-After` if the cut outlives `BUILD_WAIT`. That
//! design is only sound if a cut is *seconds*. If it is minutes, then
//! building on demand is not a cache miss, it is an outage, and popular
//! regions have to be pre-built — a different endpoint, a different
//! deployment, a different plan.
//!
//! # Why this is synthetic, and why that is not a dodge
//!
//! The production artifacts live on the k3s node and are not reachable
//! from here. But the question is a *scaling* question, and the three
//! phases scale on three different, independently controllable axes:
//!
//! | phase | reads | scales with |
//! |-------|-------|-------------|
//! | dem   | the tile directory, then copies the kept tiles | source **tile count**, output bytes |
//! | mask  | the whole raster payload, crops it | source **cell count** |
//! | graph | every node and every edge, rebuilds the CSR | source **edge count** |
//! | verify| re-opens the source DEM (r-tree over every tile) | source **tile count** |
//!
//! Two of those need no synthesis at all. The mask in `.data/artifacts`
//! is **already national** — 13411 × 20707 cells at 100 m is 1341 ×
//! 2071 km, the whole of Norway — so its number is a measurement, not
//! an extrapolation. The graph and DEM are regional, and this example
//! grows them.
//!
//! What synthesis cannot reproduce is the k3s node's storage. The
//! sweeps below run warm — a file this program just wrote is in cache
//! by construction — and the real source cut in 4.51 s cold against
//! 0.16 s warm, a 28x gap, so a warm-only answer would be wrong by more
//! than an order of magnitude. `SCALING_MODE` exists for that: it
//! splits generation from the build so the page cache can be dropped
//! between them.
//!
//! Even then the guest cannot drop the *host's* cache, so the portable
//! number is not wall time but **bytes read from the block device**,
//! which every row reports. Divide by the target machine's read
//! throughput.
//!
//! # Usage
//!
//! ```text
//! cargo run --release -p turbo-geodata-pack --example scaling
//!
//! # and, for the cold national figure:
//! SCALING_MODE=gen   cargo run --release ... --example scaling
//! sync; echo 3 > /proc/sys/vm/drop_caches
//! SCALING_MODE=build cargo run --release ... --example scaling
//! ```
//!
//! Set `SCALING_SRC` to a real artifact directory (default
//! `../../.data/artifacts` from the crate) and `SCALING_TMP` to
//! somewhere with room — the fat sweep writes ~0.8 GB and the national
//! source ~1.8 GB.
//!
//! Findings and the two defects this turned up: `docs/m2-pack-build-scaling.md`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use turbo_tiles_artifacts::{write_header, ArtifactKind, Header, HEADER_BYTES};

/// The region every sweep cuts: a plausible app viewport inside the
/// Sjunkhatten source, so the numbers stay comparable across sweeps.
/// Roughly 12 x 11 km — about what one screen of map asks for.
const REGION: [f64; 4] = [15.00, 67.03, 15.28, 67.13];

/// Tiles are 256 cells at 10 m: 2560 m to a side.
const TILE_SPAN_M: f64 = 256.0 * 10.0;

/// Anchor the synthetic tile grid on the real DEM's origin, so the
/// region above is covered by tiles wherever the grid is truncated.
const ORIGIN_X: f64 = 499_795.0;
const ORIGIN_Y: f64 = 7_500_205.0;

/// What one real Sjunkhatten tile compresses to: 118.3 MB / 1360.
const REAL_TILE_BYTES: usize = 87_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let src: PathBuf = std::env::var("SCALING_SRC")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../.data/artifacts")
        });
    let tmp: PathBuf = std::env::var("SCALING_TMP")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("pack-scaling"));
    std::fs::create_dir_all(&tmp)?;

    println!("source: {}", src.display());
    println!("scratch: {}\n", tmp.display());

    // Cold mode, in two halves so the page cache can be dropped between
    // them. It has to be two processes: the sweeps above all run warm,
    // because a file this program just wrote is in cache by
    // construction — and the real baseline showed cold is 28x warm, so
    // a warm-only answer would be the wrong one by more than an order
    // of magnitude.
    //
    //   SCALING_MODE=gen  ./scaling      # write the national source
    //   sync; echo 3 > /proc/sys/vm/drop_caches
    //   SCALING_MODE=build ./scaling     # cut one pack out of it, cold
    match std::env::var("SCALING_MODE").as_deref() {
        Ok("gen") => {
            let dir = tmp.join("src-national");
            let (tiles, payload) = national_dem_shape();
            println!("generating national source: {tiles} tiles at ~{payload} B, 5.87 M edges");
            synth_source(&src, &dir, tiles, payload, 2_500_000, 5_872_603)?;
            let mut total = 0u64;
            for e in std::fs::read_dir(&dir)? {
                let e = e?;
                let n = e.metadata()?.len();
                total += n;
                println!("  {:<20} {:>8.0} MB", e.file_name().to_string_lossy(), n as f64 / 1e6);
            }
            println!("  {:<20} {:>8.0} MB total", "", total as f64 / 1e6);
            return Ok(());
        }
        Ok("build") => {
            let dir = std::env::var("SCALING_BUILD_SRC")
                .map(PathBuf::from)
                .unwrap_or_else(|_| tmp.join("src-national"));
            let label = std::env::var("SCALING_LABEL").unwrap_or_else(|_| "NATIONAL".into());
            let r = run(&dir, &tmp.join("out-build"))?;
            print_row(&label, &r);
            return Ok(());
        }
        _ => {}
    }

    // 0. The real thing, for calibration. Everything below is judged
    //    against this: a synthetic source at the same scale must
    //    reproduce it, or the synthesis is measuring the wrong thing.
    println!("== baseline: the real regional source");
    let base = run(&src, &tmp.join("out-real"))?;
    print_row("real 209 MB", &base);
    println!();

    // 1. Tile count, with THIN payloads. Isolates the per-tile term:
    //    the directory read in `slice_dem` (five unbuffered reads per
    //    tile) plus the r-tree bulk-load in `verify`. Thin payloads keep
    //    this affordable on disk — 240 k tiles at 87 KB would be 21 GB.
    println!("== sweep A: source TILE COUNT, thin payloads (per-tile cost)");
    let mut a = Vec::new();
    for &n in &[1_360usize, 5_000, 20_000, 50_000, 120_000] {
        let dir = tmp.join(format!("src-a-{n}"));
        synth_source(&src, &dir, n, 0, 70_322, 157_074)?;
        let r = run(&dir, &tmp.join("out-a"))?;
        print_row(&format!("{n:>7} tiles thin"), &r);
        a.push((n as f64, r.dem + r.verify));
        std::fs::remove_dir_all(&dir).ok();
    }
    println!();

    // 2. Payload size at fixed tile count. Tests the claim that source
    //    *bytes* are nearly free: the copy loop touches only the kept
    //    tiles, and the rest of the file is never read. If that claim is
    //    wrong these two rows diverge, and the thin sweep above is
    //    measuring a fiction.
    //
    // 10 000, not 20 000: at 87 KB a tile that is already 0.9 GB, and
    // this container has 3.6 GB free.
    println!("== sweep B: source BYTES at fixed 10 000 tiles (is source size free?)");
    for &bytes in &[0usize, REAL_TILE_BYTES] {
        let dir = tmp.join("src-b");
        synth_source(&src, &dir, 10_000, bytes, 70_322, 157_074)?;
        let on_disk = std::fs::metadata(dir.join("norway.dem"))?.len();
        let r = run(&dir, &tmp.join("out-b"))?;
        let label = if bytes == 0 { "thin" } else { "fat" };
        print_row(
            &format!("10000 tiles {label} {:.0} MB", on_disk as f64 / 1e6),
            &r,
        );
        std::fs::remove_dir_all(&dir).ok();
    }
    println!();

    // 3. Edge count. `slice_graph` walks every node and every edge in
    //    the source regardless of how few it keeps.
    println!("== sweep C: source EDGE COUNT (graph phase)");
    let mut c = Vec::new();
    for &(nc, ec) in &[
        (70_322usize, 157_074usize),
        (300_000, 700_000),
        (1_000_000, 2_400_000),
        (2_500_000, 6_000_000),
    ] {
        let dir = tmp.join("src-c");
        synth_source(&src, &dir, 1_360, 0, nc, ec)?;
        let r = run(&dir, &tmp.join("out-c"))?;
        print_row(&format!("{ec:>9} edges"), &r);
        c.push((ec as f64, r.graph));
        std::fs::remove_dir_all(&dir).ok();
    }
    println!();

    println!("== the answer");
    let per_tile = slope(&a);
    let per_edge = slope(&c);
    println!(
        "  per source tile (dem+verify): {:.2} us\n  per source edge (graph):      {:.3} us",
        per_tile * 1e6,
        per_edge * 1e6
    );
    // Norway's mainland is ~324 000 km2; a tile covers 2.56^2 = 6.55.
    let national_tiles = 324_000.0 / (TILE_SPAN_M / 1000.0).powi(2);
    let national_edges = 157_074.0 * (324_000.0 / 8_666.0);
    println!(
        "  national estimate: {:.0} DEM tiles, {:.0} directed edges",
        national_tiles, national_edges
    );
    println!(
        "  predicted cut: dem+verify {:.1} s + graph {:.1} s + mask {:.1} s (measured, already national) \
         + output copy {:.1} s (measured) = {:.1} s",
        per_tile * national_tiles,
        per_edge * national_edges,
        base.mask,
        base.dem,
        per_tile * national_tiles + per_edge * national_edges + base.mask + base.dem,
    );
    Ok(())
}

/// The national DEM's tile count, and the payload size that fits.
///
/// Norway's mainland is ~324 000 km²; a 256-cell tile at 10 m covers
/// 2.56² = 6.55 km², so a complete DTM10 pyramid is ~49 400 tiles. At
/// the real 87 KB per tile that is 4.3 GB, which does not fit here —
/// `SCALING_PAYLOAD` trims it, at the cost of shortening the seeks
/// between the tiles the slice copies. That biases the cold DEM number
/// optimistic; it is the one term this container cannot measure
/// faithfully, and the write-up says so.
fn national_dem_shape() -> (usize, usize) {
    let payload = std::env::var("SCALING_PAYLOAD")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(REAL_TILE_BYTES / 3);
    (49_438, payload)
}

/// One build's phase timings, in seconds.
struct Row {
    dem: f64,
    mask: f64,
    graph: f64,
    verify: f64,
    manifest: f64,
    total: f64,
    detail: String,
}

/// Bytes this process has actually pulled off the block device.
///
/// The most portable number in this whole example. Wall time here is a
/// property of one container on one VM whose *host* cache nothing in
/// the guest can drop; bytes-read is a property of the algorithm, and
/// it is what predicts the cut on hardware nobody here can touch —
/// divide by that machine's read throughput.
fn disk_read_bytes() -> u64 {
    std::fs::read_to_string("/proc/self/io")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("read_bytes:")?.trim().parse().ok())
        })
        .unwrap_or(0)
}

fn run(src: &Path, dst: &Path) -> Result<Row, Box<dyn std::error::Error>> {
    std::fs::remove_dir_all(dst).ok();
    let io_before = disk_read_bytes();
    let r = turbo_geodata_pack::build(src, dst, &turbo_geodata_pack::Region::Bbox(REGION), 1000.0)?;
    let read_mb = (disk_read_bytes().saturating_sub(io_before)) as f64 / 1e6;
    let get = |n: &str| {
        r.shrink
            .iter()
            .find(|s| s.name.starts_with(n))
            .map(|s| s.elapsed.as_secs_f64())
            .unwrap_or(0.0)
    };
    let row = Row {
        dem: get("dem"),
        mask: get("mask"),
        graph: get("graph"),
        verify: r.verify_elapsed.as_secs_f64(),
        manifest: r.manifest_elapsed.as_secs_f64(),
        total: r.total_elapsed.as_secs_f64(),
        detail: format!(
            "{:.1} MB out, {read_mb:>6.1} MB read from disk",
            r.after_bytes() as f64 / 1e6
        ),
    };
    std::fs::remove_dir_all(dst).ok();
    Ok(row)
}

fn print_row(label: &str, r: &Row) {
    println!(
        "  {label:<24} dem {:>6.2}  mask {:>5.2}  graph {:>5.2}  verify {:>5.2}  \
         manifest {:>5.2}  TOTAL {:>6.2} s  {}",
        r.dem, r.mask, r.graph, r.verify, r.manifest, r.total, r.detail
    );
}

/// Least-squares slope through the origin — the marginal cost per unit.
fn slope(pts: &[(f64, f64)]) -> f64 {
    let sxy: f64 = pts.iter().map(|(x, y)| x * y).sum();
    let sxx: f64 = pts.iter().map(|(x, _)| x * x).sum();
    sxy / sxx
}

/// Write a synthetic source directory at the requested scale.
///
/// The mask is **hard-linked from the real source**, not generated: it
/// is already national, so a synthetic one could only be less faithful.
/// The DEM and graph are generated, because the real ones are regional.
fn synth_source(
    real: &Path,
    dir: &Path,
    tiles: usize,
    payload_bytes: usize,
    nodes: usize,
    edges: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    let mask = dir.join("norway.mask");
    if !mask.exists() {
        // Hard link where possible; the file is 69 MB and every sweep
        // step would otherwise copy it.
        std::fs::hard_link(real.join("norway.mask"), &mask)
            .or_else(|_| std::fs::copy(real.join("norway.mask"), &mask).map(|_| ()))?;
    }
    synth_dem(&dir.join("norway.dem"), tiles, payload_bytes)?;
    synth_graph(dir, nodes, edges)?;
    Ok(())
}

/// A DEM with `tiles` entries laid out on the real grid.
///
/// Every tile shares one compressed payload. That is legitimate for
/// this measurement and not for anything else: the slice copies
/// payloads byte for byte without decoding them, and `verify` decodes
/// only the handful under the sampled region — where a constant surface
/// is as valid as any other. What is being measured is the *directory*,
/// and the directory is real.
fn synth_dem(
    path: &Path,
    tiles: usize,
    payload_bytes: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use turbo_tiles_elev::{
        write_meta, write_tile_entry, DemMeta, TileEntry, COMPRESSION_ZSTD, DEFAULT_TILE_CELLS,
        DEM_FORMAT_VERSION, DEM_META_BYTES, NODATA_SENTINEL, TILE_ENTRY_BYTES,
    };

    let cells = DEFAULT_TILE_CELLS as usize;
    let mut surface = vec![420.0f32; cells * cells];
    if payload_bytes > 0 {
        // zstd cannot compress noise, so noisy cells are the dial that
        // sets the payload size. One cheap LCG, no dependency.
        let mut s: u32 = 0x1234_5678;
        // ~4 bytes of output per noisy cell, found by measurement below.
        let noisy = (payload_bytes / 4).min(surface.len());
        for v in surface.iter_mut().take(noisy) {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *v = 100.0 + (s >> 8) as f32 / 1e4;
        }
    }
    let payload = zstd::encode_all(bytemuck::cast_slice(&surface), 3)?;

    let mut out = std::io::BufWriter::with_capacity(1 << 22, std::fs::File::create(path)?);
    write_header(
        &mut out,
        &Header {
            kind: ArtifactKind::Dem,
            format_version: DEM_FORMAT_VERSION,
            build_timestamp_unix_sec: 0,
        },
    )?;
    write_meta(
        &mut out,
        &DemMeta {
            tile_count: tiles as u32,
            tile_cells: DEFAULT_TILE_CELLS,
            pixel_size_m: 10.0,
            nodata: NODATA_SENTINEL,
            compression: COMPRESSION_ZSTD,
        },
    )?;

    // A square-ish grid anchored on the real origin, so the sampled
    // region is covered at every tile count.
    let side = (tiles as f64).sqrt().ceil() as usize;
    let mut at = (HEADER_BYTES + DEM_META_BYTES + tiles * TILE_ENTRY_BYTES) as u64;
    for i in 0..tiles {
        write_tile_entry(
            &mut out,
            &TileEntry {
                ulx: ORIGIN_X + (i % side) as f64 * TILE_SPAN_M,
                uly: ORIGIN_Y - (i / side) as f64 * TILE_SPAN_M,
                offset: at,
                compressed_size: payload.len() as u32,
            },
        )?;
        at += payload.len() as u64;
    }
    for _ in 0..tiles {
        out.write_all(&payload)?;
    }
    out.flush()?;
    Ok(())
}

/// A graph with `nodes` nodes and `edges` directed edges, with three
/// cost profiles and one polyline vertex pair per edge — the shape
/// `slice_graph` walks.
///
/// # Why the first 528 nodes are special
///
/// The sweep varies how much the slice must *read*. If it also varied
/// how much the slice *keeps*, the two would move together and the
/// resulting number would be neither one. So the graph is built in two
/// parts: a fixed block inside the region — 528 nodes and 1056 edges,
/// chosen to match what the real source yields for this bbox (490 and
/// 1044) — and everything else parked far to the south-west, outside
/// any box this example cuts. Kept output is then constant at every
/// scale, and the only thing changing is the length of the two loops.
fn synth_graph(dir: &Path, nodes: usize, edges: usize) -> Result<(), Box<dyn std::error::Error>> {
    use byteorder::{LittleEndian, WriteBytesExt};
    use turbo_tiles_graph::{
        write_graph_geom_meta, write_meta, EdgeRecord, GraphGeomMeta, GraphMeta, NodePos,
        GRAPH_FORMAT_VERSION, GRAPH_GEOM_FORMAT_VERSION,
    };
    const PC: usize = 3;
    /// 24 x 22 at 600 m, anchored inside the halo'd REGION.
    const IN_COLS: usize = 24;
    const IN_ROWS: usize = 22;
    const IN_NODES: usize = IN_COLS * IN_ROWS; // 528
    const IN_EDGES: usize = IN_NODES * 2; // 1056
    assert!(nodes > IN_NODES && edges > IN_EDGES);

    let out_side = ((nodes - IN_NODES) as f64).sqrt().ceil() as usize + 1;
    let node_at = |i: usize| {
        if i < IN_NODES {
            NodePos {
                x: (499_100.0 + (i % IN_COLS) as f64 * 600.0) as f32,
                y: (7_433_800.0 + (i / IN_COLS) as f64 * 600.0) as f32,
            }
        } else {
            // Far south-west of anything this example cuts.
            let k = i - IN_NODES;
            NodePos {
                x: (200_000.0 + (k % out_side) as f64 * 200.0) as f32,
                y: (7_000_000.0 - (k / out_side) as f64 * 200.0) as f32,
            }
        }
    };

    let t = Instant::now();
    let mut out =
        std::io::BufWriter::with_capacity(1 << 22, std::fs::File::create(dir.join("norway.graph"))?);
    write_header(
        &mut out,
        &Header {
            kind: ArtifactKind::Graph,
            format_version: GRAPH_FORMAT_VERSION,
            build_timestamp_unix_sec: 0,
        },
    )?;
    write_meta(
        &mut out,
        &GraphMeta {
            node_count: nodes as u32,
            edge_count: edges as u32,
            profile_count: PC as u32,
            srid: 25833,
        },
    )?;
    for i in 0..nodes {
        let p = node_at(i);
        out.write_all(bytemuck::bytes_of(&p))?;
    }
    // Each edge joins a node to its east or north neighbour, so BOTH
    // endpoints land inside the box — the only case `slice_graph`
    // retains. The first IN_EDGES stay within the in-region block; the
    // rest stay within the far block, so neither straddles.
    let edge_at = |e: usize| {
        let (from, to) = if e < IN_EDGES {
            let f = e / 2;
            let n = if e % 2 == 0 { f + 1 } else { f + IN_COLS };
            (f, if n < IN_NODES { n } else { f })
        } else {
            let k = (e - IN_EDGES) % (nodes - IN_NODES);
            let f = IN_NODES + k;
            let n = if e % 2 == 0 { f + 1 } else { f + out_side };
            (f, if n < nodes { n } else { f })
        };
        EdgeRecord {
            from_id: from as u32,
            to_id: to as u32,

            length_m: 200.0,
            gain_m: 4.0,
            loss_m: 3.0,
            slope_max_deg: 7.0,
            fkb_type: 2,
            marking: 1,
            surface: 1,
            source: 1,
            attr_flags: 0,
        }
    };
    for e in 0..edges {
        out.write_all(bytemuck::bytes_of(&edge_at(e)))?;
    }
    // CSR over the same edges.
    let mut counts = vec![0u32; nodes];
    for e in 0..edges {
        counts[edge_at(e).from_id as usize] += 1;
    }
    let mut offsets = Vec::with_capacity(nodes + 1);
    let mut acc = 0u32;
    offsets.push(0u32);
    for c in &counts {
        acc += c;
        offsets.push(acc);
    }
    out.write_all(bytemuck::cast_slice(&offsets))?;
    let mut cursor: Vec<u32> = offsets[..nodes].to_vec();
    let mut csr = vec![0u32; edges];
    for e in 0..edges {
        let n = edge_at(e).from_id as usize;
        csr[cursor[n] as usize] = e as u32;
        cursor[n] += 1;
    }
    out.write_all(bytemuck::cast_slice(&csr))?;
    for e in 0..edges {
        for p in 0..PC {
            out.write_f32::<LittleEndian>(200.0 + e as f32 % 7.0 + p as f32)?;
        }
    }
    out.flush()?;
    drop(out);

    // The polyline sibling: two vertices per edge, the minimum that
    // exercises the index rebuild.
    let mut out = std::io::BufWriter::with_capacity(
        1 << 22,
        std::fs::File::create(dir.join("norway.graph_geom"))?,
    );
    write_header(
        &mut out,
        &Header {
            kind: ArtifactKind::GraphGeom,
            format_version: GRAPH_GEOM_FORMAT_VERSION,
            build_timestamp_unix_sec: 0,
        },
    )?;
    write_graph_geom_meta(
        &mut out,
        &GraphGeomMeta {
            edge_count: edges as u32,
            total_vertices: (edges * 2) as u32,
        },
    )?;
    for e in 0..edges {
        out.write_u32::<LittleEndian>((e * 2) as u32)?;
        out.write_u32::<LittleEndian>(2)?;
    }
    for e in 0..edges {
        let r = edge_at(e);
        out.write_all(bytemuck::bytes_of(&node_at(r.from_id as usize)))?;
        out.write_all(bytemuck::bytes_of(&node_at(r.to_id as usize)))?;
    }
    out.flush()?;
    eprintln!(
        "   (generated {nodes} nodes / {edges} edges in {:.1} s)",
        t.elapsed().as_secs_f64()
    );
    Ok(())
}
