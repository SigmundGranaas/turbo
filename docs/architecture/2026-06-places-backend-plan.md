# `Turbo.Places` backend service — complete implementation plan

**Date:** 2026-06-09
**Scope:** the server half of the native search + reverse-geocoding subsystem —
a distributed, PostGIS-backed HTTP service that owns the Norwegian open datasets
and answers forward search + reverse geocoding from our own stack, plus produces
the per-region bundles the offline phase consumes. Norwegian hiking domain only.

This supersedes the server sections of `2026-06-native-search-geocoding-plan.md`
with file-, schema-, and query-level detail. The clients, offline bundles, and
`flutter_rust_bridge` wiring remain in that document; the shared `place-core`
crate already exists under `packages/place-core/`.

---

## 0. The crux: this is a data problem, not a code problem

The hard, valuable, risky work is **ingesting the authoritative datasets and
indexing them** — not the ranking (already done in `place-core`) and not the
HTTP plumbing (a copy of the Geo module). Everything below is organised so the
data pipeline is proven first, on a small slice, before any breadth.

`Turbo.Places` is unlike the existing `Turbo.Geo` module in one fundamental way:
**it has no user-owned write aggregate.** Geo persists user markers via
commands → aggregate → outbox. Places is a **read model over reference data**
that nobody edits at runtime; the "writer" is a batch/stream **ingestion job**.
So Places deliberately drops the aggregate / command-handler / outbox machinery
and keeps only the CQRS *read* side + a separate ingester. Don't copy Geo's
write path.

---

## 1. Data sources (all open, all Kartverket/Miljødirektoratet)

| Dataset | Source | Use | Format / CRS | Volume |
|---|---|---|---|---|
| **SSR – Sentralt stedsnavnregister** | Geonorge download (GPKG/GeoJSON) | toponyms (peaks, water, settlements, farms) | GPKG, EPSG:4258/UTM33 25833 | ~1.1M names |
| **Matrikkelen – Vegadresser** | Geonorge download | nearest civic address | GPKG/CSV, 25833 | ~2.6M addresses |
| **Administrative enheter** (kommune + fylke) | Geonorge download | containment + enrichment | GPKG polygons, 25833 | ~357 + 15 polygons |
| **Naturvernområder** | Miljødirektoratet / Geonorge | protected-area containment | GPKG polygons, 25833 | ~7k polygons |
| **DTM (Høydedata)** | hoydedata.no DTM50 | elevation at peaks/features | GeoTIFF / WCS | national raster (heavy) |

