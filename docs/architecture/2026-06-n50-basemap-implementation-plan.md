# Self-Hosted N50 Topo Basemap — Implementation Plan

**Date:** 2026-06-09
**Companion to:** `2026-06-self-hosted-tiles-feasibility.md`
**Scope:** Norway only.
**Goal:** Deliver the Norwegian **N50 topographic map** (vector *and* raster
tiles) from our own services, replacing the Kartverket Norgeskart WMTS
basemap, and make it restylable. Cloudflare **R2 is used strictly as a
distributed cache** — never the origin or system of record.

> **Status — contour pipeline validated against real data (2026-06-09).**
> The contour source and the new ingest in §4.1 are no longer a proposal.
> Real N50 Kartdata for Oslo (fylke `03`) was pulled from the Geonorge
> Nedlasting API in PostGIS/EPSG:25833, restored through the production
> `n50-restore` path, and the new `n50-hoydekurve-upsert` job (implemented in
> this change) loaded **2 268 contour features** into `terrain.contour`
> (2 263 main + 3 auxiliary + 2 depression). Main-contour elevations are
> exactly 20/40/60/…/620 m — confirming the **20 m equidistance**; 422 lines
> are flagged `is_index` (every 100 m). A `ST_AsMVT` basemap tile (z12 over
> Nordmarka) rendered **85 contour features → 13.9 KB**, proving the
> serve path end-to-end. The existing N50 upserts also ran clean on the same
> real data (water 434, landcover 2 619, place names 1 148, roads 16 996).
> See §A.
>
> **Status update (2026-06-09, later):** the serve path is now built and
> tested end-to-end on small real samples. Landed since: buildings +
> coastline ingest (fixture + e2e coverage), the fkb_type vocabulary
> reconciliation (N50 roads now surface in the resource views), the
> multi-layer `/v1/basemap/{z}/{x}/{y}.mvt` endpoint with TileJSON
> descriptor (§4.2 — done), the house `n50-topo` MapLibre style served at
> `/v1/basemap/style.json` with a style↔tiles contract test, the
> `turbomap-style-maplibre` loader so the native renderer consumes the same
> style document (§6 — first slice done), public-only boot (JWT_SECRET now
> optional, gates only /admin), and the R2 pull-through Worker
> (`infra/edge/tiles-worker`, §5 — done, node-tested). Remaining from the
> plan: glyphs/sprites for MapLibre GL clients, per-zoom matviews +
> full-Norway ingest (M2), and promoting the new layer to default in the
> client.
>
> **Status update (2026-06-09, raster fallback):** §7/M1 and the first §8
> slice landed. `turbo-tiles-raster` rasterises the same PostGIS layers
> with the same `n50-topo` style at the origin (tiny-skia + embedded
> DejaVu labels with halo + keep-out); served at
> `/v1/raster/n50/{z}/{x}/{y}.png` (max native z16) behind the worker's
> existing allowlist. Verified on real Oslo data — z11/z12/z14 tiles render
> recognisable topo cartography. The Flutter app registers a
> `TurboN50TopoConfig` local layer ("N50 topo (Turbo)") next to Norgeskart;
> flipping the default (and retiring the WMTS) is the remaining M1 step,
> gated on full-Norway ingest.

---

## 0. Design rules (non-negotiable)

1. **R2 is a cache, not a store.** The system of record is PostGIS
   (`tiles-db`) plus the deterministic build that turns it into tiles. Every
   byte in R2 is regenerable from origin. Wiping the bucket must lose
   *nothing* but a warm cache. No build step writes "the only copy" of
   anything to R2.
