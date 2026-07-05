//! Profiling the GeoJSON line path: per-tile clip + tessellate, with and
//! without clipping, over the tiles a Bergen route crosses. Demonstrates
//! the work clipping saves (each tile tessellates only its slice instead
//! of the whole route).

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use turbomap_core::{
    tessellate, Color, Filter, Paint, Rule, TileId, VectorStyle, VectorTileSource,
};
use turbomap_engine::GeoJsonVectorSource;
use turbomap_scene::geo::mercator_normalized;
use turbomap_scene::LatLng;

/// A wiggly ~`n`-point route across the Bergen area as a GeoJSON LineString.
fn route_geojson(n: usize) -> String {
    let mut coords = String::from("[");
    for i in 0..n {
        let t = i as f64 / (n - 1) as f64;
        let lng = 5.0 + t * 0.6;
        let lat = 60.35 + 0.08 * (t * 12.0).sin();
        if i > 0 {
            coords.push(',');
        }
        coords.push_str(&format!("[{lng:.5},{lat:.5}]"));
    }
    coords.push(']');
    format!(r#"{{"type":"LineString","coordinates":{coords}}}"#)
}

/// The z9 tiles around Bergen the route crosses.
fn z9_tiles() -> Vec<TileId> {
    let n = 512.0;
    let (wx, wy) = mercator_normalized(LatLng::new(60.39, 5.32));
    let (bx, by) = ((wx * n) as i64, (wy * n) as i64);
    let mut tiles = Vec::new();
    for dx in -3..=3 {
        for dy in -2..=2 {
            tiles.push(TileId::new(9, (bx + dx) as u32, (by + dy) as u32));
        }
    }
    tiles
}

fn style() -> VectorStyle {
    VectorStyle {
        background: Color::rgba(0, 0, 0, 0),
        rules: vec![Rule {
            source_layer: "geojson".to_string(),
            filter: Filter::Always,
            paint: Paint::Line {
                color: Color::rgb(4, 132, 255),
                width: 80.0,
            },
            min_zoom: 0,
            max_zoom: 22,
            interactive: false,
        }],
    }
}

fn bench(c: &mut Criterion) {
    let data = route_geojson(300);
    let clipped = GeoJsonVectorSource::new(&data);
    let unclipped = GeoJsonVectorSource::new(&data).unclipped();
    let tiles = z9_tiles();
    let style = style();

    c.bench_function("request_clipped", |b| {
        b.iter(|| {
            for &t in &tiles {
                black_box(clipped.request(t).unwrap());
            }
        })
    });
    c.bench_function("request_unclipped", |b| {
        b.iter(|| {
            for &t in &tiles {
                black_box(unclipped.request(t).unwrap());
            }
        })
    });
    c.bench_function("request_tessellate_clipped", |b| {
        b.iter(|| {
            for &t in &tiles {
                let vt = clipped.request(t).unwrap();
                black_box(tessellate(t, &vt, &style));
            }
        })
    });
    c.bench_function("request_tessellate_unclipped", |b| {
        b.iter(|| {
            for &t in &tiles {
                let vt = unclipped.request(t).unwrap();
                black_box(tessellate(t, &vt, &style));
            }
        })
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
