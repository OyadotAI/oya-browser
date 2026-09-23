# Oya on Amazon ECS

The Oya control plane runs as a load-balanced ECS service with Postgres, and starts every browser as its own Fargate task in the same cluster. AWS access is managed entirely through IAM roles:

- **The control plane uses its task role.** There are no access keys in its configuration, in your code, or in the SDK.
- **Browsers get no task role.** They run arbitrary web pages, so they hold no AWS permissions at all.
- **Nothing on the internet reaches a browser or the database.** Browsers connect back to the control plane inside your VPC through a private DNS name.

This has been deployed and tested end to end with `deploy.sh`: fresh deploy, smoke tests across two replicas, a redeploy with changed settings, and destroy.

---

## 📐 Architecture

```
                                   your VPC (two availability zones)
 ┌───────────────────────────────────────────────────────────────────────────────────────┐
 │  public subnets                        private subnets (NAT gateway per zone)         │
 │  ┌──────────────────────┐   :3100     ┌──────────────────────────────┐                │
 │  │ Application Load     │ ──────────▶ │ control plane × 2            │ ──5432──▶ RDS  │
 │  │ Balancer, HTTPS      │             │ ECS service, task role       │   Postgres     │
 │  │ (TLS 1.2+, your ACM  │             └──────┬───────────────▲───────┘   Multi-AZ,    │
 │  │ certificate)         │     RunTask /      │               │           encrypted    │
 │  └──────────▲───────────┘     StopTask       ▼               │ ws://control.<stack>   │
 │             │                     ┌──────────────────────────┴──┐   .internal:3100/ws │
 │             │                     │ browser workers (Fargate)   │   (Cloud Map)       │
 │             │                     │ one task per browser        │                     │
 │             │                     │ no task role, no inbound    │ ──NAT──▶ the web    │
 │             │                     └─────────────────────────────┘                     │
 └─────────────┼─────────────────────────────────────────────────────────────────────────┘
               │ HTTPS
        SDK / API clients
```

What happens on `oya.browser.start()`:

1. The request reaches a control-plane replica through the load balancer.
2. The replica calls `ecs:RunTask` for the worker task definition, signed with its task role. The task is tagged with its owner (a hash of the API key) and gets the environment it needs to connect back.
3. Fargate starts the task in a private subnet, typically in 30 to 60 seconds.
4. The browser connects to `ws://control.<stack>.internal:3100/ws`. Any replica can accept it; replicas route requests for a browser to the one that holds it.
5. `browser.close()` (or the end of an `await using` block) calls `ecs:StopTask`. If the browser is never closed, it stops itself after its lifetime.

---

## 🚀 Deploy

Requirements: AWS CLI v2, Docker, `jq`, `openssl`, `git`, and a certificate for your domain in ACM in the same region.

```bash
cd deployments/ecs
./deploy.sh deploy --certificate-arn arn:aws:acm:us-east-1:123456789012:certificate/...
```

The first deploy takes about 20 minutes. It:

1. creates a VPC (`<stack>-network`: two zones, public and private subnets, a NAT gateway per zone), unless you pass your own with `--vpc-id`, `--public-subnets` and `--private-subnets`;
2. builds the image from this checkout, pushes it to a private ECR repository (immutable tags, scan on push) and pins it by digest;
3. pins the browser image by digest;
4. stores your optional settings (`oya.env`) in Secrets Manager;
5. deploys [`template.yaml`](template.yaml), generating the API key, the profile and cluster secrets, and the database password on the first deploy only;
6. waits until the service is stable, then prints the URL and the API key.

Then point your domain's CNAME at the load balancer (the `Url` output), and check it end to end:

```bash
./deploy.sh smoke
# ==> Smoke test passed in 36s: started, connected, loaded a page, stopped
```

Redeploy the same way to update. Secrets, data and settings you didn't change are kept, and ECS rolls the replicas one at a time, rolling back automatically if the new version doesn't become healthy.

### Options

