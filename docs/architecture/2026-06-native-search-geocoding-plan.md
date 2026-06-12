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
1. **Shared decision core, native data access.** The ranking/classification logic
   (cascade, tier selection, distance bands, qualifier, dedup, status penalty,
   Naturbase filtering) lives in **one place** — a pure, I/O-free **Rust core** —
   bound into every runtime via FFI: `flutter_rust_bridge` (Dart), **UniFFI**
   (Kotlin + Swift), **P/Invoke** (.NET). Each platform keeps its *own* data
   access (PostGIS KNN on the server, SQLite R\*Tree/FTS on device) and funnels
   candidates into the same `rank()`. We share the *decision*, not the plumbing.
   Tunables stay a versioned *ruleset struct* passed into the core — shared
   algorithm + shared-as-data tuning, bumpable without recompiling the native lib.
2. **One golden fixture, much smaller drift surface.** The existing 28+
   reverse-geocode + search invariants become a single `golden.json` (input →
   expected) whose primary job is validating the **one core** (`cargo test`),
   plus thin per-binding smoke tests (and server parity if the server keeps a C#
   ranker — see Open decisions). Not three full interpreters to keep in lockstep.

**Why a Rust core (vs. the alternatives):** KMP can't target .NET *or* Dart, so it
can't span our three runtimes; codegen-from-spec means maintaining a brittle
generator for branchy logic. Rust is fast and low-footprint (serves the NFRs
directly), has the best FFI bridges, and — the real win — lets us share the **whole
embedded query engine** (rusqlite R\*Tree + FTS + rank) in Phase 3, not just the
~300 lines of ranking. That engine is what genuinely hurts to maintain across 2–3
native clients. Costs are real and tracked in **Risks & costs** below.

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

## Phase 0 — Shared core + ruleset + golden fixtures (do first, behavior-preserving)

The point of Phase 0 is to extract today's behavior into a single shared core and a
golden fixture **before** changing any data source, so every later phase is guarded.

### 0A. The `place-core` Rust crate + ruleset struct *(~medium)*
One pure, I/O-free crate at `packages/place-core/` exposing:
- `classify(featureType, distanceM, &ruleset) -> Option<(Tier, Qualifier)>`
- `rank(candidates: &[Candidate], query, &ruleset) -> Option<LocationDescription>`
  — the full cascade (tight toponym → protected area → loose toponym → address →
  kommune), dedup, status penalty, Naturbase/parcel/"Ukjent" rejection.
- `forward_rank(...)` for search ordering (proximity + match score).

The crate is binding-agnostic; FFI surfaces are generated per platform
(`flutter_rust_bridge`, UniFFI, P/Invoke) in later phases. `cargo test` runs the
golden fixture directly — this crate is the single source of truth for behavior.

The **ruleset** stays *data* passed into the core (schema-versioned, so tuning ships
without recompiling the lib):
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
- New artifact: `packages/place-core/ruleset.v1.json` (+ README on the schema).
  Lives in the repo, embedded in clients at build time and served by the API at
  `GET /api/places/ruleset/{version}`.

### 0B. Author the golden fixture *(~medium)*
Translate the 28 reverse-geocode invariants + the forward-search/composite
invariants (catalogued from `kartverket_reverse_geocoder_test.dart`,
`ReverseGeocodeTest.kt`, `stedsnavn_search_backend_test.dart`,
`composite_search_service_test.dart`) into one declarative file:
- `packages/place-core/golden.json`: cases `{ candidates[], query, rulesetVersion }
  → { title, qualifier, secondary, kommune, fylke, distanceMeters?,
  elevationMeters? }`.
- `cargo test` runs it against `place-core` directly. Each binding later adds a
  thin smoke test that the FFI surface returns identical results for a sampled
  subset. This file is the contract.

### 0C. Dart binding + refactor the Flutter reverse-geocoder onto the core *(~medium)*
- Generate the Dart FFI for `place-core` via `flutter_rust_bridge`; build the
  Android/iOS/desktop artifacts and wire them into `apps/flutter`.
