# turbomap-web

The **web host** for the turbomap wgpu engine — `turbomap-core`/`turbomap-engine`
compiled to WASM and driven from a browser `<canvas>` over WebGPU. The third host
after Android (JNI, `turbomap-ffi/src/surface.rs`) and the desktop winit app
(`turbomap-app`). All three drive the *same* `TurbomapEngine` over the same
control plane (scene in, host-driven tile IO, `render()` per frame), so the
browser runs the device's exact render code paths.

This is the renderer foundation for the new React web app that replaces the
Flutter app (see `.claude-priv/plans/…` — "Native web app on the turbomap wgpu
renderer").

## Build

```bash
./build.sh            # dev (fast)
./build.sh --release  # optimised (wasm-opt)
```

Produces `pkg/` — a standard npm package (`turbomap_web.js` + `_bg.wasm` +
`.d.ts`). Import from the React app as an ES module:

```ts
import init, { TurboMap } from 'turbomap-web'; // or a relative path to pkg/
await init();
const map = await TurboMap.create(canvas, w, h, lat, lng, zoom);
```

`pkg/` is a build artifact (git-ignored) — run `./build.sh` after a clean
checkout or any Rust change.

## Smoke test (needs a WebGPU browser: Chrome/Edge, Safari 18+, FF w/ flag)

```bash
./build.sh
(cd crates/turbomap-web && python3 -m http.server 8000)
# open http://localhost:8000/smoke/
```

`smoke/index.html` boots a map over Bergen against the live
`kart-api.sandring.no` tiles, drives the host-side tile-fetch + render loop, and
supports drag-to-pan / wheel-to-zoom. It validates: WebGPU init, scene apply,
host-driven tile ingest, and a rendered frame — before any React work.

## API surface

Mirrors the Kotlin/Swift FFI (`turbomap-ffi/src/lib.rs`): `TurboMap.create`
(async), `apply_scene`, the streaming plan (`streaming_plan_json` →
`ingest_raster_tile` / `ingest_terrain_tile` / `ingest_vector_tile`, with
`report_fetch_failed` / `report_fetch_cancelled`), `render`, `tick`,
`is_animating`, `resize`, `set_camera`, `camera_json`, `pan_by_pixels`,
`zoom_around`, `orbit_around`, `ease_to`, `project`, `unproject`,
`set_viewport_inset`.

Tile IO is **host-driven** via the STREAMING PLAN (plan P5.1):
`streaming_plan_json(max_start)` mints priority-ordered
`{"start":[{id,kind,layer,z,x,y}],"cancel":[ids]}`; the host fetches (it owns
the URL templates + auth + caching), pushes bytes back via the matching
`ingest_*`, and reports failures/cancels by request id.

Content is **scene-declared** (plan P5.2): lighting, terrain shadows, haze,
basemap gain, clouds, and route tubes live in the Scene IR's `environment`
block and `tube` layers — there are no imperative content setters on this
surface.
