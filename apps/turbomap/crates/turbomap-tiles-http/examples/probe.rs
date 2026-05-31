use turbomap_core::{TileId, TileSource};
fn main() {
    let source = turbomap_tiles_http::HttpRasterSource::kartverket_topo_grey().unwrap();
    for _ in 0..3 {
        let started = std::time::Instant::now();
        let tile = TileId::new(11, 1054, 590);
        let result = source.request(tile);
        let elapsed = started.elapsed();
        match result {
            Ok(t) => println!("OK in {:?}, {} bytes", elapsed, t.bytes.len()),
            Err(e) => println!("FAIL in {:?}: {}", elapsed, e),
        }
    }
}
