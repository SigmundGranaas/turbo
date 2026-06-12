# Self-Hosted Map Tiles & Geometry — Architectural Feasibility Study

**Date:** 2026-06-09
**Scope:** Norway only.
**Question:** Can we serve our own raster + vector tiles, geometry, and map
data — and build our own custom map styles — instead of depending on
Kartverket Norgeskart (the "Norwegian topo" basemap) and the other external
Geonorge/NVE/Google services the app fetches live today?

**Short answer:** Yes, and we are most of the way there structurally. The
data is open (NLOD / CC-BY 4.0), and the two hardest pieces — a PostGIS
ingest + MVT pipeline (`apps/tileserver`) and a GPU vector renderer with its
own style engine (`apps/turbomap`) — already exist in this repo. What is
missing is not a new platform; it is (1) promoting the basemap feature data
we *already ingest* from routing-only inputs to a served **basemap vector
tileset**, (2) a styling/glyph/sprite story, and (3) the serving + storage
posture (PMTiles + CDN) to make it cheap and offline-friendly. This document
assesses the gap and proposes a phased path.

---

## 1. Recommendation up front

1. **Go vector-first, not raster-first.** Serving a vector basemap (MVT) and
   styling it on-device is what unlocks "custom map styles" and removes the
   Norgeskart raster dependency in one move. Pre-rendered raster is a
   *derived* fallback for `flutter_map`, not the primary target.
2. **Generate, don't proxy.** Build the tiles from open Kartverket source data
   we already ingest into PostGIS, package them as **PMTiles**, and serve them
   as static range-request reads behind the existing Traefik ingress / a CDN.
   This is dramatically cheaper and more reliable than live dynamic rendering
   for a country-sized, slowly-changing basemap.
3. **Reuse what exists.** `turbo-tiles-*` already ingests N50/FKB/DTM and
   renders MVT; `turbomap-core` already has a `style` module, MVT decode,
   hillshade, terrain, and PMTiles read support. The work is integration and
   data-promotion, not greenfield.
4. **Sequence by dependency removal, not by feature.** Each phase should
   delete one external hostname from the Flutter client and prove parity.

This is feasible for a single-maintainer / small-team footprint because
Norway-only keeps the data volume and tile pyramid bounded (see §6 sizing).

---

## 2. Where we are today (grounded current state)

### 2.1 What the Flutter client fetches externally

The client (`apps/flutter`, `flutter_map` v8.2.2) currently depends on these
external services:

| Concern | External source | File |
| --- | --- | --- |
| **Raster basemap ("topo")** | Kartverket WMTS `cache.atgcp1-prod.kartverket.cloud` (`layer=topo`) | `lib/features/tile_providers/data/providers/norges_kart_topo.dart` |
| Satellite | Google `mt0.google.com/vt` | `.../google_sattelite.dart` |
| Global street | OSM `tile.openstreetmap.org` | `.../osm_tiles.dart` |
| Trail overlay (raster) | Geonorge WMS `wms.geonorge.no/.../wms.friluftsruter2` | `.../nasjonal_turbase_overlay.dart` |
| Trail vectors (live) | Geonorge WFS `wfs.geonorge.no/.../wfs.turogfriluftsruter` | `lib/features/external_vector_layers/.../nasjonal_turbase_source.dart` |
| N50 paths (live) | Geonorge WMS `wms.geonorge.no/.../traktorveg_skogsbilveger` | `.../n50_sti_source.dart` |
| OSM paths (live) | Overpass `overpass-api.de` | `.../osm_path_source.dart` |
| Avalanche | NVE `gis3.nve.no/arcgis/.../Bratthet_med_utlop_2024` | `.../avalanche_overlay.dart` |
| Place / address / elevation / municipality search | Geonorge `ws.geonorge.no/{stedsnavn,adresser,hoydedata,kommuneinfo}` | `lib/features/search/...`, `lib/core/api/kartverket_hoydedata_client.dart` |
| **Curated paths (vector, ours)** | **Our tileserver** `api.sandring.no/api/tiles/v1/{resource}/tiles/{z}/{x}/{y}.mvt` | `lib/features/curated_paths/providers/curated_path_providers.dart` |

The single most important dependency to remove is the **Kartverket topo
WMTS basemap** — it is the map everything else draws on top of, it is a
single un-fallback'd CDN host, and it dictates our cartography (we cannot
restyle someone else's pre-rendered PNGs).

