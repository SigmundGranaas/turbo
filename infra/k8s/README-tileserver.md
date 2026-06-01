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

1. **Stage the ~14 GB artifacts on the node** at `/mnt/tiles-artifacts`
   (rsync'd directly from your machine over the LAN — no R2). Commands in
   "Staging the artifacts" below.

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
   `limits.memory=6Gi`. Verify the node has room for `/mnt/tiles-artifacts`
   (~14 GB) plus tiles-db's ~12 Gi PVC, and ideally that `/mnt/tiles-artifacts`
   is on SSD/NVMe (the DEM is mmap'd and randomly read).

4. **Image — already automated.** `tileserver_build.yml` builds and pushes
   `ghcr.io/sigmundgranaas/turbo-tileserver:latest` on merge to main.
   Nothing to do beyond merging.

## Staging the artifacts (you run this once)

The tileserver reads the artifacts from a hostPath PV at
`/mnt/tiles-artifacts` on the node — they're **not** pulled at boot, so you
copy them onto the node directly from your machine over the LAN. Much
faster than an R2 round-trip (gigabit ≈ 2–4 min vs uploading 14 GB to the
cloud first).

On the node (the k3s VM), create the directory on the SSD/cache-backed
disk:
```sh
mkdir -p /mnt/tiles-artifacts
```

From your machine, rsync the artifacts the routing solver needs (skip the
`*.prewater.bak` backups):
```sh
rsync -av --progress \
  ~/turbo-artifacts/norway.{dem,mask,graph,graph_geom,vectors,anchors} \
  ~/turbo-artifacts/norway.*.health.json \
  <user>@192.168.1.210:/mnt/tiles-artifacts/
```

The tileserver container runs as non-root (uid 65532) and mounts the volume
read-only, so make the files world-readable:
```sh
# on the node
chmod -R a+rX /mnt/tiles-artifacts
ls -lh /mnt/tiles-artifacts        # expect norway.dem (~11G) + the rest
```

That's the only manual data step. At boot the container mmaps these files
(seconds — it does not read all 11 GB into RAM); pages fault in on demand
during solves. The data survives pod/PVC churn (the PV is `Retain` +
hostPath), so you only stage it once.

## Flip to deploy

Once 1–4 are done, uncomment the four "FLIP TO DEPLOY" lines:

- `base/kustomization.yaml` → `- tiles-db.yaml` and `- tileserver.yaml`
- `envs/prod/kustomization.yaml` → `- tileserver-artifacts-pv.yaml`, the two
  sealed-secret resources (`tiles-db-app-sealedsecret.yaml`,
  `tiles-db-secrets-sealedsecret.yaml`) **and** the `turbo-tileserver`
  image entry.

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
- **Seeding:** direct rsync onto the node + a hostPath PV (chosen — it's a
  single self-hosted k3s VM on the LAN, so a direct copy is faster and
  simpler than an R2 round-trip, and needs no extra secret). If the cluster
  ever goes multi-node or loses node access, switch back to an R2 init
  container or a `local` PV + nodeAffinity.
