# Turbo

Monorepo for the Turkart product.

| Path | What it is |
| --- | --- |
| [`apps/flutter`](apps/flutter) | Flutter app (iOS / Android / web / desktop). The user-facing Turkart client. |
| [`apps/api`](apps/api) | .NET API — auth, geo, tracks, collections, activities, gateway. |
| `apps/tileserver` | Reserved for the upcoming custom tile server. |
| [`infra/compose`](infra/compose) | Docker Compose stacks for local + CI. |
| [`infra/k8s`](infra/k8s) | Kubernetes manifests. |
| [`infra/observability`](infra/observability) | Prometheus, Loki, OTel collector, Promtail configs. |
| [`infra/migrations`](infra/migrations) | Shared bootstrap SQL (per-service migrations live in `apps/api`). |
| [`infra/performance`](infra/performance) | k6 load-test suites. |
| [`infra/env`](infra/env) | Shared env files (`.env.shared`). |

## Quickstart

```sh
# Flutter app
cd apps/flutter
flutter pub get && flutter run

# .NET API
cd apps/api
dotnet restore Turboapi.sln
dotnet build  Turboapi.sln
```

### Local stack

Three deploy shapes share the same compose primitives. **Default for local
dev is the modulith** — one .NET process binds every module, no NATS,
fastest boot, easiest to debug.

```sh
cd infra/compose

# Recommended for local dev: modulith on one shared Postgres.
docker compose --env-file ../env/.env.shared \
  -f compose.databases.shared.yaml -f compose.modulith.yaml up

# Microservices topology, one Postgres container per service (mirrors prod).
docker compose -f compose.yaml -f compose.services.yaml up

# Microservices topology on a single shared Postgres (lighter on resources).
docker compose --env-file ../env/.env.shared \
  -f compose.databases.shared.yaml -f compose.services.yaml up
```

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
