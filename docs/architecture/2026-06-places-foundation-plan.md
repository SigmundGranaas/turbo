# Places foundation plan — national API + embedded engine, hardest problems first

**Date:** 2026-06-09
**Scope:** take the verified `Turbo.Places` slice to a **standalone, national,
production-correct search + reverse-geocoding API**, and build the **embedded
(on-device) engine** in `place-core`. **Client integration is explicitly out of
scope** — the embedded engine is verified through its own harness, not an app.
**Bar:** "works perfectly" — defined as concrete acceptance gates (§6), not
vibes.

Supersedes the M4–M6 rows of `2026-06-places-backend-plan.md`. Current state
(verified): sample-scale ingestion via REST, full cascade + containment +
search over PostGIS, `place-core` shared ranking in 4 runtimes, HTTP API with
ETag, 10/10 deterministic behaviour tests, CI + images + gateway routes.

**Ordering principle:** the two genuinely hard, foundational problems are
**(1) national data at full fidelity** (bulk formats, reprojection, scale,
versioned swap, resume) and **(2) provable correctness** (differential parity
against the live composition, server ≡ embedded equality). Everything else is
finishing work. We do the hard parts first and gate each phase on measurable
acceptance criteria.

## Methodology: TDD, strictly

Every unit of work below starts with a **failing test committed first**, then
the implementation that turns it green, then refactor. Concretely:

- **No production code without a failing test demanding it.** Each P-item
  lists its *Test first* artifact — that artifact lands (red) before the code.
- **Fixtures are specs.** Committed fixtures (mini-GPKG, control points,
  relevance queries, hand-built bundle) are written before the code that
  consumes them; they define the contract the way `golden.json` already does
  for ranking.
- **The right harness per layer:** `cargo test` for `place-core`;
  xunit + Testcontainers behaviour tests at the module seam (the established
  `Turbo.Places.Behaviour` pattern); the parity/relevance harnesses for
  properties only real national data can falsify (run per dataset version,
  not in CI).
- **Acceptance gates (§6) are executable** — each is a test, a harness run, or
  an asserted budget, never a judgement call.
- Bug fixing follows the same rule: reproduce with a failing test first (the
  P0 defects below each name theirs).

---

## Status — 2026-06-09 (TDD, all on `claude/custom-search-geocoding-lzb9vo`)

Done and tested (Places behaviour suite 41/41; `place-core` cargo
test/clippy/fmt green across `cabi`,`embedded`):

- **P0** — all four defects (ETag from `places.dataset`; cabi engine handle +
  `OnceLock`; startup native probe; `/ruleset/{version}`).
- **P1a** — UTM33→WGS84 (Krüger series, <0.5 m vs 4 Kartverket control points
  incl. 30°E); GDAL-free GeoPackage reader (GPB→WKB→NTS); `Normalization`
  (shared name rules); `GpkgPlaceIngestor` (GPKG→stage end-to-end).
- **P1b** — `places_staging` + atomic `SwapAsync` (DELETE+INSERT under MVCC):
  sweep, resume, and the **swap-under-load** durability gate (8 readers, zero
  failures, complete-version reads, ETag flips once).
- **P1 data front-door** — `GeonorgeClient` built against the **real** download
  API (capabilities → codelists → `POST /order` → file URLs), unit-tested on a
  captured order response.
- **P3** — the embedded engine: `Bundle` (rusqlite R*Tree + polygon
  containment, reusing `rank()`/`forward_search()`); the `/api/places/bundle`
  builder; the **P3d server≡embedded equality gate** (a server-built bundle,
  opened via FFI, reverse-geocodes byte-identically to `/reverse` across all
  cascade paths); the standalone `bundle_cli` harness (P3e); icon-map fix.

Remaining (need national data / large downloads — the M4 dry-run):

- **P1 national run** — order+download the real GPKG/GeoJSON, per-format
  readers (GeoJSON polygons for admin/Naturbase; GPKG points for SSR),
  set-based enrichment, Matrikkel addresses + the dead address cascade step.
