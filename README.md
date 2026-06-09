# Turbo

Monorepo for the Turkart product.

| Path | What it is |
| --- | --- |
| [`apps/flutter`](apps/flutter) | Flutter app (iOS / Android / web / desktop). The user-facing Turkart client. |
| [`apps/api`](apps/api) | .NET API — auth, geo, tracks, collections, activities, gateway. |
| [`apps/tileserver`](apps/tileserver) | Rust tile server — N50 ingest, multi-layer basemap MVT + style, routing, search. |
| [`apps/turbomap`](apps/turbomap) | wgpu map renderer (desktop now; Android/iOS/web by design). |
| [`infra/compose`](infra/compose) | Docker Compose stacks for local + CI. |
| [`infra/edge`](infra/edge) | Cloudflare Workers (R2 pull-through tile cache). |
| [`infra/k8s`](infra/k8s) | Kubernetes manifests. |
| [`infra/observability`](infra/observability) | Prometheus, Loki, OTel collector, Promtail configs. |
| [`infra/migrations`](infra/migrations) | Shared bootstrap SQL (per-service migrations live in `apps/api`). |
| [`infra/performance`](infra/performance) | k6 load-test suites. |
| [`infra/env`](infra/env) | Shared env files (`.env.shared`). |

## Quickstart

```sh
cd apps/api && dotnet restore Turboapi.sln && dotnet build Turboapi.sln
cd apps/flutter && flutter pub get
```

### Day-to-day dev — fast loop with hot reload

The fastest local-dev path is the modulith running natively under
`dotnet watch`, against a containerised Postgres + NATS. Backend hot-
reloads on save; Flutter hot-reloads on `r`.

```sh
# 1) infra: shared Postgres + NATS only (no .NET hosts in compose).
docker compose -f infra/compose/compose.databases.shared.yaml up -d

# 2) backend: modulith natively with hot reload. launchSettings points
#    at the shared Postgres on localhost:5432 and binds to port 5055.
cd apps/api
dotnet watch --project hosts/Turbo.Host.Modulith run

# 3) Flutter: hot reload is built in. r = reload, R = restart, q = quit.
cd apps/flutter
flutter run --dart-define=API_BASE_URL=http://localhost:5055
```

When `dotnet watch` can apply the change in-process it does so silently;
when it can't (entry-point edits, type changes), it rebuilds + restarts
the host. Either way you just save the file.

### Full-stack compose (no native processes)

Use these when you want the production-shaped deploy or you don't want
local SDK installs. Backend goes through the gateway at
`http://localhost:8080`; Flutter base URL is the same.

```sh
cd infra/compose

# Modulith on one shared Postgres. One .NET host, no NATS broker
# (events flow in-process).
docker compose --env-file ../env/.env.shared \
  -f compose.databases.shared.yaml -f compose.modulith.yaml up

# Microservices topology, one Postgres container per service (mirrors prod).
docker compose -f compose.yaml -f compose.services.yaml up

# Microservices topology on a single shared Postgres (lighter on resources).
docker compose --env-file ../env/.env.shared \
  -f compose.databases.shared.yaml -f compose.services.yaml up
```

Cold-boot any of the three and the log output is clean (zero
fail/warn/error lines); cross-flow smoke (register → login → POST an
activity) returns 200/201 end-to-end.

The activities module ships seven kinds (fishing / hiking / backcountry-ski /
xc-ski / packrafting / freediving + the cross-kind summary store) but they
all share one `activities` database and isolate themselves with Postgres
schemas owned internally by each module. Hosts see a single
`ConnectionStrings__Activities` regardless of topology.

## CI scoping

Workflows in `.github/workflows/` use `paths:` triggers so Flutter and API
pipelines only fire when their own directories change:

- `flutter_*` and `github_publish` — triggered by `apps/flutter/**`
- `api_*` — triggered by `apps/api/**` and the relevant `infra/**` subtrees

## Adding a new backend service

1. Scaffold under `apps/<service>/`.
2. Add per-service compose entries in `infra/compose/` and wire them into the umbrella `compose.yaml`.
3. Add a workflow under `.github/workflows/<service>_*.yml` with a `paths:` filter scoped to `apps/<service>/**`.
4. Update this README's table.

## Cloudflare Pages note

The Flutter app deploys via Cloudflare Pages. After the monorepo move, the
project's **Root directory** in the Pages dashboard must be set to
`apps/flutter` (or the build command needs the `cd apps/flutter` prefix kept
in `apps/flutter/.cloudflare/pages.toml`).

## History

Both halves of the monorepo were merged via `git subtree`, so commit history
from both repos is reachable from `main`. The pre-merge state is tagged
`monorepo-merge-pre-flight` in both archived repos.