| Option | Default | |
|:---|:---|:---|
| `--certificate-arn ARN` | required | ACM certificate for the load balancer. Plain HTTP is redirected to HTTPS. |
| `--http-only` | | Instead of a certificate, serve plain HTTP. For trying it out only. |
| `--stack NAME` | `oya` | Names everything. |
| `--region REGION` | your CLI's region, else `us-east-1` | |
| `--vpc-id`, `--public-subnets`, `--private-subnets` | a new VPC | Your own network. Private subnets need a NAT route; use at least two zones. |
| `--allowed-cidr CIDR` | `0.0.0.0/0` | Who may reach the load balancer. |
| `--replicas N` | `2` | Control-plane replicas. |
| `--db-class CLASS` | `db.t4g.small` | |
| `--single-az-db` | Multi-AZ | No standby database. Cheaper, not for production. |
| `--browser-image REF` | `ghcr.io/oyadotai/oya-browser:latest` | Pinned by digest at deploy time. |
| `--server-image REF` | build this checkout | A published Oya server image to build the deployment image on. |
| `--env-file FILE` | `oya.env` here, if present | Optional settings. See [`../ENVIRONMENT.md`](../ENVIRONMENT.md). |

### Other commands

| Command | What it does |
|:---|:---|
| `./deploy.sh smoke` | Start a browser, load a page, stop it. |
| `./deploy.sh status` | Stack state, URL, replicas and running browsers. |
| `./deploy.sh keys` | The API keys, from Secrets Manager. |
| `./deploy.sh logs` | Follow the control plane and worker logs. |
| `./deploy.sh destroy [--yes]` | Delete everything. The database is kept as a final snapshot, and the command prints how to delete it. |

All of them take `--stack` and `--region`.

---

## 🔐 IAM

### The control plane's task role

The Oya server leaves `OYA_ECS_AUTH` unset, so the AWS SDK signs in with the task role ECS gives the container:

| Statement | Allows | Why |
|:---|:---|:---|
| `RunOnlyTheWorkerInThisCluster` | `ecs:RunTask` on `<stack>-worker:*`, in this cluster only | It can start browsers and nothing else, anywhere else. |
| `ManageTasksInThisCluster` | `ecs:StopTask`, `DescribeTasks`, `ListTasks` in this cluster | Stop a browser, find it again after a restart, list a key's browsers. |
| `TagWorkersAtLaunch` | `ecs:TagResource` only as part of `RunTask` | Every browser is tagged with its owner. Tags are checked before any stop, so no API key can stop another key's browser. |
| `PassOnlyTheWorkerExecutionRole` | `iam:PassRole` on the worker execution role, to ECS only | `RunTask` hands ECS that role to pull the image. No other role can be passed, so no task with more permissions can be started. |

### The other roles

| Role | Used by | Allows |
|:---|:---|:---|
| `ControlPlaneExecutionRole` | ECS, starting the control plane | Pull its image, write logs, read the `<stack>/control-plane` and `<stack>/settings` secrets. |
| `WorkerExecutionRole` | ECS, starting a browser | Pull the browser image, write logs. |
| *(none)* | browser tasks | Nothing. Any task role would put a credential at `169.254.170.2`, next to the pages being browsed. |

### Prove it in your account

CloudTrail records who called `RunTask`. Allow a few minutes after a start:

```bash
aws cloudtrail lookup-events --lookup-attributes AttributeKey=EventName,AttributeValue=RunTask --max-results 5 \
  --query 'Events[].CloudTrailEvent' --output text | tr '\t' '\n' | \
  python3 -c "import json,sys; [print(e['userIdentity']['arn'], (e.get('requestParameters') or {}).get('startedBy'), e.get('errorCode') or '') for e in map(json.loads, filter(None, sys.stdin.read().split('\n')))]"
# arn:aws:sts::<account>:assumed-role/<stack>-ControlPlaneTaskRole-<id>/<task id> oya-browser-<browser id>
```

CloudTrail replaces the browser's environment in these events with `HIDDEN_DUE_TO_SECURITY_REASONS`.

### Using a separate launcher role

To keep the task role minimal and assume a dedicated role for launching browsers, put these in `oya.env`:

```
OYA_ECS_AUTH=role
OYA_ECS_ROLE_ARN=arn:aws:iam::<account>:role/oya-browser-launcher
```

Then:
- give that role the policy above, and a trust policy for the control plane's task role;
- give the task role only `sts:AssumeRole` on it.

---

## 🧱 What the stack creates