- **P2** — the 2,000-point differential-parity harness vs. the live
  composition; planner-assertion + k6 perf gates at national volume; relevance
  fixture.
- **P3 polish** — bundle FTS5 (LIKE today; fine at region scale); footprint
  budgets (G6) at real scale.
- **P4** — gateway rate limit, attribution surfacing, ingestion runbook.

The hard, novel risks (geodesy, GDAL-free parsing, atomic-swap-under-load,
shared-core across 4 runtimes, server≡embedded equality) are retired with
tests. What's left is breadth (national data) + polish.

---

## Status — 2026-06-10: SSR national run (P1 national + P2b/P3d at scale)

The SSR (toponym) bulk path is built and proven on the **real national
dataset**, end to end against the live Geonorge API.

- **SSR GML reader** (`GmlPlaceReader`) — SSR ships GML-only. A GDAL-free,
  forward-only `XmlReader` lifts one `app:Sted` at a time (memory flat on the
  ~3 GB national file). Resolves `srsName` from the nearest ancestor (the
  download GML carries it on the `gml:MultiPoint`/`Surface`, not the inner
  `gml:Point` — caught as a real reprojection bug on live data), branching
  25833→`Utm33` vs 4258/4326 pass-through; handles Point/MultiPoint/
  LineString/Surface. Tested against a real WFS extract + the download
  MultiPoint shape, plus a PostGIS end-to-end stage→swap test.
- **National run** — `bulk-ssr landsdekkende 0000` ordered + streamed the
  national GML: **1,058,735 places staged in 272 s (~3,900 rows/s)**, zero
  un-reprojected coordinates. `BulkPlaceIngestor` streams in bounded batches;
  `swap`/`bundle` resume modes added for the runbook.
- **P1b finding (fixed)** — the atomic swap of 1.06 M rows takes **34 s**, past
  Npgsql's 30 s default → it timed out. `SwapAsync` now sets a 600 s bound
  (national swaps legitimately run minutes; not unlimited, so a stuck swap
  still fails).
- **P2b perf at national volume (1.06 M rows)** — reverse (KNN, the hot path):
  **0.2 ms**, clean `places_centroid_gist` index scan (31 buffers, no seq scan).
  Search was the weak spot: the old single query OR-ed trigram `%` with prefix
  `LIKE`, and a common stem (`storvatn`) matched ~41 k index hits → 3 681 rows
  with `similarity()` computed on all of them → **~170–230 ms, CPU-bound even
  warm**. **Fixed** (commit) with prefix-first retrieval: since place-core ranks
  exact > prefix > substring, once there are `limit` prefix matches no fuzzy
  match can surface — so prefix runs alone on a new `places_name_prefix` btree
  (`text_pattern_ops`) range scan, and trigram fuzzy runs only to fill remaining
  slots, at a raised `pg_trgm.similarity_threshold` (0.45, `SET LOCAL`-scoped)
  so the index stays selective. Result: `storvatn` **232 ms → ~1.3 ms warm /
  10.6 ms cold**; typo `galdhopiggen`→Galdhøpiggen via fuzzy **8.6 ms** (a short
  generic stem with < `limit` prefix hits is the worst fuzzy case, ~90 ms).
  Distribution sane (adressenavn 112 k, bruk 97 k, tjern 58 k…).
