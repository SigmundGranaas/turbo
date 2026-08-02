//! **Region packs** — cutting one, naming one, checking one.
//!
//! An L5 format, not an engine format: the engine never hears of a pack,
//! it is handed artifacts. This lives in its own crate because the code
//! used to sit inside the CLI binary, which meant the *server* could not
//! serve what the CLI could build — and serving packs is the whole point
//! of building them for a phone. `tileserver slice-pack` and
//! `GET /v1/packs/...` are two callers of one implementation.
//!
//! Cut a small, self-contained routing pack out of a large artifact set.
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
//!
//! # Nothing here prints
//!
//! [`build`] returns a [`Report`]; the CLI formats it. A library that
//! writes to stdout is one a server cannot call without polluting its
//! logs, and the server is now the more important caller.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use turbo_tiles_artifacts::{
    read_header, write_header, ArtifactKind, Header, PackFile, PackManifest, PackMeta,
    HEADER_BYTES, PACK_FORMAT_VERSION,
};

/// A TOML number as `f64`.
///
/// `as_float()` alone returns `None` for an integer-valued coordinate —
/// a corpus vertex written `67` rather than `67.0` — which surfaced as
/// the useless error "lat". Coordinates are numbers; how they were
/// spelled is not the reader's problem.
fn num(v: &toml::Value) -> Option<f64> {
    v.as_float().or_else(|| v.as_integer().map(|i| i as f64))
}

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

/// One line of the report: what shrank, by how much, and how long it took.
///
/// `elapsed` is here because the sizes alone cannot answer the question
/// the server asks. `GET /v1/packs/...` cuts a region on demand and
/// waits [a bounded time][1] before answering "come back"; whether that
/// bound is right depends on which phase costs what, and the three
/// phases scale on different axes — the DEM on the source's *tile
/// count*, the mask on the source's *cell count*, the graph on the
/// source's *edge count*. A single total hides which one to fix.
///
/// [1]: ../turbo_tiles_api/packs/constant.BUILD_WAIT.html
pub struct Shrink {
    pub name: &'static str,
    pub before: u64,
    pub after: u64,
    pub detail: String,
    pub elapsed: std::time::Duration,
}

/// Run `f`, returning its [`Shrink`] with `elapsed` filled in.
fn timed(
    f: impl FnOnce() -> Result<Shrink, Box<dyn std::error::Error>>,
) -> Result<Shrink, Box<dyn std::error::Error>> {
    let t = std::time::Instant::now();
    let mut s = f()?;
    s.elapsed = t.elapsed();
    Ok(s)
}

/// What region to cut, and how it was specified.
///
/// Two callers with different knowledge. The routing gate has a corpus
/// and wants a pack that covers every hike in it. An app has a map
/// viewport and wants a pack for that rectangle — and has no corpus,
/// because the whole point is to route somewhere nobody has walked yet.
/// Deriving one from the other is not possible in either direction, so
/// the tool takes both and the caller says which it has.
pub enum Region {
    /// Cover every vertex of every hike in this corpus.
    Corpus(std::path::PathBuf),
    /// Cover this WGS84 rectangle: `[min_lon, min_lat, max_lon, max_lat]`.
    Bbox([f64; 4]),
}

impl Region {
    /// The requested region in planar metres, BEFORE the halo.
    fn planar(&self) -> Result<Bbox, Box<dyn std::error::Error>> {
        match self {
            Region::Corpus(p) => corpus_bbox(p),
            Region::Bbox([min_lon, min_lat, max_lon, max_lat]) => {
                if min_lon >= max_lon || min_lat >= max_lat {
                    return Err(format!(
                        "bbox must be min_lon,min_lat,max_lon,max_lat with min < max; \
                         got [{min_lon}, {min_lat}, {max_lon}, {max_lat}]"
                    )
                    .into());
                }
                // All four corners, not two: a lon/lat rectangle is not a
                // rectangle in UTM, and projecting only the diagonal
                // corners would cut inside the requested area along the
                // bowed edges.
                let corners = [
                    turbo_geo_frame::wgs84_to_utm33n(*min_lon, *min_lat),
                    turbo_geo_frame::wgs84_to_utm33n(*min_lon, *max_lat),
                    turbo_geo_frame::wgs84_to_utm33n(*max_lon, *min_lat),
                    turbo_geo_frame::wgs84_to_utm33n(*max_lon, *max_lat),
                ];
                Ok(Bbox {
                    min_x: corners.iter().map(|p| p.x).fold(f64::INFINITY, f64::min),
                    min_y: corners.iter().map(|p| p.y).fold(f64::INFINITY, f64::min),
                    max_x: corners
                        .iter()
                        .map(|p| p.x)
                        .fold(f64::NEG_INFINITY, f64::max),
                    max_y: corners
                        .iter()
                        .map(|p| p.y)
                        .fold(f64::NEG_INFINITY, f64::max),
                })
            }
        }
    }
}

