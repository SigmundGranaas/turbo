# tileserver

Self-owned curated paths + tiles API for the Turbo Norwegian outdoor app.
Replaces the Flutter app's live Kartverket WFS pulls with a topology-first
PostGIS store, MVT and GeoJSON endpoints, routing, and an admin panel.

See `../../docs` and `/root/.claude/plans/plan-this-out-in-quirky-whistle.md`
for the full architecture.

## Layout

```
apps/tileserver/
  Cargo.toml                  # workspace
  migrations/                 # sqlx-embedded, run at startup
  crates/
    turbo-tiles-core/         # domain types, no I/O
    turbo-tiles-db/           # pool, migrations
    turbo-tiles-mvt/          # ST_AsMVT + GeoJSON queries
    turbo-tiles-auth/         # JWT validation (HS256, shared with .NET)
    turbo-tiles-routing/      # pgRouting cost functions (M5)
    turbo-tiles-ingest/       # FKB / Turbase / DNT / DTM10 jobs
    turbo-tiles-api/          # public /v1 router
    turbo-tiles-admin/        # HTMX + Askama /admin (M3-M4)
    turbo-tiles-bin/          # `tileserver serve` and `tileserver ingest`
```

## Run locally

```bash
docker compose -f ../../infra/compose/compose.services.yaml up tiles-db tileserver
```

This starts a PostGIS+pgRouting database on port 5446 and the server on
port 8090. Migrations run automatically at startup.

## Commands

```bash
tileserver serve            # HTTP server (default)
tileserver ingest --job fkb-sti
tileserver migrate          # apply migrations and exit
```

## Endpoints (V1)

```
GET  /healthz
GET  /readyz
GET  /v1/catalog
GET  /v1/{resource}/tiles/{z}/{x}/{y}.mvt
GET  /v1/{resource}?bbox=w,s,e,n&limit=500
GET  /v1/{resource}/{id}
POST /v1/routing/route          # 501 until M5
POST /v1/routing/isochrone      # 501 until M5
GET  /v1/routing/profiles
GET  /admin                     # cookie/JWT-gated, role=curator
```

`{resource}` is one of `hiking-trails`, `ski-tracks`, `forest-roads`,
`cycling-routes`.

## Required env

```
DATABASE_URL    # postgres://user:pass@host:port/db
JWT_SECRET      # shared with apps/api JwtConfig.Key (HS256)
JWT_ISSUER      # optional, matches apps/api
JWT_AUDIENCE    # optional, matches apps/api
PUBLIC_BASE_URL # used to build catalog URL templates
BIND            # default 0.0.0.0:8080
```
