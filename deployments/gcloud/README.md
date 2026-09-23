# Oya on Google Cloud

GKE Autopilot runs the control plane and one pod per browser, and Cloud SQL holds the state. Access is managed through roles:
- **Kubernetes RBAC:** the control plane may only create and stop browser pods in its namespace.
- **A Google service account** bound through Workload Identity, whose only role is `roles/cloudsql.client`.

There are no keys or passwords for Google Cloud anywhere.

```
 clients ──HTTPS──▶ Google load balancer (managed certificate, TLS 1.2+)
                          │
                          ▼
     GKE Autopilot: control-plane pods ── Cloud SQL Auth Proxy sidecar ──▶ Cloud SQL (private IP, regional)
                          │  (Workload Identity → oya-control-plane@PROJECT)
                          ▼
                    browser pods (no token, NetworkPolicy)
```

`deploy.sh` creates the Google Cloud resources and then deploys [`../k8s`](../k8s) onto the cluster with a GKE component. That component adds the SQL proxy sidecar and the load balancer settings, plus the HTTPS Ingress when you pass `--domain`.

## 🚀 Deploy

```bash
cd deployments/gcloud
./deploy.sh deploy --project my-project --domain oya.example.com
```

The first run takes about 25 minutes, mostly the cluster and the database. It:

1. enables the APIs and reserves a private range, so Cloud SQL has only a private IP;
2. creates the Artifact Registry repository, then builds and pushes the image from this checkout;
3. creates the GKE Autopilot cluster;
4. creates the `oya-control-plane` Google service account (`roles/cloudsql.client`) and lets the control plane's Kubernetes ServiceAccount use it;
5. creates Cloud SQL for PostgreSQL 16:
   - private IP only, regional (with a standby);
   - daily backups and point-in-time recovery;
   - deletion protection on;
6. with `--domain`, reserves a global IP and a TLS 1.2+ policy;
7. runs `../k8s/deploy.sh` with the GKE component and waits for the rollout.

Point your domain's A record at the IP it prints. Google issues the certificate once DNS resolves, which can take up to an hour.

`--project` is always required, so it never deploys to whichever project `gcloud` has active.

Check it end to end:

```bash
./deploy.sh smoke --project my-project
```

## 🧰 Commands

| Command | What it does |
|:---|:---|
| `./deploy.sh deploy --project ID [options]` | Deploy or update; every step is safe to repeat. |
| `./deploy.sh smoke --project ID` | Start a browser, load a page, stop it. |
| `./deploy.sh status \| keys \| logs --project ID` | As in `../k8s`. |
| `./deploy.sh destroy --project ID [--yes]` | Delete everything it created, **including the database and its backups**. |

| Deploy option | Default | |
|:---|:---|:---|
| `--region` | `us-central1` | |
| `--name` | `oya` | Prefix for the cluster, database, registry and service account. |
| `--domain` | none | HTTPS with a managed certificate. Without it there's no public endpoint; reach it with `kubectl port-forward`. |
| `--replicas` | `2` | Control-plane replicas. |
| `--db-tier` | `db-custom-1-3840` | Cloud SQL machine (1 vCPU, 3.75 GB). |
| `--single-zone-db` | regional | No standby database; cheaper, not for production. |
| `--browser-image` | `ghcr.io/oyadotai/oya-browser:latest` | Pinned by digest. |
| `--env-file` | `oya.env` here, if present | Optional settings. See [`../ENVIRONMENT.md`](../ENVIRONMENT.md). |

## 🔐 Roles

| Identity | Has | Used for |
|:---|:---|:---|
| Kubernetes ServiceAccount `oya/oya-control-plane` | Role `oya-browser-launcher`: pods create, get, list, delete; secrets create | Starting and stopping browser pods. |
| Google service account `oya-control-plane@PROJECT` | `roles/cloudsql.client` | The Cloud SQL Auth Proxy sidecar, through Workload Identity. |
| Browser pods | nothing | No ServiceAccount token, and NetworkPolicy blocks the metadata server. |

## 🏭 Notes

- **Architecture.** Autopilot schedules on `linux/amd64` by default, which is what the browser image is built for. On an arm64 machine, the image build runs under emulation and is slower the first time.
- **Capacity.** Each browser is one Autopilot pod (250m CPU, 512 MiB requested). Autopilot bills per pod resource, so you pay for browsers only while they run.
- **Database password.** It's generated once, stored in Cloud SQL and in the `oya-secrets` Secret, and never printed.
