//! `tileserver slice-pack` — cut a small, self-contained routing pack
//! out of a large artifact set.
//!
//! The full Sjunkhatten artifacts are **209 MB**, which is fine on a
//! workstation and impossible in git. Without something checked in, the
//! routing gate can only run where someone has already provisioned the
//! data by hand — so in practice it runs on one machine, occasionally,
//! and the invariants it protects erode between runs. That is the
//! problem this solves: a pack small enough to commit, so the gate runs
//! in CI on every change.
//!
//! # What it does not claim
//!
//! A sliced pack does **not** reproduce the full pack's geometry
//! hashes, and this is not a rounding artefact. Experiment E10 found
//! that `Dem::sample` is not a pure function of position when tiles
//! overlap (76 overlapping pairs in the Sjunkhatten DEM): which of two
//! tiles answers depends on the r-tree's traversal, and dropping one by
//! slicing changes the answer. That is defect D8, still open.
//!
//! So a sliced pack is a **self-consistent gate with its own baseline**,
//! not a cheaper way to compute the same numbers. It catches what a
//! regression gate must catch — a structural change that was supposed
//! to be behaviour-preserving and wasn't — because that shows up as a
//! hash change against *its own* baseline. It cannot be compared across
//! packs, and the tooling never invites you to try.
//!
//! # Halo
//!
//! E10 also measured how much terrain beyond the routes a slice needs
//! before the answers stop changing: **500 m suffices**; the default
//! here is 1000 m, since the marginal cost of the extra ring is small
//! and the marginal cost of discovering it was too tight is a
//! mysteriously-moving baseline.
//!
//! # Usage
//!
//! ```text
//! tileserver slice-pack \
//!     --src ~/.data/artifacts --dst tools/ci-pack \
//!     --corpus tools/sjunkhatten-ci-corpus.toml --halo-m 1000
//! ```

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use turbo_tiles_artifacts::{read_header, write_header, ArtifactKind, Header, HEADER_BYTES};

/// A planar bounding box, the unit this whole tool works in.
#[derive(Debug, Clone, Copy)]
pub struct Bbox {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Bbox {
    fn expand(self, m: f64) -> Self {
        Self {
            min_x: self.min_x - m,
            min_y: self.min_y - m,
            max_x: self.max_x + m,
            max_y: self.max_y + m,
        }
    }
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }
}

/// One line of the report: what shrank, and by how much.
struct Shrink {
    name: &'static str,
    before: u64,
    after: u64,
    detail: String,
}

pub fn run(
    src: &Path,
    dst: &Path,
    corpus: &Path,
    halo_m: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let bbox = corpus_bbox(corpus)?.expand(halo_m);
    println!(
        "bbox (halo {halo_m:.0} m): [{:.0}, {:.0}] .. [{:.0}, {:.0}]  ({:.1} x {:.1} km)",
        bbox.min_x,
        bbox.min_y,
        bbox.max_x,
        bbox.max_y,
        (bbox.max_x - bbox.min_x) / 1000.0,
        (bbox.max_y - bbox.min_y) / 1000.0,
    );
    std::fs::create_dir_all(dst)?;

    let report = [
        slice_dem(&src.join("norway.dem"), &dst.join("norway.dem"), bbox)?,
        slice_mask(&src.join("norway.mask"), &dst.join("norway.mask"), bbox)?,
        slice_graph(src, dst, bbox)?,
    ];

    verify(src, dst, corpus)?;

    println!();
    let (mut b, mut a) = (0u64, 0u64);
    for r in &report {
        println!(
            "  {:<14} {:>8.1} MB -> {:>6.1} MB   {}",
            r.name,
            r.before as f64 / 1e6,
            r.after as f64 / 1e6,
            r.detail
        );
        b += r.before;
        a += r.after;
    }
    println!(
        "  {:<14} {:>8.1} MB -> {:>6.1} MB   ({:.1}% of original)",
        "TOTAL",
        b as f64 / 1e6,
        a as f64 / 1e6,
        100.0 * a as f64 / b as f64
    );
    Ok(())
}