### 2.2 What we already own (and it is a lot)

**`apps/tileserver`** — Rust, AGPL, PostGIS + pgRouting. Already in the repo:

- **MVT serving** via `ST_AsMVT` (`turbo-tiles-mvt`,
  `/v1/{resource}/tiles/{z}/{x}/{y}.mvt`) with proper
  `Cache-Control: public, max-age=86400, stale-while-revalidate=604800`.
  Today scoped to **4 curated path resources only** (`hiking-trails`,
  `ski-tracks`, `forest-roads`, `cycling-routes` — `turbo-tiles-core/resource.rs`).
- **Ingest jobs** (`turbo-tiles-ingest`) for the exact open datasets a
  basemap needs: `fkb-sti`, `turbase`, `dnt`, `dtm10`/`dtm-bulk-load`, and
  the **N50** family — `n50-vann-upsert` (water), `n50-vegnett-upsert`
  (roads), `n50-stedsnavn-upsert` (place names), `n50-landcover-upsert`,
  `n50-isogbre-upsert` (glaciers).
- **A generic vector-feature builder** driven by `tools/vector-layers.toml`
  — already producing `water`, `ocean`, `streams`, `wetland`, `cultivated`,
  `building` collections from N50 staging. **These are basemap layers**; they
  are currently consumed only as routing cost layers and offline artifacts,
  not *served* as basemap tiles. That is the key latent asset.
- **DEM**: `norway.dem` artifact (~11 GiB), a **Terrain-RGB PNG tile
  endpoint** (`/v1/dem/rgb/{z}/{x}/{y}.png`, Mapbox encoding), plus
  slope/elev/mask endpoints — i.e. the inputs for **hillshade and contours**.
- **Search** (`turbo-tiles-search`, FST anchors from `n50-anchors`) — a path
  off Geonorge `stedsnavn`.
- **Admin panel**, health audits, routing/isochrone (FMM + pgRouting).

**`apps/turbomap`** — Rust, AGPL, `wgpu` renderer designed as an
**FFI-ready library** (targets desktop today; Android JNI / wasm / iOS by
design). Already in the repo:

- A **`style` module** (`turbomap-core/src/style.rs`): `VectorStyle` =
  ordered `Rule`s with `Paint::{Fill, Line, Text}` + colors. "Intentionally
  narrower than the Mapbox Style Spec — with room to grow." This is our
  custom-style foundation.
- **MVT decode** (`turbomap-mvt`), vector tessellation, raster tiles,
  **hillshade** + **terrain** shaders, markers, text/labels.
- **PMTiles v3 reader** (`turbomap-tiles-pmtiles`) for raster *and* vector
  archives — local file today, "can be served via HTTP range requests."
- A clean `TileSource` / `VectorTileSource` pull-push contract; currently
  wired to a Kartverket Turkart raster HTTP source as its MVP demo.

**Infra** (`infra/k8s`): a reviewed-but-inert `tileserver.yaml` (single
replica, CNPG PostGIS+pgRouting `tiles-db`, **14 GiB artifacts on a hostPath
PV**, Traefik ingress at `kart-api.sandring.no`, GHCR image built by CI).
Deliberately not yet wired into `kustomization.yaml`. Node is small
(~5.7 GiB RAM); the DEM is mmap'd so cold pages evict to SSD.

**Implication:** we are not asking "should we build a tile platform"; we
have one. The study is really about **promoting basemap data we already
ingest into a served, styled basemap** and choosing the cheapest serving
posture.

---

## 3. Goals & non-goals

**Goals**
- Remove the hard dependency on the Kartverket topo raster basemap.
- Serve a Norway vector basemap (MVT/PMTiles) we can **restyle freely** —
  multiple house styles (summer/topo, winter/ski, hike, satellite-hybrid).
- Serve raster tiles as a derived fallback for `flutter_map` clients that
  cannot run the vector renderer yet.
- Own the geometry/data endpoints (paths, water, contours, place search)
  so the live WFS/WMS/Overpass calls go away.
- Keep it cheap, offline-capable, and operable by a small team.

**Non-goals (for this study)**
- Worldwide coverage. Norway only; OSM/Google stay as optional global layers.
- Live, second-fresh data. The basemap is a slowly-changing dataset; weekly/
  monthly rebuilds are fine.