- Point `KartverketReverseGeocoder` at `place_core.rank(...)` instead of the inline
  logic in `stedsnavn_descriptors.dart` (which becomes a thin candidate-mapping
  shim: parse Kartverket JSON → `Candidate`, then call the core).
- Keep the live Kartverket backends exactly as-is for now; only the *decision* moves
  into the core. Existing tests + the binding smoke test stay green. This proves the
  shared-core + FFI path end-to-end with zero user-visible change.

**Exit criteria for Phase 0:** `place-core` + ruleset + golden committed and green
under `cargo test`; Flutter still uses live Kartverket data but routes all ranking
through the core via FFI. Server (P/Invoke) and Android (UniFFI) bindings follow in
their phases — no new hand-written interpreters.

---

## Phase 1 — Ingestion pipeline + `Turbo.Geo`-adjacent read model (server)

### 1A. New module skeleton `Turbo.Places` *(~medium)*
Mirror the Geo module layout:
- `apps/api/src/Places/Turbo.Places.Contracts/` — `Place` value objects, domain
  events (`PlaceIngested`, `PlaceRetired`), `PlacesScope : IModuleScope`.
- `Turbo.Places.Core/` — domain model (`Place` aggregate-lite), query handlers
  (`SearchPlacesHandler`, `ReverseGeocodeHandler`, `BuildBundleHandler`), and a
  thin P/Invoke wrapper over `place-core` (`rank`/`classify`) + a `golden.json`
  parity smoke test. (See Open decisions for the C#-native-ranker alternative.)
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

### 1C. Query primitives + core binding (P/Invoke) *(~medium)*
- **Reverse**: `ReverseGeocodeHandler` — `geometry <-> @point` KNN over GIST,
  pull the K nearest candidates, hand them to `place-core.rank(...)` via P/Invoke
  → `LocationDescription`. Ship the `.so` in the server container; native data
  access (Postgres) stays in C#, the decision is the shared core.
- **Forward**: `SearchPlacesHandler` — `pg_trgm` fuzzy + `tsvector` rank, biased by
  distance to `near` (proximity matters: dozens of "Storvatnet"). Behind an
  `IPlaceSearchIndex` port so Typesense/OpenSearch is a later drop-in.
- `golden.json` parity smoke test green through the P/Invoke surface.

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

### 2C. Android remote repositories → `Turbo.Places` + UniFFI binding *(~medium)*
- New `RemoteSearchRepository : SearchRepository` and
  `RemoteReverseGeocodeRepository : ReverseGeocodeRepository` using the provided
  Ktor `HttpClient`.
- Flip the two providers in `core/data/.../di/NetworkModule.kt:42-61`. Keep the
  `Synthetic*` debug stand-ins.
- Generate the Kotlin binding for `place-core` via **UniFFI** (+ JNI libs for the
  NDK ABIs); reuse it for the offline path in Phase 3. Binding smoke test green —
  no hand-written Kotlin ranker. (A Swift/iOS binding comes from the same UniFFI
  definition if/when the iOS client needs it.)

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

### 3B. Extend `place-core` to own the embedded query engine *(~medium)* — the payoff
This is where shared code earns its keep: rather than reimplement the offline reader
in Dart *and* Kotlin (*and* Swift), `place-core` gains an optional `embedded`
feature using **`rusqlite`** that, given a bundle path + query, runs the R\*Tree
nearest / FTS5 lookup and feeds candidates straight into the same `rank()`:
- `reverse(bundle, lat, lon) -> LocationDescription?` and
  `search(bundle, q, near) -> [Hit]` — one implementation, exposed through the
  existing FFI bindings to every client.
- **SQLite linking:** build `rusqlite` against the *system/bundled* SQLite the
  clients already ship (`sqlite3` on Flutter, Android system SQLite) — do **not**
  vendor a second copy. (Tracked in Risks.)
- Clients become thin: download the bundle (below), hand its path to the core.

### 3C. Client local store + download wiring, over the existing rails *(~medium)*
- Hook bundle download into the existing offline-region flow: when a user downloads
  a map region (or a route corridor via `corridorBounds()`), enqueue a bundle fetch
  on the **same** `TileJobQueue` (reuse retry + circuit breaker). Track a coverage
  index (which bboxes are present + their `datasetVersion`).
- Flutter stores the bundle file + coverage index in `sqflite`; Android via Room +
  the MapLibre-region hook. Both call `place-core`'s `embedded` API for the lookup —
  no per-language query/rank code. Disk-resident / mmap'd → negligible RAM.

### 3D. `TieredPlaceService` — coverage- & connectivity-aware routing *(~medium)*
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

1. **Server: bind `place-core` or keep a C# ranker?** — default: **P/Invoke the
   core** on the server too (one implementation everywhere, zero drift; a Linux
   `.so` in the container is straightforward). Alternative: keep ranking in C#
   for a pure-managed deployment, accepting exactly one reimplementation guarded by
   `golden.json`. Lean: bind the core.
2. **Remote forward-search engine** — default: Postgres `pg_trgm` + `tsvector`
   behind an `IPlaceSearchIndex` port (no extra infra; fine for ~1–2M Norwegian
   features). Swap to Typesense/OpenSearch later if needed.
3. **Offline elevation** — peaks get `elevationM` precomputed at ingest. For
   arbitrary tapped points offline: derive from already-downloaded contour vector
   tiles, or omit and degrade gracefully (`elevationMeters` is already nullable).
   Default: omit-or-derive to protect footprint; remote serves a DTM point lookup.
4. **Offline default scope** — per-downloaded-region (default; minimal footprint)
   vs. an optional whole-Norway pack (~50–150 MB names+addresses) for power users.

---

## Risks & costs of the shared Rust core (eyes open)

- **Toolchain + CI:** cross-compile `place-core` for Android NDK ABIs
  (arm64/armv7/x86_64), iOS, desktop, and the Linux server `.so`. New CI matrix and
  a Rust toolchain in a C#/Dart/Kotlin shop.
- **FFI boundary:** struct/error/async marshalling — mitigated by
  `flutter_rust_bridge` + UniFFI codegen, but the generated layer must be owned.
- **SQLite double-linking (Phase 3B):** `rusqlite` must link the system/bundled
  SQLite the clients already ship, not vendor its own, or you get two SQLite copies
  and subtle breakage. Validate per platform early.
- **Binary size:** the ranking core is tiny; with the `embedded` (rusqlite) feature
  expect a few hundred KB–1 MB per ABI. Acceptable, but measure.
- **Skillset/onboarding:** introduces Rust as a maintained language. Keep the crate
  small, pure, and well-tested (`golden.json`) so the FFI surface stays narrow.
- **Mitigation:** Phase 0 scopes the core to *pure ranking only* (no SQLite, no
  cross-compile beyond the Flutter dev targets) — the toolchain cost is proven on a
  small surface before the embedded engine lands in Phase 3.

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

- **Phase 0** (`place-core` Rust crate + ruleset + golden + Dart/FFI refactor) —
  behavior-preserving, proves the shared-core + toolchain on a small surface,
  unblocks everything.
- **Phase 1** (ingestion + `Turbo.Places` + PostGIS read model; server P/Invokes
  the core) — the heavy lift; ships server-only, no client change yet.
- **Phase 2** (contract + client remote rewiring + Android UniFFI binding) — clients
  now own their data; identical responses.
- **Phase 3** (offline bundles + `place-core` embedded engine + `TieredPlaceService`)
  — the shared core now owns the on-device query path too; offline lights up;
  online unchanged.

Each phase is independently shippable; behavior is guarded by one `golden.json`
against one core, not three hand-written interpreters.
