# Building routing packs on the device

A proposal. Nothing here is implemented.

The question: can a phone build its own routing pack from Kartverket,
rather than downloading one the tileserver cut? The obvious objection is
that pack building needs PostGIS. That objection is wrong, and this
document is mostly about why.

## 1. PostGIS is a staging store, not a compute engine

Read the three builders and ask what each actually computes.

**`dem_builder`** streams `paths.dem` ordered by `rid`, decodes each
row's `f32` array, zstd-compresses it, appends it, and records
`(ulx, uly, offset, size)` in a tile directory it rewrites at the end.
That is a transcoder. The SQL is `SELECT` — the rows are raster tiles
that `raster2pgsql -t 256x256` put there, each already at its own
origin, which is precisely why format v2 stores one tile per source
raster instead of forcing a global grid.

**`mask_builder`** parses WKB into `geo::Polygon` and rasterises with a
per-polygon scanline fill in Rust. Its SQL is a bbox aggregate and
`ST_AsBinary(ST_Dump(geom))` — "give me the polygons, exploded, as
bytes".

**`graph_builder`** makes exactly three PostGIS calls: `ST_X`, `ST_Y`,
`ST_AsBinary`. CSR construction, the per-profile cost blend, and DEM
sampling for elevation gain are all pure Rust already.

So of the whole pipeline, one genuine algorithm lives in the database:

```sql
SELECT pgr_createTopology('paths.edge', 1.0, 'geom', 'id',
                          'source_node', 'target_node')
```

pgRouting's `createTopology` does **not** split edges at crossings. It
snaps edge *endpoints* to shared nodes within a 1 m tolerance and
populates `paths.node`. That is endpoint clustering, not planar noding —
a uniform grid hash and a union-find, on the order of a hundred lines of
Rust. The hard thing people assume is in there is not in there.

Everything else PostGIS contributes is storage, spatial indexing for the
national incremental-update path, and geometry accessors.

## 2. The sources are already per-region

Both halves are bbox-queryable public services. Measured today, not
assumed:

**Elevation — WCS.** `wcs.geonorge.no/skwms1/wcs.hoyde-dtm-nhm-25833`,
coverage `nhm_dtm_topo_25833`.

| | |
|---|---|
| WCS 2.0.1 `subset=` | HTTP 400, ArcGIS, unhelpful body |
| **WCS 1.0.0 `BBOX`+`WIDTH`/`HEIGHT`** | **HTTP 200** |

A 5.12 km box at `WIDTH=HEIGHT=512` returned a 1 049 839-byte tiled
float32 GeoTIFF in **2.24 s**. Decoded: pixel scale exactly 10.0 m, tie
point exactly the requested corner, 262 144 samples **100 % valid**,
379.4–1352.7 m, mean 986.3 m. Native `offsetVector` is 1 m, so the
server resamples to whatever `WIDTH`/`HEIGHT` asks for — resolution is a
request parameter, not a fixed property.

The coverage is **EPSG:25833**, which is `utm33n` — the pack's own
frame. No reprojection anywhere in the pipeline.

**Trails — WFS.** `wms.geonorge.no/skwms1/wms.traktorveg_skogsbilveger`
(the `wms.` host is deliberate; `wfs.` 500s on GetFeature, and
`fkb_wfs.rs` already carries that workaround). A GetFeature over
`66.83,15.03 → 66.95,15.20` returned **57 features, 124 KB, 1.46 s**,
`numberMatched == numberReturned`, so nothing truncated.

`fkb_wfs.rs` is already a bbox-grid WFS client with a GML 3.2.1 parser
that chunks requests so the per-request feature cap can't silently
truncate. Its only coupling to the database is the staging step.

## 3. Architecture: invert the dependency

Today `Builder { pool: DbPool, out_dir }` — the builders reach for a
database. Instead let them take sources:

```
turbo-pack-build            (new; no sqlx, cross-compiles to Android)
  trait ElevationTiles  → (origin_x, origin_y, cells, Vec<f32>) stream
  trait LineFeatures    → (geometry, fkb_type, attrs) stream
  trait AreaFeatures    → (polygon, kind) stream + overall bbox
  fn build_pack(sources, opts) -> PackFiles + pack.toml

turbo-pack-source-pg        today's SQL, behaviour unchanged
turbo-pack-source-ogc       WCS 1.0.0 + WFS 2.0.0 clients
```

The builders move into `turbo-pack-build` essentially as they are; what
changes is where their rows come from. The value of this shape is not
tidiness — it is that **the server and the device run the same builder**,
so "does a device-built pack match a server-built one" becomes a test
you can actually write, in the style of E10's slice-fidelity test,
rather than a hope.

Five components sit on top:

1. **Region planner.** `GeoBounds` → a deterministic fetch plan: WCS
   tiles and WFS cells. Deterministic matters — it makes the plan
   cacheable, resumable, and diffable between two runs.

   Granularity is a free parameter. `DEFAULT_TILE_CELLS` is 256, so a
   256 px WCS request at 10 m is one DEM tile with no re-tiling — but
   that is 441 requests for a 53 km region. A 1024 px request is ~4 MB,
   36 requests for the same region, split locally into 16 tiles each.
   The second is obviously right; the point is the DEM tile size does
   not dictate the request size.

2. **Fetcher.** Bounded concurrency, backoff, resumable. `PackDownloader`
   already establishes the discipline — verify each unit, stage in
   `.partial`, rename only when whole — and the same rules apply with
   the unit being a source tile instead of a pack file.

3. **Noder.** Replaces `pgr_createTopology`: grid-hash endpoints at 1 m,
   union-find, assign node ids.

4. **Builders.** Existing code, dependency-inverted.

5. **Manifest.** The device writes its own `pack.toml`, same format,
   plus provenance — source URLs, fetch date, requested resolution — so
   a pack on disk can say how it was made. A server-built and a
   device-built pack are then distinguishable when one misbehaves.

## 4. Memory: the DEM streams, everything else is small

The national build's constraint does not survive the trip to a region.

- **DEM.** Never held. Tile in → zstd → append; the artifact is the
  accumulator. Peak is one tile.
- **Mask.** `mask_builder`'s working buffer is `cells_x * cells_y` bytes
  at `DEFAULT_RESOLUTION_M = 100`, which its own comment puts at ~200 MB
  for Norway. Norway is ~324 000 km²; a 53 km region with halo is
  ~3 000 km². That is ~2 MB.
- **Graph.** The Sjunkhatten pack has 4 293 nodes and 9 712 directed
  edges. Nothing.

So the phone never holds the region, and the one buffer that scales with
area scales down by more than a hundredfold.

Budget for a 53 × 53 km region: ~36 WCS requests (~130 MB raw float32),
~10 WFS requests (~1 MB GML), producing the same ~55 MB pack. Minutes on
Wi-Fi, dominated by network, and resumable.

## 5. What I would not hand-wave

**Load on Geonorge is the real risk, and it is not technical.** Today one
server fetches nationally on a cadence. This makes it N phones each
pulling a region — different traffic in kind, not just volume. The data
is open (CC BY 4.0, attribution already carried for the basemap), but
"open" is not "unmetered". Mitigation is to keep the tileserver as the
default path — it is effectively a cache in front of Kartverket — and
treat device build as fallback and opt-in, with caching and honest
backoff. This is a decision to make deliberately, not to discover from
an angry email.

**Water and glacier polygons are the one unverified input.** I confirmed
elevation and trails. `terrain.water_polygon` and `terrain.glacier_polygon`
come from N50 Vann and IsogBre via the Nedlasting API, which is
area-keyed (kommune/fylke) rather than bbox. If there is no bbox WFS for
them, that half needs either a different service or a larger download
than the rest. **Check this before committing to the plan** — it is the
one thing that could change the shape.

**Correctness drift.** WCS resampling, feature-version skew between a
phone fetching today and the server's last national pull, and float
ordering could all make two packs of the same region differ. The shared
builder makes this testable; it does not make it absent. E1 already
established the solver is cross-ISA deterministic, so the exposure is in
the inputs, not the solve.

**APK size.** Adds a GML parser and a TIFF decoder to the Android
library. `quick_xml` is already a dependency and a minimal tiled-float32
TIFF reader is small — the probe above is 20 lines of Python. zstd
already ships, since the DEM is zstd.

**Time and battery.** Needs a foreground service with progress and
cancel. The offline download service already exists and is the natural
host.

## 6. Phasing

Each step is independently useful, and the first two need no phone.

- **S1 — Invert.** Builders take source traits; `turbo-pack-source-pg`
  keeps the server identical. Prove it by rebuilding an artifact and
  diffing byte-for-byte against today's output.
- **S2 — OGC adapter + noder.** `tileserver build-region --bbox` with no
  database at all. Compare against a `slice-pack` cut of the same region
  the way E10 compared slices. This alone gives CI and developers a way
  to cut packs without standing up PostGIS, which is worth having
  whether or not the phone ever does it.
- **S3 — Cross-compile.** Android target, FFI entry point with progress,
  cancel and resume, foreground service.
- **S4 — Surface it.** "Build this region on device" beside the existing
  download.

S2 is the honest decision point: if the device build is going to be
wrong, the desktop run against the same bbox will show it, before any of
the Android work is spent.

## 7. What this unlocks

The framing that makes it worth doing is not "avoid a 55 MB download".
It is that the device stops depending on a region someone else chose to
cut. Any bbox becomes routable, at a resolution the request picks,
without a tileserver in the path at all — and the tileserver stops being
the only thing that can produce a pack, which is the property whose
absence started this whole thread.