- Turn-by-turn road navigation. Routing already exists separately.
- Replacing satellite imagery (we have no aerial source; Google/Norge i bilder
  stays an external opt-in layer).

---

## 4. Data sources & licensing (Norway)

All the primary inputs are **open** and already ingested or ingestable:

| Layer group | Source dataset | Owner | Licence |
| --- | --- | --- | --- |
| Land/water/roads/contour-base topo | **N50 Kartdata** (and finer N20/FKB where needed) | Kartverket | NLOD / CC-BY 4.0 |
| Detailed paths | **FKB / Elveg / traktorveg** | Kartverket | NLOD |
| Elevation / hillshade / contours | **DTM (DTM10 / nasjonal høydemodell)** | Kartverket | NLOD / CC-BY 4.0 |
| Trails & cabins | **Nasjonal Turbase**, DNT | Kartverket / DNT | CC-BY 4.0 (attribution) |
| Place names (search + labels) | **SSR / Stedsnavn (N50 stedsnavn)** | Kartverket | NLOD |
| Hazard overlay | NVE Bratthet/avalanche | NVE | NLOD (keep as overlay) |

**Licensing posture:** NLOD and CC-BY 4.0 permit redistribution and derived
products (incl. self-served tiles and custom styles) **with attribution**.
We must carry "© Kartverket" / "© Kartverket, Nasjonal Turbase / DNT" /
"© NVE" in the style's attribution block — the client already tracks
per-source attribution strings, so this is a data-carry, not new mechanism.
Our own tile code is AGPL-3.0 (matches `tileserver`/`turbomap`); serving
tiles over the network triggers AGPL's network clause, which is fine for a
first-party service but should be a conscious choice if any of this is ever
offered as a third-party tile API.

**Action item:** confirm the exact attribution text and any N50 redistribution
notice with Geonorge's terms before GA. This is the one true external
dependency that does *not* go away — the licence obligation — but it is a
footer string, not a runtime call.

---

## 5. Architecture options

### Option A — Dynamic MVT from PostGIS (extend what runs today)

Add basemap resources (water, roads, landcover, contours, labels…) to the
existing `ST_AsMVT` path and the `Resource` enum, serve them live like the
curated paths already are.

- **Pros:** smallest code delta; reuses `turbo-tiles-mvt` verbatim; always
  fresh; no tile storage.
- **Cons:** every basemap tile is a live PostGIS query. A basemap is
  requested *constantly* (it is the bottom layer). On the current single
  small node this is the wrong cost curve — CPU and DB load scale with users,
  and low-zoom tiles that cover all of Norway are expensive to generate
  on demand. Caching helps but you still pay first-hit and cache-miss storms.
- **Verdict:** good for *curated, sparse, changing* layers (where it is used
  today). Wrong primary choice for the dense, hot, static basemap.

### Option B — Pre-generated PMTiles, served static (recommended)

Build the whole Norway basemap pyramid offline into one (or a few) **PMTiles
v3** archives, then serve them as immutable static files via HTTP range
requests (Traefik today; Cloudflare R2 + CDN later). The Flutter client
reads vector tiles and styles them with our style engine; a parallel raster
PMTiles archive feeds `flutter_map` as a fallback.

- **Pros:** serving is a static byte-range read — trivially cacheable, CDN-
  friendly, near-zero per-request CPU, horizontally free. **Offline is
  native** (the same `.pmtiles` file the server uses can be shipped/synced to
  the device — `turbomap-tiles-pmtiles` already reads it). Rebuild cadence is
  decoupled from serving. One artifact = one atomic, versioned basemap.
- **Cons:** a build pipeline (tippecanoe-style MVT generation, or our own
  `ST_AsMVT`-to-PMTiles packer) and storage for the archive. Data freshness
  is rebuild-cadence (acceptable per non-goals).
- **Verdict:** this is the right primary for a country basemap and the one
  that aligns with `turbomap`'s existing PMTiles support and offline goals.

### Option C — Pre-rendered raster PNG/WebP tiles

Render styled raster tiles to PNG/WebP (also packable into PMTiles) for
`flutter_map`.

- **Pros:** drop-in replacement for the Norgeskart WMTS source the client
  uses today — removes that dependency with *no client renderer change*.
