# Shared ingestion infrastructure — plan (2026-07)

## Problem

Two services ingest external geodata on a schedule, with almost no shared
machinery and very uneven maturity:

- **turbo-places** (.NET) — a weekly k8s CronJob (`bulk-ssr`) that orders SSR from
  Geonorge, stream-parses GML, stages into Postgres, and atomically swaps.
  **No freshness gate** (re-downloads ~3 GB + rewrites ~1 M rows every week even
  when SSR is unchanged), **no run tracking**, **no metrics/alerting**. Sources
  are hardcoded consts + CronJob args.
- **turbo-tileserver** (Rust) — a mature-ish framework: boot-provision + an
  in-process refresh loop + admin/CLI triggers, SHA-256 content-hash freshness
  (`should_skip`), a `paths.ingest_job` run ledger, `paths.provision_state`
  provenance, dedicated batch DB pools, per-source fetchers (Geonorge N50,
  Turrutebasen, DNT JSON, FKB WFS, DTM10). Sources still hardcoded as consts.

They are different languages against different databases (turbo-db/places vs
tiles-db), so **there is no single shared engine to build** — a cross-language
ingestion framework would mean rewriting the GML reader + place-core
classification in Rust, or the artifact bake in C#. Not worth it.

## Goal / non-goals

**Goal:** a thin, shared *contract + control plane* — implemented per-language —
that gives: one declarative catalog of sources, one run-ledger shape, one
freshness rule, uniform scheduling, and unified staleness alerting. Bring Places
up to the tileserver's model and unify observability across both.

**Non-goals:** a single cross-language ingestion engine; a heavyweight
orchestrator (Airflow/Dagster/Argo — too heavy for the ~6 GiB single node);
merging the two databases; changing the artifact-bake pipeline
(`tileserver build-artifacts`) — that stays a separate offline step.

## The shared contract (four pieces)

### 1. Source catalog manifest — `infra/k8s/base/ingest/catalog.toml`
One declarative file, one schema, each service reads the entries it owns.
Externalizes today's hardcoded UUIDs/endpoints/format/projection/cadence.

```toml
[[source]]
id            = "ssr"
owner         = "places"          # places | tileserver
kind          = "geonorge-order"  # geonorge-order | wfs | http-json | pgdump-file | kartverket-raster
metadata_uuid = "30caed2f-454e-44be-b5cc-26bb5c0110ca"
format        = "GML"             # GML | PostGIS | GeoPackage | GeoTIFF | JSON
srs           = "EPSG:25833"
area          = "landsdekkende"
cadence       = "weekly"          # informational + drives the staleness alert threshold
license       = "© Kartverket (NLOD)"

[[source]]
id            = "n50"
owner         = "tileserver"
kind          = "geonorge-order"
metadata_uuid = "ea192681-..."
format        = "PostGIS"
srs           = "EPSG:25833"
area          = "national"
cadence       = "monthly"
license       = "© Kartverket (NLOD)"
# ... turrutebasen, dnt-cabins, fkb-sti, dtm10 as further entries
```

- **Rust** reads it with `serde` + `toml` (already a dep pattern — the bake config
  `tools/vector-layers.toml` uses the same). `turbo-tiles-ingest::geonorge::Dataset`
  (an enum built to extend, `geonorge.rs:27`) becomes catalog-driven.
- **.NET** reads it with **Tomlyn** (small, maintained); `BulkSsrDemo`'s
  `StedsnavnUuid`/format/srs consts (`BulkSsrDemo.cs:18-21`) become a catalog
  lookup by `id`.
- Cadence is not a scheduler (k8s owns timing) — it's the *expected* interval,
  used to compute the staleness alert threshold and to document intent.

### 2. Ingest-run ledger — one schema, per-DB table
Adopt the tileserver's `paths.ingest_job` shape (`job.rs:411-452`) as the common
contract; Places grows the same table in its DB:

```
ingest.run (
  run_id       uuid pk,
  source_id    text,        -- catalog id ("ssr", "n50", …)
  status       text,        -- running | success | skipped_unchanged | failed
  started_at   timestamptz,
  finished_at  timestamptz,
  source_version text,      -- upstream version / content hash (see #3)
  rows_in      bigint,
  rows_written bigint,
  error_text   text
)
```

