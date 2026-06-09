# Automated Data Provisioning — Implementation Plan

**Date:** 2026-06-09
**Companion to:** `2026-06-n50-basemap-implementation-plan.md`, `apps/tileserver/docs/ingesting-n50.md`
**Goal:** No human ever touches a data dump. The tileserver orders, downloads,
restores, and upserts Kartverket data itself — triggered from the admin panel
or a schedule. "Drop a 25 GB zip on the volume, then POST a trigger" becomes
"click Provision" (or: it just happens on a cron).

---

## 1. Where the manual work is today

The ingest pipeline is already software once a dump is on disk:

```
[MANUAL]                    [AUTOMATED — exists]
human downloads N50    →    n50-restore (psql load + schema rename)
from Geonorge, rsync   →    n50-vann/hoydekurve/bygning/... upserts
or TUS-uploads the zip →    paths.ingest_job rows, admin /api/ingest/jobs polling
```

Everything left of the arrow is a person with a browser and `rsync`. The
`incoming_dir` + TUS upload + `trigger_bulk` machinery exists *because* a human
puts the file there. That is the only manual step — and it's the one this plan
deletes.

**What already exists and is reused:**
- `run_job_with_options` + `paths.ingest_job` — job execution + progress log.
- `n50::restore` (idempotent: skips when `n50_staging` exists unless `force`)
  and the seven `n50::upsert_*` functions.
- `reqwest` is already an ingest dependency (FKB WFS, DTM, DNT all fetch HTTP).
- `scripts/pull-n50-fylke.sh` — the exact Geonorge Nedlasting API order/download
  flow, proven; this plan ports it into the server.
- The TUS sweeper (`spawn_sweeper`) — the precedent for a boot-time background
  task.

---

## 2. Design

Three new pieces, each small and composable:

### 2.1 `geonorge` fetch module (ingest crate)

Ports the Nedlasting API flow into Rust:

1. `POST /api/order` with `{ metadataUuid, areas:[{code,type}], formats:[PostGIS],
   projections:[25833] }` → `referenceNumber` + `files[]`.
2. Poll `GET /api/order/{ref}` until every file is `ReadyForDownload` (small
   county orders are ready immediately; the national order queues for minutes).
3. Stream each `downloadUrl` to `<incoming>/<name>.zip` (resumable-friendly,
   checks `Content-Length`).
4. Return the local path — handed straight to `n50::restore`.

Pure, testable seam: `build_order_body(dataset, area)` and area-code validation
are unit-tested offline; the network round-trip is covered by the live e2e.

**Datasets** modelled as an enum so adding Turrutebasen / DTM later is a variant,
not a fork: `Dataset::N50` (UUID `ea192681-…`), with `national` vs per-`fylke`
area selection.

**Idempotency:** the fetch records the dump's `oppdateringsdato`/ETag in a small
`ingest.source_version` table. A re-fetch with the same version is a no-op
(returns the cached path); `force` overrides. So a daily schedule only does real
work when Kartverket actually published.

### 2.2 `provision-n50` orchestration job

One job that runs the whole chain, logged as a single `ingest_job` row with
per-step progress in the structured log:

```
provision-n50(area, force):
  zip   = geonorge::fetch_n50(area, incoming_dir, force)   # download (or cached)
  restore(zip, force)                                       # psql load + rename
  for upsert in [vann, hoydekurve, bygning, kystkontur,
                 isogbre, landcover, stedsnavn, vegnett]:    # canonical tables
      upsert()
  refresh_basemap_matviews()                                # when M2 lands
  edge_attrs(); skeleton_build()                            # routing mesh
```

Re-runnable and resumable: each sub-step is independently idempotent
(truncate-and-load upserts, `CREATE OR REPLACE`, `ON CONFLICT`), so a retry after
a mid-chain failure converges. `force` re-downloads + re-restores from scratch.

### 2.3 Admin + CLI + schedule surfaces

- **CLI:** `tileserver ingest --job provision-n50 --area 03` (add `--area`).
  `geonorge-fetch` is also exposed standalone for download-only.
- **Admin:** `POST /admin/api/provision {area, force}` → spawns the job, returns
  `run_id`; the existing `/api/ingest/jobs` polling shows progress.
  `GET /admin/api/geonorge/areas` proxies the Nedlasting area codelist so the
  SPA renders a county dropdown (+ "Whole country").
- **SPA:** a "Provision data" panel — pick area, click, watch the job log. (The
  React page; backend-first so it works via `curl`/CLI immediately.)
- **Schedule (hands-off):** a boot-time hosted task (mirroring the TUS sweeper).
  `TILESERVER_PROVISION_ON_BOOT=1` provisions on startup if the DB is empty;
  `TILESERVER_PROVISION_SCHEDULE=<cron|interval>` re-checks on a cadence (a
  no-op when the source version is unchanged). Default off — explicit opt-in.

---

## 3. Safety & operability

- **Path safety unchanged:** downloads land in `incoming_dir`; the same
  `resolve_under_incoming` guard the manual path uses applies.