| Resource | Details |
|:---|:---|
| ECS cluster `<stack>` | Fargate, Container Insights on. |
| ECS service `control-plane` | `--replicas` tasks (1 vCPU, 2 GB) in private subnets. Rolling updates with automatic rollback. |
| Task definition `<stack>-worker` | The browser: 2 vCPU, 4 GB, x86_64, **no task role**. |
| Application Load Balancer | HTTPS (TLS 1.2+ policy), HTTP→HTTPS redirect, one-hour idle timeout for WebSockets, invalid headers dropped, health check on `/health`. |
| RDS PostgreSQL 16 | Private, encrypted, Multi-AZ, 7-day backups, deletion protection, final snapshot on delete. TLS is verified against the RDS certificate authority. |
| Secrets Manager `<stack>/control-plane` | API keys, profile secret, cluster secret, database URL. |
| Secrets Manager `<stack>/settings` | Your optional settings, from `oya.env`. |
| Cloud Map `<stack>.internal` | `control.<stack>.internal` resolves to the replicas; browsers connect back here. |
| Security groups | Load balancer: 80/443 from `AllowedCidr`. Control plane: 3100 from the load balancer, workers and itself. Database: 5432 from the control plane. Workers: no inbound. |
| IAM roles | Above. |
| Log group `/oya/<stack>` | 30 days. |
| ECR `<stack>/control-plane` | The control-plane image. |
| `<stack>-network` (unless you bring a VPC) | VPC, 2 public and 2 private subnets, 2 NAT gateways. |

---

## ⚙️ Settings

The template sets everything it needs itself (listed under "Set for you" in [`../ENVIRONMENT.md`](../ENVIRONMENT.md)). For optional settings, such as an LLM key, a residential proxy, a CAPTCHA solver or rate limits:

```bash
cp ../oya.env.example oya.env    # edit it
./deploy.sh deploy --certificate-arn ...
```

`deploy.sh` stores the file as the `<stack>/settings` secret, and the control plane restarts with it. Values never appear in the template or the task definition.

---

## 💵 Cost

Approximate us-east-1 on-demand prices at the time of writing (see AWS pricing for current rates):

| What | Approx. per month |
|:---|:---|
| Control plane, 2 × (1 vCPU, 2 GB, ARM64) | ~$58 |
| RDS `db.t4g.small`, Multi-AZ, 20 GB | ~$55 (about half with `--single-az-db`) |
| NAT gateways, 2 (plus data processed) | ~$66 |
| Application Load Balancer (plus usage) | ~$20 |
| Secrets Manager, Cloud Map, logs, ECR | a few dollars |
| **Each browser while it runs** (2 vCPU, 4 GB, x86_64) | **~$0.10 per hour**: a five-minute browser costs about one cent |

Bringing your own VPC saves the NAT gateways if you already have them.

---

## 🛠 Troubleshooting

| Symptom | Likely cause | Fix |
|:---|:---|:---|
| `deploy.sh` stops at "Waiting for the control plane to be healthy" | The new version isn't healthy; ECS rolls back by itself. | `./deploy.sh logs`: a database or settings error shows at startup. |
| `502 ... not authorized to perform: ecs:RunTask` | A worker task definition outside `<stack>-worker`, or a changed cluster. | Keep the family, or widen `RunOnlyTheWorkerInThisCluster`. |
| `502 ECS did not start the browser: RESOURCE:ENI` | The private subnets are out of addresses, or you've hit the account's ENI limit. | Bigger or more subnets. |
| A worker stops with `CannotPullContainerError` | No route to the registry. | Private subnets need a NAT route (or ECR/S3 VPC endpoints for private images). |
| `start()` times out and the worker is running | The worker can't reach the control plane. | The control plane's security group must allow 3100 from the worker group, and the VPC needs DNS hostnames on. The worker's log stream in `/oya/<stack>` shows its connection attempts. |

---

## 🔒 Security notes

- **The worker's API key.** Each browser receives the Oya API key it belongs to, so it can connect. CloudTrail hides it, but **anyone in the account with `ecs:DescribeTasks` on this cluster can read it** from the task's overrides. Limit that permission to the people who run Oya.
- **Settings and secrets** live only in Secrets Manager, readable by the control plane's execution role.
- **Back up the profile secret.** It's in `<stack>/control-plane`. It encrypts stored credentials; if it's lost or replaced, they can't be decrypted.
- **Revoking access.** Removing a permission from the task role takes effect on the next call.
