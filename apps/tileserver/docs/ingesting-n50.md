# Ingesting Norwegian map data (N50 + friends)

How we pull open Kartverket data into the tileserver's PostGIS store and turn
it into the canonical tables the basemap, routing, and search read. Written
to be reproducible from a clean machine.

The guiding principle for dev and CI: **use small samples.** The whole-country
N50 PostGIS dump is ~25 GB; a single county (fylke) is a few MB and exercises
every ingest code path against real Kartverket data in seconds. The automated
tests use an even smaller 5 KB synthetic fixture.

---

## 1. Data source — Geonorge Nedlasting API

N50 Kartdata is downloaded from the Geonorge "Nedlasting" (download) API. It
supports format, projection, and **area** selection, so we can pull one county
at a time. We always take **PostGIS** in **EUREF89 UTM33 (EPSG:25833)** — the
SRID every `terrain.*` / `paths.*` table stores geometry in, so no reprojection
is needed at load.

| | |
| --- | --- |
| Metadata UUID | `ea192681-d039-42ec-b1bc-f3ce04c189ac` |
| Capabilities | `GET https://nedlasting.geonorge.no/api/capabilities/{uuid}` |
| Formats | FGDB, GML, SOSI, **PostGIS** |
| Projections | 25832 / **25833** / 25835 |
| Area units | per **fylke** (county) or whole country |

A single county dump arrives as a `pg_dump` SQL file inside a zip, e.g.
`Basisdata_03_Oslo_25833_N50Kartdata_PostGIS.zip` (Oslo ≈ 8 MB zipped → 34 MB
SQL). Every county uses a per-dump **hash-named schema**
(`n50kartdata_<hex>`); the restore step renames it to `n50_staging`.

### Pull a county

```sh
# Oslo (03) is the smallest/fastest; Vestland (46) has fjords + glaciers.
apps/tileserver/scripts/pull-n50-fylke.sh 03 /tmp/n50oslo
```

That script POSTs an order and downloads + unzips the result. The raw API call
it makes:

```sh
curl -fsS -X POST https://nedlasting.geonorge.no/api/order \
  -H 'Content-Type: application/json' \
  -d '{"email":"noreply@example.com",
       "orderLines":[{"metadataUuid":"ea192681-d039-42ec-b1bc-f3ce04c189ac",
         "areas":[{"code":"03","type":"fylke"}],
         "formats":[{"name":"PostGIS"}],
         "projections":[{"code":"25833",
           "codespace":"http://www.opengis.net/def/crs/EPSG/0/25833"}]}]}'
# → response has files[].downloadUrl (status "ReadyForDownload"); GET it.
```

---

## 2. Database

The tiles DB is PostGIS + pgRouting. For local work, build the image and run it:

```sh
docker build -t turbo-tiles-db infra/compose/postgis-pgrouting
docker run -d --name tiles-db --network host \
  -e POSTGRES_USER=postgres -e POSTGRES_PASSWORD=testpass -e POSTGRES_DB=tiles \
  turbo-tiles-db
export DATABASE_URL=postgres://postgres:testpass@localhost:5432/tiles
```

> **Image pin.** The Dockerfile pins a **stable** `postgis/postgis:17-3.5` tag.
> Do not move it back to `:17-master` — that rolling dev tag has shipped a
> PostGIS whose SQL/C library drift out of sync (`CREATE EXTENSION postgis`
> fails with `could not find function "ST_MMin"`), which breaks every
> migration at boot.

The migrations create the four extensions they need (`postgis`,
`postgis_raster`, `pgrouting`, `pg_trgm`).

---

## 3. Migrate → restore → upsert

The ingest pipeline is two-phase: one heavy **restore** (psql-loads the dump
into `n50_staging`, ~10–20 min for the whole country, seconds for a county),
then several cheap **upserts** that read `n50_staging` and write the canonical
tables. Re-running an upsert is cheap because the restore is amortised.

```sh
tileserver migrate                                          # apply schema
tileserver ingest --job n50-restore --file /tmp/n50oslo/n50.zip

# Canonical upserts (order independent):
tileserver ingest --job n50-vann-upsert         # → terrain.water_polygon
tileserver ingest --job n50-hoydekurve-upsert   # → terrain.contour
tileserver ingest --job n50-bygning-upsert      # → terrain.building_polygon
tileserver ingest --job n50-kystkontur-upsert   # → terrain.coastline
tileserver ingest --job n50-isogbre-upsert      # → terrain.glacier_polygon
tileserver ingest --job n50-landcover-upsert    # → terrain.landcover_patch
tileserver ingest --job n50-stedsnavn-upsert    # → anchors.anchor
tileserver ingest --job n50-vegnett-upsert      # → paths.edge
```

Each prints `{"job":…,"rows_in":N,"rows_upserted":N}`. An upsert run before its
restore fails loudly (`…not found; run n50-restore`) rather than silently
no-op'ing.

### Verify