/// What a build did, for the caller to report as it sees fit.
pub struct Report {
    /// The region as cut, planar, INCLUDING the halo.
    pub bbox: Bbox,
    /// Per-artifact before/after sizes.
    pub shrink: Vec<Shrink>,
    /// The manifest written into the pack.
    pub manifest: turbo_tiles_artifacts::PackManifest,
    /// Points sampled by [`verify`], for the caller to report.
    pub verified_points: usize,
    /// Time in [`verify`] — re-opening BOTH packs and sampling.
    ///
    /// Separate from the slice phases because it is the one phase whose
    /// cost is not obviously proportional to anything: it opens the
    /// *source* DEM, which bulk-loads an r-tree over every tile in it.
    /// On a regional source that is invisible; on a national one it is
    /// the largest single term, and a total would not say so.
    pub verify_elapsed: std::time::Duration,
    /// Time in [`write_manifest`] — dominated by digesting the output.
    pub manifest_elapsed: std::time::Duration,
    /// Wall time for the whole of [`build`].
    pub total_elapsed: std::time::Duration,
}

impl Report {
    pub fn before_bytes(&self) -> u64 {
        self.shrink.iter().map(|s| s.before).sum()
    }
    pub fn after_bytes(&self) -> u64 {
        self.shrink.iter().map(|s| s.after).sum()
    }
}