/// The bbox enclosing every vertex of every hike in the corpus.
///
/// Endpoints are not enough: a hike's polyline can bow well outside the
/// box its two ends define, and a slice cut to the endpoints would drop
/// terrain the route walks through.
fn corpus_bbox(path: &Path) -> Result<Bbox, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)?;
    let doc: toml::Value = toml::from_str(&text)?;
    let hikes = doc
        .get("hike")
        .and_then(|h| h.as_array())
        .ok_or("corpus has no [[hike]] entries")?;
    let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
    let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    let mut vertices = 0usize;
    for h in hikes {
        let poly = h
            .get("polyline")
            .and_then(|p| p.as_array())
            .ok_or("hike without a polyline")?;
        for c in poly {
            let c = c.as_array().ok_or("polyline vertex is not a pair")?;
            let (lon, lat) = (c[0].as_float().ok_or("lon")?, c[1].as_float().ok_or("lat")?);
            let p = turbo_geo_frame::wgs84_to_utm33n(lon, lat);
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
            vertices += 1;
        }
    }
    println!("corpus: {} hikes, {vertices} vertices", hikes.len());
    Ok(Bbox {
        min_x,
        min_y,
        max_x,
        max_y,
    })
}

/// The DEM is a tile directory plus opaque compressed payloads, so
/// slicing is a filter on the directory — tiles are copied byte for
/// byte, never re-encoded. That matters: a re-encode would change
/// sample values and make the pack's numbers a property of this tool's
/// zstd settings rather than of the terrain.
fn slice_dem(src: &Path, dst: &Path, bbox: Bbox) -> Result<Shrink, Box<dyn std::error::Error>> {
    use turbo_tiles_elev::format::{read_meta, read_tile_entry};
    use turbo_tiles_elev::{
        write_meta, write_tile_entry, TileEntry, DEM_META_BYTES, TILE_ENTRY_BYTES,
    };
    let before = std::fs::metadata(src)?.len();
    let mut f = std::fs::File::open(src)?;
    let hdr = read_header(&mut f)?;
    let meta = read_meta(&mut f)?;
    let span = meta.tile_cells as f64 * meta.pixel_size_m as f64;

    let mut entries = Vec::with_capacity(meta.tile_count as usize);
    for _ in 0..meta.tile_count {
        entries.push(read_tile_entry(&mut f)?);
    }
    // A tile covers [ulx, ulx+span] x [uly-span, uly]; keep any that
    // intersects the box at all.
    let keep: Vec<&TileEntry> = entries
        .iter()
        .filter(|e| {
            e.ulx + span >= bbox.min_x
                && e.ulx <= bbox.max_x
                && e.uly >= bbox.min_y
                && e.uly - span <= bbox.max_y
        })
        .collect();

    let mut out = std::io::BufWriter::new(std::fs::File::create(dst)?);
    write_header(&mut out, &hdr)?;
    write_meta(
        &mut out,
        &turbo_tiles_elev::DemMeta {
            tile_count: keep.len() as u32,
            ..meta
        },
    )?;
    let mut payload_at = (HEADER_BYTES + DEM_META_BYTES + keep.len() * TILE_ENTRY_BYTES) as u64;
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
    for e in &keep {
        f.seek(SeekFrom::Start(e.offset))?;
        let mut buf = vec![0u8; e.compressed_size as usize];
        f.read_exact(&mut buf)?;
        out.write_all(&buf)?;
    }
    out.flush()?;
    drop(out);
    Ok(Shrink {
        name: "dem",
        before,
        after: std::fs::metadata(dst)?.len(),
        detail: format!("{} / {} tiles", keep.len(), meta.tile_count),
    })
}

