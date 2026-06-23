//! End-to-end of the *data* path that the GPU golden bypasses: a REAL N50
//! basemap MVT tile (fetched from prod, committed as a fixture) must decode and
//! tessellate into a non-empty **water mesh**. The golden test feeds a synthetic
//! `VectorTile` struct straight in; this proves the actual server bytes →
//! `decode_mvt` → `tessellate` → water-mesh path works. If the water mesh comes
//! out empty, the app shows "topo tiles, no water" no matter how the tiles are
//! fetched.
//!
//! Fixture: `https://kart-api.sandring.no/v1/basemap/9/271/132.mvt` (Helgeland
//! coast — 84 water polygons per a `mapbox_vector_tile` decode).

use turbomap_core::tessellate::tessellate;
use turbomap_core::vector::decode_mvt;
use turbomap_core::{Color, Filter, Paint, Rule, TileId, VectorStyle};

const REAL_TILE: &[u8] = include_bytes!("fixtures/bodo_z9_water.mvt");

fn water_style() -> VectorStyle {
    VectorStyle {
        background: Color::rgb(255, 255, 255),
        rules: vec![Rule {
            source_layer: "water".into(),
            filter: Filter::Always,
            paint: Paint::Fill {
                color: Color::rgb(40, 90, 150),
            },
            min_zoom: 0,
            max_zoom: 22,
            interactive: false,
        }],
    }
}

#[test]
fn real_server_tile_decodes_to_water_polygons() {
    let vtile = decode_mvt(REAL_TILE).expect("real N50 MVT must decode");
    let water = vtile
        .layers
        .iter()
        .find(|l| l.name == "water")
        .expect("real basemap tile must carry a `water` layer");
    assert!(
        !water.features.is_empty(),
        "the water layer must carry features"
    );
    eprintln!(
        "water layer: {} features, extent {}",
        water.features.len(),
        water.extent
    );
}

#[test]
fn real_server_water_tessellates_into_the_water_mesh() {
    let vtile = decode_mvt(REAL_TILE).expect("decode");
    let out = tessellate(TileId::new(9, 271, 132), &vtile, &water_style());
    assert!(
        !out.water_mesh.is_empty(),
        "real server water polygons must tessellate into the water mesh \
         (else the water pipeline has nothing to draw)"
    );
}

const DEVICE_Z11_A: &[u8] = include_bytes!("fixtures/device_z11_1104_501.mvt");
const DEVICE_Z11_B: &[u8] = include_bytes!("fixtures/device_z11_1107_500.mvt");

#[test]
fn device_z11_tiles_tessellate_water() {
    // The exact tiles the device logged as water_idx=0. Reproduce the engine's
    // tessellation locally to find why a tile with 9/61 water polygons yields
    // an empty water mesh on-device.
    for (bytes, id, label) in [
        (DEVICE_Z11_A, TileId::new(11, 1104, 501), "z11/1104/501"),
        (DEVICE_Z11_B, TileId::new(11, 1107, 500), "z11/1107/500"),
    ] {
        let vtile = decode_mvt(bytes).expect("decode");
        let water_feats = vtile
            .layers
            .iter()
            .find(|l| l.name == "water")
            .map(|l| l.features.len())
            .unwrap_or(0);
        let out = tessellate(id, &vtile, &water_style());
        eprintln!(
            "{label}: water_feats={water_feats} water_idx={} main_idx={}",
            out.water_mesh.indices.len(),
            out.mesh.indices.len()
        );
        assert!(
            !out.water_mesh.is_empty(),
            "{label}: {water_feats} water polygons must tessellate (device showed water_idx=0)"
        );
    }
}
