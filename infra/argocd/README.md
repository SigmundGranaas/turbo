# Argo CD bootstrap

One-time setup to give a k3s cluster automatic merge-to-deploy for the
modulith. After this is in place a push to `main` that touches
`apps/api/**` builds a new image and Argo CD rolls the cluster forward
on its own — no `kubectl apply` needed.

## 1. Install Argo CD

```bash
kubectl create namespace argocd
kubectl apply -n argocd \
  -f https://raw.githubusercontent.com/argoproj/argo-cd/stable/manifests/install.yaml
```

The default web UI:

```bash
kubectl -n argocd port-forward svc/argocd-server 8080:443
# admin password:
kubectl -n argocd get secret argocd-initial-admin-secret \
  -o jsonpath='{.data.password}' | base64 -d
```

## 2. Install Argo CD Image Updater

Image Updater watches ghcr.io and writes new image digests back to
`infra/k8s/overlays/prod/kustomization.yaml`, so the build pipeline
doesn't need write access to `main`.

```bash
kubectl apply -n argocd \
  -f https://raw.githubusercontent.com/argoproj-labs/argocd-image-updater/stable/manifests/install.yaml
```

### Give Image Updater push access to the repo

`write-back-method: git` requires a credential. Create a GitHub personal
access token with `repo` scope (fine-grained: contents:write on this
repo), then:

```bash
kubectl -n argocd create secret generic git-creds \
  --from-literal=username=<github-user> \
  --from-literal=password=<pat>

# Tell Image Updater to use it for this repo
kubectl -n argocd patch configmap argocd-image-updater-config \
  --type merge -p '{"data":{"git.user":"<github-user>","git.email":"<github-user>@users.noreply.github.com"}}'
```

Argo CD itself also needs to know about the repo if you want it to
write through that secret:

```bash
kubectl -n argocd apply -f - <<EOF
apiVersion: v1
kind: Secret
metadata:
  name: turbo-repo
  namespace: argocd
  labels:
    argocd.argoproj.io/secret-type: repository
stringData:
  type: git
  url: https://github.com/SigmundGranaas/turbo
  username: <github-user>
  password: <pat>
EOF
```

(Skip this block if the repo stays public AND you set
`write-back-method: argocd` instead — but then tag bumps live only in
Argo CD's internal state, not in git, which defeats the GitOps
audit trail.)

## 3. App-level secrets (one-time, out-of-band)

Argo CD does **not** manage these — it would happily sync them into git
if it did. Create them by hand in the `default` namespace:

```bash
# Postgres password — used by the turbo-db StatefulSet AND fanned out
# into every ConnectionStrings__* env var on the modulith pod.
kubectl create secret generic db-secrets \
  --from-literal=postgres-password="$(openssl rand -base64 24)"

# JWT signing key + Google OAuth credentials for the auth module.
kubectl create secret generic auth-secrets \
  --from-literal=jwt-key="$(openssl rand -base64 64)" \
  --from-literal=google-client-id="<your-google-oauth-client-id>" \
  --from-literal=google-client-secret="<your-google-oauth-client-secret>"
```

If the ghcr packages are private, also create the registry pull secrets:

```bash
# For the modulith pod
kubectl create secret docker-registry ghcr-auth \
  --docker-server=ghcr.io \
  --docker-username=<github-user> \
  --docker-password=<pat-with-read:packages>
kubectl patch serviceaccount default \
  -p '{"imagePullSecrets":[{"name":"ghcr-auth"}]}'

# For Image Updater (in the argocd namespace)
kubectl -n argocd create secret docker-registry ghcr-creds \
  --docker-server=ghcr.io \
  --docker-username=<github-user> \
  --docker-password=<pat-with-read:packages>
```

If the ghcr packages are public, remove the
`argocd-image-updater.argoproj.io/modulith.pull-secret` annotation from
`infra/argocd/application.yaml`.

## 4. Register the application

```bash
kubectl apply -f infra/argocd/application.yaml
```

Argo CD will immediately sync `infra/k8s/overlays/prod` into the
`default` namespace.

## 5. Verify

```bash
kubectl -n argocd get applications
kubectl get pods                               # turbo-db + turboapi-modulith
kubectl logs deploy/turboapi-modulith -f       # watch the 11 EF Core migrations on first boot
kubectl get ingress
```

The first boot runs 11 migrations against a fresh Postgres, so the
startup probe is given five minutes before failing. Steady-state restarts
take a few seconds.

## How a new release actually rolls out

1. Merge to `main` touching `apps/api/**`.
2. `.github/workflows/api_image_build.yaml` builds and pushes
   `ghcr.io/sigmundgranaas/turboapi-modulith:latest` (and a
   sha-prefixed tag).
3. Image Updater polls ghcr.io every ~2 min, sees a new digest under
   `latest`, edits `overlays/prod/kustomization.yaml`'s `newTag`, and
   commits to `main`.
4. Argo CD detects the commit, re-renders kustomize, applies, and
   rolls the modulith Deployment.
