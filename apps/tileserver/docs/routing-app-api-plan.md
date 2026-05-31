# Analysis + Plan: Routing as a first-class app API

Goal: expose the routing feature to the Flutter app via a proper, stable
REST API; well-structured, maintainable modules; deployed on the same
stack as the rest of the app. Analysis based on `origin/main` (fetched
2026-05-31; not merged — see "Repo state" at the end).

## Current state — most of the plumbing already exists

- **Engine + API**: the Rust **tileserver** (`apps/tileserver`) is the
  routing engine *and* an axum `/v1/*` HTTP API. It is containerised
  (`apps/tileserver/Dockerfile`) and runs in `infra/compose` alongside a
  `postgis-pgrouting` DB.
- **Gateway**: the app traffic goes through a **YARP API gateway**
  (`apps/api/src/Gateway`) at `/api/*`. It already has a `tileserver-cluster`
  (`http://tileserver:8080`) with two routes:
  - `/api/tiles/{**}` → tileserver (strips `/api/tiles`) — *public*
  - `/api/tiles-admin/{**}` → tileserver `/admin/...` — *admin*
  So `…/api/tiles/v1/pathfind` already reaches the solver today.
- **Flutter client**: `lib/core/api/api_client.dart` is a Dio client with
  `baseUrl = EnvironmentConfig.apiBaseUrl` (`https://kart-api.sandring.no`
  prod), a `Authorization: Bearer <accessToken>` interceptor and 401
  refresh-retry. Features add a small `api.dart` (e.g. `core/sharing/api.dart`).
  A routing client slots straight into this.
- **Modules**: `apps/api` is a modulith of clean modules (Auth, Geo,
  Tracks, Collections, Activities, **Sharing**). `Sharing` is the freshest
  template: `*.Api` (controllers + request/response + integration),
  `*.Core` (domain model/service), `*.Infrastructure` (data/migrations),
  `*.Contracts` (value types).

### Gaps to close
1. **The `/v1` API is dev/admin-shaped.** It exposes debug endpoints,
   per-event recording, layer weights, raw cost-config overrides, and the
   internal `Path` shape. The app needs a small, **curated, versioned,
   documented** contract — not the internal surface.
2. **Not deployed to prod.** The tileserver is in `infra/compose` but NOT
   in `infra/k8s` (prod is GitOps-managed k8s: modulith + CNPG DB + web).
   Routing isn't deployed to the real stack yet.
3. **Auth** on the public tiles route needs to match the rest of the app
   (JWT bearer), while admin/debug stays locked down.

## Where should the app-facing API live? (decision)

| Option | What | Verdict |
|---|---|---|
| **A. Expose `/v1` directly** via the existing `/api/tiles/*` | Zero work; already routable | ✗ couples the app to the internal dev API + leaks debug endpoints |
| **B. Curated public routing surface in the tileserver** (`/v1/route/*`), versioned + OpenAPI, fronted by a dedicated gateway route `/api/route/*` | One engine, one source of truth, stable contract, no second codebase | ✅ **Recommended now** |
| **C. Thin .NET `Routing` module** (mirrors `Sharing`) that owns the contract and calls the tileserver | Adds a hop + second DTO set | ✅ **Later**, only when routes must be *saved / shared / owned* by users, combined with collections, quota'd, or cached server-side |

Recommendation: **B now**, **C when persistence/ownership is needed**.
At that point the .NET module persists a `Route` aggregate and delegates
geometry to the tileserver — it does not re-implement routing.

## The public routing contract (stable, versioned, decoupled from `Path`)

Small surface — the app needs ~3 things. New module
`crates/turbo-tiles-api/src/v1/route.rs` (public), separate from the
existing `pathfind.rs` (which stays the admin/debug surface).

- `POST /api/route/v1/plan`
  - body: `{ "points": [[lon,lat], …≥2], "preset"?: "balanced|avoid_roads|direct|easy_grade|trail_purist", "profile"?: "foot" }`
  - 200: `{ distance_m, duration_s, ascent_m, on_trail_pct,
    surfaces: { trail_m, road_m, off_trail_m, … },
    geometry: <GeoJSON LineString>,
    legs: [{ from_index, to_index, distance_m }] }`
  - errors (typed envelope `{ error, code, details }`):
    `segment_failed{leg_index}`, `endpoint_refused{which,reason}`,
    `no_coverage`, `degenerate_input`.
- `GET /api/route/v1/presets` → `[{ name, label, description }]` (curate
  the existing one).
- *(optional, later)* `POST /api/route/v1/plan/stream` — SSE live preview.
  Probably defer for mobile (a spinner + final route is usually enough);
  the machinery already exists if wanted.

Explicitly **out** of the public contract (admin-only, stay behind
`/api/tiles-admin`): `/v1/debug/*`, `/pathfind/record`, layer weights,
`cost_config_override`, the recording/tracer payloads.

Principles: `/v1` versioned path, additive-only evolution, an OpenAPI spec
committed in-repo (drives the Flutter client + docs), a typed error
envelope, and defaults so the app sends minimal input (just `points`).

## Module shape (maintainability)

