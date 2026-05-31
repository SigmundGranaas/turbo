# Deploying the tileserver (routing API) to k8s

Stage 3 of the app-facing routing rollout. `base/tileserver.yaml` is a
**reviewed draft that is intentionally NOT referenced by
`base/kustomization.yaml`** — `kustomize build` ignores unreferenced
files, so nothing ships until you complete the prerequisites below and
wire it in. Do this only when ready to go live; the user gates the prod
deploy.

## What it deploys

A single-replica Rust tileserver in **full mode** (connected to a tiles
Postgres; `--no-db` is deliberately **not** used). Routing (`/v1/route/*`)
is the first consumer, but we only make this available once **all the
data is in place** — both the tiles DB (provisioned + ingested) and the
on-disk artifacts. Exposed at `https://kart-api.sandring.no/api/route/*`
via a Traefik `ReplacePathRegex` middleware (`/api/route/<x>` →
`/v1/route/<x>`), since there is **no YARP gateway in k8s** (the gateway
is the compose/local proxy only).

## Prerequisites (manual, before wiring in)

0. **Provision + ingest the tiles Postgres.** Full mode requires a
   reachable PostGIS database. Stand up a tiles DB (a dedicated CNPG
   cluster, mirroring `base/db.yaml`, or a database on the existing
   cluster), ingest the source data, and create a `tiles-db-secrets`
   sealed secret with a `connectionstring` key. This is the gate: routing
   is not made available until the DB **and** artifacts are both ready.

1. **Upload the ~14 GB Norway artifacts to R2.** They go into the
   **existing (now-cleared) `turbo-db-backups` bucket** under
   `artifacts/norway/` — the same R2 account as the CNPG backups, so **no
   new secret is needed** (the init container reuses `r2-backup-creds`).
   See "Pushing the artifacts" below for the exact commands.

2. **No new secret.** The init container reads `ACCESS_KEY_ID` /
   `ACCESS_SECRET_KEY` from the existing `r2-backup-creds`; the R2 endpoint
   is inline in the manifest. Nothing to seal.

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

## Pushing the artifacts (you run this once)

Everything is wired to pull from `turbo-db-backups/artifacts/norway/`. To
populate it, push your local `~/turbo-artifacts` there with the **same R2
credentials** that back `r2-backup-creds`.

What you need:
- **Endpoint:** `https://ab82d920d2b19b70985673b43e474dd8.r2.cloudflarestorage.com`
- **Bucket / prefix:** `turbo-db-backups` / `artifacts/norway/`
- **Access key id + secret:** your Cloudflare R2 access key (the same pair
  stored in the `r2-backup-creds` secret). If you don't have them to hand,
  mint a new R2 API token (S3-compatible) scoped to this bucket in the
  Cloudflare dashboard.

Configure an rclone remote once:
```sh
rclone config create r2 s3 \
  provider=Cloudflare \
  access_key_id=<YOUR_R2_ACCESS_KEY_ID> \
  secret_access_key=<YOUR_R2_SECRET> \
  endpoint=https://ab82d920d2b19b70985673b43e474dd8.r2.cloudflarestorage.com
```

Push the six artifacts the routing solver needs (skip the `*.prewater.bak`
backups):
```sh
rclone copy ~/turbo-artifacts/ r2:turbo-db-backups/artifacts/norway/ \
  --include 'norway.dem' --include 'norway.mask' \
  --include 'norway.graph' --include 'norway.graph_geom' \
  --include 'norway.vectors' --include 'norway.anchors' \
  --include 'norway.*.health.json' \
  --progress --transfers 4 --s3-chunk-size 64M
```

Verify:
```sh
rclone ls r2:turbo-db-backups/artifacts/norway/   # expect norway.dem (~11G) + the rest
```
That's the only manual data step — the init container does the pull on the
pod's first boot.

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
- **DB / curated tiles:** full mode means the tiles PostGIS (CNPG) +
  ingestion is a hard prerequisite (step 0), not a later add-on. Once it's
  up, curated MVT tiles can also be served from this same deploy.
- **Seeding:** R2 init-container vs an in-cluster build Job. The draft
  uses the R2 pull (practical: artifacts already build locally).
