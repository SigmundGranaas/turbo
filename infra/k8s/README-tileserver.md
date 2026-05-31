# Deploying the tileserver (routing API) to k8s

Stage 3 of the app-facing routing rollout. Everything is authored and
committed but **deliberately inert**: `base/tileserver.yaml` and
`base/tiles-db.yaml` are **commented out in `base/kustomization.yaml`**
(the "FLIP TO DEPLOY" block), so `kustomize build` ignores them and prod
is unchanged until you uncomment. The tileserver image is published
automatically by `.github/workflows/tileserver_build.yml` on merge to
main. Activation is the checklist below — do it when ready to go live.

## What it deploys

A single-replica Rust tileserver in **full mode** (connected to a tiles
Postgres; `--no-db` is deliberately **not** used). Routing (`/v1/route/*`)
is the first consumer, but we only make this available once **all the
data is in place** — both the tiles DB (provisioned + ingested) and the
on-disk artifacts. Exposed at `https://kart-api.sandring.no/api/route/*`
via a Traefik `ReplacePathRegex` middleware (`/api/route/<x>` →
`/v1/route/<x>`), since there is **no YARP gateway in k8s** (the gateway
is the compose/local proxy only).

## Prerequisites (do these before flipping)

1. **Push the ~14 GB Norway artifacts to R2** — into the existing
   (now-cleared) `turbo-db-backups` bucket under `artifacts/norway/`. Same
   R2 account as the CNPG backups, so the init container reuses
   `r2-backup-creds` (no new R2 secret). Commands in "Pushing the
   artifacts" below.

2. **Seal the two tiles-DB secrets.** `base/tiles-db.yaml` is a CNPG
   PostGIS cluster (`tiles-db`); the tileserver connects to it in full
   mode. You provide:
   - **`tiles-db-app`** — the CNPG app-role credential (basic-auth
     `username: app` + a password you choose), adopted by the cluster.
     Mirror `envs/prod/turbo-db-app-sealedsecret.yaml`.
   - **`tiles-db-secrets`** — key `connectionstring` =
     `postgres://app:<that-password>@tiles-db-rw:5432/tiles` (the
     tileserver reads `DATABASE_URL` from this).

     Seal each with kubeseal, e.g.:
     ```sh
     kubectl create secret generic tiles-db-secrets -n turbo-prod \
       --from-literal=connectionstring='postgres://app:<PWD>@tiles-db-rw:5432/tiles' \
       --dry-run=client -o yaml \
     | kubeseal --format yaml > infra/k8s/envs/prod/tiles-db-secrets-sealedsecret.yaml
     ```
   Routing reads the on-disk artifacts, not this DB — so it only needs to
   exist + migrate at boot. (Curated MVT tiles need data ingested into it;
   that's a separate step, not required for routing.)

3. **Confirm capacity.** The 11 GB DEM is mmap'd; a Norway-wide solve
   touches large regions. `tileserver.yaml` sets `requests.memory=1Gi`,
   `limits.memory=6Gi`. Verify a node can host that + the 20 Gi RWO
   artifacts PVC + tiles-db's 12 Gi (the plan flags a possible dedicated
   node/taint).

4. **Image — already automated.** `tileserver_build.yml` builds and pushes
   `ghcr.io/sigmundgranaas/turbo-tileserver:latest` on merge to main.
   Nothing to do beyond merging.

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

## Flip to deploy

Once 1–4 are done, uncomment the four "FLIP TO DEPLOY" lines:

- `base/kustomization.yaml` → `- tiles-db.yaml` and `- tileserver.yaml`
- `envs/prod/kustomization.yaml` → the two sealed-secret resources
  (`tiles-db-app-sealedsecret.yaml`, `tiles-db-secrets-sealedsecret.yaml`)
  **and** the `turbo-tileserver` image entry.

Validate before ArgoCD syncs:
```sh
kustomize build infra/k8s/envs/prod | kubectl apply --dry-run=server -f -
```
Merging that to the GitOps branch is what triggers the actual deploy.
Boot order: `tiles-db` comes up → tileserver init container pulls the
artifacts from R2 (first boot only, several minutes for 14 GB) → tileserver
connects, migrates, serves.

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
