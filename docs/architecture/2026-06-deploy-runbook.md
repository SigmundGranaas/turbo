# Deploy Runbook — Self-Hosted N50 Basemap & Map Data

**Date:** 2026-06-10
**Companion to:** `2026-06-n50-basemap-implementation-plan.md`,
`infra/k8s/README-tileserver.md`, `infra/edge/tiles-worker/README.md`,
`apps/tileserver/docs/ingesting-n50.md`
**Audience:** whoever takes the built-and-tested stack to production.

Everything in the plan is built and code-validated; what's left is a
**sequenced deploy with real ordering hazards**. This is that sequence, with
the gotchas called out. Read §1 (gaps) before touching anything — several
manifest values are still routing-only and will silently 404 the basemap.

---

## 0. What gets deployed, and the topology

```
Flutter app ──► tiles.turkart.no  (Cloudflare Worker)
                  │  L1 edge cache → L2 R2 (turbo-tiles-cache) → ORIGIN
                  ▼
            kart-api.sandring.no  (Traefik ingress → tileserver Service:8080)
                  │  serves /v1/basemap, /v1/basemap/style.json, /v1/raster,
                  │  /v1/dem/rgb, /v1/slope, /fonts, /sprite, /v1/route, /admin
                  ▼
            tileserver (Rust)  ──►  tiles-db (CNPG PostGIS+pgRouting)
                  │                    └ N50 data, provisioned from Geonorge
                  └ norway.dem artifact on hostPath PV (hillshade/slope/Terrain-RGB)
```

Two independent data inputs:
- **PostGIS** (`tiles-db`) — N50 vector data. Populated by **provisioning**
  (download from Geonorge → restore → upsert). Drives basemap MVT, raster,
  routing, search.
- **`norway.dem`** — the elevation artifact on the hostPath PV. Drives
  Terrain-RGB, hillshade, and the slope overlay. Staged separately (rsync).

---

## 1. ⚠️ Config reconciliations REQUIRED before the basemap serves

The k8s manifest (`infra/k8s/base/tileserver.yaml`) was authored for the
**routing API only**. As-is it will serve `/api/route` but **404 every
basemap/style/font/slope request**. Fix these first or nothing downstream
works:

1. **Ingress exposes only `/api/route`.** The single ingress rule forwards
   `/api/route` (rewritten to `/v1/route`) to the tileserver. The public tile
   surface is unreachable. **Add ingress paths** (no rewrite) for:
   `/v1/basemap`, `/v1/raster`, `/v1/dem`, `/v1/slope`, `/fonts`, `/sprite`
   → tileserver Service:8080. (Or expose `/v1` wholesale + `/fonts` + `/sprite`;
   keep `/admin` gated/internal.)

2. **`PUBLIC_BASE_URL` is routing-prefixed.** It's
   `https://kart-api.sandring.no/api/route`. The style endpoint substitutes
   this into `{BASE_URL}`, so the served `style.json` would emit
   `…/api/route/v1/basemap/{z}/{x}/{y}.mvt` and `…/api/route/fonts/…` — wrong
   host, wrong prefix. **Set `PUBLIC_BASE_URL` to the public client origin =
   the Worker host**, `https://tiles.turkart.no`. Then the style emits
   `tiles.turkart.no/v1/basemap/…`, which the Worker caches and forwards to
   `ORIGIN` (`kart-api.sandring.no`) where the ingress (step 1) routes it.

3. **Worker `ORIGIN` must reach the tile paths.** `wrangler.toml` sets
   `ORIGIN=https://kart-api.sandring.no`. After step 1 the ingress routes
   `/v1/basemap` etc. there, so the Worker's pass-through works. Confirm the
   ingress host == `ORIGIN` host.

4. **Client base URL vs. Worker host.** The Flutter basemap/slope providers
   build URLs from `tileserverBaseUrlProvider`, whose prod default is
   `https://api.sandring.no/api/tiles` (the curated-paths gateway). The
   basemap/raster/slope/dem endpoints live behind the **Worker**
   (`tiles.turkart.no`), not that gateway. **Pass
   `--dart-define=TURBO_TILESERVER_URL=https://tiles.turkart.no`** at build so
   all `$base/v1/...` URLs hit the cached Worker host. (Curated-paths MVT also
   moves under the same host, or keep its own define — decide in §7.)

> These four are the load-bearing gotchas. Everything below assumes they're done.

---

## 2. Prerequisites (one-time)

- **Cluster**: the k3s node with the CNPG operator (already runs the modulith).
  Confirm disk headroom — see §4 sizing (national needs **~40 GB peak free**
  on the SSD/cache disk, plus the tiles-db PVC).
- **DNS**: `tiles.turkart.no` proxied (orange-cloud) at the `turkart.no` zone
  (Worker route); `kart-api.sandring.no` → the node (ingress).
- **Egress allowlist**: `*.geonorge.no` and `nedlasting.geonorge.no` must be
  reachable from the tileserver pod (provisioning downloads from there). The
  download client trusts both webpki + OS roots, so a TLS-intercepting proxy is
  fine, but the hostnames must be permitted.