- **Disk:** national N50 is ~272 MB zip × per-county or ~ several GB national
  SQL. The fetch streams to disk and deletes the zip after a successful restore
  unless `KEEP_DUMPS=1`. A pre-flight `statvfs` check refuses to start a fetch
  that wouldn't fit.
- **Auth:** all admin provisioning routes stay behind `RequireRole<Curator>`.
- **Network policy:** `nedlasting.geonorge.no` + `*.geonorge.no` must be on the
  environment's egress allowlist (document in README-tileserver).
- **Failure semantics:** a failed fetch/restore marks the `ingest_job` row
  `failed` with `error_text`; partial staging is dropped on the next `force`
  restore. Never leaves a half-served basemap (artifacts/matviews only swap on
  full success).

---

## Status (2026-06-09) — implemented & tested

P0–P4 landed and verified end-to-end against the **live** Geonorge API:

- `geonorge.rs` (order/poll/stream + `build_order_body`/area validation, unit-
  tested) and `provision.rs` (the chain), wired as `geonorge-fetch` /
  `provision-n50` jobs with a new `JobOptions.area` and `--area` CLI flag.
- Admin `POST /admin/api/provision` + `GET /admin/api/geonorge/areas`; a
  "Provision data" SPA panel (county dropdown → click → live job table).
- Boot-time auto-provision: `TILESERVER_PROVISION_ON_BOOT=<area>` self-
  populates an empty deploy; restarts no-op.
- **Proof:** empty DB → `provision-n50 --area 03` → 27 924 rows across all
  layers, one `succeeded` job row, in seconds, zero manual steps; a
  `PROVISION_ON_BOOT=03` server populated itself from empty and then served a
  32 KB `/v1/basemap` tile.

Two fixes fell out of the live testing:
1. **TLS roots:** the fetch/proxy clients now trust the OS trust store *and*
   bundled webpki roots, so download works behind a TLS-intercepting proxy
   (CI/dev) and in distroless prod. (`reqwest` `rustls-tls-native-roots` +
   `.tls_built_in_native_certs(true)`.)
2. **Batch timeouts:** ingest/provision pools run with `statement_timeout = 0`
   (batch, not serving) so a cold-cache contour/vegnett upsert isn't killed by
   the 10 s serving default.

Not yet done: per-source version recording for fetch-skip (a re-fetch always
re-orders today), a recurring refresh schedule (only the boot/empty trigger
exists), and folding the new `coastline`/raster work — which lives on a later
branch state — back in.

## 4. Milestones

| # | Deliverable | Acceptance |
| --- | --- | --- |
| **P0** | `geonorge` fetch module + `geonorge-fetch` job + `--area` CLI | `tileserver ingest --job geonorge-fetch --area 03` downloads the Oslo zip into the incoming dir, no human steps. Order-body/area unit tests pass. |
| **P1** | `provision-n50` orchestration | Against an **empty** DB, `provision-n50 --area 03` ends with every canonical table populated + a single succeeded `ingest_job` row. Re-run is a fast no-op (cached version). |
| **P2** | Admin endpoints | `POST /admin/api/provision {area:"03"}` returns a `run_id`; `/api/ingest/jobs` shows it progress to `succeeded`. `GET /admin/api/geonorge/areas` lists fylker. |
| **P3** | SPA panel | Curator picks a county, clicks Provision, watches the log — zero terminal use. |
| **P4** | Schedule | `TILESERVER_PROVISION_ON_BOOT=1` on a fresh deploy self-populates; the cadence check re-provisions only when Kartverket publishes. |

---

## 5. File touchpoints

- `crates/turbo-tiles-ingest/src/geonorge.rs` — **new**: order/poll/download +
  `build_order_body`, `Dataset`, area validation, `source_version` recording.
- `migrations/…_source_version.sql` — **new**: `ingest.source_version(dataset,
  area, version, fetched_at)`.
- `crates/turbo-tiles-ingest/src/job.rs` — `JobName::{GeonorgeFetch,
  ProvisionN50}`, `JobOptions.area`, dispatch.
- `crates/turbo-tiles-ingest/src/provision.rs` — **new**: the orchestration.
- `crates/turbo-tiles-bin/src/main.rs` — `--area` arg; boot-time
  auto-provision/schedule hosted task.
- `crates/turbo-tiles-admin/src/routes/{provision.rs,mod.rs}` + `lib.rs` —
  **new** endpoints; `geonorge` areas proxy.
- `apps/admin/` — **new** "Provision data" SPA panel.
- `infra/k8s/README-tileserver.md` — egress allowlist + the now-automated
  data step (delete the manual rsync instructions).

---

## 6. Outcome

After this, standing up Norway is: deploy the tileserver with
`TILESERVER_PROVISION_ON_BOOT=1` and the tiles DB — it downloads N50, restores,
upserts, and starts serving the basemap with no human in the loop. Refreshing is
a schedule, not a chore. The manual `rsync`/TUS path remains for air-gapped or
one-off imports, but is no longer the happy path.