2. **Origin owns truth, edge owns speed.** Tiles are produced by the
   tileserver (dynamic MVT) or the offline build (raster/vector PMTiles on
   the node's PV). Cloudflare (Worker + R2 + edge cache) sits *in front* and
   pulls through on miss.
3. **Versioned, immutable tiles.** Every tile URL carries a `data_version`
   (e.g. `n50-2026.06`). A rebuild bumps the version; old cache keys orphan
   and lifecycle-expire. No in-place purge needed for correctness.
4. **One pipeline, two outputs.** The same N50 PostGIS data drives both the
   vector basemap (restylable) and the raster fallback (drop-in for
   `flutter_map`). No second source of truth.
5. **Reuse, don't rebuild.** Extend `turbo-tiles-*` (MVT, ingest, build) and
   `turbomap` (style, render, PMTiles); do not introduce a parallel stack.

---

## 1. What already exists (and what we reuse)

Confirmed in-repo (see feasibility study §2 for detail):

- **N50 vector data is already in PostGIS.** Ingest jobs populate:
  `terrain.water_polygon` (lake/river/river_dry/sea), `terrain.landcover_patch`
  (forest/wetland/open), `terrain.glacier_polygon`, `paths.edge` (N50 `vegnett`
  roads/paths, typed sti/traktorvei/skogsvei/vei), and `anchors.anchor`
  (N50 `stedsnavn` place names + summits, with `name`, `kind`, `elevation_m`).
  Raw tables live under `n50_staging.*`.
- **MVT serving** exists (`turbo-tiles-mvt::render_tile`, `ST_AsMVTGeom` +
  `ST_AsMVT`, EPSG:25833→3857) but is **single-layer per `Resource`** and
  scoped to 4 curated path resources.
- **Renderer + style engine** exist (`turbomap-core::style`: `VectorStyle`,
  `Rule`, `Paint::{Fill,Line,Text}`, `Filter::{Always,Eq,In}`, `HillshadeStyle`),
  plus MVT decode, hillshade/terrain shaders, and a **PMTiles v3 reader**.
- **DEM** (`norway.dem`, ~11 GiB) + a Terrain-RGB tile endpoint.
- **Infra**: inert `tileserver.yaml` (CNPG PostGIS, 14 GiB hostPath artifact
  PV, Traefik at `kart-api.sandring.no`, GHCR image via CI).

**Gaps this plan closes:** (a) a **multi-layer basemap MVT** endpoint; (b)
**contours** (`hoydekurve` ingest or DEM-derived); (c) buildings/coastline as
basemap layers; (d) **glyphs + sprites + an N50 house style**; (e) the
**raster render pipeline**; (f) the **R2-as-cache** edge tier; (g) **client
wiring** + offline.

---

## 2. Target architecture

```
  Kartverket N50 / DTM (open, NLOD/CC-BY)
            │  ingest jobs (exist; + new hoydekurve/bygning)
            ▼
   ┌──────────────────────────────────────────────┐
   │ PostGIS tiles-db  ── SYSTEM OF RECORD          │
   │  terrain.* / paths.edge / anchors.* / contour  │
   └──────────────────────────────────────────────┘
            │                              │
   (live, dynamic)                 (offline build job)
   ST_AsMVT multi-layer            raster render → styled PNG/WebP
   /v1/basemap/{z}/{x}/{y}.mvt     + vector pack  → *.pmtiles (on node PV)
            │                              │
            └────────────┬─────────────────┘
                         ▼  ORIGIN  (kart-api.sandring.no, Traefik → tileserver)
            ┌────────────────────────────────────────────┐
            │ Cloudflare Worker  (tiles.sandring.no)        │
            │  L1: edge Cache API                          │
            │  L2: R2 bucket  ── DISTRIBUTED CACHE ONLY    │
            │  miss → fetch origin → put R2 → return        │
            └────────────────────────────────────────────┘
                         ▼
       ┌─────────────────────────────┬───────────────────────────┐
       ▼                             ▼                            ▼
  turbomap (vector + N50 style   flutter_map (raster N50      offline: ship the
  + hillshade) — custom styles   PMTiles) — drop-in topo      same .pmtiles to device
```

**Request lifecycle (R2 as cache):**
1. Client → `tiles.sandring.no/v1/basemap/{v}/{z}/{x}/{y}.mvt`.
2. Worker checks **edge cache** (L1) → hit returns immediately.
3. Miss → checks **R2** (L2) key `basemap/{v}/{z}/{x}/{y}.mvt` → hit puts into
   edge cache, returns.
4. Miss → fetches **origin** tileserver, `R2.put(key, bytes)` (write-through),
   caches at edge, returns. R2 now warm; origin untouched on subsequent hits.
5. Rebuild bumps `{v}` → new key space → old keys lifecycle-expire (30 d).

Because every R2 object is keyed by `data_version` and is byte-for-byte
reproducible from origin, the bucket is disposable. Lose it → cold cache,
correctness intact.

---

## 3. Vector basemap tile schema

One MVT tile carries **multiple named layers** (today's `render_tile`
produces one). Layers, source tables, and per-zoom rules, defined in a new
`apps/tileserver/tools/basemap-layers.toml` (mirrors the existing
`vector-layers.toml` pattern so adding a layer is config-only):

| MVT layer | Source (already in DB) | Geom | Min z | Notes / generalization |
| --- | --- | --- | --- | --- |
| `water` | `terrain.water_polygon` (lake/river/sea) | polygon | 4 | `ST_SimplifyPreserveTopology(tol(z))`; drop area < f(z). |
| `glacier` | `terrain.glacier_polygon` | polygon | 6 | snow/ice fill. |
| `landcover` | `terrain.landcover_patch` (forest/wetland/open) | polygon | 7 | `class` property drives fill. |
| `contour` | `terrain.contour` *(new §4.1, from N50 Høyde theme)* | line | 11 | `elev_m`, `kind`, `is_index` (bold every 100 m). 20 m base equidistance. |
| `waterway` | `n50_staging.elvbekk` (streams) | line | 11 | `width_m`. |
| `transportation` | `paths.edge` (N50 `vegnett`) | line | 8 | `fkb_type`/`typeveg` → road class; thin→thick by z. |
| `building` | `n50_staging.bygning_omrade` | polygon | 14 | footprints. |
| `place` | `anchors.anchor` (N50 stedsnavn) | point | 6 | `name`, `kind`, `elev_m`, `rank` for label collision. |

**Conventions:** geometry stored EPSG:25833, projected to 3857 in-tile
(reuse the existing `ST_TileEnvelope` + `ST_Transform` CTE). Extent 4096,
buffer 64. `Cache-Control: public, max-age=31536000, immutable` (safe because
the version is in the path). Low-zoom column projection like today's
`select_columns` to keep tiles small.

---

## 4. Server changes (`apps/tileserver`)

### 4.1 New ingest: contours + buildings

- **Contours — primary source: N50 Kartdata, "Høyde" theme (`Høydekurve`).**
  Contour lines *do* exist as a bulk dataset; they are not separate from what
  we already use. The **Høyde** theme of **N50 Kartdata** carries
  `Høydekurve` (main contours, **20 m equidistance**), `Hjelpekurve`
  (auxiliary contours, ~10 m in flat terrain), and `Forsenkningskurve`
  (depression contours) — all `senterlinje` LineStrings with a `høyde`
  elevation attribute, in EPSG:25833. This is the **same product the repo
  already restores**: `n50_staging.terrengpunkt` (also a Høyde-theme object)
  is already present, and the dump is the national
  **`n50-kartdata-utm33-hele-landet-postgis`** distribution from Geonorge /
  `data.kartverket.no`. The Høyde *line* tables were simply not loaded.

  **No fallback needed — N50 Høyde is the source.** Work:
  1. Ensure the N50 restore includes the Høyde theme tables (`n50_staging.hoydekurve`,
     `n50_staging.hjelpekurve`, `n50_staging.forsenkningskurve`). If the
     current dump excluded them, re-pull N50 Kartdata UTM33 (whole-country
     PostGIS) — same source, same `n50-restore`/`pgdump_load` path, just
     don't filter out the Høyde layers.
  2. New migration `terrain.contour(id, geom LineString 25833, elev_m double
     precision, kind text /* main|auxiliary|depression */, is_index bool)`.
  3. `upsert_n50_hoydekurve.sql` + `JobName::N50HoydekurveUpsert`, mirroring
     `upsert_n50_vegnett.sql` exactly (read `senterlinje`/`høyde` from the
     three staging tables, tag `kind`, set `is_index = (elev_m % 100 = 0)`
     for bold index contours). N50's 20 m base interval means index every
     100 m is the natural cartographic choice.

  Cartographically this is the *right* source, not a compromise: it is the
  identical contour geometry Norgeskart's own N50 topo renders, so our map
  matches the official one line-for-line. (We keep `norway.dem` for hillshade
  and Terrain-RGB — it complements the vector contours, it does not replace
  them.)
- **Buildings.** `n50_staging.bygning_omrade` already loads (referenced in
  `vector-layers.toml`); just expose it as a basemap layer. No new ingest.

### 4.2 Multi-layer basemap MVT endpoint

New module `turbo-tiles-mvt/src/basemap.rs`:

```rust
// Build one MVT from N layers. Each layer runs its own ST_AsMVTGeom +
// ST_AsMVT against the tile envelope; the bytes are concatenated (MVT is
// a concatenation of length-delimited layer messages, so appending
// independently-encoded single-layer tiles is valid).
pub async fn render_basemap_tile(pool, coord, layers: &[BasemapLayer]) -> Result<Vec<u8>>;
```

`BasemapLayer` (parsed from `basemap-layers.toml`): `name`, `table_or_view`,
`geom_column`, `min_zoom`, `max_zoom`, `kind` (polygon/line/point), `attrs`,
`simplify_tol_fn`. Reuse the exact envelope CTE from `tile.rs`; per layer add
`WHERE min_zoom <= z` short-circuit (skip the query entirely when out of
range) and `ST_SimplifyPreserveTopology(geom, tol(z))` for lines/polys.

New API route in `turbo-tiles-api/src/v1`:
`GET /v1/basemap/{version}/{z}/{x}/{y}.mvt` (version is opaque to the server;
it exists so the URL is immutable for caching — server serves current data,
the build/version bump is what changes the bytes). Add `/v1/basemap/style.json`
(serves the active style, §6) and extend `/v1/catalog` with a `basemap` entry
(zoom 4–16, attribution `© Kartverket`).

### 4.3 Generalization & performance

- Precompute simplified geometry columns per zoom band (e.g. materialized
  views `basemap.water_z{4,8,12}`) so hot low-zoom tiles don't simplify on
  every request. Refresh in the rebuild job.
- Keep dynamic MVT as origin, but **rely on the R2/edge cache** (§5) for
  fan-out; origin only ever renders each `{v,z,x,y}` once.
- Optional later: pre-pack the whole vector basemap into `norway-basemap.pmtiles`
  on the PV and serve via range reads (origin still self-hosted) for the
  lowest-latency cold path. Vector PMTiles writer is the one new build
  capability (`turbomap` only *reads* today) — add `turbo-tiles-build`
  `pmtiles_writer.rs` or shell out to `tippecanoe` in the build job.

---

## 5. R2 distributed cache (Cloudflare)

A Cloudflare Worker is the public tile host; **R2 is its L2 cache only.**

`infra/edge/tiles-worker/` (new):

```js
// pseudocode — full Worker in the milestone
export default {
  async fetch(req, env) {
    const url = new URL(req.url);
    const key = url.pathname.replace(/^\//, "");          // basemap/{v}/{z}/{x}/{y}.mvt
    const cache = caches.default;
    let hit = await cache.match(req);                      // L1 edge
    if (hit) return hit;
    const obj = await env.TILES.get(key);                  // L2 R2
    if (obj) { const r = tileResp(obj.body, obj); ctx.waitUntil(cache.put(req, r.clone())); return r; }
    const origin = await fetch(env.ORIGIN + "/" + key);    // ORIGIN tileserver
    if (!origin.ok) return origin;
    const buf = await origin.arrayBuffer();
    ctx.waitUntil(env.TILES.put(key, buf, { httpMetadata: origin.headers })); // write-through
    const r = tileResp(buf, origin);
    ctx.waitUntil(cache.put(req, r.clone()));
    return r;
  }
}
```

**Cache-only guarantees:**
- The Worker **never** serves from R2 without an origin able to regenerate
  the same key. Health check fails the deploy if origin is unreachable.
- R2 lifecycle rule: expire objects after 30 days (cold keys re-pull on
  demand; orphaned old-version keys self-clean).
- Versioned keys (`basemap/n50-2026.06/...`) mean a rebuild needs **no purge**.
- A `scripts/warm-cache.sh` can optionally pre-pull a low-zoom pyramid
  (z4–10, a few thousand tiles) after a rebuild so the first users hit warm
  edges — but warming is an optimization, not a correctness requirement.
- `env.ORIGIN` = `https://kart-api.sandring.no` (Traefik → tileserver). DNS:
  `tiles.sandring.no` → Worker route.

**Why a Worker rather than R2-public-bucket-as-origin:** a public R2 bucket
would make R2 the origin (violates rule 1). The Worker keeps PostGIS/the
build as the only source of truth and uses R2 purely as warm storage between
edge and origin.

---

## 6. Styling: N50 house style + glyphs + sprites

### 6.1 Grow the style engine (`turbomap-core/style.rs`)

Current engine handles flat `Fill`/`Line`/`Text` with `Eq`/`In` filters —
enough for a first N50 style, but N50 cartography needs:

- **Zoom-interpolated values** (line width / fill by zoom): add
  `Value::Stops(Vec<(u8, f32)>)` for width and an interpolation helper.
- **Line styling**: dash patterns (paths vs roads), casing (outline+fill for
  roads) → extend `Paint::Line` with `dash: Option<Vec<f32>>`, `casing:
  Option<(Color,f32)>`.
- **Label placement**: line labels (contour elevations, road names) and
  point-label collision priority via the `rank` property → extend
  `Paint::Text` with `placement: Point|Line` and `rank_field`.
- **Filter `>=`/`<` numeric** (e.g. show contour index bolder) → add
  `Filter::Cmp`.

Keep the engine "narrower than Mapbox spec" but adopt **MapLibre style JSON
as the on-disk interchange format** so styles are authorable/diffable and we
can later cross-check against MapLibre GL JS on web. Add a
`turbomap-core/src/style/maplibre.rs` loader mapping the subset we support.

### 6.2 Author styles

`apps/tileserver/styles/` (served via `/v1/basemap/style.json`):
- `n50-topo.json` — the default, matching Norgeskart topo conventions
  (water #9CC, forest #C8E6C0, contours #B89, roads cased, glaciers #FFF).
- `n50-winter.json` — muted, ski-track emphasis.
- `n50-hike.json` — trail emphasis, hillshade on.

Hillshade uses the existing `HillshadeStyle` + Terrain-RGB endpoint (NW sun
315°/45°, opacity 0.55 — already the default).

### 6.3 Glyphs + sprites (new, required for labels/icons)

- **Glyphs**: generate SDF font PBFs (range `0-255` etc.) for 1–2 faces
  (e.g. "Noto Sans Regular/Bold") with a one-off `build-glyphs` step; serve
  static at `/fonts/{fontstack}/{range}.pbf` (cached via the same Worker).
- **Sprites**: a sprite sheet (`sprite.png` + `sprite.json`) for POI/anchor
  icons (cabin, summit, water) keyed off `anchors.anchor.kind`.
- Both are tiny, immutable, and live behind the same R2 cache.

---

## 7. Raster fallback pipeline (drop-in for `flutter_map`)

So we can delete the Norgeskart dependency *before* the on-device vector
renderer ships everywhere:

1. Offline build job renders the `n50-topo` style over the vector basemap +
   hillshade to **PNG/WebP**, z0–15, into `norway-n50-raster.pmtiles` on the
   node PV. Use a headless MapLibre/`turbomap` snapshot path (turbomap already
   has `examples/snapshot.rs`).
2. Serve via `/v1/raster/n50/{version}/{z}/{x}/{y}.webp` (origin) behind the
   same Worker/R2 cache.
3. Flutter points its existing topo provider at this URL (§8). No client
   renderer change — it's still raster XYZ.

This raster archive is a *derived cache artifact* too (rebuildable from the
style + data), consistent with rule 1, though it lives on the node PV (origin),
not R2.

---

## 8. Client wiring (`apps/flutter`)

- **Raster topo swap (Phase 1):** repoint
  `lib/features/tile_providers/data/providers/norges_kart_topo.dart` from
  `cache.atgcp1-prod.kartverket.cloud` to
  `https://tiles.sandring.no/v1/raster/n50/{version}/{z}/{x}/{y}.webp`, behind
  a feature flag in `TileRegistry`
  (`lib/features/tile_providers/data/tile_registry.dart`). Keep the Kartverket
  provider registered as a fallback/alternative during rollout.
- **Vector basemap (Phase 3+):** add a vector basemap provider consuming
  `/v1/basemap/{version}/{z}/{x}/{y}.mvt` + `style.json`. On platforms where
  `turbomap` is embedded (desktop/web first, then mobile via FFI), render
  vector; elsewhere fall back to raster. The `vector_tile` package already in
  `pubspec.yaml` covers an interim flutter_map vector path if needed.
- **Attribution:** the registry already tracks per-source attribution
  strings; set `© Kartverket` (+ `Nasjonal Turbase / DNT`, `© NVE` for
  overlays). Carry it in `style.json` `attribution` too.
- **Offline:** the SQLite tile store already downloads XYZ regions → works
  immediately for the raster basemap. For vector offline, ship/sync the
  `norway-basemap.pmtiles` (turbomap reads it directly).

---

## 9. Build & ingest pipeline (versioned, reproducible)

A single `tileserver build-basemap --version n50-YYYY.MM` flow (extend
`turbo-tiles-bin`):

1. `ingest` the N50 product into `n50_staging.*` (jobs exist; add
   `n50-hoydekurve-upsert`).
2. Run upserts → `terrain.*`, `paths.edge`, `anchors.anchor`, `terrain.contour`.
3. Refresh per-zoom materialized views (§4.3).
4. (Optional) pack `norway-basemap.pmtiles` + render `norway-n50-raster.pmtiles`.
5. Run **health audits** (`turbo-tiles-build/health.rs` already exists) —
   refuse to publish if a layer is empty / geometry spans the whole country /
   contour count implausible.
6. Stamp the new `data_version` into the catalog/config; the version flows
   into every tile URL.

Wire as a CI job (`.github/workflows/tileserver_basemap_build.yml`) or a
manual `make basemap` run on the build box; publish the image + bump the
version. **No tile bytes are uploaded to R2 by the build** — R2 fills lazily
via the Worker pull-through (or the optional warm step), preserving "R2 =
cache only".

---

## 10. Infra / k8s

- **Wire the tileserver live** per `infra/k8s/README-tileserver.md` (it's a
  reviewed, inert manifest today): stage artifacts on the node PV, seal
  `tiles-db` secrets, uncomment the "FLIP TO DEPLOY" block. Routing + curated
  MVT + the new `/v1/basemap` all serve from this one deploy.
- **Ingress:** add the `/v1/basemap`, `/v1/raster`, `/fonts`, `/sprite`
  paths to the Traefik route to the tileserver Service.
- **Cloudflare:** new R2 bucket `turbo-tiles`, Worker `tiles-worker`
  (`infra/edge/`), route `tiles.sandring.no/*`, lifecycle rule (30 d),
  `ORIGIN` binding → `kart-api.sandring.no`. Reuse the existing R2 creds
  pattern (`envs/prod/r2-backup-sealedsecret.yaml`) — but note this bucket is
  a cache, separate from the backup bucket.
- **Capacity:** vector basemap is small; the raster PMTiles (~tens of GiB)
  should live on the node PV or be range-served — confirm PV headroom (the PV
  is already sized 20 GiB for routing artifacts; raster fallback may want its
  own PV or to skip on-node and render to R2-fronted origin storage).

---

## 11. Milestones

| # | Deliverable | Acceptance |
| --- | --- | --- |
| **M0** | Spike: multi-layer `render_basemap_tile` over existing `terrain.*`/`paths.edge`/`anchors` for one region; `turbomap` renders it with a hand-written `n50-topo` style. | A Bergen-extent tile shows water + forest + roads + labels, styled, in turbomap. Validates the layer schema + style reach. |
| **M1** | **Raster N50 fallback live.** Build `norway-n50-raster.pmtiles`; serve `/v1/raster/n50`; Worker+R2 cache; Flutter topo provider repointed behind a flag. | App basemap loads from `tiles.sandring.no`; Norgeskart CDN no longer hit (flag on); R2 fills on miss, origin hit once per tile. **Kartverket topo dependency removed.** |
| **M2** | Contours + buildings ingested from the N50 Høyde theme; full Norway N50 in `tiles-db`; per-zoom matviews; health audits green. | `terrain.contour` populated nationally from `Høydekurve`/`Hjelpekurve`/`Forsenkningskurve` (20 m equidistance, index every 100 m); basemap tiles at z14 show buildings + contours matching Norgeskart N50; audit job passes. |
| **M3** | **Vector basemap GA** on desktop/web: `/v1/basemap` + `style.json` + glyphs + sprites; grown style engine; 3 house styles. | turbomap renders the national vector basemap with any of 3 styles; labels place without overlap; style switch is instant (no re-fetch). |
| **M4** | Own geometry/search endpoints: route live WFS/WMS/Overpass + Geonorge search through tileserver (curated MVT + `turbo-tiles-search`). | `wfs.geonorge.no`, `wms.geonorge.no`, `overpass-api.de`, `ws.geonorge.no` no longer called at runtime (fallbacks retained). |
| **M5** | Mobile vector renderer (turbomap via FFI) replaces raster fallback; retire raster where vector parity holds. | Android/iOS render vector N50 with custom styles + hillshade; raster kept only as ultimate fallback. |

Each milestone removes or de-risks one external dependency and is shippable
on its own. M1 is the highest-value step (it deletes the Norgeskart
dependency with no client-renderer change).

---

## 12. Testing & observability

- **Server:** unit tests for `basemap-layers.toml` parsing, per-zoom layer
  inclusion, and MVT validity (decode the bytes, assert layer names + feature
  counts) — mirror existing `turbo-tiles-mvt` test style. Golden-tile tests
  for a fixed `{z,x,y}` against the `*_mini` fixtures.
- **Cache:** Worker integration test asserting miss→origin→R2-put→hit, and
  that a wiped bucket still serves (re-pull) — proves "cache only".
- **Render:** `turbomap` snapshot tests (it has `examples/snapshot.rs`) for
  each style at representative tiles.
- **Health audits:** reuse `turbo-tiles-build/health.rs`; block publish on
  empty/oversized layers (the "49195 sti components" class of silent failure).
- **Metrics:** Prometheus (already in `infra/observability`): origin tile
  render latency/RPS, Worker cache hit ratio (L1/L2/origin), R2 op counts,
  bytes egress. Alert if origin RPS spikes (cache regression) or hit ratio
  drops.

---

## 13. Risks & mitigations

| Risk | Mitigation |
| --- | --- |
| Treating R2 as a store creeps in (e.g. build uploads tiles). | Enforced by rule 1 + the versioned-key + lifecycle design; CI lint that the build job has no `R2.put`. Bucket is wipe-tested in M1 acceptance. |
| Cartographic parity with official N50 is hard. | Vector + iterative styling; raster fallback (M1) means we're never worse than today while styling matures. |
| Style engine too narrow for N50 labels/contours. | M0 spike sizes exactly what's needed; grow `style.rs` incrementally; MapLibre JSON as interchange. |
| Dynamic origin overloaded on cache-cold low-zoom tiles (cover all Norway). | Per-zoom matviews + optional vector PMTiles on PV + post-rebuild warm of z4–10. |
| Høyde-theme line tables missing from the current N50 restore. | They are part of the national `n50-kartdata-utm33-hele-landet-postgis` product (same source as the `terrengpunkt` we already load) — re-pull N50 without filtering the Høyde layers; no new data source, no fallback needed. |
| Mobile vector renderer (FFI) slips. | M1–M4 don't depend on it; raster covers mobile until M5. |
| Licence/attribution. | NLOD/CC-BY allow self-serve + derived styles **with attribution**; carry `© Kartverket` in `style.json` + client (already tracked). Confirm exact text before GA. |
| Node capacity for raster PMTiles. | Range-serve from origin storage / separate PV; keep raster max-zoom capped; rely on edge cache. |

---

## 14. File-by-file touchpoints

**tileserver (server + data):**
- `crates/turbo-tiles-mvt/src/basemap.rs` — **new** multi-layer tile builder.
- `crates/turbo-tiles-core/src/resource.rs` — add a `Basemap` concept /
  catalog entry (or a separate basemap config type).
- `crates/turbo-tiles-api/src/v1/{basemap.rs,mod.rs}` — **new** routes
  `/v1/basemap/{version}/{z}/{x}/{y}.mvt`, `/v1/basemap/style.json`,
  `/v1/raster/n50/...`, `/fonts/...`, `/sprite...`.
- `tools/basemap-layers.toml` — **new** layer definitions (config-only growth).
- `migrations/2026xxxx_contour_schema.sql` — **new** `terrain.contour`.
- `crates/turbo-tiles-ingest/{src/n50_hoydekurve.rs, sql/upsert_n50_hoydekurve.sql,
  src/job.rs}` — **new** contour ingest from N50 Høyde theme
  (`n50_staging.{hoydekurve,hjelpekurve,forsenkningskurve}` → `terrain.contour`).
- `crates/turbo-tiles-ingest/src/pgdump_load.rs` / the N50 restore — ensure
  the Høyde-theme line tables are included when restoring the national N50
  UTM33 PostGIS dump (today only `terrengpunkt` from that theme is loaded).
- `crates/turbo-tiles-build/src/{pmtiles_writer.rs, raster_render.rs}` — **new**
  optional vector pack + raster render; reuse `health.rs`.
- `crates/turbo-tiles-bin/src/main.rs` — `build-basemap --version` subcommand.
- `styles/{n50-topo,n50-winter,n50-hike}.json` — **new** house styles.

**renderer (style):**
- `apps/turbomap/crates/turbomap-core/src/style.rs` — zoom-stops, line
  dash/casing, label placement + rank, numeric filters.
- `apps/turbomap/crates/turbomap-core/src/style/maplibre.rs` — **new** loader.
- `apps/turbomap/crates/turbomap-tiles-pmtiles/` — confirm range-request HTTP
  source (currently local-file) for serving the optional vector PMTiles.

**client:**
- `apps/flutter/lib/features/tile_providers/data/providers/norges_kart_topo.dart`
  — repoint to our raster URL (flagged).
- `apps/flutter/lib/features/tile_providers/data/tile_registry.dart` — feature
  flag + new vector basemap provider + attribution.

**edge / infra:**
- `infra/edge/tiles-worker/` — **new** Cloudflare Worker (R2-as-cache).
- `infra/k8s/base/{kustomization,ingress}.yaml` + `README-tileserver.md` —
  flip tileserver live, add basemap/raster/fonts/sprite paths.
- `infra/k8s/envs/prod/` — R2 cache bucket secret (separate from backup).
- `.github/workflows/tileserver_basemap_build.yml` — **new** versioned build.

---

## A. Validation against real N50 data (2026-06-09)

The contour leg of this plan was executed and verified against the live
dataset, not just designed on paper.

**Pull.** Ordered N50 Kartdata for Oslo (fylke `03`) via the Geonorge
Nedlasting API (`POST /api/order`, format `PostGIS`, projection `25833`),
downloaded `Basisdata_03_Oslo_25833_N50Kartdata_PostGIS.zip` (8.5 MB → 34 MB
SQL). The dump contains the Høyde-theme tables exactly as predicted:
`hoydekurve`, `hjelpekurve`, `forsenkningskurve`, each
`(objid, objtype, senterlinje geometry(_,25833), hoyde integer, …)`.

**Restore + existing pipeline.** Spun up the PostGIS+pgRouting image,
applied all migrations, and restored the dump through the production
`tileserver ingest --job n50-restore` path (hash schema → `n50_staging`).
The existing upserts ran clean on real data — water 434, glaciers 0 (none in
Oslo), landcover 2 619, place names 1 148, roads/paths 16 996 — confirming
the current ingest works on real Kartverket data (previously tested only
against a 5 KB synthetic fixture).

**New contour pipeline (implemented in this change).**
- `migrations/20260603000001_contour_schema.sql` → `terrain.contour`.
- `sql/upsert_n50_hoydekurve.sql` (main/auxiliary/depression, `is_index` at
  100 m, `ST_Dump` to keep clean LineStrings).
- `n50::upsert_hoydekurve` + `JobName::N50HoydekurveUpsert`
  (`n50-hoydekurve-upsert`).

Running it loaded **2 268 features** into `terrain.contour`: 2 263 main
(422 index lines), 3 auxiliary, 2 depression. Distinct main elevations are
`20,40,60,80,100,120,…,620` — **20 m equidistance confirmed**.

**Serve path.** A `ST_AsMVT` basemap tile (z12 / x2170 / y1189, over
Nordmarka) carrying `elev_m`, `kind`, `is_index` rendered **85 contour
features → 13 924 bytes** — the exact `render_basemap_tile` mechanism §4.2
describes, proven on real geometry.

All 35 `turbo-tiles-ingest` unit tests still pass with the new job wired in.

---

## 15. Summary

The N50 topo basemap is largely **already in our database**; this plan turns
that latent data into served, restylable tiles. The critical path is short:
a multi-layer basemap MVT endpoint, contour ingest, glyphs/sprites + a grown
style engine, a raster fallback, and a Cloudflare Worker that uses **R2
strictly as a pull-through distributed cache** in front of our self-hosted
origin. M1 alone removes the Kartverket Norgeskart dependency with no client
renderer change; M3 unlocks custom N50 map styles; M5 brings vector rendering
to mobile. Every tile in R2 is versioned, immutable, and regenerable from
origin — the bucket is disposable by construction.