/// Cut a pack for `region` into `dst`, keeping `halo_m` of terrain
/// beyond it.
///
/// Writes the artifacts, verifies them against the source, and writes
/// the manifest last — so a directory with a `pack.toml` is a directory
/// whose contents were checked. A caller that finds a pack without one
/// is looking at an interrupted build.
pub fn build(
    src: &Path,
    dst: &Path,
    region: &Region,
    halo_m: f64,
) -> Result<Report, Box<dyn std::error::Error>> {
    let started = std::time::Instant::now();
    let requested = region.planar()?;
    let bbox = requested.expand(halo_m);
    std::fs::create_dir_all(dst)?;

    let shrink = vec![
        timed(|| slice_dem(&src.join("norway.dem"), &dst.join("norway.dem"), bbox))?,
        timed(|| slice_mask(&src.join("norway.mask"), &dst.join("norway.mask"), bbox))?,
        timed(|| slice_graph(src, dst, bbox))?,
    ];

    let t = std::time::Instant::now();
    let verified_points = verify(src, dst, region, requested)?;
    let verify_elapsed = t.elapsed();

    let t = std::time::Instant::now();
    let manifest = write_manifest(dst, region, requested, halo_m)?;
    let manifest_elapsed = t.elapsed();

    Ok(Report {
        bbox,
        shrink,
        manifest,
        verified_points,
        verify_elapsed,
        manifest_elapsed,
        total_elapsed: started.elapsed(),
    })
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
            let (lon, lat) = (
                num(&c[0]).ok_or("polyline vertex has a non-numeric lon")?,
                num(&c[1]).ok_or("polyline vertex has a non-numeric lat")?,
            );
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
        elapsed: Default::default(),
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
        elapsed: Default::default(),
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
        elapsed: Default::default(),
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
fn verify(
    src: &Path,
    dst: &Path,
    region: &Region,
    requested: Bbox,
) -> Result<usize, Box<dyn std::error::Error>> {
    use turbo_tiles_elev::{Dem, PointXY};
    use turbo_tiles_mask::Mask;

    // A grid over the requested region, ALWAYS — the check that does not
    // depend on having a corpus, and the one that would have caught the
    // mask-row inversion that shipped in the first version of this tool
    // (rows run down from max_y; indexing up from min_y mirrors the
    // raster, so land reads as water and endpoints get refused). Corpus
    // vertices missed it because they happened to sit near the middle.
    //
    // 64 x 64 over the region: dense enough that a mirrored or shifted
    // raster cannot hide between samples, cheap enough to be unnoticed.
    const GRID: usize = 64;
    let mut pts: Vec<turbo_tiles_pathfind::Point> = Vec::with_capacity(GRID * GRID);
    for j in 0..GRID {
        for i in 0..GRID {
            let t = |a: f64, b: f64, k: usize| a + (b - a) * (k as f64 / (GRID - 1) as f64);
            pts.push(turbo_tiles_pathfind::Point {
                x: t(requested.min_x, requested.max_x, i),
                y: t(requested.min_y, requested.max_y, j),
            });
        }
    }
    let _grid_pts = pts.len();

    // Plus every corpus vertex, when there is a corpus: those are the
    // points a route actually walks, and the gate's baseline depends on
    // them agreeing exactly.
    if let Region::Corpus(corpus) = region {
        let text = std::fs::read_to_string(corpus)?;
        let doc: toml::Value = toml::from_str(&text)?;
        let hikes = doc.get("hike").and_then(|h| h.as_array()).unwrap();
        for h in hikes {
            for c in h.get("polyline").and_then(|p| p.as_array()).unwrap() {
                let c = c.as_array().unwrap();
                pts.push(turbo_geo_frame::wgs84_to_utm33n(
                    num(&c[0]).unwrap(),
                    num(&c[1]).unwrap(),
                ));
            }
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
        // Since D8 was fixed this should be unreachable: `find_tile`
        // applies a canonical rule, so a slice reports the same terrain
        // as its source. Kept as a loud diagnostic rather than deleted —
        // if it ever fires, the tie-break has regressed and the pack is
        // not trustworthy.
        tracing::warn!(
            dem_diff,
            "DEM values differ between source and slice. D8 was fixed; this \
             should not happen. Check `Dem::find_tile`'s tie-break before trusting \
             this pack."
        );
    }
    Ok(pts.len())
}

/// Write `pack.toml` beside the artifacts.
///
/// The extent recorded is the REQUESTED region, projected back to WGS84
/// — not the halo, and not the DEM's tile-aligned coverage. Both of
/// those are larger, and a host that gated its UI on them would offer
/// routing in a margin that exists to make routing *inside* the region
/// correct, not to be routed in itself.
fn write_manifest(
    dst: &Path,
    region: &Region,
    requested: Bbox,
    halo_m: f64,
) -> Result<PackManifest, Box<dyn std::error::Error>> {
    // A bbox request echoes the caller's own rectangle; a corpus
    // request has no lon/lat to echo and projects its planar box back.
    //
    // The distinction is not pedantry. A lon/lat rectangle is not a
    // rectangle in UTM, so `Region::planar` takes the bounding box of
    // all four projected corners — which CONTAINS the request. Round-
    // tripping that back would record an extent slightly larger than
    // what was asked for (~35 m of longitude here), and an extent that
    // over-promises is the one thing this field must never do: a host
    // gates "can I route here?" on it.
    let [min_lon, min_lat, max_lon, max_lat] = match region {
        Region::Bbox(b) => *b,
        Region::Corpus(_) => {
            let (min_lon, min_lat) =
                turbo_geo_frame::utm33n_to_wgs84(requested.min_x, requested.min_y);
            let (max_lon, max_lat) =
                turbo_geo_frame::utm33n_to_wgs84(requested.max_x, requested.max_y);
            [min_lon, min_lat, max_lon, max_lat]
        }
    };

    let manifest = PackManifest {
        pack: PackMeta {
            format_version: PACK_FORMAT_VERSION,
            created_by: format!("tileserver slice-pack {}", env!("CARGO_PKG_VERSION")),
            frame: "utm33n".to_string(),
            extent: [min_lon, min_lat, max_lon, max_lat],
            halo_m,
            files: pack_files(dst)?,
        },
    };
    // `to_string`, not `to_string_pretty`: pretty splits arrays across lines,
    // and `extent` is read back by a four-line parser on the Android side that
    // deliberately does not take a TOML dependency to read one array out of a
    // file this repo also writes. A single-line array keeps that honest — and
    // reads better in the manifest besides.
    let body = toml::to_string(&manifest)?;
    let header = match region {
        Region::Corpus(p) => format!("# Cut from corpus {}\n", p.display()),
        Region::Bbox(b) => format!("# Cut from bbox {b:?}\n"),
    };
    std::fs::write(
        dst.join(PackManifest::FILENAME),
        format!(
            "# Region pack manifest — written by `tileserver slice-pack`,\n\
             # read by `RouteEngine::open`. Hand-editing the extent does not\n\
             # change what the pack contains.\n{header}\n{body}"
        ),
    )?;
    Ok(manifest)
}

/// Every artifact in `dir`, sized and digested, sorted by name.
///
/// Sorted so the manifest is byte-stable for a given pack: directory
/// order is filesystem-dependent, and a manifest that differs between
/// two builds of the same region would make the pack look changed to
/// every cache in front of it.
///
/// The manifest itself is excluded — it cannot contain its own digest,
/// and its integrity is the transport's problem: a client that got a
/// corrupt manifest cannot parse it, which is a loud failure rather than
/// a silent one.
fn pack_files(dir: &Path) -> Result<Vec<PackFile>, Box<dyn std::error::Error>> {
    use sha2::{Digest, Sha256};

    let mut out = Vec::new();
    let mut names: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n != PackManifest::FILENAME)
        .collect();
    names.sort();

    for name in names {
        let path = dir.join(&name);
        let mut f = std::fs::File::open(&path)?;
        let mut hasher = Sha256::new();
        // Streamed: the DEM is tens of megabytes and there is no reason
        // to hold it, especially on a server building packs on demand
        // under a concurrency cap.
        let mut buf = vec![0u8; 1 << 20];
        let mut bytes = 0u64;
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            bytes += n as u64;
            hasher.update(&buf[..n]);
        }
        out.push(PackFile {
            name,
            bytes,
            sha256: hasher
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        });
    }
    Ok(out)
}