- **Secrets** (sealed, per `infra/k8s/README-tileserver.md`):
  - `tiles-db-app` — CNPG app credential (`uri` key) for `DATABASE_URL`.
  - `auth-secrets` (`jwt-key`) — gates `/admin` only; **optional** now (the
    server boots public-only without it), but set it so the admin panel works.
- **Image**: `ghcr.io/sigmundgranaas/turbo-tileserver:latest` is built by CI on
  merge to main (`.github/workflows/tileserver_build.yml`). Nothing to do.
- **R2 bucket** (once): `wrangler r2 bucket create turbo-tiles-cache` then a
  30-day lifecycle rule (`infra/edge/tiles-worker/README.md`).

---

## 3. Deploy sequence (origin → data → edge → client)

The ordering matters: **data before the default flip; Worker before pointing
clients at its host; never a county provision over national.**

### Step A — Origin live (DB + tileserver)
`tiles-db.yaml` + `tileserver.yaml` are already referenced in
`infra/k8s/base/kustomization.yaml` (marked LIVE). With §1 done:
```sh
kustomize build infra/k8s/envs/prod | kubectl apply --dry-run=server -f -
```
Merge to the GitOps branch → ArgoCD syncs. Boot order: `tiles-db` ready →
tileserver migrates at boot (`AUTO_MIGRATE=true`) → serves.

### Step B — Stage the DEM artifact (hillshade / slope / Terrain-RGB)
The `norway.dem` (+ routing artifacts) are read from the hostPath PV at
`/mnt/tiles-artifacts`, **not pulled at boot**. rsync them onto the node per
`infra/k8s/README-tileserver.md` ("Staging the artifacts"). Without it,
`/v1/dem/rgb`, `/v1/slope`, and the raster hillshade return 503/flat — the
vector basemap still works.

### Step C — Provision the data (national)
Set on the tileserver Deployment (not in the routing-only manifest yet — add):
```yaml
- { name: TILESERVER_PROVISION_ON_BOOT,    value: "national" }
- { name: TILESERVER_PROVISION_REFRESH_SECS, value: "86400" }   # daily freshness
- { name: TILESERVER_INCOMING_DIR,         value: "/tmp/incoming" }
```
On a fresh (empty) DB the server provisions itself in the background at boot
(~30–45 min national, see §4). Watch the `provision-n50` job:
```sh
kubectl logs deploy/tileserver -f | grep provision
# or the admin panel "Provision data" / GET /admin/api/ingest/jobs?name=provision-n50
```
**Coverage guard**: once national is loaded, a stray county provision is
refused without `force` — good. **Never** set `PROVISION_ON_BOOT` to a county
in prod.

### Step D — Deploy the Worker (edge cache)
```sh
cd infra/edge/tiles-worker
# confirm wrangler.toml: ORIGIN=kart-api host, DATA_VERSION current, bucket name
wrangler deploy
```
Smoke (§5). Optionally warm z4–10 after a data version bump.

### Step E — Verify end to end (§5) before any client change.

### Step F — Client wiring + the default flip (§7) — **last, gated on D+data**.

---

## 4. Provisioning sizing & freshness (measured)

From the county scaling probe (`2026-06-automated-provisioning-plan.md` §
"Scaling probe"):

| | Oslo (03) | Innlandet (34) | **National (projected)** |
| --- | --- | --- | --- |
| Zip | 8.6 MB | 664 MB | ~5–7 GB |
| Rows | 28 k | 750 k | ~7–8 M |
| Wall-clock | ~15 s | ~3 min | **~30–45 min** |
| DB after | — | 2.2 GB | ~22–28 GB |
| Peak disk (zip+sql+staging+WAL) | — | — | **~35–40 GB** |

- **Freshness**: the daily refresh (`REFRESH_SECS`) re-checks the loaded area;
  it downloads + content-hashes and **skips restore/upserts when unchanged**
  (the expensive part), doing a full refresh only when Kartverket republishes.
  Hands-off.
- **Overviews**: provisioning rebuilds the `basemap.*_overview` matviews; no
  separate step.

---

## 5. Smoke tests (run at the matching step)

```sh
# Origin (Step A) — through the ingress host directly:
curl -fsS https://kart-api.sandring.no/healthz
curl -fsS https://kart-api.sandring.no/v1/basemap | jq '.vector_layers|length'   # 8

# After provisioning (Step C) — a real tile is non-empty:
curl -fsS -o /dev/null -w '%{size_download}\n' \
  https://kart-api.sandring.no/v1/basemap/12/2170/1189.mvt                        # >0

# Style resolves to the PUBLIC origin (Worker host), not /api/route:
curl -fsS https://kart-api.sandring.no/v1/basemap/style.json | jq '.glyphs, .sprite, .sources.n50.tiles[0]'
# → must be https://tiles.turkart.no/...  (proves §1.2 is fixed)

# Glyphs / sprites / slope:
curl -fsS -o /dev/null -w '%{http_code} %{size_download}\n' https://kart-api.sandring.no/fonts/DejaVu%20Sans/0-255.pbf
curl -fsS -o /dev/null -w '%{http_code}\n' https://kart-api.sandring.no/sprite.json
curl -fsS -o /dev/null -w '%{http_code}\n' https://kart-api.sandring.no/v1/slope/tiles/12/2170/1189.png   # 200 (DEM staged) or 503

# Edge (Step D) — cache tier reports its layer:
curl -sI https://tiles.turkart.no/v1/basemap/12/2170/1189.mvt | grep -i x-tiles-cache
# first hit: miss → same PoP again: edge → other PoP: r2
```