/// The mask is a dense 2-bit raster, so slicing means re-cropping.
///
/// **Rows run downward from `max_y`**, not upward from `min_y` —
/// `Mask::refused` computes `row = (max_y - y) / res`. Getting this
/// backwards is not a subtle off-by-one: it mirrors the raster
/// vertically, so land reads as water and routes are refused at
/// endpoints that are plainly dry. That is exactly what the first
/// version of this function did, and it cost 10 of 25 corpus routes.
/// [`verify`] exists because of it.
fn slice_mask(src: &Path, dst: &Path, bbox: Bbox) -> Result<Shrink, Box<dyn std::error::Error>> {
    use turbo_tiles_mask::{read_meta, write_meta, MaskMeta, MASK_META_BYTES};
    let before = std::fs::metadata(src)?.len();
    let mut f = std::fs::File::open(src)?;
    let hdr = read_header(&mut f)?;
    let meta = read_meta(&mut f)?;
    let res = meta.resolution_m as f64;

    // Columns run right from min_x; rows run DOWN from max_y.
    let col_of = |x: f64| ((x - meta.min_x) / res).floor() as i64;
    let row_of = |y: f64| ((meta.max_y - y) / res).floor() as i64;
    let col0 = col_of(bbox.min_x).clamp(0, meta.cells_x as i64) as u32;
    let col1 = (col_of(bbox.max_x) + 1).clamp(0, meta.cells_x as i64) as u32;
    // max_y maps to the SMALLEST row, so the top row comes from bbox.max_y.
    let row0 = row_of(bbox.max_y).clamp(0, meta.cells_y as i64) as u32;
    let row1 = (row_of(bbox.min_y) + 1).clamp(0, meta.cells_y as i64) as u32;
    let (nx, ny) = (
        col1.saturating_sub(col0).max(1),
        row1.saturating_sub(row0).max(1),
    );

    let mut payload = Vec::new();
    f.seek(SeekFrom::Start((HEADER_BYTES + MASK_META_BYTES) as u64))?;
    f.read_to_end(&mut payload)?;

    let get = |col: u32, row: u32| -> u8 {
        let idx = row as usize * meta.cells_x as usize + col as usize;
        let byte = idx / 4;
        if byte >= payload.len() {
            return 0;
        }
        (payload[byte] >> (2 * (idx % 4))) & 0b11
    };

    let mut packed = vec![0u8; (nx as usize * ny as usize).div_ceil(4)];
    for j in 0..ny {
        for i in 0..nx {
            let v = get(col0 + i, row0 + j);
            if v == 0 {
                continue;
            }
            let idx = j as usize * nx as usize + i as usize;
            packed[idx / 4] |= v << (2 * (idx % 4));
        }
    }

    let mut out = std::io::BufWriter::new(std::fs::File::create(dst)?);
    write_header(&mut out, &hdr)?;
    write_meta(
        &mut out,
        &MaskMeta {
            min_x: meta.min_x + col0 as f64 * res,
            max_x: meta.min_x + col1 as f64 * res,
            // Row `row0` is the TOP of the crop, so it carries max_y.
            max_y: meta.max_y - row0 as f64 * res,
            min_y: meta.max_y - row1 as f64 * res,
            cells_x: nx,
            cells_y: ny,
            resolution_m: meta.resolution_m,
        },
    )?;
    out.write_all(&packed)?;
    out.flush()?;
    drop(out);
    Ok(Shrink {
        name: "mask",
        before,
        after: std::fs::metadata(dst)?.len(),
        detail: format!("{nx}x{ny} of {}x{} cells", meta.cells_x, meta.cells_y),
    })
}

