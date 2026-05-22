# Kubernetes Dashboard for k3s
  
  This README provides instructions for setting up and accessing the Kubernetes Dashboard on your k3s cluster using the latest Helm-based installation.
  
  ## Installation
  
  ```bash
  # Add kubernetes-dashboard repository
  helm repo add kubernetes-dashboard https://kubernetes.github.io/dashboard/
  
  # Deploy a Helm Release named "kubernetes-dashboard" 
  helm upgrade --install kubernetes-dashboard kubernetes-dashboard/kubernetes-dashboard \
  --create-namespace --namespace kubernetes-dashboard
  ```

## Accessing the Dashboard

After installation, you can access the dashboard using port-forwarding:
  
```bash
# Forward the Kong proxy service to your local machine
kubectl -n kubernetes-dashboard port-forward svc/kubernetes-dashboard-kong-proxy 8443:443
  ```

Then access the dashboard at: https://localhost:8443

## Creating a Dashboard User

To create a user with admin privileges and get a token:
  
  ```bash
  # Create a service account
  kubectl apply -f service-account.yaml
  
  # Generate a token (valid for 30 days)
  kubectl -n kubernetes-dashboard create token admin-user --duration=720h
  ```
  
  Copy the token output and use it to log in to the dashboard.

## Troubleshooting

If you can't access the dashboard:

1. Check if the pods are running:
  ```bash
  kubectl get pods -n kubernetes-dashboard
  ```

2. Check service status:
  ```bash
  kubectl get svc -n kubernetes-dashboard
  ```

3. Check for any events:
  ```bash
  kubectl get events -n kubernetes-dashboard
  ```

4. Check the logs:
  ```bash
  kubectl logs -n kubernetes-dashboard -l "app.kubernetes.io/name=kubernetes-dashboard"
  ```

## Uninstalling the Dashboard

If you want to remove the dashboard:
  
  ```bash
  # Uninstall the Helm release
  helm uninstall kubernetes-dashboard -n kubernetes-dashboard
  
  # Clean up the namespace
  kubectl delete namespace kubernetes-dashboard
  
  # Remove ClusterRoleBinding
  kubectl delete clusterrolebinding admin-user
  ```

## Additional Notes

- As of Dashboard v7.0.0+, only Helm-based installation is supported (manifest-based installation is no longer available)
- The dashboard now uses a multi-container setup with Kong as the gateway
- For more customization options, check the Helm chart values at [ArtifactHub](https://artifacthub.io/packages/helm/kubernetes-dashboard/kubernetes-dashboard)