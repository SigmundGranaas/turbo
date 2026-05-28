# Production deployment & cutover runbook

Prod is GitOps-managed by ArgoCD. Normal changes = commit to `main`; ArgoCD
syncs `infra/k8s/envs/prod` into the `turbo-prod` namespace automatically.
This doc covers the one-time bootstrap and the cutover from the legacy
3-microservices stack (in `default`) to the modulith (in `turbo-prod`).

## Prerequisites

- `kubectl` pointing at the k3s cluster (context `k3s`, server `192.168.1.210:6443`).
- sealed-secrets controller installed and `infra/k8s/envs/prod/sealed-secrets.yaml`
  generated + committed (see `README.md`).
- The `turbo-modulith` image published to `ghcr.io/sigmundgranaas/turbo-modulith`
  (CI: `.github/workflows/api_image_build.yaml`).

## Bootstrap

```bash
kubectl apply -f argocd/root.yaml          # app-of-apps; creates turbo-prod + syncs
kubectl -n argocd get applications         # turbo-root, turbo-prod -> Synced/Healthy
kubectl -n turbo-prod get pods             # shared-db + turbo-modulith Running
```

The modulith runs every module's EF Core migrations in-process at startup, so
no migration Jobs are needed. First boot is slower (readiness gives it ~60s).

## Verify

```bash
kubectl -n turbo-prod logs deploy/turbo-modulith | grep -i migrat   # migrations applied
curl -fsS https://kart-api.sandring.no/api/auth/...                  # via ingress
kubectl -n turbo-prod exec deploy/shared-db -- psql -U postgres -l   # auth/geo/tracks/... present
```

## Cutover from the legacy stack (DESTRUCTIVE — data is nuked)

The old per-service databases are **not** migrated; the shared-db starts empty
(existing users must re-register). Run only when ready to take the API over.

```bash
# 1. Stop ArgoCD managing the old stack
kubectl -n argocd delete application turboapi          # legacy imperative app

# 2. Bring up the new stack (if not already)
kubectl apply -f argocd/root.yaml

# 3. Once turbo-prod is Healthy and the ingress serves kart-api.sandring.no,
#    delete the legacy resources in the default namespace:
kubectl -n default delete deploy turboapi-auth turboapi-geo turboapi-activity \
                                 auth-db geo-db activity-db nats
kubectl -n default delete svc  turboapi-auth turboapi-geo turboapi-activity \
                                 auth-db geo-db activity-db nats \
                                 prod-turboapi-auth prod-turboapi-geo prod-turboapi-activity \
                                 prod-auth-db prod-geo-db prod-activity-db prod-kafka
kubectl -n default delete pvc auth-db-pvc geo-db-pvc activity-db-pvc \
                                 prod-auth-db-pvc prod-geo-db-pvc prod-activity-db-pvc
kubectl -n default delete secret db-secrets auth-secrets google-oauth-secrets ghcr-auth prod-db-secrets
```

Both ingresses target the same node IP; the new `turbo-ingress` in `turbo-prod`
takes over once the old one is gone. Confirm DNS/Traefik routing after deletion.

## Rollback

The legacy stack is plain Deployments; until the step-3 deletions run, the old
pods keep serving. To revert before cutover, delete the `turbo-prod`
Application (`kubectl -n argocd delete application turbo-prod`). After the
destructive deletions there is no rollback (data is gone) — redeploy from git.