/// The graph and its polyline sibling, sliced together.
///
/// They must be sliced together because `graph_geom`'s index is keyed
/// by edge position: keeping an edge in one and dropping it from the
/// other silently shifts every subsequent polyline onto the wrong edge.
/// `Graph::attach_geom` checks the counts match, which turns that class
/// of mistake into a load error rather than a routing mystery — but
/// only if the counts actually diverge, so the safe thing is to build
/// both from one decision.
///
/// An edge is kept when **both** endpoints are inside the box. Keeping
/// half-edges would leave dangling node ids; extending the box to
/// include the far endpoint would grow it unboundedly along a long
/// trail.
fn slice_graph(src: &Path, dst: &Path, bbox: Bbox) -> Result<Shrink, Box<dyn std::error::Error>> {
    use byteorder::{LittleEndian, WriteBytesExt};
    use turbo_tiles_graph::{
        write_graph_geom_meta, write_meta, EdgeRecord, Graph, GraphGeomMeta, GraphMeta, NodePos,
        GRAPH_FORMAT_VERSION, GRAPH_GEOM_FORMAT_VERSION,
    };

    let graph_src = src.join("norway.graph");
    let geom_src = src.join("norway.graph_geom");
    let before = std::fs::metadata(&graph_src)?.len()
        + std::fs::metadata(&geom_src).map(|m| m.len()).unwrap_or(0);

    let mut g = Graph::open(&graph_src)?;
    let has_geom = geom_src.exists() && g.attach_geom(&geom_src)?;
    let stats = g.stats();
    let (nc, ec, pc) = (
        stats.meta.node_count as usize,
        stats.meta.edge_count as usize,
        stats.meta.profile_count as usize,
    );

    // Node id -> new id, for the nodes inside the box.
    let mut remap: HashMap<u32, u32> = HashMap::new();
    let mut nodes: Vec<NodePos> = Vec::new();
    for id in 0..nc as u32 {
        if let Some(p) = g.node(id) {
            if bbox.contains(p.x as f64, p.y as f64) {
                remap.insert(id, nodes.len() as u32);
                nodes.push(p);
            }
        }
    }

    let mut edges: Vec<EdgeRecord> = Vec::new();
    let mut kept_edge_ids: Vec<u32> = Vec::new();
    for eid in 0..ec as u32 {
        let e = match g.edge(eid) {
            Some(e) => e,
            None => continue,
        };
        let (Some(&from), Some(&to)) = (remap.get(&e.from_id), remap.get(&e.to_id)) else {
            continue;
        };
        edges.push(EdgeRecord {
            from_id: from,
            to_id: to,
            ..*e
        });
        kept_edge_ids.push(eid);
    }

    // Rebuild CSR adjacency over the renumbered nodes.
    let mut per_node: Vec<Vec<u32>> = vec![Vec::new(); nodes.len()];
    for (new_eid, e) in edges.iter().enumerate() {
        per_node[e.from_id as usize].push(new_eid as u32);
    }
    let mut csr_offsets: Vec<u32> = Vec::with_capacity(nodes.len() + 1);
    let mut csr_edges: Vec<u32> = Vec::with_capacity(edges.len());
    csr_offsets.push(0);
    for adj in &per_node {
        csr_edges.extend_from_slice(adj);
        csr_offsets.push(csr_edges.len() as u32);
    }

    let mut out = std::io::BufWriter::new(std::fs::File::create(dst.join("norway.graph"))?);
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
            node_count: nodes.len() as u32,
            edge_count: edges.len() as u32,
            profile_count: pc as u32,
            srid: stats.meta.srid,
        },
    )?;
    out.write_all(bytemuck::cast_slice(&nodes))?;
    out.write_all(bytemuck::cast_slice(&edges))?;
    out.write_all(bytemuck::cast_slice(&csr_offsets))?;
    out.write_all(bytemuck::cast_slice(&csr_edges))?;
    for &eid in &kept_edge_ids {
        for p in 0..pc {
            out.write_f32::<LittleEndian>(g.edge_cost(eid, p))?;
        }
    }
    out.flush()?;
    drop(out);

    let mut after = std::fs::metadata(dst.join("norway.graph"))?.len();

    if has_geom {
        let mut index: Vec<(u32, u32)> = Vec::with_capacity(kept_edge_ids.len());
        let mut verts: Vec<NodePos> = Vec::new();
        for &eid in &kept_edge_ids {
            let pts = g.edge_polyline(eid);
            index.push((verts.len() as u32, pts.len() as u32));
            verts.extend_from_slice(&pts);
        }
        let mut out =
            std::io::BufWriter::new(std::fs::File::create(dst.join("norway.graph_geom"))?);
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
                edge_count: index.len() as u32,
                total_vertices: verts.len() as u32,
            },
        )?;
        for (off, cnt) in &index {
            out.write_u32::<LittleEndian>(*off)?;
            out.write_u32::<LittleEndian>(*cnt)?;
        }
        out.write_all(bytemuck::cast_slice(&verts))?;
        out.flush()?;
        drop(out);
        after += std::fs::metadata(dst.join("norway.graph_geom"))?.len();
    }

    Ok(Shrink {
        name: "graph+geom",
        before,
        after,
        detail: format!("{} / {nc} nodes, {} / {ec} edges", nodes.len(), edges.len()),
    })
}

