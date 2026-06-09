# Native search + reverse-geocoding — implementation plan

**Date:** 2026-06-09
**Goal:** replace the live third-party search & reverse-geocoding (Kartverket /
Geonorge / Miljødirektoratet, called per-request from the clients) with our own
native subsystem that is fast, low-footprint, extensible, distributed for remote
HTTP search, and works offline on-device with intelligent fallback. Norwegian
hiking domain only.

**Core thesis:** stop proxying, start owning. Today reverse-geocode fans out 5
live HTTP calls and forward-search hits Geonorge live — latency is bound by the
slowest external call, ranking is not ours, and offline is impossible because the
data lives elsewhere. We ingest the authoritative open datasets once, index them,
and serve **one ingestion → two read paths** (a distributed server index and
per-region embedded SQLite bundles). Both derive from the same source, so remote
and offline answers are consistent by construction.

**Two architectural principles (carry through every phase):**
1. **Policy is data, not code.** The tier / distance-band / qualifier rules become
   a versioned *ruleset artifact* consumed by a small interpreter in each of the
   three runtimes (C#, Dart, Kotlin). Behavior changes = bump the ruleset, not
   three codebases.
2. **One golden fixture pins behavior across languages.** The existing 28+
   reverse-geocode invariants + search invariants become a single shared
   `golden.json` (input → expected) run by xUnit, Flutter `test`, and the Android
   suite. This is how three implementations of the spec avoid drift.

Reuse what exists — don't add parallel infrastructure:
- Server: the `Turbo.{Module}.{Contracts,Core,Infrastructure,Api}` layering,
  PostGIS + EF Core, the outbox/CQRS read-model + `OutboxDispatcherHostedService`
  worker pattern, YARP, NATS subscribers.
- Clients: the offline-region download system (`features/tile_storage/
  offline_regions/`, `DownloadOrchestrator`, `TileJobQueue` with its transactional
  claim + circuit breaker), the SQLite stores (`sqflite` / Room), `corridorBounds()`,
  the `connectivityProvider`, and the existing `SearchRepository` /
  `LocationService` / `ReverseGeocoder` interfaces (so consumers don't change).

---

## The canonical model: `Place`

Everything ingested collapses to one entity that both read paths and all three
clients understand:

```
Place {
  id           // namespaced: "ssr:1234", "adr:...", "nve:VV00002858"
  primaryName  // + altNames[] (bokmål / nynorsk / northern sami)
  nameFold     // diacritic/case-folded for æøå-aware FTS
  featureType  // canonical enum, mapped once from navneobjekttype / source type
  geometry     // point or polygon (EPSG:4326)
  kommune, fylke   // PRECOMPUTED at ingest via spatial join
  elevationM   // PRECOMPUTED for peaks/features at ingest (nullable)
  attributes   // protectedAreaKind, postcode/poststed, status, source, ...
  version
}
```

The two precomputes (`kommune`/`fylke` containment, peak `elevationM`) are what
collapse today's 5-call cascade into a single KNN query: the kommune-info and
elevation HTTP calls disappear for named features.

---

## Phase 0 — Ruleset + golden fixtures (do first, behavior-preserving)

The point of Phase 0 is to extract today's behavior into portable, testable
artifacts **before** changing any data source, so every later phase is guarded.

### 0A. Define the ruleset schema + classification artifact *(~medium)*
Encode today's hardcoded logic as data (JSON, schema-versioned):
- feature-type → tier class (exactContact / inSettlement / closeToPeak / periphery)
- per-type tight + loose distance caps (peak ≤800 m "closeTo", settlement ≤4 km
  "near" but ≤1500 m "inSettlement", water ≤100 m "at", building ≤50 m, …)
- qualifier mapping (On / At / In / Near)
- status penalty (+50 for non-`aktiv`)
- the cascade order (tight toponym → protected area → loose toponym → address →
  kommune)
- Naturbase-code rejection pattern, "Ukjent"/"Unknown" rejection, parcel-code
  detection.

Source of truth for the values: `apps/flutter/lib/features/search/data/
stedsnavn_descriptors.dart` (`categorizeFeature`, the kind sets, status penalty),
`kartverket_reverse_geocoder.dart` (`_describeUnbounded` cascade), and Android's
`ReverseGeocodeRepository.kt` (`compose`, `categoryOf`, `qualifierFor`).
- New artifact: `packages/place-ruleset/ruleset.v1.json` (+ a short README on the
  schema). Lives in the repo, embedded in clients at build time and served by the
  API at `GET /api/places/ruleset/{version}`.

### 0B. Author the shared golden fixture *(~medium)*
Translate the 28 reverse-geocode invariants + the forward-search/composite
invariants (catalogued from `kartverket_reverse_geocoder_test.dart`,
`ReverseGeocodeTest.kt`, `stedsnavn_search_backend_test.dart`,
`composite_search_service_test.dart`) into one declarative file:
- `packages/place-ruleset/golden.json`: list of cases `{ candidates[], query,
  rulesetVersion } → { title, qualifier, secondary, kommune, fylke,
  distanceMeters?, elevationMeters? }`.
- Each runtime gets a thin harness that feeds `golden.json` through its interpreter
  and asserts equality. This file is the contract.

### 0C. Interpreter #1 (Dart) + refactor the Flutter reverse-geocoder onto it *(~medium)*
- New `apps/flutter/lib/features/search/data/ruleset/` — a pure
  `PlaceRuleset` loader + `classify(featureType, distanceM)` and `pickWinner(...)`
  that today's `KartverketReverseGeocoder` calls instead of the inline logic in
  `stedsnavn_descriptors.dart`.
- Keep the live Kartverket backends exactly as-is for now; only the *decision*
  logic moves behind the interpreter. Existing tests + the new `golden.json`
  harness must stay green. This proves policy-as-data end-to-end with zero user-
  visible change.

**Exit criteria for Phase 0:** ruleset + golden committed; Flutter still uses live
Kartverket data but routes all ranking through the interpreter; `golden.json`
passes in Dart. Android + C# interpreters can follow in their phases.

---

## Phase 1 — Ingestion pipeline + `Turbo.Geo`-adjacent read model (server)

### 1A. New module skeleton `Turbo.Places` *(~medium)*
Mirror the Geo module layout:
- `apps/api/src/Places/Turbo.Places.Contracts/` — `Place` value objects, domain
  events (`PlaceIngested`, `PlaceRetired`), `PlacesScope : IModuleScope`.
- `Turbo.Places.Core/` — domain model (`Place` aggregate-lite), query handlers
  (`SearchPlacesHandler`, `ReverseGeocodeHandler`, `BuildBundleHandler`), the C#
  ruleset interpreter (interpreter #2) + `golden.json` test.
- `Turbo.Places.Infrastructure/` — `PlacesReadContext` (EF Core + NetTopologySuite),
  GIST index on `geometry`, `pg_trgm` GIN + `tsvector` on `nameFold`, migrations.
- `Turbo.Places.Api/` — `PlacesModule.cs` DI wiring (copy `GeoModule.cs`),
  `PlacesController`, request/response DTOs, `AddPlacesNatsSubscribers`.
- Register in `hosts/Turbo.Host.Modulith/Program.cs` + run migrations via
  `MigrateModuleDatabaseAsync<PlacesReadContext>`.

### 1B. Ingestion worker *(~large — the heart of the system)*
A background ingester following the `OutboxDispatcherHostedService` hosted-service
pattern, run as a CLI/job (not request-path):
- Pull bulk open datasets from Geonorge / Naturbase: SSR (Sentralt
  stedsnavnregister), Matrikkel vegadresser, kommune/fylke boundaries, Naturbase
  protected areas, and a DTM (DTM50 to start).
- Normalize each into canonical `Place`; map source feature types → canonical enum
  (reuse the kind sets from `stedsnavn_descriptors.dart`).
- Spatial-join kommune/fylke; sample DTM at peak/feature points → `elevationM`.
- Upsert into `PlacesReadContext`; emit `PlaceIngested` via the outbox.
- Versioned + incremental: support periodic full dumps and Geonorge change feeds;
  carry a `datasetVersion` so bundles and clients can detect staleness.
- New project: `apps/api/src/Places/Turbo.Places.Ingestion/` (console host).

### 1C. Query primitives + interpreter #2 *(~medium)*
- **Reverse**: `ReverseGeocodeHandler` — `geometry <-> @point` KNN over GIST,
  pull the K nearest candidates, run the C# ruleset interpreter → `LocationDescription`.
- **Forward**: `SearchPlacesHandler` — `pg_trgm` fuzzy + `tsvector` rank, biased by
  distance to `near` (proximity matters: dozens of "Storvatnet"). Behind an
  `IPlaceSearchIndex` port so Typesense/OpenSearch is a later drop-in.
- C# `golden.json` harness green.

---

## Phase 2 — Public contract + switch clients' remote path

### 2A. API surface *(~small)*
Controllers (match the Geo controller style, `[Authorize]` as appropriate):
- `GET /api/places/search?q=&near=lat,lon&limit=` → ranked hits (maps to
  `SearchHit` / `LocationSearchResult`).
- `GET /api/places/reverse?lat=&lon=` → `LocationDescription`-shaped response
  (title, qualifier, secondary, kommune, fylke, distanceMeters, elevationMeters).
- `GET /api/places/ruleset/{version}` → the classification artifact.
- `GET /api/places/bundle?bbox=&rulesetVersion=&since=` → per-region offline bundle
  (Phase 3).

### 2B. Flutter remote backends → `Turbo.Places` *(~medium)*
- New `RemotePlaceSearchBackend` (impl `LocationService`) + `RemoteReverseGeocoder`
  (impl `ReverseGeocoder`) calling our API via the existing `ApiClient`.
- Rewire `stedsnavnSearchBackendProvider` (`composite_search_service.dart:12`) and
  `reverseGeocoderProvider` (`reverse_geocoder.dart:50`). The other composite
  sources (markers/paths/trails/activities) are untouched.
- Retire the live `backends/*_backend.dart` once parity is confirmed by `golden.json`
  + existing tests.

### 2C. Android remote repositories → `Turbo.Places` + interpreter #3 *(~medium)*
- New `RemoteSearchRepository : SearchRepository` and
  `RemoteReverseGeocodeRepository : ReverseGeocodeRepository` using the provided
  Ktor `HttpClient`.
- Flip the two providers in `core/data/.../di/NetworkModule.kt:42-61`. Keep the
  `Synthetic*` debug stand-ins.
- Port the Kotlin ruleset interpreter (#3); Android `golden.json` harness green.

**Exit criteria for Phase 2:** both clients answer search + reverse-geocode from
`Turbo.Places`; no live third-party calls in the request path; responses identical
to today per the golden fixtures.

---

## Phase 3 — Offline embedded index + intelligent fallback

### 3A. Offline bundle format + builder *(~medium)*
- `BuildBundleHandler`: given a bbox (+ margin) + rulesetVersion, emit a compact
  **SQLite bundle**: `places` table + an **R\*Tree** virtual table (reverse-geocode
  nearest-feature) + **FTS5** (trigram tokenizer over `nameFold`, æøå-aware) +
  the embedded ruleset + `datasetVersion`.
- Pre-bake/cache bundles per region; serve via `GET /api/places/bundle`. Footprint
  scales with area (a national park ≈ hundreds of KB); whole-Norway pack is an
  opt-in extra, not the default.

### 3B. Client local index store, over the existing download rails *(~medium)*
- Flutter: a `placeBundleStore` (new `sqflite` DB or attached file) + a
  `LocalPlaceSearchBackend` / `LocalReverseGeocoder` querying R\*Tree + FTS5 via the
  Dart interpreter. Disk-resident / mmap'd → negligible RAM.
- Hook bundle download into the existing offline-region flow: when a user downloads
  a map region (or a route corridor via `corridorBounds()`), enqueue a bundle fetch
  on the **same** `TileJobQueue` (reuse retry + circuit breaker). Track a coverage
  index (which bboxes are present + their `datasetVersion`).
- Android: equivalent Room-backed store + MapLibre-region hook.

### 3C. `TieredPlaceService` — coverage- & connectivity-aware routing *(~medium)*
- One class per client implementing the existing interfaces (zero consumer change):
  - **Reverse-geocode**: local-first when the point is inside a downloaded region
    (instant + offline-resilient in the mountains); else remote; remote
    failure/timeout → local fallback. Generalize the 250 m `GeoQuery` cache into a
    write-through (remote results → local cache).
  - **Forward search**: merge local (downloaded regions, instant) + remote (full
    national breadth, when online) — mirrors today's `CompositeSearchService`
    fan-out of local + remote sources.
- Routing inputs: coverage index, `connectivityProvider`, a latency budget.

**Exit criteria for Phase 3:** in a downloaded region with the radio off, search +
reverse-geocode work from the local bundle and pass `golden.json`; online behavior
unchanged; remote failures degrade to local silently.

---

## Open decisions (defaults chosen; override if desired)

1. **Remote forward-search engine** — default: Postgres `pg_trgm` + `tsvector`
   behind an `IPlaceSearchIndex` port (no extra infra; fine for ~1–2M Norwegian
   features). Swap to Typesense/OpenSearch later if needed.
2. **Offline elevation** — peaks get `elevationM` precomputed at ingest. For
   arbitrary tapped points offline: derive from already-downloaded contour vector
   tiles, or omit and degrade gracefully (`elevationMeters` is already nullable).
   Default: omit-or-derive to protect footprint; remote serves a DTM point lookup.
3. **Offline default scope** — per-downloaded-region (default; minimal footprint)
   vs. an optional whole-Norway pack (~50–150 MB names+addresses) for power users.

---

## Behavioral invariants to preserve (the spec, enforced by `golden.json`)

1. Tier-based winner selection; the `isTight` gate decides whether to consult
   fallback sources (don't fold tier into a flat score).
2. Distance bands are feature-type-specific (peak 800 m "close"; settlement 4 km
   still "near"); no uniform radius.
3. Qualifier semantics: On = at a peak/glacier/island; At = water/building; In =
   inside a bounded area (settlement/park/kommune); Near = nearby but outside;
   null = no spatial relationship.
4. Status penalty (+50) biases against inactive features; don't drop them outright.
5. Elevation + kommune are parallel enrichments that merge onto *any* winner.
6. Address dedup: prefer "Storgården 4" over parcel codes ("155/1/73"); skip bare
   gnr/bnr when kommune is available.
7. Naturbase codes ("VV00002858") never surface as titles.
8. Name extraction tolerates both `/navn` and `/punkt` shapes and rejects
   "Ukjent"/"Unknown"; prefer `hovednavn`.
9. ~250 m reverse-geocode cache grid; resilient (network errors → null/empty, never
   throw); elevation validated to [-1000 m, 9000 m].
10. Forward-search icon mapping (Fjell→mountain, By→city, Elv/Innsjø→water, …,
    default place); UTF-8 / æøå handled.

---

## Sequencing summary

- **Phase 0** (ruleset + golden + Dart interpreter refactor) — behavior-preserving,
  unblocks everything, independently shippable.
- **Phase 1** (ingestion + `Turbo.Places` + PostGIS read model) — the heavy lift;
  ships server-only, no client change yet.
- **Phase 2** (contract + client remote rewiring + C#/Kotlin interpreters) — clients
  now own their data; identical responses.
- **Phase 3** (offline bundles + `TieredPlaceService`) — offline lights up; online
  unchanged.

Each phase is independently shippable and guarded by the shared golden fixtures.
