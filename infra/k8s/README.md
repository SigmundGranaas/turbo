# Kubernetes manifests

## Database deployment: dedicated vs shared

The persistence layer is configurable. Pick one of:

### Dedicated databases (default)

Each module gets its own Postgres pod and PVC. Edit `base/kustomization.yaml`
so it includes `databases.yaml` (the current default). Connection-string
secrets stay as the per-module hosts and ports
(`auth-db:5432`, `geo-db:5435`, `activity-db:5436`).

```bash
kubectl kustomize k8s/base | kubectl apply -f -
```

### Shared database

A single PostGIS pod hosts `auth`, `geo`, and `activity` as logical
databases. Edit `base/kustomization.yaml` so it includes
`databases.shared.yaml` *instead of* `databases.yaml`. The shared file
declares Services named `auth-db`, `geo-db`, and `activity-db` that all
alias the single `shared-db` Pod, so the application Deployments don't
care which variant is active.

`db-secrets` must use port `5432` for all three connection strings in this
variant (the dedicated variant uses `5435`/`5436` for `geo`/`activity`):

```yaml
stringData:
  connectionstring-auth: "Host=auth-db;Port=5432;Database=auth;Username=postgres;Password=yourpassword"
  connectionstring-geo: "Host=geo-db;Port=5432;Database=geo;Username=postgres;Password=yourpassword"
  connectionstring-activity: "Host=activity-db;Port=5432;Database=activity;Username=postgres;Password=yourpassword"
```

```bash
kubectl kustomize k8s/base | kubectl apply -f -
```

## Compose mirror

The Docker Compose layout follows the same dedicated/shared split — see
`compose.yaml` (default, dedicated) vs `compose.databases.shared.yaml` +
`.env.shared` at the repo root.
