# Tile-Data Architecture — Formats & Tradeoffs

_Analysis for Workstream B. Authored 2026-06-10._

Before wiring "real data", we analysed the format landscape against **what the
repo already has**. The headline correction: the tile stack is largely built
and the desktop app already renders **real OSM vector tiles**. So Workstream B
is mostly *hardening + serverless/offline + real-data testing*, not greenfield.

## Corrected baseline — what already exists

| Concern | State | Where |
|---|---|---|
| Vector encoding | **MVT** decode/encode | `turbomap-mvt` (`decode`/`encode`) |
| Vector source over HTTP | **Done** — `HttpVectorTileSource`, preset `versatiles_osm()` | `turbomap-tiles-http` |
| Raster source over HTTP | Done — `HttpRasterSource` (Kartverket topo, terrain-RGB) | `turbomap-tiles-http` |
| Disk cache | **Done** — `DiskCachedSource<S>`, `<root>/z/x/y`, atomic writes | `turbomap-tiles-cache` |
| PMTiles archive | **Done (file)** — v3 header/directory/Hilbert/gzip reader | `turbomap-tiles-pmtiles` |
| Real schema + style | **Done** — Shortbread layers, hand-authored `Rule` style | `turbomap-app/src/app.rs` |
| Source seam | `TileSource` / `VectorTileSource` traits (`request(TileId)`) | `turbomap-core` |

The renderer is **schema-agnostic**: it tessellates whatever layers the style
references. The "data decision" is therefore three near-independent axes —
**schema**, **packaging/transport**, **style ingestion** — plus encoding
(already settled on MVT).

## Axis 1 — Encoding: MVT (settled)

Mapbox Vector Tiles (protobuf, per-layer 4096 extent, geometry commands) are
the universal interchange; every option below emits MVT and we already decode
it. **No decision needed.**

## Axis 2 — Schema (the real fork)

What layer/field taxonomy the data — and our style rules — speak.

| Schema | Layers | Tiles available | Notes |
|---|---|---|---|
| **Shortbread** (in use) | ~15 (`streets`, `water_lines`, `buildings`, `boundaries`, `place_labels`, `street_labels`, `land`, `ocean`, `bridges`, …) | VersaTiles (free planet, XYZ **and** `.versatiles`/PMTiles) | Already wired + styled; open; simple. |
| **OpenMapTiles** | ~16 (`transportation`, `transportation_name`, `water`, `landuse`, `boundary`, `place`, `poi`, `building`, …) | MapTiler (free tier + paid), self-host | Largest ecosystem of MapLibre styles target it; best-documented `brunnel`/class fields. |
| **Protomaps Basemap** | ~12 (`roads`, `water`, `landuse`, `buildings`, `places`, `pois`, `boundaries`, `transit`, …) | Protomaps **free planet `.pmtiles`** + extracts | PMTiles-native; serverless distribution is the design goal. |
| Mapbox Streets v8 | ~proprietary | Mapbox (token, ToS) | Vendor lock-in; rejected. |

Tradeoff: **Shortbread** is lowest-effort (style + wiring exist) and has free
planet PMTiles. **OMT** unlocks the most existing GL styles *if* we build the
style-JSON loader. **Protomaps** is the cleanest serverless story (download one
planet `.pmtiles`, range-serve from any static host) but means re-authoring the
style to its layer names.

## Axis 3 — Packaging / transport

How tiles are stored and reach the renderer.

| Packaging | Server needed? | Offline | Random access | Dep cost | Status here |
|---|---|---|---|---|---|
| **XYZ HTTP** (`/z/x/y.pbf`) | yes (tile server/CDN dir) | weak (cache only) | per-tile GET | `reqwest` (have) | **Done** |
| **MBTiles** (SQLite) | no (local file) | strong | SQL row | `rusqlite` (heavy, C dep) | absent |
| **PMTiles** (single file) | **no** | **strong** | **HTTP range** *or* file offset | pure-Rust (have) | **file done; range not** |