// ---- naming a pack --------------------------------------------------

/// The zoom level pack keys quantise to.
///
/// A z12 cell is **3.8 km at 67°N and 4.9 km at 60°N** — fine enough
/// that snapping a viewport outward wastes little ground, coarse enough
/// that a region is a handful of cells rather than hundreds.
pub const PACK_GRID_Z: u32 = 12;

/// A pack's name: a rectangle of grid cells.
///
/// # Why a pack has to be named this way
///
/// The edge tier in front of the tileserver keys its cache on the
/// request path and requires every cached object to be regenerable from
/// origin. A free-form `?bbox=14.9013,67.0217,…` satisfies neither: two
/// people framing the same valley produce two keys, the hit rate
/// collapses, and the cache fills with near-duplicates of a
/// multi-megabyte object.
///
/// So the request is snapped **outward** to a fixed grid and the pack is
/// named by the cells it spans. Same valley, same key, one cached copy.
/// The client computes the key with arithmetic — it never asks the
/// server what a region is called.
///
/// # Why the grid is not the pack
///
/// Tempting to make a pack *be* one cell: a finite key space and perfect
/// cache behaviour. It breaks on the engine, which binds one pack per
/// `RouteEngine` and needs a single pack to contain every waypoint of a
/// route. At 3.8 km, most real routes straddle two cells and would be
/// refused while the user stands inside a downloaded area. Packs are
/// therefore region-sized and cells are only how regions are *named*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackKey {
    pub z: u32,
    /// Inclusive cell range. `y0` is the NORTH edge — slippy `y` grows
    /// southward, which is the sign error this type exists to contain.
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

impl PackKey {
    /// The smallest grid rectangle containing this WGS84 bbox.
    pub fn covering(min_lon: f64, min_lat: f64, max_lon: f64, max_lat: f64, z: u32) -> Self {
        let n = 2f64.powi(z as i32);
        let lon_to_x =
            |lon: f64| (((lon + 180.0) / 360.0 * n).floor().max(0.0) as u32).min(n as u32 - 1);
        let lat_to_y = |lat: f64| {
            let r = lat.clamp(-85.05112878, 85.05112878).to_radians();
            let v = (1.0 - (r.tan() + 1.0 / r.cos()).ln() / std::f64::consts::PI) / 2.0 * n;
            (v.floor().max(0.0) as u32).min(n as u32 - 1)
        };
        Self {
            z,
            x0: lon_to_x(min_lon),
            // North edge is the LARGER latitude, hence the smaller y.
            y0: lat_to_y(max_lat),
            x1: lon_to_x(max_lon),
            y1: lat_to_y(min_lat),
        }
    }

