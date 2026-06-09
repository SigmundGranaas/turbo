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
Rust lane (tests + lints) and a golden lane on Lavapipe.

## License

AGPL-3.0-only (matches `apps/tileserver`).
