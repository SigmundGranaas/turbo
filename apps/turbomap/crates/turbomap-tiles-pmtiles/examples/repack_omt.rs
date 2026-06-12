//! Repack a directory of raw OpenMapTiles `.pbf` tiles (`<dir>/z/x/y.pbf`)
//! into a single slim `.pmtiles` archive, keeping only the layers + fields
//! a basemap style consumes. Raw OMT tiles carry every `name:*` translation
//! and POI detail — stripping them typically cuts the size ~4×, which is
//! what makes a committed real-data test fixture practical.
//!
//! This is how `turbomap-golden/tests/fixtures/bergen-omt.pmtiles` was
//! produced, from OpenFreeMap planet tiles (ODbL, © OpenStreetMap
//! contributors):
//!
//! ```text
//! for x in 8432..=8436, y in 4721..=4726:
//!   curl --compressed https://tiles.openfreemap.org/planet/<build>/14/$x/$y.pbf
//! cargo run -p turbomap-tiles-pmtiles --example repack_omt -- <dir> <out.pmtiles>
//! ```

use std::collections::HashMap;
use std::path::Path;

use turbomap_core::TileId;
use turbomap_mvt::encode::TileEncoder;
use turbomap_mvt::{decode, Geometry};
use turbomap_tiles_pmtiles::{writer::write_archive, TileType};

/// layer name → fields to keep. Layers not listed are dropped entirely.
fn keep_list() -> HashMap<&'static str, Vec<&'static str>> {
    HashMap::from([
        ("water", vec!["class"]),
        ("landcover", vec!["class", "subclass"]),
        ("landuse", vec!["class"]),
        ("park", vec!["class"]),
        ("transportation", vec!["class", "subclass", "brunnel", "ramp"]),
        ("building", vec!["render_height", "render_min_height"]),
        ("boundary", vec!["admin_level", "maritime"]),
        ("place", vec!["class", "name", "rank"]),
        ("transportation_name", vec!["class", "name", "ref"]),
        ("water_name", vec!["class", "name"]),
        // POIs: name + category so the style can pick what to show and
        // (later) which icon to draw. `rank` orders collisions.
        ("poi", vec!["class", "subclass", "name", "rank"]),
    ])
}

fn main() {
    let mut args = std::env::args().skip(1);
    let in_dir = args.next().expect("usage: repack_omt <tile-dir> <out.pmtiles>");
    let out_path = args.next().expect("usage: repack_omt <tile-dir> <out.pmtiles>");
    let keep = keep_list();

    let mut tiles: Vec<(TileId, Vec<u8>)> = Vec::new();
    let mut bytes_in = 0usize;
    for z_entry in std::fs::read_dir(&in_dir).expect("read tile dir") {
        let z_dir = z_entry.expect("dir entry").path();
        let Some(z) = name_of(&z_dir).and_then(|s| s.parse::<u8>().ok()) else { continue };
        for x_entry in std::fs::read_dir(&z_dir).expect("read z dir") {
            let x_dir = x_entry.expect("dir entry").path();
            let Some(x) = name_of(&x_dir).and_then(|s| s.parse::<u32>().ok()) else { continue };
            for y_entry in std::fs::read_dir(&x_dir).expect("read x dir") {
                let y_file = y_entry.expect("dir entry").path();
                let Some(y) = name_of(&y_file)
                    .and_then(|s| s.strip_suffix(".pbf"))
                    .and_then(|s| s.parse::<u32>().ok())
                else {
                    continue;
                };
                let raw = std::fs::read(&y_file).expect("read tile");
                bytes_in += raw.len();
                let slim = repack_tile(&raw, &keep);
                tiles.push((TileId::new(z, x, y), slim));
            }
        }
    }
    assert!(!tiles.is_empty(), "no z/x/y.pbf tiles found under {in_dir}");

    let archive = write_archive(TileType::Mvt, &tiles).expect("write archive");
    std::fs::write(&out_path, &archive).expect("write output");
    println!(
        "{} tiles: {:.1} MiB raw -> {:.2} MiB archive ({out_path})",
        tiles.len(),
        bytes_in as f64 / (1024.0 * 1024.0),
        archive.len() as f64 / (1024.0 * 1024.0),
    );
}

fn name_of(p: &Path) -> Option<&str> {
    p.file_name().and_then(|s| s.to_str())
}

/// Decode one tile and re-encode only the kept layers/fields.
fn repack_tile(raw: &[u8], keep: &HashMap<&str, Vec<&str>>) -> Vec<u8> {
    let tile = decode(raw).expect("decode raw OMT tile");
    let mut out = TileEncoder::new();
    for layer in &tile.layers {
        let Some(fields) = keep.get(layer.name.as_str()) else { continue };
        let mut enc = out.layer(&layer.name, layer.extent);
        for feature in &layer.features {
            let props: Vec<(&str, turbomap_mvt::Value)> = fields
                .iter()
                .filter_map(|f| feature.properties.get(*f).map(|v| (*f, v.clone())))
                .collect();
            enc = match &feature.geometry {
                Geometry::Point(points) => enc.points(points, &props),
                Geometry::LineString(lines) => enc.lines(lines, &props),
                Geometry::Polygon(rings) => enc.polygon_rings(rings, &props),
            };
        }
        out = enc.finish();
    }
    out.finish()
}