    /// WGS84 `[min_lon, min_lat, max_lon, max_lat]` this key covers.
    pub fn extent(&self) -> [f64; 4] {
        let n = 2f64.powi(self.z as i32);
        let x_to_lon = |x: u32| x as f64 / n * 360.0 - 180.0;
        let y_to_lat = |y: u32| {
            let t = std::f64::consts::PI * (1.0 - 2.0 * y as f64 / n);
            t.sinh().atan().to_degrees()
        };
        [
            x_to_lon(self.x0),
            // The south edge of the southernmost cell.
            y_to_lat(self.y1 + 1),
            // The east edge of the easternmost cell.
            x_to_lon(self.x1 + 1),
            y_to_lat(self.y0),
        ]
    }

    /// Cells spanned — a cheap proxy for the *shape* of a request.
    ///
    /// Not a proxy for its cost. A z12 cell is square on the ground at
    /// every latitude, but its size falls as latitude rises: the same
    /// cell is 5.2 km a side at 58°N and 3.8 km at 67°N, so equal cell
    /// counts differ by 1.9x in area across Norway. Use
    /// [`area_sq_km`](Self::area_sq_km) to bound work.
    pub fn cells(&self) -> u64 {
        (self.x1 - self.x0 + 1) as u64 * (self.y1 - self.y0 + 1) as u64
    }

    /// Ground area covered, in km² — what a build's cost is proportional to.
    ///
    /// The DEM copy, the mask crop, the digest and the download all
    /// scale with this and none of them scale with cell count. Measured
    /// (M2): about 14.6 KB of pack and 1.3 ms of build per km².
    ///
    /// Spherical-Earth arithmetic on the mean latitude, which is
    /// accurate to well under a percent over a region of this size and
    /// is deliberately simple enough for the Android side to mirror
    /// exactly — the client must reach the same verdict as the server
    /// or it will offer downloads that get refused.
    pub fn area_sq_km(&self) -> f64 {
        const KM_PER_DEG: f64 = 111.32;
        let [min_lon, min_lat, max_lon, max_lat] = self.extent();
        let mid = (min_lat + max_lat) / 2.0;
        (max_lon - min_lon) * KM_PER_DEG * mid.to_radians().cos() * (max_lat - min_lat) * KM_PER_DEG
    }

    /// `z12_1092_378_1095_381`.
    pub fn parse(s: &str) -> Option<Self> {
        let rest = s.strip_prefix('z')?;
        let mut it = rest.split('_');
        let z = it.next()?.parse().ok()?;
        let (x0, y0, x1, y1) = (
            it.next()?.parse().ok()?,
            it.next()?.parse().ok()?,
            it.next()?.parse().ok()?,
            it.next()?.parse().ok()?,
        );
        if it.next().is_some() || x1 < x0 || y1 < y0 {
            return None;
        }
        // A key naming a cell outside its own grid is a client bug, and
        // serving it would mean building a pack for nowhere.
        let max = 2u32.saturating_pow(z);
        if z > 24 || x1 >= max || y1 >= max {
            return None;
        }
        Some(Self { z, x0, y0, x1, y1 })
    }
}

impl std::fmt::Display for PackKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "z{}_{}_{}_{}_{}",
            self.z, self.x0, self.y0, self.x1, self.y1
        )
    }
}

#[cfg(test)]
mod key_tests {
    use super::*;

    #[test]
    fn a_key_round_trips_through_its_string() {
        let k = PackKey::covering(14.95, 67.02, 15.20, 67.12, PACK_GRID_Z);
        assert_eq!(PackKey::parse(&k.to_string()), Some(k));
    }

    #[test]
    fn the_key_contains_what_was_asked_for() {
        // Snapping must go OUTWARD. A key that covers less than the
        // request is a pack with a hole in it, and the hole is at the
        // edge — exactly where a user who framed their viewport
        // deliberately is going to tap.
        let (w, s, e, n) = (14.95, 67.02, 15.20, 67.12);
        let ext = PackKey::covering(w, s, e, n, PACK_GRID_Z).extent();
        assert!(ext[0] <= w, "west edge {} > requested {w}", ext[0]);
        assert!(ext[1] <= s, "south edge {} > requested {s}", ext[1]);
        assert!(ext[2] >= e, "east edge {} < requested {e}", ext[2]);
        assert!(ext[3] >= n, "north edge {} < requested {n}", ext[3]);
    }