- **P3d parity at scale** — sliced a Jotunheimen region bundle (5 847 places)
  from the national DB; embedded reverse/search match the server on real
  coords (Galdhøpiggen→"On Galdhøpiggen · Lom"; Besseggen-pt→"Close to
  Veslfjellet · Vågå") — same rows, same `rank()`, same answer.

Still open: SSR carries `kommunenavn`/`fylkesnavn` inline (no enrichment join
needed for toponyms), but **admin/Naturbase polygons + Matrikkel addresses**
remain to be loaded alongside for the full cascade at national scale; the
2,000-point differential-parity harness (P2a) and footprint budgets (G6) are
the remaining "works perfectly" gates.

---

## Status — 2026-06-11: k8s deployment + pre-prod security hardening (P4)

Made Places actually deployable in the cluster (it runs **inside the modulith**
— the prod pattern — already wired in code) and closed the security gaps a
reference-data service faces before taking traffic.

Security/robustness (tested):
- **FFI panic guard** — the place-core C ABI wraps its compute paths
  (reverse/search + bundle reverse/search) in `catch_unwind`, so a panic in the
  pure core can't unwind across `extern "C"` and abort the host; it degrades to
  `null`/`[]`. Unit-tested.
- **`/bundle` DoS cap** — the bbox is capped to a region (≤ 2.5° lat × 6° lng)
  and the file is streamed with `DeleteOnClose` instead of `ReadAllBytes`; a
  whole-country bbox would otherwise build ~1 M rows into SQLite and load it
  fully into RAM per request. Behaviour-tested.
- **Rate limit owned by the module** — `AddPlacesModule` registers a per-client
  fixed-window policy keyed on the real client IP from `X-Forwarded-For` (behind
  ingress `RemoteIpAddress` is the proxy → one shared bucket). It travels into
  the modulith (no gateway there) via `[EnableRateLimiting]` + `UseRateLimiter`;
  only Places endpoints are affected. The standalone host gains `/readyz`
  (DB + active dataset) distinct from `/healthz`.
- Already-good: parameterized SQL throughout; XXE-safe XML defaults (and only in
  the offline ingester); Norway-envelope input validation; anonymous-by-design.

Deployment — Places runs as its **own service** `turbo-places` (not in the
modulith), matching repo conventions (chiseled non-root, CNPG DBs, sealed
`db-secrets`, ghcr images):
- Pulled **out of the modulith** (csproj/Program/Dockerfile/`modulith.yaml` all
  cleaned of Places + its native lib) so the two don't co-deploy.
- `places.yaml` — `turbo-places` Deployment + Service (the `Turbo.Host.Places`
  image, which bundles the place-core cdylib), `/healthz` liveness + `/readyz`
  readiness (DB reachable, not "has data", so a fresh deploy rolls out before
  its first ingest), PDB.
- `ingress.yaml` — a more-specific `/api/places` rule routes to `turbo-places`;
  the modulith keeps the `/` catch-all (Traefik orders by rule length).
- `db.yaml` — the `places` CNPG `Database` (postgis + pg_trgm); **storage bumped
  to 12 Gi (+ WAL 4 Gi, mem 1 Gi, shared_buffers 256 MB)** for the national
  dataset.
- `places-ingest.yaml` — a weekly **CronJob** (the sole writer) running the
  **national** `bulk-ssr landsdekkende` → swap, with a 6 Gi `/work` emptyDir for
  the ~3 GB GML extract and a 1 h deadline.
- CI builds `turbo-places` + `turbo-places-ingest` images (matrix + Dockerfiles).
- **One manual prerequisite** (needs the cluster's kubeseal cert, by design not
  in-repo): re-run `scripts/seal-prod-secrets.sh` (now emits
  `connectionstring-places`) and commit the refreshed `sealed-secrets-db.yaml`.
  On first deploy, kick the ingest once (`kubectl create job --from=cronjob/
  turbo-places-ingest places-ingest-init`) rather than waiting for the schedule.

---

## P0 — Foundation defects (prerequisite, ~small)

Fixes for the four verified defects from the 2026-06-09 architecture review;
all block later phases.

| # | Fix | Test first | Why it gates later work |
|---|---|---|---|
| P0.1 | **ETag version source**: stop calling `StatsAsync` (two `count(*)`) per request. Introduce `places.dataset` as the authoritative version table; cache the active version in-process. | Behaviour test: ETag equals the `places.dataset` active version and **flips after a version swap** — red against today's `max(dataset_version)`-from-counts implementation. | `count(*)` per request is a table scan at 1M rows — invalidates every P2 perf number. |
| P0.2 | **cabi: `OnceLock` the embedded ruleset + ruleset-parameterized handle API** (`place_core_engine_new(ruleset_json)`, `_reverse(handle,…)`, `_free(handle)`), keeping the `_default` shims. | `cargo test`: construct an engine from a *modified* ruleset JSON (one distance band changed) and assert the changed verdict through the handle — impossible against the current `_default`-only ABI. | Hot-loaded ruleset versions; the embedded engine (P3) reuses the handle API. |
| P0.3 | **Startup native probe**: boot runs one `place-core` call; fail fast if the `.so` is missing/broken. | Behaviour test: host with `PLACE_CORE_LIB` pointed at an empty dir fails startup with the actionable message (currently boots fine and 500s later). | Converts a late runtime 500 into a deploy-time failure. |
| P0.4 | **`GET /api/places/ruleset/{version}`** serving the ruleset artifact. | Behaviour test for `200` + content `version=="1"` + `404` unknown version — red until the endpoint exists. | Closes the documented contract; P3 bundles embed the same artifact. |

Also: correct the backend-plan §2 text — we store **raw** `navneobjekttype`
(the ruleset matches raw types); the canonical-enum sentence is wrong, the
implementation is right.

---

## P1 — National ingestion (the hardest problem; do it first)

Goal: all of Norway in `places.places` / `places.areas`, reproducibly, with
versioned atomic swap and resume. Sources (all NLOD): **SSR** (~1M names),
**Administrative enheter** (357 kommuner + fylker), **Naturbase vern** (~7k
polygons), **Matrikkel vegadresser** (~2.6M), elevation for elevatable types.

### P1a. GPKG reader without GDAL *(hard, foundational)*

GeoPackage **is SQLite**. Rather than depending on `ogr2ogr` (absent from our
images and dev environment), read GPKG directly:

- `Turbo.Places.Ingestion/gpkg/GpkgReader.cs` — `Microsoft.Data.Sqlite` over
  the downloaded file; parse the GeoPackage binary geometry (GPB header:
  magic `GP`, flags, envelope, then standard WKB) with **NetTopologySuite's
  `WKBReader`** (already a solution dependency).
- **Reprojection**: Geonorge ships EPSG:25833 (UTM33). Add `ProjNet` (or a
  self-contained UTM→geographic implementation) behind a small
  `ICoordinateTransform`; **verify against Kartverket-published control
  points** (unit test with known UTM33↔WGS84 pairs, tolerance < 0.5 m).
- Stream rows (no full-file materialization); per-source mapping functions
  GPKG row → canonical `Place`/`Area` (reusing the name/`Ukjent`/Naturbase-code
  rejection rules already in the REST clients — extract them to a shared
  `Normalization` class so REST sampling and GPKG bulk produce identical rows).
- **Fallback** (recorded decision): if GPKG direct-read hits a wall (exotic
  envelope flags, multi-layer quirks), run GDAL in the ingestion container
  only — the runtime API image never needs it.

**Test first:** (1) commit the mini-GPKG fixture (a handful of real features,
GPB geometries in 25833) **and** the failing round-trip test (GPKG → canonical
`Place` with expected WGS84 coords + names) before `GpkgReader` exists;
(2) commit the Kartverket control-point pairs + the failing reprojection test
(tolerance < 0.5 m) before choosing/wiring the transform library. These two
red tests *are* the P1a spec.

### P1b. Download + staging + atomic swap + resume *(hard)*

- `GeonorgeDownloader`: resolve dataset → newest national file via the
  Geonorge Nedlasting/Atom API; stream to disk; record `(source, version,
  sha256)` in `places.dataset` with status `staging`.
- **Order**: admin → naturbase → SSR → addresses (enrichment spatial-join
  depends on admin polygons being in first).
- **Swap**: load into `places.places_staging` (same DDL), then in one
  transaction: delete the source's rows from live, insert from staging, mark
  the dataset row `active`, mark the prior `superseded`. Old data serves reads
  until the instant of swap. This also implements the missing **sweep**.
- **Resume**: each source's load is checkpointed (`rows_loaded` on the dataset
  row + idempotent re-run from the staging table); a crashed national run
  resumes, never duplicates (deterministic ids).