Source of truth stays per-DB (two clusters), but the **shape is identical**, and
both services emit the same OTel metrics (see Observability) so one dashboard +
one alert rule cover everything.

### 3. Freshness contract — every source records a `source_version`
Two-level, cheapest-first:

- **Pre-download check (preferred):** ask upstream for a version marker *before*
  downloading. For Geonorge, the dataset metadata carries a `dateUpdated`/edition;
  the order response and/or the Kartverket metadata API expose it. If the marker
  equals the last successful `source_version`, **skip entirely** — this is what
  saves Places the ~3 GB weekly download. *(Implementation must confirm the exact
  Geonorge field; fall back to level 2 if none is reliable.)*
- **Post-download hash (fallback / integrity):** SHA-256 the downloaded artifact
  and skip the expensive stage+promote when unchanged. This is exactly what the
  tileserver already does (`provision.rs:48`, `should_skip` `provision.rs:224`).

Both services expose the same trait/interface conceptually:
`latest_upstream_version() -> token` (cheap) and `artifact_hash() -> token`
(after download). Skip when either equals the stored last-success version.

### 4. Scheduling + provenance — standardize on k8s CronJobs
- Places is already a CronJob (`places-ingest.yaml`, `0 4 * * 0`). Keep.
- **Move the tileserver's in-process refresh loop** (`main.rs:508-577`,
  `TILESERVER_PROVISION_REFRESH_SECS`) to a **k8s CronJob** that runs
  `tileserver ingest --job provision-n50` (the CLI path already exists,
  `main.rs:579`). Benefits: k8s-level history/retry/visibility uniform with
  Places, and the serving pod stops carrying a background ingest loop. Keep
  boot-provision (`TILESERVER_PROVISION_ON_BOOT`) as empty-DB self-heal.
- Every dataset ends with common provenance `{source_version, fetched_at,
  published_at}`: tileserver has it in `provision_state`; Places adds
  `source_version` + `fetched_at` to `places.dataset` (today only `version` +
  `published_at`, `PgPlaceStore.cs`).

## Per-phase delivery (each independently shippable)

### Phase 0 — Contract spec (no behaviour change)
- Write `infra/k8s/base/ingest/catalog.toml` encoding today's sources (SSR, N50,
  turrutebasen, dnt-cabins, fkb-sti, dtm10).
- Document the `ingest.run` schema + OTel metric names in this doc's appendix.
- No code wired yet — this is the shared vocabulary both phases below target.

### Phase 1 — Places freshness (highest value; deploy first)
Stops the weekly full re-ingest when SSR is unchanged.
- Add a source-version pre-check to `GeonorgeClient` (`GeonorgeClient.cs`):
  fetch the dataset's upstream `dateUpdated`/edition; if it equals the active
  `places.dataset.source_version`, exit `skipped_unchanged` before download.
- Add post-download hash fallback: hash the extracted GML set; skip `SwapAsync`
  when equal to the stored hash.
- Stamp `source_version` (+ `fetched_at`) into `places.dataset`
  (`PgPlaceStore.cs` schema + `SwapAsync`/`PublishDatasetVersionAsync`).
- Verify: a no-op re-run logs `skipped_unchanged` and does **zero** DB writes;
  a changed source still swaps. Testcontainers integration + a fake Geonorge.
- Deploy path: `turbo-places` image only (see [[places-search-quality]] for the
  build/pin flow).

### Phase 2 — Places run ledger + query endpoint  ✅ (ledger); OTel → Phase 5
- Add `places.ingest_run` table (tileserver `ingest_job` shape) + write a row per
  run (`running` → `success|skipped_unchanged|failed`), from `BulkSsrDemo`.
- Expose `GET /api/places/ingest/runs` (mirror tileserver's
  `/api/ingest/jobs`, `admin/routes/ingest.rs:173`).
- **OTel emission deferred to Phase 5:** the standalone Places host wires no
  OpenTelemetry SDK today (only the modulith does). Rather than add the SDK to a
  512Mi service for one gauge, Phase 5 wires OTel once and publishes the ingest
  gauges (`ingest_last_success_timestamp{source}`, …) by reading this ledger — a
  long-lived-service observable gauge, not export from the short-lived CronJob.