    #[test]
    fn nearby_viewports_share_a_key() {
        // The entire reason for quantising: two people framing the same
        // valley must hit the same cache object.
        let a = PackKey::covering(14.9500, 67.0200, 15.2000, 67.1200, PACK_GRID_Z);
        let b = PackKey::covering(14.9513, 67.0217, 15.1988, 67.1183, PACK_GRID_Z);
        assert_eq!(
            a, b,
            "a few hundred metres of framing must not fork the cache"
        );
    }

    #[test]
    fn a_z12_cell_is_a_few_kilometres_in_norway() {
        // The size the whole design rests on: fine enough that snapping
        // wastes little, coarse enough that a region is a few cells.
        for (lat, lo, hi) in [(67.0, 3.5, 4.2), (60.0, 4.5, 5.3)] {
            let k = PackKey::covering(15.0, lat, 15.0001, lat + 0.0001, PACK_GRID_Z);
            let e = k.extent();
            let km = (e[2] - e[0]) * 111.320 * lat.to_radians().cos();
            assert!(km > lo && km < hi, "z12 cell at {lat}N is {km:.1} km");
        }
    }

    #[test]
    fn slippy_y_grows_southward() {
        // The sign error this type exists to contain. North edge is the
        // LARGER latitude and therefore the SMALLER y; getting it
        // backwards yields a key whose extent is inverted and a pack cut
        // for the wrong side of the fjord.
        let k = PackKey::covering(15.0, 67.0, 15.1, 67.2, PACK_GRID_Z);
        assert!(k.y0 <= k.y1, "y0 must be the north edge");
        let e = k.extent();
        assert!(e[1] < e[3], "extent must be south-then-north: {e:?}");
        assert!(e[1] <= 67.0 && e[3] >= 67.2);
    }

    #[test]
    fn a_malformed_key_is_rejected_rather_than_guessed() {
        for bad in [
            "z12_1_2_3",          // too few
            "z12_1_2_3_4_5",      // too many
            "12_1_2_3_4",         // no z
            "z12_5_2_3_4",        // x1 < x0
            "z12_1_9_3_4",        // y1 < y0
            "z12_1_2_99999999_4", // outside the grid
            "z99_1_2_3_4",        // absurd zoom
            "zabc_1_2_3_4",
            "",
        ] {
            assert!(PackKey::parse(bad).is_none(), "{bad:?} must not parse");
        }
    }

    #[test]
    fn cells_counts_the_rectangle() {
        let k = PackKey::parse("z12_10_20_13_22").unwrap();
        assert_eq!(k.cells(), 4 * 3);
    }
}

#[cfg(test)]
mod area_tests {
    use super::*;

    /// Reference values the Android side asserts against.
    ///
    /// `RoutingPack.areaSqKm` is a hand-written copy of
    /// [`PackKey::area_sq_km`] in another language, and the client's
    /// pre-flight check is only worth anything if the two agree: the
    /// point of checking locally is to never ask for what would be
    /// refused, and a client that computed a *smaller* area than the
    /// server would ask anyway and fail the download it was trying to
    /// protect.
    ///
    /// Two copies of a formula cannot share a test, so they share
    /// numbers instead. These are printed here and asserted verbatim in
    /// `RoutingPackTest.kt`; changing one without the other fails there.
    #[test]
    fn area_reference_values_for_the_android_side() {
        let cases = [
            // (min_lon, min_lat, max_lon, max_lat, expected km²)
            (15.00, 67.03, 15.28, 67.13, 232.54),
            (8.00, 60.00, 11.00, 61.20, 23_407.79),
            (5.20, 58.90, 5.60, 59.10, 762.00),
        ];
        for (min_lon, min_lat, max_lon, max_lat, expected) in cases {
            let key = PackKey::covering(min_lon, min_lat, max_lon, max_lat, PACK_GRID_Z);
            let got = key.area_sq_km();
            assert!(
                (got - expected).abs() < 0.5,
                "{key}: expected {expected} km², got {got:.2} — update \
                 RoutingPackTest.kt in the same commit"
            );
        }
    }
}
