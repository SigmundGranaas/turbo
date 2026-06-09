# turbomap

A custom wgpu-based map renderer in Rust, designed as a library with a clean
host boundary so it can later target Android (JNI), web (wasm-bindgen), and iOS.

The MVP here is a **desktop binary** that opens a window, fetches Norwegian
Kartverket Turkart raster tiles, and lets you pan and zoom around Norway.

## Crates

| Crate | Role |
| --- | --- |
| `turbomap-core` | The renderer library. Knows about `wgpu`. Has no I/O, no HTTP, no winit. The FFI-ready boundary. |
| `turbomap-tiles-http` | A `TileSource` implementation over `reqwest::blocking`. Includes preconfigured Kartverket raster + Turbo basemap (`/v1/basemap`) sources. |
| `turbomap-style-maplibre` | Lowers a MapLibre Style Spec subset (the tileserver's `/v1/basemap/style.json`) onto `turbomap-core`'s `VectorStyle`. One style document, every renderer. |
| `turbomap-app` | The desktop binary. winit window + wgpu surface + tile fetch pump. |

Dependency direction is strict and one-way:

```
turbomap-app  ──>  turbomap-tiles-http  ──>  turbomap-core
       └─────────────depends on───────────────────^
```

## Run

```sh
cd apps/turbomap
cargo run --release -p turbomap-app
```

The window opens centered on Bergen (60.39°N, 5.32°E) at zoom 11. Click-drag
to pan, scroll wheel to zoom.

To render the self-hosted N50 basemap with the tileserver's own style instead
of the VersaTiles demo (requires a running `apps/tileserver` with N50 data):

```sh
TURBO_BASEMAP_URL=http://localhost:8090 cargo run --release -p turbomap-app
```

## Tests

```sh
cargo test --workspace
```

Tests live only at value boundaries — the public APIs consumers depend on
(Mercator math, camera intent, the pull-push tile contract, URL expansion).
Internal renderer plumbing is verified by the manual smoke test above.

## License

AGPL-3.0-only (matches `apps/tileserver`).