- **ETag/version**: the active-version read in P0.1 now reflects swap atomically.

**Test first** (behaviour tests over Testcontainers, small synthetic
datasets, written before the swap/resume code):
- *atomic swap under load*: a task hammers `/reverse` while a v2 swap runs —
  assert zero failed requests, results flip from a v1-only feature to a
  v2-only feature exactly once, ETag flips with them;
- *sweep*: a feature present in v1 and absent in v2 is gone after swap;
- *resume*: abort a load between staging and swap (simulated crash), re-run,
  assert no duplicates and a single active version.

### P1c. Enrichment at national scale *(hard)*

Replace the per-point REST calls (2 calls/name ≈ 2M+ requests — non-viable):

- **kommune/fylke**: one set-based SQL spatial join over the loaded admin
  polygons (`UPDATE places SET kommune_name… FROM areas WHERE ST_Contains`).
  Index-driven; minutes, not days.
- **Elevation** (decision with default): elevation is only *needed* for
  elevatable feature types (peaks/glaciers/landforms — the ruleset's `on`
  groups; ~50–100k rows nationally). **Default: batched Høydedata API for
  those rows only**, rate-limited (~6 rps ⇒ hours, run once per dataset
  version, resumable via a `needs_elevation` flag). DTM50 raster in
  `postgis_raster` remains the documented upgrade when/if per-point tapped
  elevation is wanted server-side. This keeps the national first run tractable
  and honest.

