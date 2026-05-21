## Install rancher desktop
https://docs.rancherdesktop.io/getting-started/installation
Remember this if you're using ubuntu: `sudo sysctl -w net.ipv4.ip_unprivileged_port_start=80`
Ingress resources will not work without this.

## Build images

Build the host images (one per microservice host plus the modulith and gateway):

```bash
docker build -t turboapi-auth:latest      -f ./hosts/Turbo.Host.Auth/Dockerfile .
docker build -t turboapi-geo:latest       -f ./hosts/Turbo.Host.Geo/Dockerfile .
docker build -t turboapi-activity:latest  -f ./hosts/Turbo.Host.Activity/Dockerfile .
docker build -t turboapi-modulith:latest  -f ./hosts/Turbo.Host.Modulith/Dockerfile .
docker build -t turboapi-gateway:latest   -f ./src/Gateway/Dockerfile .
```

No separate migration images — each host runs EF Core migrations
in-process at startup via `MigrateModuleDatabaseAsync`. To evolve the
schema:

```bash
dotnet ef migrations add <Name> --project src/<Module>/Turbo.<Module>.Infrastructure --context <Context>
```

Install the monitoring stack:

```bash
helm install prometheus prometheus-community/kube-prometheus-stack \
  -f prometheus-values.yaml \
  --namespace monitoring \
  --create-namespace
```

## Apply Kubernetes resources

```bash
kubectl apply -f kubernetes-resources.yaml
```

See `k8s/README.md` for the dedicated-vs-shared database deploy options.
