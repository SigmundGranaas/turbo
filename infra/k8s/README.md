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
  app-config-patch.yaml      # SINGLE source of truth for all per-env values
  application-patch.yaml     # env→ConfigMap/Secret refs, no literals
  ingress.yaml               # host: is rewritten by kustomization.yaml
  kustomization.yaml         # image + replicas + replacements wiring
```

## Changing the domain

Every domain-derived value (Ingress host, FrontendUrl, CORS origins,
Google OAuth redirect, cookie domain) is consolidated in
`overlays/prod/app-config-patch.yaml`. The Deployment env reads them
all via `configMapKeyRef`, and `kustomization.yaml`'s `replacements:`
block copies `app-config.data.Domain__ApiHost` into the Ingress
`spec.rules[0].host`. There is nothing else in the manifests to edit.

To rehost the stack on `example.com`:

1. Edit the six `Domain__* / FrontendUrl / COOKIE_DOMAIN / CORS__* /
   Authentication__Google__RedirectUri` entries in
   `app-config-patch.yaml`.
2. Update the Google OAuth client's authorised redirect URI to match.
3. Commit. Argo CD will sync; the pod restarts pick up the new env via
   the ConfigMap change (a `kubectl rollout restart` may be needed if
   the ConfigMap reload is not configured to trigger one).

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