**Test first:** spatial-join test with fixture polygons (a place inside /
outside a committed kommune square gets the right `kommune_name`/null);
elevation-flagging test (only `on`-group types marked `needs_elevation`).

### P1d. Addresses + the missing cascade step

- Load Matrikkel vegadresser as `feature_type='Adressenavn'`-class rows is
  **wrong** — addresses pollute toponym search (observed in Tromsø). Load them
  into the same table but with `feature_type='address'` excluded from search
  retrieval by default and from the toponym KNN, queried only by the dedicated
  nearest-address query (plan §5.1c).
- Wire `ReverseInputDto.Address` (the core already implements the step +
  parcel-code rejection, golden-tested, currently dead): nearest address
  within 200 m feeds the cascade between park and kommune.

**Test first** (behaviour, red before the wiring): seed an address row 150 m
from a query point with no qualifying toponym → reverse returns
`Near <adressetekst>` with the postcode subtitle; seed a parcel-code-only
address → kommune fallback wins; the existing Tromsø cathedral case must stay
green (addresses must not displace tight toponyms); search results contain no
address-class rows by default.

Acceptance for P1 overall: `places.places` ≥ 0.9M toponyms + 2.5M addresses,
all kommuner/fylker/parks loaded; a re-run produces a new version and swaps
atomically under live read load (test: hammer `/reverse` during swap, zero
errors, version flips once).

---

## P2 — Correctness + performance gates ("works perfectly", measurably)

### P2a. Differential parity harness *(the strongest correctness instrument)*

`tools/places-parity/` (console): for a deterministic sample of N=2,000
coordinates (stratified: cities, trails, coast, wilderness, park interiors —
seeded RNG), compute:
- **ours**: `GET /api/places/reverse`
- **theirs**: the live Kartverket 5-call composition (reuse the REST clients)

Diff `title/qualifier/kommune` and **categorize**: identical · equivalent
(same feature, different spelling source) · ours-better (e.g. live returns
"Ukjent") · theirs-better (regression — must be zero or adjudicated) ·
no-coverage. Output a committed report; the *theirs-better* bucket is the
work queue. Gate: **0 unadjudicated regressions** on the sample. Same harness
re-runs per dataset version (it is the ingestion smoke test).

### P2b. Performance gate at national volume

- Latency budget (single replica, warm): **reverse p95 < 50 ms, search p95
  < 100 ms, health < 10 ms**; measured by a k6 script in
  `apps/api/tests/` (the repo already has `api_k6_performance.yml`).
- **Planner assertions**: behaviour-style test that runs `EXPLAIN (FORMAT
  JSON)` for the three hot queries against the national DB and asserts
  index usage (KNN ordered scan on `places_centroid_gist`, GIN on trigram,
  GIST on containment) — catches planner regressions when row counts shift.