- **tileserver**: split the axum surface into `route` (public, curated)
  and `pathfind` (debug/admin) groups. Public handlers map the internal
  `Path` → the stable public DTO in ONE place, so the engine stays free to
  change without breaking the app. Add an OpenAPI doc (utoipa or a
  committed `route.openapi.yaml`).
- **Flutter**: a `features/routing` module — `api.dart` (typed
  `RouteRequest`/`RouteResult`), `RoutingRepository`, reusing the shared
  `ApiClient` (Bearer + refresh for free). Geometry renders on the existing
  map layer. Mirrors `core/sharing/api.dart`.
- **(later) .NET `Routing` module**: copy the `Sharing` skeleton —
  `Turbo.Routing.{Api,Core,Infrastructure,Contracts}` — for saved routes.

## Deploy to the same stack (k8s GitOps)

The recent infra commits moved prod to GitOps-managed k8s (modulith +
CloudNativePG DB + web at `kart.sandring.no`). To put routing there:

1. **Tileserver k8s manifests** in `infra/k8s` (mirror the modulith deploy):
   `Deployment` + `Service` (`tileserver:8080`), resource requests/limits,
   readiness/liveness on `/` or `/healthz`.
2. **Artifacts** (DEM / graph / vectors — GBs): mount from a `PVC`
   populated by a one-shot/CronJob build (`build-artifacts` /
   `build-norway.sh`) — NOT baked into the image. Refresh on data updates.
3. **DB**: vector tiles need PostGIS+pgrouting — either a dedicated
   `postgis-pgrouting` workload or reuse CNPG with the extensions; confirm
   which the tileserver needs at runtime vs only at build time.
4. **Gateway**: add `/api/route/{**}` → `tileserver-cluster` (clean public
   path next to `/api/tiles/*`), behind the standard JWT auth policy; keep
   `/api/tiles-admin/*` admin-only.
5. **Memory**: the DEM is mmap'd and the node is ~6Gi-limited (per the
   observability commits) — set requests/limits carefully or schedule on a
   roomier node; the per-leg cache adds bounded memory.

## Stages

1. **Public API** in the tileserver: `/v1/route/plan` + `/v1/route/presets`,
   stable DTOs + error envelope + OpenAPI. Keep debug separate. Unit +
   contract tests.
2. **Gateway**: `/api/route/*` route + auth policy. ✅ **DONE** — added a
   `route-route` in **both** topology blocks (`Microservices` + `Modulith`)
   of `apps/api/src/Gateway/appsettings.json`: `/api/route/{**catch-all}` →
   `tileserver-cluster`, transforms `[PathRemovePrefix /api/route,
   PathPrefix /v1/route]` so `/api/route/plan` → tileserver `/v1/route/plan`.
   Covered by `GatewayTopologyBehaviour.route_plan_front_door_is_registered_*`
   (verified end-to-end: under the dev config the gateway forwards to the
   live tileserver, which returns 405 for GET-on-POST — proving the chain).
   **Auth caveat:** the gateway is currently a *pure reverse proxy with no
   authentication middleware* (`Program.cs` only wires `AddReverseProxy` +
   CORS) — no route has an `AuthorizationPolicy`, and auth is enforced
   downstream by each .NET module. So `/api/route/*` is reachable exactly
   like the existing public `/api/tiles/*`. The tileserver itself does **not**
   validate JWTs, so routing is effectively **open** today. Gating it
   requires a deliberate follow-up (see Auth-model risk below): either add
   authentication to the gateway and `RequireAuthorization` on this route, or
   JWT validation in the tileserver.
3. **k8s**: tileserver Deployment/Service + artifacts PVC + seed Job; wire
   into prod GitOps; smoke via the gateway.
4. **Flutter**: `features/routing` client + models + map rendering, against
   `/api/route/v1/*`.
5. **(later) .NET Routing module** for saved/shared routes (persist +
   delegate to tileserver), exposed at `/api/routing/*`.

## Risks & mitigations
- **Artifact size / deploy** → PVC + build Job, not in-image; version the
  artifacts.
- **Memory (mmap DEM) on a small node** → explicit requests/limits;
  possibly a dedicated node/taint.
- **Abuse / cost** → JWT-gate `/api/route/*` + gateway rate limit.
- **Contract drift** → freeze the public DTO + OpenAPI; the internal `Path`
  stays free to change behind the mapping layer.
- **Auth model** → confirm the gateway’s default authorization applies to
  the new `/api/route/*` route (it must require a valid app JWT).

## Repo state (the "pull")
`origin/main` was fetched. It is **26 commits ahead** of this branch (infra
GitOps: CNPG DB, observability, k8s modulith deploy; a new `Sharing`
module; Flutter photo-map/ocean layers) and this branch is **25 ahead**
(all the routing/UX/perf work from this session). A straight `git pull`
(merge) was NOT run because the working tree holds ~100 files of
pre-existing uncommitted WIP (`apps/api`, `apps/flutter`), 13 of which the
incoming commits also touch — a merge would clobber/conflict that WIP.
Recommended path: commit or stash that WIP, then merge `origin/main`, OR
rebase the session's routing commits onto `origin/main` on a clean tree.
