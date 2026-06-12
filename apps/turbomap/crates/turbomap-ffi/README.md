# turbomap-ffi

uniffi bindings for the turbomap engine — the **control plane** that
Kotlin (Android) and Swift (iOS) hosts talk to.

## What crosses the FFI, and what doesn't

| Crosses uniffi | Stays in per-platform glue |
| --- | --- |
| Scene (as Scene-IR JSON) via `applyScene` | Surface creation (`ANativeWindow` / `CAMetalLayer`) |
| Camera get/set/animate (`easeTo` + `tick`) | The vsync render loop (Choreographer / CADisplayLink) |
| `project` / `unproject` / `hitTest` | |
| Pull/push tile IO: `pendingTiles` → host fetch → `ingest*Tile` | |
| `renderPng` offscreen snapshot (verification from any language) | |

Tile IO is **host-driven** by design: the engine lists what it needs, the
host fetches (it owns auth, caching, offline) and pushes the encoded bytes
back. Inline GeoJSON needs no IO and drains in-process via
`pumpLocalTiles()`.

## Generating bindings

```sh
cargo build -p turbomap-ffi
cargo run -p turbomap-ffi --bin uniffi-bindgen -- generate \
  --library target/debug/libturbomap_ffi.so \
  --language kotlin --language swift --out-dir target/ffi-bindings
```

CI generates both languages on every PR and uploads them as the
`ffi-bindings` artifact.

## Host loop sketch (Kotlin)

```kotlin
val map = TurboMap.headless(1024u, 768u, Camera(60.39, 5.32, 11.0, 0.0, 0.0))
map.applyScene(sceneJson)
map.pumpLocalTiles()                      // inline GeoJSON
for (req in map.pendingTiles()) {         // remote tiles: host fetches
    val bytes = http.get(urlFor(req))
    map.ingestRasterTile(req.layerId!!, req.z, req.x, req.y, bytes)
}
val png = map.renderPng()                 // or render via the surface glue
```

## Testing

`tests/roundtrip.rs` plays the role of a foreign host and drives the map
only through the exported surface — scene JSON in, pull/push tiles,
camera/projection/hit-test, PNG snapshot out with pixel assertions.
GPU-gated like the rest of the render tests:

```sh
cargo test -p turbomap-ffi --features gpu-tests
```
