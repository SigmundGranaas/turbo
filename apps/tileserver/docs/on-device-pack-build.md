# Building routing packs on the device

Started as a proposal; now implemented. `turbo-pack-build` is the crate,
`tileserver build-region --bbox ... --kommune ...` the entry point, and
`turbo_route_ffi::build::build_pack` the FFI the phone calls.

The question was: can a phone build its own routing pack from Kartverket,
rather than downloading one the tileserver cut? The obvious objection is
that pack building needs PostGIS. That objection is wrong, and this
document is mostly about why.

**What it does now.** A 12 x 12 km region takes 14.5 s and 5.3 MB: 25
DEM tiles, 453 of 14 637 mask cells refused, 959 nodes and 1 502 directed
edges from 674 trails and 78 roads. Opened through `RouteEngine::open` —
the same entry point the Android app uses — it returns a 528-point,
8 741 m route with 394 m of ascent. No database, no GDAL, no national
artifacts.

Where this document was wrong, it now says so rather than being quietly
edited: see §3 on the refactor that proved unnecessary, and §5 on the
APK cost I guessed low.

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

**This part was unnecessary, and the implementation skipped it.** The
seam already existed: `turbo-tiles-elev`, `-mask` and `-graph` carry no
sqlx, so the format layer was always database-free. A new crate writing
through them coexists with `turbo-tiles-build` rather than refactoring
it, which leaves the server's working pipeline untouched and still lets
the two be diffed.

What *did* move, and for exactly the reason this section gives, are the
two pieces of shared *logic*: the mask's scanline fill into
`turbo-tiles-mask`, and the cost model and attribute encoders into
`turbo-tiles-graph`. Both builders call the same code now. That is the
whole fidelity argument — two builders that rasterise or cost nearly the
same produce artifacts differing along every shoreline or route, and
neither difference announces itself.

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

**Water and glacier polygons — checked, and there is no bbox service.**
The Geonorge catalogue lists N50 only as `GEONORGE:DOWNLOAD`;
`wfs.geonorge.no/skwms1/wfs.n50` answers *"UKJENT APPLIKASJON"*. So the
vector half is ordered per kommune: 25.6 MB zipped for Sørfold, holding
95 MB of Arealdekke with `Innsjø` (6 657), `ElvBekk` (7 936), `Havflate`
(86), `Elv` (85), `InnsjøRegulert` (11) and `SnøIsbre` (52), plus
`Veglenke` (1 341) in Samferdsel. Trails stay on the bbox-scoped FKB sti
WFS. This is the one place the pipeline fetches more than the region
needs, and the shape survived it.

**Correctness drift.** WCS resampling, feature-version skew between a
phone fetching today and the server's last national pull, and float
ordering could all make two packs of the same region differ. The shared
builder makes this testable; it does not make it absent. E1 already
established the solver is cross-ISA deterministic, so the exposure is in
the inputs, not the solve.

**APK size — measured, and I was wrong about it.** I guessed this would
be small because the parsers are small. The parsers *are* small; the
HTTP stack is not. Measured on `aarch64-linux-android`, release:

| | libturbo_route_ffi.so |
|---|---|
| routing only | 1.59 MB |
| with the pack builder | 4.79 MB |

**+3.2 MB per ABI**, roughly tripling the library, and almost none of it
is GML or TIFF — it is `reqwest` + `rustls` + `tokio`, a second TLS
stack and a second async runtime inside an app that already has OkHttp.

That is still far cheaper than the 53 MB bundle it replaces, so it is
not disqualifying. But the duplication suggests a better shape: have the
*host* fetch and hand the Rust side bytes, keeping only the decode,
rasterise, node and write steps in the library. The builder is already
split that way internally — `geotiff`, `gml`, `mask`, `node`, `graph`
and `pack` never touch the network; only `wcs`, `wfs` and `n50` do — so
inverting the fetch is a matter of taking a callback rather than a
`reqwest::Client`. It would drop most of the 3.2 MB and reuse the
retry, proxy and certificate handling the app already has.

Not done here, because it is a real refactor and the current shape
works. Worth doing before this ships to users.

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