PMTiles is the standout: one artifact is **both** an offline bundle (open the
local file) **and** an online source (HTTP range requests against dumb static
storage — S3/R2/CDN, no tile server). MBTiles only does offline and drags in
SQLite. The single missing piece is **PMTiles-over-HTTP-range** in our reader
(it currently takes a `File`); the directory/Hilbert/decompress logic is
already there, so this is a focused addition (a `RangeReader` trait with `File`
and `reqwest`-range impls).

## Axis 4 — Style ingestion

| Approach | Pro | Con |
|---|---|---|
| **Hand-authored `Scene`/`Rule`** (in use) | Full control; no expression-eval surface | Author styles in Rust; can't drop in community styles |
| **MapLibre GL Style JSON loader** | Drop in any public style; ecosystem | Large surface — `interpolate`/`case`/`match`/`step` + data expressions; our `Paint<T>` covers a subset (Const/Zoom/Match) |

A **partial** GL-JSON loader (paint/layout props + the common expressions, mapped
onto `Paint<T>`, widening it as needed) is high-value but is its own workstream.
Not a blocker for real data — the hand-authored Shortbread style already works.

## Recommendation

Lowest-regret, builds on what exists, maximises the serverless+offline story,
and is fully testable headless:

1. **Encoding** — MVT (keep).
2. **Schema** — **keep Shortbread**. The style + wiring exist and VersaTiles
   publishes free planet tiles in both XYZ and PMTiles. (Adopt Protomaps only
   if we specifically want its planet `.pmtiles` cadence — a re-style, deferable.)
3. **Packaging** — **extend the PMTiles reader to HTTP range** behind a small
   `RangeReader` trait (`File` + `reqwest` byte-range impls). This yields one
   artifact that serves online (range, no server) and offline (local file) —
   the biggest architectural win still on the table.
4. **Real-data golden** — bundle a **tiny Shortbread `.pmtiles` extract** (one
   small city, a few z-levels, ≤ a couple MB) as a test fixture and render it
   through the real style → the first deterministic **real-data** golden,
   replacing synthetic-only proof. Fully headless.
5. **Tile-stack hardening** (incremental, each testable): conditional requests
   (ETag/`Cache-Control`), exponential backoff + jitter honouring `Retry-After`,
   disk-cache **LRU eviction** (today it only grows), viewport + next-zoom
   **prefetch**, and a host-implementable `VectorTileSource` over uniffi.
6. **z-order / brunnel** — add an explicit sort key to `Rule` and order
   bridge/tunnel/`brunnel` correctly; verify with the real-data golden.
7. **Style-JSON loader** — separate, later workstream; widen `Paint<T>` toward
   GL expressions as it lands.

Sequence: **(3)+(4) first** — they convert "renders real tiles in the app" into
"renders real tiles *and proves it in CI, online-or-offline, from one file*",
which is the credibility jump. Then (5)/(6) hardening. (7) when ecosystem-style
support is wanted.

## Test strategy

- PMTiles range reader: unit-test the `RangeReader` (file vs a stubbed
  range source return identical tiles); the existing header/directory/Hilbert
  tests already cover decoding.
- Real-data golden: bundle the small extract; assert the real Shortbread style
  renders roads/water/buildings/labels; pin with a perceptual golden.
- Hardening: cache hit/miss/expiry/eviction + backoff schedule as pure unit
  tests; prefetch coverage in `turbomap-sim`.

## Open sub-decision (for sign-off)

- **Schema:** keep **Shortbread** (recommended, lowest-effort) vs adopt
  **Protomaps** (best planet-PMTiles cadence, costs a re-style) vs **OMT**
  (only worth it alongside the style-JSON loader).
- **First slice:** PMTiles-HTTP-range + real-data golden (recommended) vs
  start with tile-stack hardening (cache LRU + backoff + prefetch).