/// Sample both packs at every corpus vertex and require they agree.
///
/// This is not belt-and-braces. The first version of [`slice_mask`]
/// indexed rows upward from `min_y` where the format runs them downward
/// from `max_y`, mirroring the raster: land read as water, and 10 of 25
/// corpus routes failed with "endpoint refused by layer 'water'". The
/// pack still loaded, still solved, still produced a stable hash — it
/// was simply wrong, and a baseline taken from it would have enshrined
/// the wrongness as the thing CI protects.
///
/// A slice is a transformation with an exact, cheap oracle: the source.
/// Not checking against it would be a choice to find out later.
///
/// DEM *values* are checked, not just coverage, but with one documented
/// exception — see the mismatch handling below.
fn verify(src: &Path, dst: &Path, corpus: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use turbo_tiles_elev::{Dem, PointXY};
    use turbo_tiles_mask::Mask;

    let text = std::fs::read_to_string(corpus)?;
    let doc: toml::Value = toml::from_str(&text)?;
    let hikes = doc.get("hike").and_then(|h| h.as_array()).unwrap();
    let mut pts = Vec::new();
    for h in hikes {
        for c in h.get("polyline").and_then(|p| p.as_array()).unwrap() {
            let c = c.as_array().unwrap();
            pts.push(turbo_geo_frame::wgs84_to_utm33n(
                c[0].as_float().unwrap(),
                c[1].as_float().unwrap(),
            ));
        }
    }

    let (a_dem, b_dem) = (
        Dem::open(src.join("norway.dem"))?,
        Dem::open(dst.join("norway.dem"))?,
    );
    let (a_mask, b_mask) = (
        Mask::open(src.join("norway.mask"))?,
        Mask::open(dst.join("norway.mask"))?,
    );

    let (mut dem_diff, mut mask_diff, mut dem_gone, mut mask_gone) = (0, 0, 0, 0);
    for p in &pts {
        let q = PointXY { x: p.x, y: p.y };
        match (
            a_dem.sample(q).ok().flatten(),
            b_dem.sample(q).ok().flatten(),
        ) {
            (Some(a), Some(b)) if a != b => dem_diff += 1,
            (Some(_), None) => dem_gone += 1,
            _ => {}
        }
        match (a_mask.refused(p.x, p.y), b_mask.refused(p.x, p.y)) {
            (Ok(a), Ok(b)) if a != b => mask_diff += 1,
            (Ok(_), Err(_)) => mask_gone += 1,
            _ => {}
        }
    }

    println!(
        "verify: {} corpus vertices — dem {dem_diff} differ / {dem_gone} lost, \
         mask {mask_diff} differ / {mask_gone} lost",
        pts.len()
    );
    if mask_diff > 0 || mask_gone > 0 || dem_gone > 0 {
        return Err(format!(
            "slice disagrees with source at {} vertices (mask {mask_diff} differ, \
             {mask_gone} uncovered; dem {dem_gone} uncovered) — the pack is wrong, \
             not merely different",
            mask_diff + mask_gone + dem_gone
        )
        .into());
    }
    if dem_diff > 0 {
        // Defect D8: overlapping DEM tiles make `sample` depend on which
        // tiles are present, so dropping one of an overlapping pair can
        // legitimately change a value. Reported, never silent — but not
        // fatal, because it is the known reason a sliced pack needs its
        // own baseline.
        println!(
            "  note: {dem_diff} DEM values differ. This is defect D8 (overlapping \
             tiles), and it is why the sliced pack carries its own baseline."
        );
    }
    Ok(())
}