- **Cons:** no on-device restyling (cartography baked at build time); larger
  storage than vector (raster pyramid for Norway is many tens of GiB at high
  zoom); a style change means a full re-render.
- **Verdict:** keep as a **derived output of the same pipeline** for the
  fastest dependency-removal win and for legacy/web clients, not as the
  strategic target.

### Recommended hybrid

```
                    Kartverket open data (N50 / FKB / DTM / Turbase / SSR)
                                       │  (ingest jobs — already exist)
                                       ▼
                         PostGIS (tiles-db, CNPG)  ── routing/curated MVT (today)
                                       │
                 ┌─────────────────────┼──────────────────────────┐
                 ▼ (build, offline)    ▼ (build, offline)          ▼ (live)
        basemap.pmtiles (vector)   hillshade/terrain-rgb        curated paths
        + contours + labels        .pmtiles (raster, from DEM)  MVT (ST_AsMVT)
                 │                          │                        │
                 └──────────────┬───────────┘                       │
                                ▼  static range reads / CDN          ▼ dynamic
                       kart-api.sandring.no  (Traefik → R2/CDN)   tileserver
                                │
              ┌─────────────────┴───────────────────┐
              ▼                                       ▼
   turbomap (vector + style + hillshade)   flutter_map (raster fallback PMTiles)
   → custom styles on-device                → drop-in topo replacement
```

- **Vector basemap** = PMTiles, styled on-device by `turbomap`'s style engine
  → enables *custom styles* and offline.
- **Raster fallback** = PMTiles of styled PNG/WebP → drop-in for the existing
  `flutter_map` topo layer while the vector renderer matures on mobile.
- **Curated/changing layers** stay dynamic `ST_AsMVT` (no change).
- **Hillshade/contours** derived from the existing `norway.dem`.

---

## 6. Sizing & cost (Norway-only keeps this bounded)

Rough order-of-magnitude for a Norway basemap (~385,000 km²; mainland +
near coast). These are planning numbers to size storage and the node, not
commitments — validate during Phase 0.

| Artifact | Zooms | Est. size | Notes |
| --- | --- | --- | --- |
| Vector basemap PMTiles (N50-class, no buildings) | z0–14 | ~3–10 GiB | Dominated by z13–14 lines (roads/streams/contours). Buildings push higher. |
| Contours (50/100 m from DTM) | z10–14 | ~2–5 GiB | Can be a separate archive / overzoomed. |
| Raster fallback (WebP, styled topo) | z0–15 | ~20–60 GiB | Why raster is the *fallback*, not primary. Cap max zoom + overzoom on client. |
| Hillshade / Terrain-RGB raster | z0–13 | ~5–15 GiB | Derived from existing 11 GiB DEM. |
| Glyphs (PBF SDF) + sprites | — | tens of MiB | One-time per font/sprite set. |

**Storage/serving:** PMTiles + a CDN (Cloudflare R2 is already in the infra
vocabulary — see `r2-backup-sealedsecret.yaml`) makes serving effectively a
solved, cheap problem: egress-billed static reads, no compute. The current
single k3s node (~5.7 GiB RAM, hostPath PV for 14 GiB artifacts) can host the
**vector** basemap comfortably; the raster fallback wants object storage +
CDN rather than the node disk.

**Build cost:** the heavy step is the offline tile build (minutes-to-hours
per full rebuild). It runs as a job, not in the request path, so it does not
size the production node — it can run on a beefier build box / CI runner and
publish the artifact.

---

## 7. Gap analysis — what is missing vs. what exists

