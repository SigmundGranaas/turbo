# Kubernetes manifests (GitOps)

Turbo runs as a **single modular-monolith** (`turbo-modulith`) backed by **one
Postgres instance with a database per module** (auth, sharing, geo, tracks,
collections, activities). Events flow in-process — no NATS, no gateway, no
per-service pods. Deployment is **GitOps via ArgoCD**: nothing here is applied
by hand except the one-time bootstrap below.

## Layout

```
infra/k8s/
  base/                # namespace-agnostic: modulith, shared-db, web, ingress, config
  envs/
    prod/              # namespace turbo-prod: image tag, host, replicas, sealed secrets
  minikube/, dash/     # legacy / local helpers (not part of the GitOps flow)
argocd/
  root.yaml            # app-of-apps — apply once to bootstrap
  apps/turbo-prod.yaml # Application -> infra/k8s/envs/prod (namespace turbo-prod)
```

Adding an environment later (e.g. staging) is purely additive: copy
`envs/prod` → `envs/staging` (change `namespace:`, host, image tag), add
`argocd/apps/turbo-staging.yaml`, commit. The root app picks it up.

## Secrets (Sealed Secrets)

Secrets are encrypted with the cluster's sealed-secrets controller and
committed as `infra/k8s/envs/prod/sealed-secrets.yaml` — safe in git because
only the in-cluster controller can decrypt them.

One-time controller install:
```bash
helm repo add sealed-secrets https://bitnami-labs.github.io/sealed-secrets
helm upgrade --install sealed-secrets sealed-secrets/sealed-secrets \
  -n kube-system --set fullnameOverride=sealed-secrets-controller --wait
```

Generate / rotate the bundle (DB password + JWT key are generated; you supply
the Google OAuth creds and a GHCR pull token):
```bash
GOOGLE_CLIENT_ID=... GOOGLE_CLIENT_SECRET=... \
GHCR_USERNAME=sigmundgranaas GHCR_TOKEN=ghp_... \
./scripts/seal-prod-secrets.sh
git add infra/k8s/envs/prod/sealed-secrets.yaml && git commit -m "prod sealed secrets"
```

This produces sealed `db-secrets`, `auth-secrets`, `google-oauth-secrets`, and
the `ghcr-auth` pull secret, all scoped to `turbo-prod`.

## Bootstrap (once per cluster)

```bash
# 1. install sealed-secrets controller (above) and commit sealed-secrets.yaml
# 2. seed the app-of-apps; everything else is git-driven thereafter
kubectl apply -f argocd/root.yaml
```

ArgoCD then creates the `turbo-prod` namespace and syncs the modulith +
shared-db + ingress. See `prod-instructions.md` for the full cutover runbook.

## Validate locally

```bash
kubectl kustomize infra/k8s/envs/prod        # renders the full prod manifest set
```
