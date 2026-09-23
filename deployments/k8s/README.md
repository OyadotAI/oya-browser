# Oya on Kubernetes

Control-plane replicas behind a Service, and one pod per browser in the same namespace. The control plane manages browsers through a ServiceAccount and an RBAC Role, and nothing else.

```
 Ingress ──▶ Service oya-control-plane ──▶ control-plane pods (N replicas, ServiceAccount oya-control-plane)
                      ▲                          │ create pod + Secret (Role oya-browser-launcher)
                      │                          ▼
                      └──── ws://oya-control-plane.<ns>.svc:3100/ws ◀── browser pods (no token, NetworkPolicy)
```

Works on any conformant cluster (1.29+) whose network plugin enforces NetworkPolicy, e.g. GKE Dataplane V2, EKS with the VPC CNI's network policies, Calico or Cilium. On Google Cloud, [`../gcloud`](../gcloud) creates the cluster and database and then runs this.

## 🚀 Deploy

```bash
cd deployments/k8s
./deploy.sh deploy --context my-cluster \
  --registry ghcr.io/acme/oya-control-plane \
  --database-url 'postgres://oya:PASSWORD@db.internal:5432/oya?sslmode=require' \
  --host oya.example.com --cluster-issuer letsencrypt
```

It:
1. builds the image from this checkout, pushes it to your registry and pins it by digest;
2. creates the namespace and the `oya-secrets` Secret (first deploy only);
3. applies the manifests and waits for the rollout;
4. prints the API key.

`--context` is always required, so it never deploys to whichever cluster happens to be current.

Try it without a database or an Ingress:

```bash
./deploy.sh deploy --context kind-oya --registry localhost:5001/oya --in-cluster-postgres
./deploy.sh smoke --context kind-oya     # through a port-forward
```

## 🧰 Commands

| Command | What it does |
|:---|:---|
| `./deploy.sh deploy --context CTX ...` | Deploy or update. Options below. |
| `./deploy.sh smoke --context CTX` | Start a browser, load a page, stop it, through a port-forward. |
| `./deploy.sh status --context CTX` | Control plane, database and browser pods. |
| `./deploy.sh keys --context CTX` | The API keys. |
| `./deploy.sh logs --context CTX` | Follow the control plane's logs. |
| `./deploy.sh destroy --context CTX [--delete-data] [--yes]` | Remove it. `--delete-data` also deletes the secrets and the in-cluster database volume. |

All commands take `--namespace NS` (default `oya`).

| Deploy option | Default | |
|:---|:---|:---|
| `--registry REPO` or `--image REF` | required | Build and push this checkout, or use an image you've already pushed. |
| `--platform P` | `linux/amd64` | What to build for. |
| `--database-url URL` or `--in-cluster-postgres` | required on first deploy | Your managed Postgres, or a single Postgres StatefulSet in the namespace. |
| `--host HOST` + `--tls-secret NAME` or `--cluster-issuer NAME` | no Ingress | An Ingress with TLS, from a Secret or from cert-manager. `--ingress-class` picks the class. |
| `--replicas N` | `2` | Control-plane replicas, spread across zones. |
| `--browser-ttl MIN` | `60` | A browser stops itself after MIN + 10 minutes. |
| `--browser-image REF` | `ghcr.io/oyadotai/oya-browser:latest` | Pinned by digest at deploy time. |
| `--env-file FILE` | `oya.env` here, if present | Optional settings, stored in the `oya-settings` Secret. See [`../ENVIRONMENT.md`](../ENVIRONMENT.md). |

## 🔐 What the control plane may do

From [`base/rbac.yaml`](base/rbac.yaml), in its own namespace only:

| Resource | Verbs | Why |
|:---|:---|:---|
| `pods` | create, get, list, delete | Start a browser, find it after a restart, list a key's browsers, stop it. |
| `secrets` | create | Each browser's credentials go in a Secret owned by its pod, so they're deleted with it. The control plane can never read a Secret back. |

Browser pods:
- run with no ServiceAccount token, no capabilities, no privilege escalation and the default seccomp profile;
- get a memory-backed `/dev/shm`;
- have a hard `activeDeadlineSeconds`.

[`base/network-policy.yaml`](base/network-policy.yaml) lets nothing connect in to them. Out, they reach only the control plane, cluster DNS and public addresses: never other pods, private ranges or the cloud metadata endpoint.

## 📦 What gets created

| Object | |
|:---|:---|
| Deployment, Service, PodDisruptionBudget `oya-control-plane` | The control plane; `POD_IP` gives each replica its address for routing between replicas. |
| ServiceAccount, Role, RoleBinding | The access above. |
| NetworkPolicies `oya-browsers`, `oya-control-plane` | Browser isolation; the control plane accepts only port 3100. |
| ConfigMap `oya-config-<hash>` | Runtime settings; a change rolls the pods. |
| Secret `oya-secrets` | API keys, profile and cluster secrets, database URL. Created once, kept on redeploys. |
| Secret `oya-settings` | Your optional settings, from `oya.env`. |
| StatefulSet `oya-postgres` (with `--in-cluster-postgres`) | One Postgres with a 10 GiB volume, reachable only from the control plane. |
| Ingress (with `--host`) | TLS, with one-hour timeouts for WebSockets (ingress-nginx annotations). |

## 🏭 Production notes

- **Database.** Use a managed Postgres with backups and failover; `--in-cluster-postgres` is a single pod.
- **Nodes.** The browser image is `linux/amd64`. On a cluster with arm64 nodes, keep browsers on amd64 nodes (for example with a default node selector for the namespace).
- **Capacity.** Each browser asks for 250m CPU and 512 MiB (limit 2 GiB), plus one pod IP. Size node pools, or the pod IP range, for your peak.