| Capability | Status | Work to close |
| --- | --- | --- |
| Open source data licence allows it | ✅ NLOD/CC-BY | Confirm attribution text (§4). |
| Ingest of N50/FKB/DTM/Turbase/SSR into PostGIS | ✅ jobs exist | Verify full-Norway ingest (not just routing extent). |
| Basemap feature layers in DB | ✅ as routing cost layers (`vector-layers.toml`) | **Promote to basemap layers**: add roads (`vegnett`), labels (`stedsnavn`), landcover classes, coastline as *served* layers. |
| MVT generation | ✅ `ST_AsMVT` (curated) | Extend `Resource`/layer model to basemap layers; add zoom-dependent generalization (simplify at low z). |
| PMTiles packaging | ⚠️ read-only in `turbomap` | **Add a writer/packer** (or adopt tippecanoe in the build job) to emit `.pmtiles`. |
| Contour generation from DEM | ⚠️ DEM + slope exist | Add a contour build step (gdal_contour-equivalent) → contour layer. |
| Hillshade raster tiles | ⚠️ Terrain-RGB endpoint + hillshade shader exist | Either ship Terrain-RGB (client hillshades — preferred, `turbomap` already does) or pre-render hillshade PNGs. |
| Vector style engine | ✅ `turbomap-core/style.rs` (minimal) | Grow toward (a subset of) Mapbox Style Spec: zoom-stops, filters, label placement, sprites; author 2–3 house styles. |
| Glyphs (label fonts) + sprites (icons) | ❌ | Generate SDF glyph PBFs + a sprite sheet; serve statically. Needed for labels/POIs. |
| Place-name search (replace Geonorge stedsnavn) | ✅ `turbo-tiles-search` (FST) | Wire Flutter search to our endpoint; keep Geonorge as fallback. |
| On-device vector renderer on mobile | ⚠️ `turbomap` is FFI-ready but desktop-only today | Android JNI + iOS bindings, or interim: raster fallback via `flutter_map`. |
| Serving/CDN posture | ⚠️ node hostPath only | Add R2 + CDN for PMTiles; Traefik range-request passthrough. |
| Infra wired live | ❌ `tileserver.yaml` inert | Flip per `README-tileserver.md` once data is staged. |

**The critical-path gaps are narrow:** (1) promote DB basemap layers + add
zoom generalization, (2) a PMTiles writer/packer, (3) glyphs/sprites + grow
the style engine, (4) a mobile rendering path (or accept raster fallback
first). Everything else is integration of parts that already exist.

---

## 8. Proposed phased roadmap

Each phase removes or de-risks one external dependency and is independently
shippable. Ordered by value-per-effort and dependency removal.

**Phase 0 — Spike & sizing (1–2 weeks).** Ingest full-Norway N50 into
`tiles-db`. Build a *one-region* vector PMTiles (e.g. Bergen tile from the
existing turbomap demo extent) end-to-end: PostGIS → MVT → `.pmtiles` →
`turbomap` renders it with a hand-written style. Measure real archive sizes
(validate §6). **Decision gate:** confirms PMTiles writer approach and the
style-engine reach needed.

**Phase 1 — Raster fallback topo (removes the Norgeskart WMTS dependency).**
Pre-render a styled raster Norway basemap to PMTiles/WebP, serve it, and
point the Flutter `norges_kart_topo` provider at our URL behind a feature
flag. This is the fastest path to "no longer depending on Kartverket's CDN"
with **zero client-renderer change**. Ship to a % of users, compare tiles.

**Phase 2 — Vector basemap + style engine (unlocks custom styles).** Promote
the DB basemap layers, generate the vector PMTiles, grow `turbomap`'s style
module (zoom-stops, filters, sprites), author the first house topo style,
add glyphs/sprites serving. Render on desktop/web first.

**Phase 3 — Contours + hillshade.** Add contour generation from `norway.dem`
and ship Terrain-RGB tiles; let `turbomap`'s existing hillshade/terrain
shaders light the basemap. This is where our map starts to look *better* than
the proxied topo (own relief shading, own contour styling).

**Phase 4 — Own geometry/search endpoints.** Move the live WFS/WMS/Overpass
trail/path layers and Geonorge `stedsnavn`/`hoydedata`/`adresser` search onto
our `tileserver` (curated MVT + `turbo-tiles-search` + elevation), removing
`wfs.geonorge.no`, `wms.geonorge.no`, `overpass-api.de`, and `ws.geonorge.no`
runtime calls. Keep them as graceful fallbacks.

**Phase 5 — Mobile vector renderer + CDN GA.** Ship `turbomap` to Android/iOS
via FFI (replacing the raster fallback with live vector + custom styles on
mobile), and move PMTiles serving to R2 + CDN for cost/scale. Retire the
raster fallback once vector parity holds. NVE avalanche + Google satellite
remain optional external overlays by choice (no open self-host source).

---

## 9. Risks & mitigations

