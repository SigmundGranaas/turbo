# Deployment Guide for TurboAPI on K3s

## Prerequisites

- K3s cluster up and running
- kubectl configured to access your K3s cluster
- Access to GitHub Container Registry images

## Step 1: Create Secrets

Create the database secret with a secure password:

```bash
# Generate a random password
DB_PASSWORD=$(openssl rand -base64 16)

# Create the secret
kubectl create secret generic db-secrets \
  --from-literal=postgres-password=$DB_PASSWORD \
  --namespace default
  
# Generate a random JWT key
JWT_KEY=$(openssl rand -base64 64)

# Generate a secure password for Google OAuth (if needed)
# Replace these with your actual Google OAuth credentials
GOOGLE_CLIENT_ID="your-id"
GOOGLE_CLIENT_SECRET="your-secret"

# Create the auth secrets
kubectl create secret generic auth-secrets \
  --from-literal=jwt-key="$JWT_KEY" \
  --from-literal=google-client-id="$GOOGLE_CLIENT_ID" \
  --from-literal=google-client-secret="$GOOGLE_CLIENT_SECRET" \
  --namespace default

# Verify the secret was created
kubectl get secret auth-secrets
```

## Step 2: Update Domain Name

Edit the `k8s/overlays/prod/ingress.yaml` file to update your domain:

```yaml
spec:
  rules:
    - host: your-domain.com  # Replace with your actual domain
```

## Step 3: Deploy with Kustomize

Apply the Kustomize configuration:

```bash
kubectl apply -k k8s/overlays/prod/
```

## Step 4: Verify Deployment

```bash
# Check that pods are running
kubectl get pods

# Verify services
kubectl get svc

# Check the ingress
kubectl get ingress
```

## Troubleshooting

### Image Pull Issues

If you see `ImagePullBackOff` errors, you may need to set up registry credentials:

```bash
# If the secret already exists.    
kubectl delete secret ghcr-auth --namespace default

# Create a docker-registry secret for ghcr.io
kubectl create secret docker-registry ghcr-auth \
  --docker-server=ghcr.io \
  --docker-username=your-github-username \
  --docker-password=your-github-token \
  --namespace default

# Patch service accounts to use the credentials
kubectl patch serviceaccount default \
  -p '{"imagePullSecrets": [{"name": "ghcr-auth"}]}' \
  --namespace default
 
```



### Database Migration Issues

If the database migrations fail, check the logs of the init containers:

```bash
kubectl logs pod/prod-turboapi-auth-xxxx -c auth-db-migration
```

## Upgrading

When you want to upgrade to newer container images:

```bash
# Update to a specific tag
kubectl set image deployment/prod-turboapi-auth turboapi-auth=ghcr.io/sigmundgranaas/turboapi-auth:new-tag

# Or simply re-apply the kustomization to pull the 'latest' tag
kubectl apply -k k8s/overlays/prod/
```

## Dashboard
```bash
kubectl -n kubernetes-dashboard port-forward svc/kubernetes-dashboard-kong-proxy 8443:443
```