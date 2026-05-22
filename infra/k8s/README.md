# Kubernetes manifests — modulith topology

The cluster runs the modulith deployment: one `Turbo.Host.Modulith`
process binds every module (auth + geo + tracks + collections +
activities) against a single Postgres (PostGIS) pod. The host's startup
code creates each per-module database on first boot
(`MigrateModuleDatabaseAsync`), so no migration containers or
`init.sql` ConfigMap are needed.

## Layout

```
base/
  config.yaml       # app-config ConfigMap (env-agnostic)
  db.yaml           # turbo-db StatefulSet + PVC + Service (PostGIS)
  modulith.yaml     # turboapi-modulith Deployment + Service
  kustomization.yaml

overlays/prod/
  app-config-patch.yaml      # CORS, JWT, Google OAuth redirect
  application-patch.yaml     # JWT key, Google OAuth secrets, cookies
  ingress.yaml               # kart-api.sandring.no → turboapi-modulith
  kustomization.yaml         # pins image + replica count
```

## Deploying

The intended deployment path is **Argo CD** — see
[`../argocd/README.md`](../argocd/README.md) for the bootstrap steps.
Once Argo CD + Image Updater are in place, the merge-to-deploy loop is
fully automatic.

For a one-shot manual apply (no Argo CD), the kustomize is
self-contained:

```bash
# Create secrets first — see ../argocd/README.md step 3 for the exact
# kubectl create secret commands.
kubectl apply -k infra/k8s/overlays/prod/
```

## Compose mirror

The Docker Compose modulith topology in
`infra/compose/compose.modulith.yaml` matches this layout one-to-one:
one Postgres, one modulith host. The compose stack also wires a gateway
in front for local parity with the microservice deployment; the k8s
layout drops the gateway because the modulith host serves every route
itself.