- Connection discipline: move `PgPlaceStore` to a shared `NpgsqlDataSource`.

### P2c. Search relevance at national collision density

With real national data (dozens of `Storvatnet`):
- add the `tsvector` leg from plan §5.2 (currently absent) for multi-word
  queries;
- **type weighting** in retrieval order (Tettsted/By/Fjell above Gard above
  address-class) — retrieval only; final ordering stays place-core's.

**Test first — P2 is TDD at system level:** the parity harness (P2a), planner
assertions and k6 thresholds (P2b), and the ~30-query relevance fixture with
expected top-3 (P2c) are all written and run **red** against the untuned
national load; tuning proceeds until they hold. The relevance fixture is
committed and re-run per dataset version (not CI).

---

## P3 — The embedded engine (`place-core` `embedded` feature)

The second product deliverable. No client code — verified by its own harness.

### P3a. Bundle format (spec first, committed as `docs/` + `packages/place-core/BUNDLE.md`)

One SQLite file per region:

```
manifest(key, value)            -- format_version, dataset_version,
                                -- ruleset_version, bbox, created_at, attribution
ruleset(json)                   -- the exact ruleset artifact (P0.4)
places(id, name, name_fold, kind, lat, lng, status,
       elevation_m, kommune, fylke)
places_rtree                    -- R*Tree virtual table (minLat,maxLat,minLng,maxLng)
places_fts                      -- FTS5(name_fold, tokenize='trigram') content table
areas(id, area_type, name, kind, geom_wkb)   -- park/kommune polygons (WKB)
areas_rtree                     -- polygon bboxes for containment prefilter
```

Containment = R\*Tree bbox prefilter → exact point-in-polygon on the WKB
(small `geo`-style point-in-ring test in the crate — no GEOS dependency).
Budget: **≤ 1 MB per 1,000 km² of typical terrain; whole-Norway pack target
≤ 150 MB** (measure, then tune: drop `Adressenavn` from bundles by default).

**Test first:** hand-build a tiny bundle fixture (a dozen rows, one park
polygon) with plain SQL and commit it; write the manifest/schema validation
test and the point-in-ring unit tests (point inside / outside / on-boundary /
in-hole) before any builder or engine code. The fixture doubles as the P3c
engine's first input.

### P3b. Builder + `/bundle` endpoint (server side)

- `BuildBundleHandler`: bbox (+margin) slice of the national DB → the SQLite
  file (constructed with `Microsoft.Data.Sqlite`; R\*Tree/FTS5 are available in
  the bundled e_sqlite3). Cache by `(bbox-hash, dataset_version)`.
- `GET /api/places/bundle?bbox=&since=` → file response; `304` when the
  client's `since` (dataset version) is current.

**Test first:** behaviour test (red before the handler) — request a bundle for
the fixture bbox, open the returned file with `Microsoft.Data.Sqlite`, assert
it passes the P3a schema-validation test and carries the expected row counts,
dataset version, and `304` on a current `since`.

### P3c. Embedded read engine (Rust, the payoff of the shared core)

- `place-core` `embedded` feature: `rusqlite` (**`bundled` feature OFF on
  mobile targets — link the platform SQLite**; document per-target linking in
  the crate README; the standalone harness may use bundled).
- API (same handle style as P0.2):
  `Bundle::open(path) -> Bundle` (validates manifest + ruleset),
  `bundle.reverse(lat, lng) -> Option<LocationDescription>`,
  `bundle.search(q, near) -> Vec<SearchHit>` — internally: R\*Tree KNN window →
  candidates → **the existing `rank()`**; FTS5/prefix retrieval → **the
  existing `forward_search()`**. Zero duplicated decision logic.
- Exposed through UniFFI + cabi like the rest of the crate.

**Test first:** `cargo test --features embedded` replaying `golden.json`
through `Bundle::reverse`/`search` against the P3a hand-built fixture — red
until the engine exists; written before `Bundle` has a single method body.