```sh
# Row counts per canonical table.
psql "$DATABASE_URL" -c "
  SELECT 'contour' t, count(*) FROM terrain.contour
  UNION ALL SELECT 'water',     count(*) FROM terrain.water_polygon
  UNION ALL SELECT 'landcover', count(*) FROM terrain.landcover_patch
  UNION ALL SELECT 'anchors',   count(*) FROM anchors.anchor
  UNION ALL SELECT 'edges',     count(*) FROM paths.edge;"

# Render an MVT tile straight from PostGIS (the basemap serve path).
psql "$DATABASE_URL" -At -c "
  WITH b AS (SELECT ST_TileEnvelope(12,2170,1189) e,
                    ST_Transform(ST_TileEnvelope(12,2170,1189),25833) e25833),
  g AS (SELECT ST_AsMVTGeom(ST_Transform(c.geom,3857),(SELECT e FROM b),4096,64,true) geom,
               c.elev_m, c.kind, c.is_index
        FROM terrain.contour c, b WHERE c.geom && b.e25833)
  SELECT length(ST_AsMVT(g.*,'contour',4096,'geom')) FROM g WHERE geom IS NOT NULL;"
```

### Reference: Oslo (fylke 03) sample, observed counts

| Upsert | Canonical table | Rows (Oslo) |
| --- | --- | --- |
| `n50-vann-upsert` | `terrain.water_polygon` | 434 |
| `n50-hoydekurve-upsert` | `terrain.contour` | 2 268 (2 263 main / 3 aux / 2 depr; 422 index) |
| `n50-isogbre-upsert` | `terrain.glacier_polygon` | 0 (no glaciers in Oslo) |
| `n50-landcover-upsert` | `terrain.landcover_patch` | 2 619 |
| `n50-stedsnavn-upsert` | `anchors.anchor` | 1 148 |
| `n50-vegnett-upsert` | `paths.edge` | 16 996 |

Contour heights are all multiples of 20 → N50's 20 m equidistance; `is_index`
flags the 100 m lines.

Oslo has no glaciers and few mapped building *areas*. To exercise those, pull a
mountainous/coastal county — e.g. Møre og Romsdal (`15`, ~272 MB zip): observed
`terrain.building_polygon` 2 691, `terrain.glacier_polygon` 297 (61.7 km²),
`terrain.contour` 98 474, `terrain.water_polygon` 25 923, `paths.edge`
(vegnett) 63 994.

---

## 3a. Serving the basemap

Once the canonical tables are populated, the multi-layer N50 basemap is served
straight from PostGIS — no build step:

```
GET /v1/basemap                      # TileJSON 3.0.0 descriptor (layers, zooms, fields)
GET /v1/basemap/{z}/{x}/{y}.mvt      # one MVT with all active layers stitched in
GET /v1/basemap/style.json           # the house n50-topo MapLibre style, wired to this server
GET /v1/raster/n50/{z}/{x}/{y}.png   # raster fallback: same data + style, rasterised at origin
```

The raster endpoint (`turbo-tiles-raster`, tiny-skia + embedded DejaVu Sans
for labels) is the M1 drop-in for `flutter_map` — the app's
`TurboN50TopoConfig` provider consumes it, replacing the Kartverket
Norgeskart WMTS without a client renderer change. Native max zoom 16
(N50 scale); clients overzoom beyond. The edge worker's allowlist covers
`/v1/raster/...`, so each tile rasterises once per data version
(~0.5 s/tile debug, much faster in release) and serves from cache after.

Layers, paint order, per-layer zoom ranges, and exposed attributes are declared
in `tools/basemap-layers.toml` (embedded fallback compiled in). Adding a feature
class is a TOML edit. The tile is assembled in one SQL statement — `ST_AsMVT`
per active layer, concatenated — and line/polygon layers can opt into
zoom-scaled `ST_SimplifyPreserveTopology` + a sub-pixel-area drop to keep
low-zoom tiles small.