---

## 6. The data-version bump (R2 cache, on every data refresh)

R2 keys are prefixed with `DATA_VERSION`; tiles are served `immutable`. When
the basemap data changes (a real republish, or a style/cartography change that
alters tile bytes):
1. Bump `DATA_VERSION` in `infra/edge/tiles-worker/wrangler.toml`
   (e.g. `n50-2026.07`), `wrangler deploy` (vars-only, instant).
2. New key space → old keys orphan → 30-day lifecycle deletes them. **No
   purge.** Cold edge until re-warmed (optionally pre-warm z4–10).

> Note: today this is manual. The provisioning freshness loop refreshes the DB
> but does **not** auto-bump the Worker's `DATA_VERSION`. If you rely on the
> edge cache, bump the version when you know data changed, or shorten the edge
> `max-age` so stale tiles age out on their own. (Follow-up: emit the data
> version from the origin and have the Worker key on it.)

---

## 7. Client: wire the app, then flip the default

1. **Point the app at the cached host**: build with
   `--dart-define=TURBO_TILESERVER_URL=https://tiles.turkart.no`. The
   `TurboN50TopoConfig` (raster basemap), `TurboSlopeOverlayConfig`, and the
   curated MVT providers then resolve under the Worker.
2. **Soft-launch**: ship with `TurboN50Topo` *available but not default*
   (current state). Compare it against Norgeskart on real devices/regions.
3. **Flip the default** (`tile_registry.dart`: change the first-launch
   `toggleLocalLayer('topo')` → `'turbo_n50_topo'`) **only after** national
   data is loaded and parity holds. Flipping earlier points users at an
   empty/county-only map — worse than Norgeskart.
4. **Retire Norgeskart** once the flip is stable (keep it registered as a
   fallback for one release). This deletes the
   `cache.atgcp1-prod.kartverket.cloud` dependency — the headline goal.

The NVE steepness layer can be retired in favour of `TurboSlopeOverlayConfig`
the same way (keep NVE for runout/utløp; see the slope provider's note).

---

## 8. Rollback

- **Client**: the registry keeps Norgeskart + NVE registered; flip the default
  back (or unset the dart-define) — no server change needed.
- **Edge**: `wrangler rollback`, or point DNS off the Worker straight to
  `kart-api.sandring.no` (origin serves everything the Worker does).
- **Origin**: ArgoCD revert. The DB + DEM persist (Retain PV); a redeploy
  re-attaches them. Provisioning is idempotent + guarded.
- **Bad provision**: `provision-n50 --area national --force` re-restores from
  scratch; the coverage guard prevents accidental shrink.

---

## 9. Known gaps / open decisions (carried forward)

- **Manifest is routing-only** (§1) — the ingress paths + `PUBLIC_BASE_URL` +
  the provisioning env are the real edits needed before the basemap serves.
- **DATA_VERSION bump is manual** (§6) — fine for a slow-changing basemap;
  automate by threading the origin's `provision_state` version to the Worker.
- **Node capacity**: national DB (~25 GB) + DEM (~11 GB) + provisioning peak
  (~40 GB) on one small node. Confirm the SSD has room; the DEM is mmap'd so
  RAM isn't the limit, disk is.
- **Search / trail layers** (`ws.geonorge.no`, `wfs/wms.geonorge.no`,
  `overpass`) are **not** yet routed through our endpoints — that's the next
  dependency-removal milestone (M4), separate from this deploy.
- **Hillshade/slope tuning** wants a visual pass against real Norwegian relief
  once the DEM is on the node (constants in `slope.rs` / `HillshadeParams`).
- **Mobile vector rendering** (turbomap via FFI) is future; today mobile uses
  the raster fallback, web uses MapLibre GL + our style/glyphs/sprites.

---

## 10. The one-paragraph version

With §1 fixed: merge the GitOps branch (DB + tileserver go live), rsync
`norway.dem` onto the node, set `PROVISION_ON_BOOT=national` +
`REFRESH_SECS=86400` and let it self-populate (~40 min), `wrangler deploy` the
Worker, smoke-test through `tiles.turkart.no`, then build the app with
`TURBO_TILESERVER_URL=https://tiles.turkart.no`, soft-launch the N50 basemap,
and flip the default once parity holds — at which point the Kartverket
Norgeskart dependency is gone.
