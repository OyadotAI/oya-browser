# Deploying Oya

Production deployments of the Oya server (the **control plane**), where every browser runs as its own container, task or pod next to it. Each folder has a `deploy.sh` that builds the image from this checkout, deploys, and runs a smoke test.

| Deployment | Runs on | Browsers run as | Database | HTTPS | Access control |
|:---|:---|:---|:---|:---|:---|
| [`docker/`](docker/) | one Docker host | containers on a private network | Postgres container | Caddy, Let's Encrypt | the Docker socket (a dedicated host) |
| [`ecs/`](ecs/) | Amazon ECS on Fargate | Fargate tasks in the same cluster | RDS Postgres, Multi-AZ | ALB with your ACM certificate | IAM task role |
| [`k8s/`](k8s/) | any Kubernetes cluster | pods in the same namespace | your managed Postgres, or one in-cluster | your Ingress controller | ServiceAccount + RBAC Role |
| [`gcloud/`](gcloud/) | GKE Autopilot | pods in the same namespace | Cloud SQL (private IP), regional | Google-managed certificate | RBAC + Workload Identity |

What they share:

- **One image.** [`image/`](image/) adds migrations, `psql`, `kubectl` and a start script to the Oya server image. The control plane applies its database schema every time it starts.
- **The same guarantees.**
  - Browsers get no cloud credentials and can't be reached from outside.
  - Secrets are generated on the first deploy and kept on every redeploy.
  - Images are pinned by digest.
  - Every browser stops itself after its lifetime (TTL + 10 minutes; 60 + 10 by default).
- **The same settings.** Optional settings go in an `oya.env` file; [`ENVIRONMENT.md`](ENVIRONMENT.md) lists every variable, required and optional.
- **The same commands.** `deploy`, `smoke`, `status`, `keys`, `logs` and `destroy`.

After a deploy, use the SDK against it:

```ts
import { Oya } from '@oya-ai/browser';

const oya = new Oya({ baseUrl: 'https://oya.example.com', apiKey: process.env.OYA_API_KEY });
await using browser = await oya.browser.start(); // a browser container, task or pod on your deployment
await browser.goto('https://example.com');
```

## Choosing

- **Trying it out, or one team:** start with [`docker/`](docker/).
- **You're on AWS:** use [`ecs/`](ecs/). No Kubernetes to run, everything is IAM, and per-second billing per browser.
- **You already run Kubernetes:** use [`k8s/`](k8s/).
- **Google Cloud:** use [`gcloud/`](gcloud/), which creates everything and then applies `k8s/`.

## Requirements

`bash`, `docker`, `jq`, `openssl` and `git` everywhere. Plus:
- `aws` CLI v2 for `ecs/`;
- `kubectl` for `k8s/`;
- `gcloud` and `kubectl` for `gcloud/`.