Verified locally (Oslo): `/v1/basemap/12/2170/1189.mvt` → 32 KB tile carrying
`water` + `contour` + `transportation` + `place`; at z14 the `building` layer
joins (it's gated to `min_zoom = 14`).

### Styling

The house style lives at `styles/n50-topo.json` (MapLibre Style Spec, embedded
in the binary; a disk copy wins for live editing). `{BASE_URL}` placeholders
resolve against `PUBLIC_BASE_URL` at serve time. Consumers:

- **MapLibre GL (web / Flutter)** — point it at `/v1/basemap/style.json`
  directly.
- **turbomap (native)** — `turbomap-style-maplibre` lowers the same document
  onto the renderer's `VectorStyle`; the desktop demo does this when
  `TURBO_BASEMAP_URL` is set (e.g. `TURBO_BASEMAP_URL=http://localhost:8090
  cargo run -p turbomap-app`).

Two test layers guard the contract: the api crate asserts every
`source-layer`/filter property in the style exists in `basemap-layers.toml`,
and `turbomap-style-maplibre`'s tests parse the real style file — growing the
style past the loader's subset fails at `cargo test` time, not on screen.

## 4. Automated tests

`crates/turbo-tiles-ingest/tests/e2e_pipeline.rs` runs the **real** production
code path (restore → every upsert → assertions → reset) against a 5 KB
synthetic dump (`data/fixtures/n50_mini.sql`) that mirrors the real schema
shape. It **skips silently** unless a DB is reachable.

```sh
export INGEST_TEST_DATABASE_URL=postgres://postgres:testpass@localhost:5432/tiles
tileserver migrate     # the test asserts against an already-migrated DB
cargo test -p turbo-tiles-ingest --test e2e_pipeline -- --test-threads=1
```

> **Run single-threaded.** Every test shares one DB and one `n50_staging`
> schema, cleaning it between runs — so they must not run concurrently
> (`--test-threads=1`). In parallel they stomp each other's staging schema.

When you add a new layer, extend the fixture **and** add an assertion — keep
the synthetic dump in lockstep with the upsert SQL (the fixture must contain
every `n50_staging.*` table the upsert SQLs reference, or the upsert errors
with `relation … does not exist`).

---

## 5. Layer status & what to ingest next

### Ingested today

| Layer | N50 source table(s) | Canonical target | Tested |
| --- | --- | --- | --- |
| Water (lakes/rivers/sea) | `innsjo`, `innsjoregulert`, `elv`, `ferskvanntorrfall`, `havflate` | `terrain.water_polygon` | ✅ |
| Contours | `hoydekurve`, `hjelpekurve`, `forsenkningskurve` | `terrain.contour` | ✅ |
| Buildings | `bygning_omrade` | `terrain.building_polygon` | ✅ |
| Coastline | `kystkontur` | `terrain.coastline` | ✅ |
| Glaciers/snow | `snoisbre` | `terrain.glacier_polygon` | ✅ |
| Landcover | `skog`, `myr`, `apentomrade`, `dyrketmark` | `terrain.landcover_patch` | ✅ |
| Place names + spot heights | `stedsnavntekst`, `terrengpunkt` | `anchors.anchor` | ✅ |
| Roads / paths | `veglenke` | `paths.edge` | ✅ |

Non-N50, already wired: FKB sti (`fkb-sti`), Turrutebasen trails
(`turbase`), DNT cabins (`dnt-cabins-load`), DTM10 raster (`dtm-bulk-load`).

### Recommended next datasets (for a full N50 topo basemap)

Prioritised by basemap impact. Each is a small, config/SQL-shaped addition:
new `terrain.*`/canonical table + `upsert_n50_*.sql` + fixture rows + e2e
assertion, then smoke against the Oslo (or a richer) county.

1. **Streams / waterways (lines)** — `elvbekk`. Used by routing already; should
   become a served basemap line layer with width, and be tested.
2. **Railways** — `bane` (+ `jernbanetype`, `stasjon`). Standard topo content.
3. **Power lines** — `ledning` / `luftledninglh`. Shown on topo maps.
4. **Protected areas** — `naturvernomrade` (national parks/reserves). High-value
   outdoor overlay.
5. **Ski & recreation** — `alpinbakke`, `skitrekk`, `lysloype`, `hoppbakke`,
   `sportidrettplass` — feeds the winter style.
6. **Urban areas** — `tettbebyggelse` / `bymessigbebyggelse` for built-up fill
   at low zoom (complements the per-footprint `building` layer).
7. **DTM10 elevation** — bulk-load a small GeoTIFF via `dtm-bulk-load` to
   exercise hillshade + Terrain-RGB and cross-check the vector contours.
   (Separate product from N50 — høydedata.no.)

When picking a real-data sample for these, prefer a **mountainous/coastal
county** (e.g. Vestland `46` or Møre og Romsdal `15`) so glaciers, fjords,
dense contours, and ski features are actually present — Oslo has none of the
first two.

### fkb_type vocabulary (reconciled)

There used to be two `fkb_type` spellings: the N50 vegnett upsert and the
graph encoder (`encode_fkb_type`) used the "vei" forms (`traktorvei`,
`skogsvei`), while the FKB WFS ingest and the resource views used the "veg"
forms (`traktorveg`, `skogsbilveg`). N50 roads therefore never surfaced in the
served `forest-roads`/`hiking`/`cycling` layers, and FKB roads encoded as 0
(unknown) in the routing graph.

Resolved by standardising on the **canonical "vei" vocabulary** —
`{sti, vei, traktorvei, skogsvei, sykkelvei, skiloype}`:
- FKB ingest normalises spellings at load (`fkb_wfs::normalize_fkb_type`).
- The resource views filter on the canonical forms
  (`migrations/20260603000002_resource_view_vocab.sql`).
- The graph encoder keeps the old "veg" forms as defensive aliases.

Verified on real data (Møre og Romsdal): after `n50-vegnett-upsert`,
`v_forest_roads` exposes 7 435 edges and `v_hiking_trails` 15 803 (both were 0
before), with `paths.edge.fkb_type ∈ {sti, traktorvei, vei}`.