| Risk | Mitigation |
| --- | --- |
| **Cartographic quality gap.** Kartverket's topo is professionally styled; matching it is real cartography work. | Vector + iterative styling lets us tune forever; ship raster fallback first so we are never *worse* than today while styling matures. |
| **Style-engine reach.** `turbomap`'s style is "intentionally narrower than Mapbox spec." Labels, collision, zoom-stops are the hard parts. | Grow incrementally; the spike (Phase 0) sizes exactly how far it must go. Consider MapLibre style JSON as the interchange format to stay compatible. |
| **Mobile rendering not yet real.** `turbomap` runs on desktop only today. | Phase 1 raster fallback decouples dependency-removal from the mobile renderer; vector-on-mobile is Phase 5, not blocking. |
| **Node capacity.** Single ~5.7 GiB k3s node already hosts 14 GiB artifacts. | Vector basemap is small; push raster + hillshade to R2/CDN; run builds off-node. |
| **Data freshness / rebuild ops.** Stale basemap if rebuilds are manual. | Scheduled CI rebuild job publishing versioned PMTiles; health audits already exist in `turbo-tiles-build`. |
| **Licence / attribution slip.** NLOD/CC-BY require visible attribution. | Carry attribution in style metadata (client already renders per-source attribution); legal check before GA (§4). |
| **AGPL network clause.** Serving our AGPL tile code. | Fine for first-party; revisit only if exposing a third-party tile API. |
| **Full-Norway ingest unproven.** Jobs exist but may have only run on routing extents. | Phase 0 explicitly ingests and audits the full country. |

---

## 10. Conclusion

Self-hosting Norway's geometry, raster, and vector tiles — and building our
own map styles — is **feasible and substantially de-risked by work already in
this repo.** The data is open, the ingest + MVT pipeline exists
(`apps/tileserver`), and a GPU renderer with its own style engine, MVT
decode, hillshade, and PMTiles support exists (`apps/turbomap`). The strategic
move is a **vector-first, pre-generated PMTiles** basemap served statically,
with a **pre-rendered raster fallback** to remove the Norgeskart dependency
immediately and a **custom on-device style engine** to unlock bespoke
cartography. The remaining work is integration and data-promotion across four
narrow gaps — a PMTiles packer, basemap-layer promotion + generalization,
glyphs/sprites + a richer style engine, and a mobile rendering path — not a
new platform. Recommended next step: execute **Phase 0** to validate sizing
and the PMTiles writer, then ship the Phase 1 raster fallback to delete the
first external hostname.

---

### Appendix A — External hostnames to retire (client) and their replacement

| Hostname (today) | Replaced by | Phase |
| --- | --- | --- |
| `cache.atgcp1-prod.kartverket.cloud` (topo) | our raster PMTiles → vector basemap | 1 → 2 |
| `wms.geonorge.no` (friluftsruter, traktorveg) | curated MVT (`tileserver`) | 4 |
| `wfs.geonorge.no` (turogfriluftsruter) | curated MVT (`tileserver`) | 4 |
| `overpass-api.de` (OSM paths) | curated MVT / N50 paths (`tileserver`) | 4 |
| `ws.geonorge.no` (stedsnavn/hoydedata/adresser/kommuneinfo) | `turbo-tiles-search` + elevation endpoints | 4 |
| `gis3.nve.no` (avalanche) | slope-angle bands from our DEM (`/v1/slope/tiles`); NVE kept only for runout ("utløp") zones | 5 |
| `mt0.google.com` (satellite) | *kept as opt-in overlay* | — |
| `tile.openstreetmap.org` | *kept as opt-in global layer* | — |

### Appendix B — Key code touchpoints

- Ingest jobs: `apps/tileserver/crates/turbo-tiles-ingest/src/job.rs`
- Served resources: `apps/tileserver/crates/turbo-tiles-core/src/resource.rs`
- MVT generation: `apps/tileserver/crates/turbo-tiles-mvt/`
- Basemap layer config: `apps/tileserver/tools/vector-layers.toml`
- DEM / Terrain-RGB: `apps/tileserver/crates/turbo-tiles-api/src/v1/dem.rs`
- Vector style engine: `apps/turbomap/crates/turbomap-core/src/style.rs`
- PMTiles reader (needs a writer): `apps/turbomap/crates/turbomap-tiles-pmtiles/`
- Client basemap provider: `apps/flutter/lib/features/tile_providers/data/providers/norges_kart_topo.dart`
- Deploy (inert): `infra/k8s/base/tileserver.yaml`, `infra/k8s/README-tileserver.md`
