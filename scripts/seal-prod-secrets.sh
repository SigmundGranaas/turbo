#!/usr/bin/env bash
# Generate the committed SealedSecret bundle for the turbo-prod namespace.
#
# Sealed secrets are encrypted with the cluster's sealed-secrets controller
# public key, so the output (infra/k8s/envs/prod/sealed-secrets.yaml) is safe
# to commit — only the controller in the cluster can decrypt it.
#
# You provide the genuinely-secret inputs via env vars; the DB password and
# JWT key are generated here (the cutover starts the DB empty, so rotating
# them is fine). Re-run + re-commit whenever a secret changes.
#
# Usage:
#   GOOGLE_CLIENT_ID=... GOOGLE_CLIENT_SECRET=... \
#   GHCR_USERNAME=sigmundgranaas GHCR_TOKEN=ghp_... \
#   ./scripts/seal-prod-secrets.sh
#
# Requires: kubeseal + kubectl on PATH, and access to the cluster (the
# sealed-secrets controller in kube-system).
set -euo pipefail

NS=turbo-prod
CONTROLLER_NS=kube-system
CONTROLLER_NAME=sealed-secrets-controller
OUT="$(cd "$(dirname "$0")/.." && pwd)/infra/k8s/envs/prod/sealed-secrets.yaml"

: "${GOOGLE_CLIENT_ID:?set GOOGLE_CLIENT_ID}"
: "${GOOGLE_CLIENT_SECRET:?set GOOGLE_CLIENT_SECRET}"
: "${GHCR_USERNAME:?set GHCR_USERNAME (GitHub username)}"
: "${GHCR_TOKEN:?set GHCR_TOKEN (GitHub PAT with read:packages)}"

# Generated (DB starts empty at cutover, so these can be fresh).
POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-$(openssl rand -base64 24 | tr -d '/+=' | head -c 32)}"
JWT_KEY="${JWT_KEY:-$(openssl rand -base64 64 | tr -d '\n')}"

conn() { echo "Host=shared-db;Port=5432;Database=$1;Username=postgres;Password=${POSTGRES_PASSWORD}"; }

seal() { kubeseal --controller-name "$CONTROLLER_NAME" --controller-namespace "$CONTROLLER_NS" --format yaml; }

{
  # db-secrets: postgres password + one connection string per module database
  kubectl create secret generic db-secrets -n "$NS" --dry-run=client -o yaml \
    --from-literal=postgres-password="$POSTGRES_PASSWORD" \
    --from-literal=connectionstring-auth="$(conn auth)" \
    --from-literal=connectionstring-sharing="$(conn sharing)" \
    --from-literal=connectionstring-geo="$(conn geo)" \
    --from-literal=connectionstring-tracks="$(conn tracks)" \
    --from-literal=connectionstring-collections="$(conn collections)" \
    --from-literal=connectionstring-activities="$(conn activities)" | seal
  echo "---"
  # auth-secrets: JWT signing key
  kubectl create secret generic auth-secrets -n "$NS" --dry-run=client -o yaml \
    --from-literal=jwt-key="$JWT_KEY" | seal
  echo "---"
  # google-oauth-secrets: OAuth client credentials (your Google Cloud project)
  kubectl create secret generic google-oauth-secrets -n "$NS" --dry-run=client -o yaml \
    --from-literal=google-client-id="$GOOGLE_CLIENT_ID" \
    --from-literal=google-client-secret="$GOOGLE_CLIENT_SECRET" | seal
  echo "---"
  # ghcr-auth: image pull secret for ghcr.io/sigmundgranaas/turbo-*
  kubectl create secret docker-registry ghcr-auth -n "$NS" --dry-run=client -o yaml \
    --docker-server=ghcr.io \
    --docker-username="$GHCR_USERNAME" \
    --docker-password="$GHCR_TOKEN" | seal
} > "$OUT"

echo "Wrote $OUT"
echo "Generated postgres password + JWT key (not printed). Commit the file:"
echo "  git add infra/k8s/envs/prod/sealed-secrets.yaml && git commit -m 'prod sealed secrets'"
