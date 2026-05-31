# Deploying the tileserver (routing API) to k8s

Stage 3 of the app-facing routing rollout. `base/tileserver.yaml` is a
**reviewed draft that is intentionally NOT referenced by
`base/kustomization.yaml`** — `kustomize build` ignores unreferenced
files, so nothing ships until you complete the prerequisites below and
wire it in. Do this only when ready to go live; the user gates the prod
deploy.

## What it deploys

A single-replica Rust tileserver serving **only** the curated routing API
(`/v1/route/*`) in `--no-db` mode (no tiles Postgres needed — routing
reads on-disk artifacts; MVT/DB endpoints 503 on demand). Exposed at
`https://kart-api.sandring.no/api/route/*` via a Traefik
`ReplacePathRegex` middleware (`/api/route/<x>` → `/v1/route/<x>`), since
there is **no YARP gateway in k8s** (the gateway is the compose/local
proxy only).

## Prerequisites (manual, before wiring in)

1. **Upload the ~14 GB Norway artifacts to R2.** From a machine with the
   built artifacts (e.g. `~/turbo-artifacts`):
   ```sh
   rclone copy ~/turbo-artifacts/ :s3:turbo-artifacts/norway/ \
     --include 'norway.dem' --include 'norway.mask' \
     --include 'norway.graph' --include 'norway.graph_geom' \
     --include 'norway.vectors' --include 'norway.anchors' \
     --include 'norway.*.health.json'
   ```
   (Skip the `*.prewater.bak` files.) The init container pulls these onto
   the PVC on first boot.

2. **Create the `r2-artifacts-creds` sealed secret** with keys
   `ACCESS_KEY_ID`, `ACCESS_SECRET_KEY`, `ENDPOINT` (the same R2 account as
   `r2-backup-creds` works; endpoint
   `https://ab82d920d2b19b70985673b43e474dd8.r2.cloudflarestorage.com`).
   Seal it like the others and add to `envs/prod/kustomization.yaml`.

3. **Confirm capacity.** The 11 GB DEM is mmap'd; a Norway-wide solve
   touches large regions. The draft sets `requests.memory=1Gi`,
   `limits.memory=6Gi`. Verify a node can host this (the plan flags a
   possible dedicated node/taint). Confirm the storage class honours a
   20 Gi RWO PVC.

4. **Add the image** to `envs/prod/kustomization.yaml` `images:`
   ```yaml
   - name: turbo-tileserver
     newName: ghcr.io/sigmundgranaas/turbo-tileserver
     newTag: latest
   ```
   and ensure CI builds/pushes that image (apps/tileserver/Dockerfile).

## Wire it in

Add to `base/kustomization.yaml` `resources:`:
```yaml
  - tileserver.yaml
```
Then `kustomize build infra/k8s/envs/prod | kubectl apply --dry-run=server -f -`
to validate before ArgoCD syncs.

## Smoke test (after sync)

```sh
curl -fsS https://kart-api.sandring.no/api/route/presets
curl -fsS -X POST https://kart-api.sandring.no/api/route/plan \
  -H 'content-type: application/json' \
  -d '{"points":[[15.371,67.398],[15.380,67.404]]}'
```

## Open decisions (carried from the plan)

- **Host:** drafted on `kart-api.sandring.no` (the live API host). The
  Flutter `routingBaseUrlProvider` and `curated_paths` currently disagree
  on host (`kart-api` vs `api.sandring.no`) — reconcile before GA.
- **DB / curated tiles:** this draft is routing-only. Serving curated MVT
  tiles in prod additionally needs a tiles PostGIS (CNPG) + ingestion —
  a separate effort.
- **Seeding:** R2 init-container vs an in-cluster build Job. The draft
  uses the R2 pull (practical: artifacts already build locally).