**Licensing:** all NLOD / CC-BY (Kartverket, Miljødirektoratet). We must surface
attribution ("Inneholder data under norsk lisens for offentlige data (NLOD)
tilgjengeliggjort av Kartverket") in-app and in bundle metadata. Tracked as a
release task.

**CRS policy:** ingest reprojects everything to **EPSG:4326** for storage and
query (matches the clients' `latlong2`/MapLibre). Geonorge serves UTM33
(EPSG:25833); reprojection happens once, at load.

---

## 2. Schema (`places` Postgres schema, EF Core + NetTopologySuite)

PostGIS + `pg_trgm` extensions (PostGIS already in use by Geo; add the
`pg_trgm` and, for elevation, optional `postgis_raster`).

### 2.1 `places.places` — point + polygon reference features

```sql
CREATE TABLE places.places (
    id              uuid PRIMARY KEY,            -- deterministic: uuidv5(source|source_id)
    source          text NOT NULL,               -- 'ssr' | 'matrikkel' | 'naturbase'
    source_id       text NOT NULL,
    feature_type    text NOT NULL,               -- canonical enum (peak/water/settlement/built/address/protected_area/...)
    primary_name    text NOT NULL,
    name_fold       text NOT NULL,               -- lowercased, diacritics normalised for trgm/fts
    geom            geometry(Geometry,4326) NOT NULL,   -- point for names/addresses, polygon for protected areas
    centroid        geography(Point,4326) NOT NULL,     -- precomputed; metric KNN/DWithin
    kommune_code    text, kommune_name text,     -- precomputed at ingest (spatial join)
    fylke_code      text, fylke_name  text,
    elevation_m     double precision,            -- precomputed for peaks/features
    status          text NOT NULL DEFAULT 'aktiv',
    attributes      jsonb NOT NULL DEFAULT '{}', -- verneform, postnummer/poststed, alt names (bm/nn/se), navnestatus
    dataset_version text NOT NULL,
    updated_at      timestamptz NOT NULL DEFAULT now(),
    ts              tsvector GENERATED ALWAYS AS (to_tsvector('simple', name_fold)) STORED,
    UNIQUE (source, source_id)
);

CREATE INDEX places_geom_gist     ON places.places USING gist (geom);
CREATE INDEX places_centroid_gist ON places.places USING gist (centroid);
CREATE INDEX places_name_trgm     ON places.places USING gin (name_fold gin_trgm_ops);
CREATE INDEX places_ts            ON places.places USING gin (ts);
CREATE INDEX places_kommune       ON places.places (kommune_code);
CREATE INDEX places_type          ON places.places (feature_type);
```

Design notes:
- **One table for point and polygon features.** Protected areas are Places with
  polygon `geom`; reverse-geocode containment is `ST_Contains(geom, :pt)` over
  `feature_type='protected_area'`. Toponyms/addresses are points. This is the
  canonical `Place` model made physical.
- **`centroid geography`** is the precomputed metric anchor: KNN (`<->`) and
  `ST_DWithin(.., metres)` use it so distances come back in metres directly —
  exactly the `distance_m` `place-core` expects.
- **Deterministic `id`** (uuidv5 of `source|source_id`) makes re-ingest
  idempotent (upsert, not duplicate).
- **`kommune_*`, `fylke_*`, `elevation_m` precomputed at ingest** — this is what
  collapses the live 5-call cascade into one query at read time.

### 2.2 `places.admin_areas` — kommune/fylke polygons (closed set)

```sql
CREATE TABLE places.admin_areas (
    code text PRIMARY KEY,        -- kommunenummer / fylkesnummer
    level text NOT NULL,          -- 'kommune' | 'fylke'
    name text NOT NULL,
    parent_code text,             -- kommune -> fylke
    geom geometry(MultiPolygon,4326) NOT NULL
);
CREATE INDEX admin_geom_gist ON places.admin_areas USING gist (geom);
```

Kept separate from `places`: small, used for runtime point-in-polygon when a
tapped coordinate isn't itself a feature, and as the join source for the
`kommune_*`/`fylke_*` precompute.

### 2.3 Ingestion bookkeeping

```sql
CREATE TABLE places.dataset (        -- one row per (source, version)
    source text NOT NULL, version text NOT NULL,
    ingested_at timestamptz, feature_count bigint, status text,  -- staging|active|superseded
    PRIMARY KEY (source, version)
);
```

EF Core: `PlacesReadContext` maps `places.places` (read entity) + `admin_areas`
+ `dataset`, configured with `UseNetTopologySuite()` and
`MigrationsHistoryTable` in the `places` schema — same pattern as
`apps/api/src/Geo/Turbo.Geo.Infrastructure/data/LocationReadContext.cs`.

---

## 3. Ingestion pipeline (`Turbo.Places.Ingestion`)

A standalone console host (run as a CronJob / manual job, **never in the request
path**), orchestrating a load→normalise→enrich→swap pipeline. This is the bulk
of the build.

**Tooling decision:** use **GDAL/`ogr2ogr`** to load Geonorge GPKG into a
`places_staging` schema, then **SQL** to normalise into the canonical model.
Rationale: GDAL is the robust workhorse for Norwegian geodata (handles GPKG,
reprojection, large files) and `ogr2ogr -f PostgreSQL` streams straight into
PostGIS with `-t_srs EPSG:4326`. Pure-.NET GeoJSON streaming with
NetTopologySuite is the fallback for small/incremental feeds. The .NET host
*orchestrates* (download, invoke ogr2ogr, run SQL, manage versions); it doesn't
hand-parse GML.

### Steps (per source)

1. **Acquire** — download the dataset's predefined national file from the
   Geonorge Nedlasting API (Atom feed → GPKG). Record `(source, version)` where
   version = Geonorge's `oppdateringsdato` / file hash.
2. **Stage** — `ogr2ogr -f PostgreSQL "PG:..." input.gpkg -t_srs EPSG:4326
   -nln places_staging.ssr -overwrite` (one staging table per source).
3. **Normalise** (SQL `INSERT … SELECT` into `places.places`):
   - map `navneobjekttype` → canonical `feature_type` (reuse the kind groups
     from `ruleset.v1.json` / `stedsnavn_descriptors.dart`);
   - `primary_name` ← `skrivemåte` (hovednavn); reject `Ukjent`/empty;
   - `name_fold` ← `unaccent(lower(primary_name))` (or a Norwegian-aware fold
     keeping æøå — see Open decisions);
   - `centroid` ← `ST_PointOnSurface`/representation point as geography;
   - alt names + `navnestatus` + (addresses) `postnummer`/`poststed` +
     (parks) `verneform` → `attributes` jsonb;
   - `dataset_version` ← this run's version, `id` ← `uuidv5`.
4. **Spatial-join enrichment** (one pass, set-based):
   - `kommune_*`/`fylke_*` ← `ST_Contains(admin_areas.geom, places.geom)`;
   - `elevation_m` for peak/feature types ← DTM sample (see §3.1).
5. **Swap** — mark the new `dataset_version` active; delete rows of the prior
   version for that source (mark-and-sweep). Done in a transaction so reads
   never see a half-loaded dataset. Admin areas load first (enrichment depends
   on them).
6. **Emit** (optional) a lightweight `PlacesDatasetActivated` notification (NATS)
   so the bundle builder can invalidate caches.

### 3.1 Elevation

`elevation_m` is only needed for **named features** (peaks etc.), computed once
at ingest. Two options, in preference order:
- **DTM50 raster in `postgis_raster`** + `ST_Value(rast, centroid)` — fully
  self-contained, no external dependency at ingest. Heavier to load.
- **Batch the Høydedata point API** during ingest (rate-limited) — simpler, but
  a build-time dependency on Kartverket.

Arbitrary tapped-point elevation (not on a feature) is **out of scope for the DB
read path**; the reverse endpoint returns feature elevation only. A separate DTM
point service (or dropping it) is a later, isolated decision.

### Versioning & refresh

Incremental by re-running the pipeline (SSR/addresses update periodically;
Geonorge offers change feeds for some). Each run is a new `dataset_version`;
mark-and-sweep keeps exactly one active version per source. A monthly CronJob is
the default cadence.

---

## 4. Module layout (mirrors `Turbo.Geo`)

```
apps/api/src/Places/
  Turbo.Places.Contracts/        # Place value objects, feature-type enum, (optional) dataset events
  Turbo.Places.Core/
    query/
      ReverseGeocodeHandler.cs    # gather candidates -> place-core -> LocationDescription
      SearchPlacesHandler.cs      # trgm/tsvector + proximity -> place-core ordering
      BuildBundleHandler.cs       # (Phase 3) bbox slice export
    ranking/
      PlaceCore.cs                # P/Invoke wrapper over libplace_core (C-ABI JSON shim)
      IPlaceSearchIndex.cs        # port: pg_trgm now, Typesense/OpenSearch later
  Turbo.Places.Infrastructure/
    data/PlacesReadContext.cs     # EF Core + NetTopologySuite, places schema
    data/PgPlaceSearchIndex.cs    # IPlaceSearchIndex over Postgres
    data/Migrations/
  Turbo.Places.Api/
    PlacesModule.cs               # DI wiring (copy GeoModule.cs)
    controller/PlacesController.cs
    controller/{request,response}/
  Turbo.Places.Ingestion/         # console host: acquire/stage/normalise/enrich/swap
hosts/Turbo.Host.Modulith/Program.cs   # + builder.Services.AddPlacesModule(config)
hosts/Turbo.Host.Places/               # optional standalone host (YARP route /api/places)
```

`PlacesModule.AddPlacesModule(services, config)` registers `PlacesReadContext`,
the query handlers, `IPlaceSearchIndex → PgPlaceSearchIndex`, `PlaceCore`, and
the controller — the exact shape of
`apps/api/src/Geo/Turbo.Geo.Api/GeoModule.cs`, minus outbox/UoW/idempotency.

---

## 5. Read queries

### 5.1 Reverse geocode (`ReverseGeocodeHandler`)

```sql
-- (a) nearest named point features within 1 km (place-core toponym candidates)
SELECT primary_name, feature_type, status,
       ST_Distance(centroid, @p) AS distance_m
FROM places.places
WHERE feature_type <> 'protected_area'
  AND ST_DWithin(centroid, @p, 1000)
ORDER BY centroid <-> @p
LIMIT 25;

-- (b) smallest containing protected area
SELECT primary_name, attributes->>'verneform' AS kind
FROM places.places
WHERE feature_type = 'protected_area' AND ST_Contains(geom, @pt)
ORDER BY ST_Area(geom) LIMIT 1;

-- (c) nearest address within 200 m
SELECT primary_name AS text, attributes->>'postnummer' || ' ' || (attributes->>'poststed') AS secondary
FROM places.places
WHERE feature_type = 'address' AND ST_DWithin(centroid, @p, 200)
ORDER BY centroid <-> @p LIMIT 1;

-- (d) containing kommune + fylke
SELECT level, code, name FROM places.admin_areas WHERE ST_Contains(geom, @pt);
```

The handler assembles these into a `place-core` `ReverseInput` (toponyms,
protected_area, address, kommune) and calls `PlaceCore.ReverseGeocode(...)`,
which returns the `LocationDescription`. Elevation comes from the winning
feature's precomputed `elevation_m` (joined when the toponym wins). All four
queries run concurrently (`Task.WhenAll`).

### 5.2 Forward search (`SearchPlacesHandler` via `IPlaceSearchIndex`)

```sql
SELECT primary_name, feature_type,
       ST_Distance(centroid, @near) AS distance_m,
       similarity(name_fold, @q) AS sim
FROM places.places
WHERE name_fold % @q                              -- pg_trgm prefilter
   OR ts @@ websearch_to_tsquery('simple', @q)
ORDER BY (name_fold = @q) DESC, sim DESC, distance_m ASC NULLS LAST
LIMIT 30;
```

SQL retrieves the top-N relevant candidates; `place-core.forward_search` applies
the final, canonical ordering (match-class + proximity) and the icon mapping, so
the server and the offline path rank identically. `@near` is the request's map
centre.

**Engine decision:** Postgres `pg_trgm`+`tsvector` behind `IPlaceSearchIndex`
(no extra infra; fine for ~3–4M Norwegian rows). The port lets us swap in
Typesense/OpenSearch later without touching the handler or controller.

---

## 6. `place-core` on the server (firming the open decision)

The server **calls `place-core`** (no C# reimplementation — zero ranking drift,
and the same core builds offline bundles). Integration is a **thin C-ABI JSON
shim** rather than UniFFI (UniFFI targets foreign languages, not idiomatic
.NET P/Invoke):

- Add a `cabi` feature to `place-core` exposing
  `place_core_reverse(input_json: *const c_char) -> *mut c_char` and
  `place_core_search(query, candidates_json) -> *mut c_char` plus a `free`,
  operating on JSON strings (the server already speaks JSON everywhere, so this
  sidesteps all struct marshalling).
- Build `libplace_core.so` (linux-x64) in CI; ship it in the API container;
  `Turbo.Places.Core/ranking/PlaceCore.cs` `DllImport`s it.
- Parity: the same `golden.json` runs in a C# xUnit test through `PlaceCore` —
  the .NET equivalent of the Python/Kotlin synthetic clients.

(This adds a tiny `cabi` surface to the crate, complementary to the existing
`uniffi` feature. The pure default build stays unchanged.)

---

## 7. HTTP API (`PlacesController`)

Controllers, matching Geo's style. **No per-user `[Authorize]`** — this is
reference data; protect at the gateway (app token / rate limit), not per-user.

| Method | Route | Returns |
|---|---|---|
| `GET` | `/api/places/reverse?lat=&lon=` | `LocationDescriptionResponse` |
| `GET` | `/api/places/search?q=&lat=&lon=&limit=` | `SearchResponse` (ranked hits) |
| `GET` | `/api/places/ruleset/{version}` | the `place-core` ruleset JSON |
| `GET` | `/api/places/bundle?bbox=&rulesetVersion=&since=` | offline bundle (Phase 3) |
| `GET` | `/api/places/health` | dataset versions + counts |

- **Response shapes** map 1:1 to the clients' `LocationDescription` /
  `SearchHit` (title, qualifier, secondary, kommune, fylke, distanceMeters,
  elevationMeters / index, title, description, icon).
- **Caching:** `ETag` = active `dataset_version`; clients revalidate. Reverse
  results are deterministic per dataset → cacheable at the CDN/gateway.
- **Validation:** reject out-of-Norway bbox/coords early (cheap envelope check).

---

## 8. Testing

- **Ingestion** — unit tests over a tiny GeoJSON fixture (a handful of SSR rows
  around Galdhøpiggen + one park polygon + one kommune polygon): assert
  normalisation, the kommune/elevation enrichment join, and version swap.
- **Read handlers** — `Testcontainers.PostgreSql` (PostGIS image), seed the
  fixture, assert `/reverse` returns "On Galdhøpiggen" and `/search` ranks a
  known query — the same invariants `golden.json` encodes.
- **place-core parity** — C# xUnit runs `golden.json` through `PlaceCore`
  (P/Invoke), alongside the existing Rust/Python/Kotlin runs.
- **Endpoints** — `WebApplicationFactory` (Microsoft.AspNetCore.Mvc.Testing,
  already a dependency) for status codes, ETag, validation.
- **Boundaries** — a `NetArchTest` rule that `Turbo.Places.*` doesn't reference
  Geo internals (the repo already uses NetArchTest).

---

## 9. Deployment & ops

- **Modulith:** `AddPlacesModule` in `Turbo.Host.Modulith`; runs migrations via
  the existing `MigrateModuleDatabaseAsync<PlacesReadContext>` startup hook.
- **Standalone / distributed:** optional `Turbo.Host.Places` behind the YARP
  gateway (`/api/places/**`); horizontally scalable because it's **stateless
  over a read-only dataset** — "distributed and powerful" = N replicas + a
  read-replica Postgres, not a search cluster.
- **Native lib:** `libplace_core.so` baked into the image; CI cross-compiles it.
- **Ingestion:** separate CronJob; needs GDAL in its image and write access to
  the Places DB. Storage estimate: places + indexes ≈ a few GB for national
  SSR+addresses; DTM50 raster is the heavy item if loaded into Postgres.
- **Observability:** reuse the existing OpenTelemetry EF Core instrumentation;
  add query timing + candidate-count metrics on the reverse path.

---

## 10. Milestones (data-first, vertical slices)

| # | Deliverable | Proves |
|---|---|---|
| **M0** | Module skeleton + schema + migration + empty endpoints (compiles, deploys, `/health`) | wiring |
| **M1** | Ingest **one kommune's** SSR slice; `/reverse` returns a real result through `place-core`, no Kartverket | **the pipeline + the whole architecture, end to end** |
| **M2** | Admin + protected-area containment + elevation enrichment; reverse parity with the current app on that region | correctness |
| **M3** | Forward search (trgm + tsvector + proximity via `IPlaceSearchIndex`) | search |
| **M4** | National ingestion (SSR + addresses + admin + parks); index/perf tuning; dataset versioning + refresh CronJob | scale |
| **M5** | `/bundle` spatial-slice export | unblocks the offline phase |
| **M6** | Gateway auth + caching/ETag + rate limit + NetArchTest + load test | production |

**M1 is the first thing worth shipping** — it replaces third-party reverse
geocoding for a real area from our own stack and de-risks the data pipeline,
which is the actual unknown.

---

## 11. Open decisions (defaults chosen)

1. **Name folding** — default `unaccent`-style fold but **preserve æ/ø/å**
   (Norwegian users type them; conflating ø→o hurts precision). Revisit if
   trigram recall suffers.
2. **Elevation** — default DTM50 in `postgis_raster` (self-contained); fall back
   to the Høydedata batch API if raster load is too heavy for the ingest box.
3. **Addresses in scope for M1–M3?** — default **no** (SSR + admin + parks
   first; addresses at M4). Addresses are the largest dataset and least
   important for hiking.
4. **Search engine** — Postgres trgm/tsvector behind `IPlaceSearchIndex`;
   Typesense/OpenSearch only if national-scale relevance/latency demands it.
5. **Auth** — gateway-level app token + rate limit, not per-user `[Authorize]`.

---

## 12. Risks

- **Ingestion is the real unknown** — SSR format/projection quirks, file sizes,
  ogr2ogr edge cases. M1 on one kommune surfaces these cheaply before national
  scale.
- **DTM elevation** — raster volume vs. API rate limits; isolated to enrichment,
  degradable (elevation is nullable).
- **trgm relevance tuning** — proximity bias + match class should cover hiking
  ("the Storvatnet near me"), but national name collisions need real-data
  testing at M4.
- **FFI deployment** — the `.so` must match the container arch; CI builds it,
  the C# parity test guards behaviour.
- **Dataset licensing/attribution** — NLOD attribution must ship in-app and in
  bundles (release checklist item).