### Phase 3 — tileserver reads the shared catalog + emits the same metrics
- Replace hardcoded source consts (`geonorge.rs:22/35`, `fkb_wfs.rs:54`,
  `dnt_cabins.rs:167`, `turbase.rs`) with catalog lookups; keep `geonorge::Dataset`
  as the typed handle populated from the catalog.
- Emit the same OTel metric names from `close_job_row` (`job.rs:426`) so the
  tileserver and Places report into one namespace.

### Phase 4 — Scheduling uniformity
- New `turbo-tileserver-ingest` CronJob (cadence from catalog) running
  `tileserver ingest --job provision-n50`; retire the in-process refresh loop
  (keep boot-provision). Batch pool settings already isolate long restores
  (`main.rs:469`).

### Phase 5 — Unified staleness alerting + dashboard
- Prometheus/Alloy rule: `time() - ingest_last_success_timestamp{source} >
  cadence_seconds(source)` → alert. Cadence per source from the catalog.
- One Grafana "Ingest" dashboard: last run, status, rows, staleness per source,
  across both services.

> **Infra reality (2026-07 investigation):** the prod alert/observability config
> is NOT in this repo — prod ships to `alloy.observability.svc` (Alloy/Grafana
> managed out-of-band); `infra/observability/*.yml` is only the local dev stack.
> Neither the tileserver nor the Places host exports metrics today. So the
> alert *rule* is authored out-of-band, not here. **Phase 2 already makes it
> feasible without new metrics:** an external synthetic/uptime check can
> `GET /api/places/ingest/runs?limit=1` and alert when the newest run is
> `failed`, or `finished_at` is older than the source cadence. The tileserver's
> `GET /api/ingest/jobs` gives the same for its sources. Concrete predicate to
> add out-of-band: alert if `now() - runs[0].finishedAt > cadence(source)` OR
> `runs[0].status == "failed"`. The OTel-gauge path (in-service metric export)
> is only needed if you want Prometheus-native alerting instead of a synthetic
> check — a larger lift (add OTel SDK + scrape) deferred until warranted.

## Verification strategy
- **Unit:** freshness skip logic in both languages (unchanged → skip; changed →
  run). Catalog parse round-trips in Rust and .NET.
- **Integration (Testcontainers):** Places ingest against a fake Geonorge —
  first run ingests, second identical run is `skipped_unchanged` with zero
  writes, a bumped version re-swaps. Ledger row + metric emitted per run.
- **Operational:** confirm a real Sunday no-op run skips the download (logs +
  ledger), and the staleness alert fires when a run is overdue.

## Risks / open questions
- **Geonorge upstream version field:** if no reliable per-dataset `dateUpdated`
  is exposed, Phase 1 falls back to download-then-hash — still skips the
  expensive stage+swap but pays the download. Confirm the field first.
- **Two DBs → no single ledger table:** unified view is via OTel metrics, not a
  joined SQL table. Acceptable; avoids a central ledger service.
- **TOML in .NET:** adds a Tomlyn dependency (small). Alternative: emit a JSON
  twin of the catalog at build time if we want zero .NET TOML dep.
- **Refresh-loop → CronJob:** an infra change; keep boot-provision so an empty DB
  still self-heals if the CronJob hasn't run.

## Not in scope
- Artifact bake (`tileserver build-artifacts`) stays a separate offline step.
- No new orchestrator service; k8s CronJobs + a per-DB ledger + OTel/Alloy alerts
  are the whole control plane.
- Matrikkel address ingest (a *new* Places source) is separate; when it lands it
  becomes another catalog entry, which is exactly the point of the catalog.

## Sequencing recommendation
1 (Places freshness) → 2 (Places ledger+OTel) → 5-lite (staleness alert on
Places) for the fastest operational payoff, then 3 → 4 → 5-full to bring the
tileserver onto the shared catalog + uniform scheduling. Phase 1 alone repays
the weekly 3 GB / 1 M-row waste and is a `turbo-places`-only deploy.
