# tiles-worker — the R2 pull-through cache tier

Cloudflare Worker fronting the self-hosted tileserver. **R2 is strictly a
distributed cache** — the system of record is PostGIS + the deterministic
build behind `kart-api.sandring.no`. Wiping the bucket loses nothing but
warmth; every object is regenerable from origin.

```
client → tiles.sandring.no (Worker)
           ├─ L1: edge Cache API (per-PoP)          x-tiles-cache: edge
           ├─ L2: R2 `turbo-tiles-cache`            x-tiles-cache: r2
           └─ origin: kart-api.sandring.no          x-tiles-cache: miss
                       └─ write-through → R2 + edge
```

## Cache-only guarantees (enforced by `test/worker.test.mjs`)

- Only idempotent GETs on the tile/style/font allowlist touch R2; routing,
  search, admin, and health proxy straight through, uncached.
- Keys are prefixed with `DATA_VERSION` (`n50-2026.06/v1/basemap/…`). A data
  rebuild bumps the var → new key space; old keys orphan and a **30-day R2
  lifecycle rule** deletes them. No purge required for correctness.
- Origin errors and empty bodies are never written to either tier.
- No build step uploads tiles to R2 — it fills lazily on miss. (An optional
  post-rebuild warm of z4–10 is an optimization, never a requirement.)

## Test

```sh
node --test test/worker.test.mjs   # plain node:test, no wrangler needed
```

## Deploy

1. Create the bucket + lifecycle rule (once):
   ```sh
   wrangler r2 bucket create turbo-tiles-cache
   wrangler r2 bucket lifecycle add turbo-tiles-cache --expire-days 30
   ```
2. DNS: `tiles.sandring.no` proxied (orange-cloud) at the `sandring.no` zone.
3. `wrangler deploy` from this directory.
4. On every basemap data rebuild: bump `DATA_VERSION` in `wrangler.toml`
   and redeploy (a vars-only deploy is instant).

## Smoke

```sh
curl -sI https://tiles.sandring.no/v1/basemap/12/2170/1189.mvt | grep x-tiles-cache
# first hit: miss → second hit (same PoP): edge → other PoP: r2
```
