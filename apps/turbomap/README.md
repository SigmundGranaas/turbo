# turbomap

A custom wgpu-based map renderer in Rust, designed as a library with a clean
host boundary so it can later target Android (JNI), web (wasm-bindgen), and iOS.

The MVP here is a **desktop binary** that opens a window, fetches Norwegian
Kartverket Turkart raster tiles, and lets you pan and zoom around Norway.

## Crates

| Crate | Role |
| --- | --- |
| `turbomap-core` | The renderer library. Knows about `wgpu`. Has no I/O, no HTTP, no winit. The FFI-ready boundary. |
| `turbomap-tiles-http` | A `TileSource` implementation over `reqwest::blocking`. Includes a preconfigured Kartverket Turkart source. |
| `turbomap-app` | The desktop binary. winit window + wgpu surface + tile fetch pump. |
| `turbomap-golden` | Headless golden-image + record/replay test harness. No I/O, no window — deterministic render tests on a software adapter. |
| `turbomap-scene` | Renderer-agnostic `Scene`/`Paint` IR, pure scene `diff`, and the `MapEngine` contract + conformance suite. No GPU, no I/O — the shared schema host languages bind to. |
| `turbomap-engine` | `TurbomapEngine`: drives `turbomap-core`'s wgpu pipelines from the `Scene` IR via the `MapEngine` contract. Renders raster, hillshade and GeoJSON line layers. Includes the `inspect` dev tool and a vector bench. |

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

## Tests

```sh
cargo test --workspace
```

Tests live at value boundaries — the public APIs consumers depend on
(Mercator math, camera intent, the pull-push tile contract, URL expansion).

Render output is verified by **golden-image tests** in `turbomap-golden`,
which render synthetic scenes headless on a software GPU adapter and
compare against committed reference PNGs. They are behind a feature flag
so the default lane stays GPU-free:

```sh
# Needs a wgpu adapter; a software one is enough:
#   sudo apt-get install -y mesa-vulkan-drivers
cargo test -p turbomap-golden --features gpu-tests

# Re-baseline references after an intentional visual change:
UPDATE_GOLDEN=1 cargo test -p turbomap-golden --features gpu-tests
```

CI runs both lanes (`.github/workflows/turbomap_build.yml`): a fast
Rust lane (tests + lints) and a golden lane on Lavapipe (which also runs
the engine conformance + scene-vs-imperative parity tests).

### Inspecting the engine

`turbomap-engine` ships an **agent-first inspection tool**: it runs a
`Scene` through the real engine headless and emits one machine-readable
JSON report covering every stage — the scene + validation, the applied
diff, which layers the backend supports, tile-drain activity, per-layer
render metrics + cache stats, and projection round-trips — plus the
rendered PNG.

```sh
# Built-in raster+hillshade scene → /tmp/turbomap-inspect.png + JSON report
cargo run -p turbomap-engine --example inspect -- --png /tmp/out.png

# Inspect a Scene JSON, reporting the delta vs a previous scene
cargo run -p turbomap-engine --example inspect -- \
  --scene scene.json --prev prev.json --report report.json
```

### Profiling

The GeoJSON line path (per-tile clip + tessellate) has a criterion bench:

```sh
cargo bench -p turbomap-engine
```

Per-tile clipping is what keeps it cheap — without it every tile
tessellates the whole line. On a Bergen route across ~35 tiles the
clipped clip+tessellate runs ~9× faster than the unclipped path.

## License

AGPL-3.0-only (matches `apps/tileserver`).