### P3d. Proving server ≡ embedded *(the "consistent by construction" gate)*

- **Equality test** (the keystone): build a bundle from the behaviour-test
  fixture data; for a grid of query points + the relevance query list, assert
  `embedded.reverse(p) == server /reverse(p)` and search parity, field for
  field. Runs in CI (the bundle builder works off the Testcontainers DB).
- Golden: `golden.json` replayed through `Bundle` calls (`cargo test
  --features embedded`).
- **Footprint/latency budget**: bundle open < 50 ms; reverse < 5 ms; search
  < 10 ms; RSS delta < 10 MB on a national-park bundle (measured in the
  harness, asserted with slack).

### P3e. Standalone verification harness (replaces client integration)

`packages/place-core/examples/bundle_cli.rs`: `bundle-cli <file> reverse
<lat> <lng>` / `search <q>` — plus a scripted demo in `clients/ci.sh`:
download (or build) the Jotunheimen bundle, run the five terrain-type queries
from the sampling run, diff against recorded server answers. This is the
product demo for the embedded path until a client adopts it.

---

## P4 — Working-API finish line

- **Rate limiting** at the gateway for `/api/places/**` (it is otherwise an
  open public geocoder) + a modest in-host `AddRateLimiter` as defense in
  depth on the standalone host.
- **Attribution**: `attribution` field in `/health` and bundle manifests
  ("© Kartverket / Miljødirektoratet, NLOD"); release-checklist item for
  client surfaces (out of scope here, recorded for later).
- **Ingestion ops**: CronJob manifest + runbook (`docs/`): run cadence,
  resume procedure, parity-report review step before promoting a dataset
  version (the swap can be staged: load → parity → promote).
- **CI**: a weekly "fylke dry-run" job — P1 pipeline over one fylke ⇒ keeps
  the GPKG/reprojection path from rotting between national runs.

---

## §6 Acceptance gates — the definition of "works perfectly"

| Gate | Criterion |
|---|---|
| G1 correctness (core) | golden suite green in Rust, Python, Kotlin, .NET — *(already holding)* |
| G2 correctness (national) | parity harness: 0 unadjudicated `theirs-better` regressions over the 2,000-point stratified sample |
| G3 consistency | server ≡ embedded byte-equality on fixture data (CI) |
| G4 performance | reverse p95 < 50 ms, search p95 < 100 ms at national volume; planner assertions green |
| G5 durability | atomic version swap under live read load, zero failed requests; resumable crashed ingest |
| G6 embedded budget | park bundle: open < 50 ms, reverse < 5 ms, search < 10 ms, ≤ 1 MB/1,000 km², whole-Norway ≤ 150 MB |
| G7 ops | rate-limited, attributed, runbook'd, fylke dry-run green weekly |

## Sequencing & dependencies

```
P0 (days) ──► P1a..d (the bulk of the work; P1a/P1b are the risk)
                 │
                 ├──► P2a/P2b/P2c (gates; iterate with P1 until G2/G4 hold)
                 │
                 └──► P3a..e (independent of P2c; needs P1 only for real
                      national bundles — fixture bundles unblock P3c earlier)
P4 last (small, mechanical)
```

P3 can start in parallel with late P1 (the bundle format + embedded engine
test against *fixture* data, not national data). The critical path is
P1a → P1b → P2a.

## Risks

- **GPKG direct-read** is the main technical bet; the GDAL-in-ingestion-image
  fallback caps the downside (recorded above).
- **Reprojection correctness** — silent meter-scale skew would poison
  everything; hence the control-point unit test in P1a, tolerance < 0.5 m.
- **Høydedata batch volume** — if Kartverket throttles harder than expected,
  elevation lands in a later dataset version (nullable by design).
- **FTS5 trigram tokenizer availability** on platform SQLite versions
  (Android's is old) — verify in P3c on the oldest supported targets; the
  fallback is LIKE-over-R\*Tree-window which the harness can A/B.
- **Whole-Norway bundle size** — if the 150 MB target misses, per-region
  bundles remain the default product (already the plan's stance).
