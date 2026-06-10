# Turbo.Places — native search + reverse-geocoding

A standalone, PostGIS-backed service that owns the Norwegian open datasets and
answers **forward search** + **reverse geocoding** from our own stack, plus
builds **offline SQLite bundles** that the shared `place-core` engine reads on
device. Ranking is the same Rust core everywhere, so offline == online.

Design + plan: `docs/architecture/2026-06-places-backend-plan.md` and
`docs/architecture/2026-06-places-foundation-plan.md`.

## Run it locally (one command, no network)

Needs **cargo**, **dotnet 10**, **docker** (Linux or macOS):

```sh
./apps/api/src/Places/places-dev.sh
```

It builds the `place-core` native lib, starts PostGIS, seeds the committed
sample data (Jotunheimen + Tromsø, offline), and serves the API on
`http://localhost:5179`. Then, from another terminal:

```sh
# reverse geocode a coordinate
curl 'http://localhost:5179/api/places/reverse?lat=61.6363&lon=8.3120'
#   -> {"title":"Galdhøpiggen","qualifier":"on","kommune":"Lom","fylke":"Innlandet",
#       "distanceMeters":29.8,"elevationMeters":2468.25}

# proximity-biased search
curl 'http://localhost:5179/api/places/search?q=galdh&lat=61.6363&lon=8.3120&limit=3'

# dataset freshness
curl 'http://localhost:5179/api/places/health'      # {"places":416,"areas":2,"datasetVersion":"samples"}

# the classification ruleset the core runs
curl 'http://localhost:5179/api/places/ruleset/1'

# an offline region bundle (SQLite: R*Tree + polygon containment + ruleset)
curl 'http://localhost:5179/api/places/bundle?bbox=8.0,61.4,8.6,61.8' -o region.sqlite
```

Reverse/search carry an `ETag` = dataset version and honour `If-None-Match`
(304). Out-of-Norway coords/bbox → 400.

## Try the offline engine on the bundle you just downloaded

```sh
cargo run --features embedded --example bundle_cli \
  --manifest-path packages/place-core/Cargo.toml -- region.sqlite reverse 61.6363 8.3120
#   -> On Galdhøpiggen · 2468 m · Lom, Innlandet
```

(`bundle_cli demo` builds its own tiny bundle and needs no server at all.)

## Seed with more / real data

- **Sample regions, live from Kartverket** (needs internet) — richer real data
  across five terrain types, with protected-area + kommune polygons:
  ```sh
  PLACES_DB="Host=localhost;Port=55432;Database=places;Username=postgres;Password=places" \
  PLACE_CORE_LIB="packages/place-core/target/debug" \
    dotnet run --project apps/api/src/Places/Turbo.Places.Ingestion -- all
  ```
- **The real national pipeline, one fylke** (Geonorge order → download →
  reproject → stage → atomic swap) — proves the bulk path on authentic data:
  ```sh
  … dotnet run --project apps/api/src/Places/Turbo.Places.Ingestion -- bulk-admin 03 Oslo
  ```

## How it's wired

- **`place-core`** (Rust, `packages/place-core`) — the shared ranking core,
  built as a cdylib with `--features cabi,embedded`. The .NET host P/Invokes it;
  the loader finds `libplace_core.{so,dylib,dll}` via the `PLACE_CORE_LIB` dir
  (the dev script + tests set it; the container images stage it at
  `/app/native`). The host fails fast at startup if the lib is missing.
- **`Turbo.Places.{Contracts,Core,Infrastructure,Api,Ingestion}`** — read-only
  CQRS module (no aggregate/outbox; the ingester is the writer), wired into both
  the standalone `Turbo.Host.Places` and the modulith.
- **Tests:** `dotnet test apps/api/tests/Turbo.Places.Behaviour` (Testcontainers
  PostGIS; builds nothing network-bound). Build the lib first:
  `cargo build --features cabi,embedded` in `packages/place-core`.

## Endpoints

| Method | Route | Notes |
|---|---|---|
| GET | `/api/places/reverse?lat=&lon=` | `LocationDescription`-shaped; ETag/304 |
| GET | `/api/places/search?q=&lat=&lon=&limit=` | ranked hits with positions |
| GET | `/api/places/ruleset/{version}` | the classification ruleset |
| GET | `/api/places/bundle?bbox=&since=` | offline SQLite region bundle |
| GET | `/api/places/health` | counts + active dataset version |
