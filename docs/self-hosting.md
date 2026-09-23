# Self-hosting Oya Browser

Everything the wizard asks about, and everything it writes. For day-two operations,
multi-replica, governance, session lifecycle, roles and recovery, see
[control-plane.md](control-plane.md).

```bash
curl -fsSL https://raw.githubusercontent.com/OyadotAI/oya-browser/main/install.sh | sh
```

It needs git, Docker (running) and Node 20+, and tells you which one is missing. It
clones the repo into `~/oya-browser` (set `OYA_DIR` to change that, rerun it to update)
and starts the wizard there. Arguments pass through: `... | sh -s -- --dry-run`.
From a checkout you already have, `make wizard` does the same.

Six questions, then it writes the config, builds the images, brings the stack up and
waits for `/readyz` before telling you it worked. It ends by printing an API key.

```
? Where should the control plane run?   › Docker on this machine
? Which database?                       › SQLite      (zero config, one replica)
? Where should browsers run?            › Docker browser workers here
? Which LLM should agents use?          › Anthropic (Claude)
? Public URL of this control plane?     › http://localhost:3100
? Configure optional services now?      › No

✔ wrote .env
  waiting for /readyz… ready
✔ http://localhost:3100 is up.
```

`--dry-run` shows the plan and writes nothing. Every answer is saved
to `oya-install.json`, no secrets, so `make wizard ARGS="--config oya-install.json"`
reproduces the same deployment without prompting, which is the CI path. Credentials come
from the environment there.

The manual route still works if you would rather write `.env` yourself: copy
[`server/.env.example`](../server/.env.example) and run `docker compose up`.

## Try it

```bash
npm install -g @oya-ai/cli

# The wizard printed a key. Your shell may export OYA_API_KEY for the hosted
# service, and an exported value beats a saved one: so clear it for a local stack.
unset OYA_API_KEY
oya login --url http://localhost:3100 --key <the key it printed>

oya ls                             # the browser workers that enrolled
oya goto https://example.com       # drive one
oya status                         # health, commands, what it has been doing
```

Or open `http://localhost:3100` and sign in with the same key. `docker compose down`
stops everything; the `oya-data` volume holds your personas and cookies, so keep it.

## What you can choose

| | Options |
|:---|:---|
| Control plane | Docker. For production on one Docker host, Amazon ECS, Kubernetes or Google Cloud, see [`deployments/`](../deployments): each has a `deploy.sh`. |
| Database | SQLite, Supabase, or any Postgres (`DATABASE_URL`). |
| Browsers | Docker workers, governed Docker (one container per session), a Kubernetes fleet (one pod per session), Oya Cloud, Browserbase, Steel, Anchor, Browser Use, or your own Chrome over CDP. |
| LLM | Anthropic, OpenAI, Gemini, Gemini Enterprise (ex-Vertex AI), any OpenAI-compatible endpoint, or a local model (Ollama, vLLM, LM Studio). |

Options the wizard cannot yet finish are listed and dimmed rather than hidden, so the
menu never promises something that does not work.

## Databases

SQLite needs no setup and is the default, but it takes a writer lock, so one replica only.
Supabase or Postgres are what more than one replica requires. Pick Postgres in the
wizard and it applies the schema for you; to run them by hand, or against Supabase:

```bash
DATABASE_URL=postgres://user:pass@host:5432/oya make migrate
```

Migrations are tracked in `public.schema_migrations`, applied one transaction per file
together with their own bookkeeping, and safe to re-run. On plain Postgres the
accounts tables are skipped: there is no Supabase Auth there, so
authentication is `API_KEYS` and the dashboard takes an API key instead of an email.

## Configuration

Everything is optional except the secrets you want to survive a restart.
[`server/.env.example`](../server/.env.example) documents all of it; the ones that matter most:

| Variable | Description |
|:---|:---|
| `OYA_PROFILE_SECRET` | Key for encrypting cookies, tokens and TOTP seeds at rest (AES-256-GCM). Left unset, the server generates one into the data volume, so keep that volume, or every stored credential becomes unreadable. |
| `API_KEYS` | Comma-separated tenant keys. The whole control plane works with nothing but these. |
| `OYA_OPERATOR_TOKEN` | Bearer token for `/metrics`, fleet drain and host config. |
| `DATABASE_URL` | Postgres. Takes precedence over Supabase. |
| `SUPABASE_URL` / `SUPABASE_SERVICE_KEY` | Supabase storage, and the only backend with email sign-in. |
| `TURNSTILE_SITE_KEY` / `TURNSTILE_SECRET_KEY` | A Cloudflare Turnstile captcha on email sign-in and sign-up. The console shows the widget with the site key; the server checks each token with the secret, and lets requests through if Cloudflare cannot be reached. Unset means no captcha. Google and GitHub sign-in also need this deployment's `/auth/callback` URL in the Supabase project's redirect allowlist. |
| `OPENAI_API_KEY` / `OPENAI_BASE_URL` / `CHAT_MODEL` | The deployment-wide LLM default. Any API key that sets its own overrides it. A private or `http://` base URL works here but is rejected from the dashboard, because tenants can set that field too. |
| `OYA_FLEET_RUNTIME` | `docker` or `k8s`, what starts a governed browser. |
| `OYA_MANAGED_*` | The governed runtime: network or NetworkPolicy, image, control URL, egress proxy. On Kubernetes the image must be digest-pinned; a tag can move between verification and scheduling. |
| `OYA_CLOUD_API_KEY` / `OYA_CLOUD_SNAPSHOT` / `OYA_PUBLIC_WS_URL` | Oya Cloud sandboxes on Daytona, and the public URL every cloud browser dials back to. |
| `OYA_CLOUD_RUNTIME` | What runs Oya Cloud browsers: `daytona` (default), `docker`, `k8s` or `ecs`. See [Cloud browser runtimes](#cloud-browser-runtimes). |
| `OYA_RESIDENTIAL_PROXY_URL` | A residential vendor gateway every Oya Cloud browser uses unless its persona has its own proxy. Never sent to desktop browsers, which could extract the credentials. `{session}` and `{geo}` in the username become a sticky per-persona session and its country. Traffic is counted in the sandbox, both directions, and metered per key as `residential_proxy_bytes`. |
| `POSTHOG_KEY` / `POSTHOG_HOST` | Product analytics. Both must be set or nothing is sent; there is no default host. The key is a PostHog project write token, which the console shows to every visitor; never a personal PostHog key. |
| `SLACK_OPS_WEBHOOK_SIGNUPS` / `SLACK_OPS_WEBHOOK_EVENTS` | Slack incoming webhooks for one-line ops messages: signups, keys, desktop downloads and desktop connections in the first; saved playbooks, CDP attaches and server errors in the second. Unset means no message. |
| `RB2B_ID` | An RB2B account id. Loads RB2B's visitor identification on the public pages (landing, docs, sign-in, sign-up), never on the console or the live view. Unset means it never loads. |

## Cloud browser runtimes

Oya Cloud browsers (`provider: 'oya-cloud'`) run the image built from [`browser/Dockerfile`](../browser/Dockerfile) on one of four runtimes. Every runtime gets the same browser, labels and enrolment environment; the browser dials back to `OYA_PUBLIC_WS_URL` over the normal WebSocket, so no port is ever opened into it.

| `OYA_CLOUD_RUNTIME` | Settings | Stops an abandoned browser |
|:---|:---|:---|
| `daytona` | `OYA_CLOUD_API_KEY`, `OYA_CLOUD_SNAPSHOT` (a snapshot of the browser image), optional `OYA_CLOUD_API_URL`, `OYA_CLOUD_TARGET` | Idle stop after `OYA_CLOUD_SANDBOX_TTL_MINUTES` (default 60), hard TTL 10 minutes later |
| `docker` | `OYA_CLOUD_IMAGE`, optional `OYA_CLOUD_DOCKER_NETWORK`, and `OYA_CLOUD_DOCKER_PLATFORM` (e.g. `linux/amd64`) for an image with no build for the host's architecture. Uses the daemon the server's `docker` CLI reaches; pull the image first. | Hard lifetime of TTL + 10 minutes; the container is removed when it exits |
| `k8s` | `OYA_CLOUD_IMAGE`, optional `OYA_K8S_NAMESPACE` (default `oya-browsers`) and `OYA_K8S_CONTEXT`. One pod per browser; its credentials sit in a Secret the pod owns. | `activeDeadlineSeconds` of TTL + 10 minutes |
| `ecs` | `OYA_ECS_CLUSTER`, `OYA_ECS_TASK_DEFINITION` (whose container, `OYA_ECS_CONTAINER`, default `browser`, runs the image), `OYA_ECS_SUBNETS`, optional `OYA_ECS_SECURITY_GROUPS`, `OYA_ECS_ASSIGN_PUBLIC_IP=true`, `AWS_REGION`, and `OYA_ECS_AUTH` (below). One Fargate task per browser. | Hard lifetime of TTL + 10 minutes |

Only Daytona has an idle stop; the others stop at the hard lifetime whether idle or not.

**ECS authentication** (`OYA_ECS_AUTH`):

| Value | Settings | Signs in with |
|:---|:---|:---|
| `default` (unset) | none | The AWS SDK's default chain: env keys, `AWS_PROFILE` (SSO profiles included), or an instance, task or IRSA role. |
| `iam` | `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, optional `AWS_SESSION_TOKEN` | An access key. |
| `role` | `OYA_ECS_ROLE_ARN`, optional `OYA_ECS_EXTERNAL_ID` | The role, assumed from the default chain's identity and refreshed before it expires. |
| `sso` | `OYA_ECS_SSO_PROFILE` | IAM Identity Center, through a profile in `~/.aws/config`, after `aws sso login` on the server's host. |

**Per key.** A key can pick its own runtime, and for ECS its own account, through the SDK or `oya settings`:

```ts
await oya.config.set({
  sandbox_runtime: 'ecs',
  ecs: {
    cluster: 'browsers',
    taskDefinition: 'oya-browser',
    subnets: ['subnet-0abc'],
    securityGroups: ['sg-0abc'],
    assignPublicIp: true,
    region: 'us-east-1',
    auth: { type: 'role', roleArn: 'arn:aws:iam::123456789012:role/oya-browsers' },
  },
});
```

`ecs.auth` is one of:

- `{ type: 'iam', accessKeyId, secretAccessKey, sessionToken? }`
- `{ type: 'role', roleArn }`. The server assumes the role from its own AWS identity. The role's trust policy must trust the server's AWS account, with `sts:ExternalId` set to `config.get().ecs.externalId`. That value is generated per API key and can't be set, so one key can't make the server assume a role another customer created. It changes if the API key changes.
- `{ type: 'sso', accessToken, accountId, roleName, ssoRegion? }`. An IAM Identity Center access token, exchanged for that role's credentials as they expire. The token itself expires (typically 1 to 12 hours). After that, starts fail with `sso_token_expired` until a fresh token is set.

A key can't name an SSO profile, because that would use this host's own SSO sessions. `config.get()` returns `ecs` with its credentials masked. Sending it back unchanged keeps the stored credentials.

With `ecs.auth` set, the key's ECS browsers run on its own account, and none of the deployment's ECS or AWS settings are mixed in. A key that sets `daytona_api_key` runs on its own Daytona account in the same way. Without these, a key runs on the deployment's account, and any cluster it names is ignored. `docker` and `k8s` run on the deployment's own daemon or cluster, so a key can choose them but can never set their image, host, namespace or kubeconfig. On ECS the browser's environment, which includes the key's Oya API key, is visible to anyone in that AWS account with `ecs:DescribeTasks`, the same as a Daytona sandbox's env vars are to its account.

## Telemetry

The server sends nothing to anyone unless you set the variables below; the CLI and SDK never send anything, and the desktop app tells only the server it connects to its version (when it connects and when it checks for an update). With `POSTHOG_KEY` and `POSTHOG_HOST` both set, it reports product events to that PostHog: `account_signed_up`, `api_key_created`, `browser_started`, `browser_stopped`, `playbook_saved`, `playbook_replayed`, `mcp_tool_called`, `cdp_attached`, `desktop_connected` (with the app version), `desktop_updated`, `download_served`, `update_checked`, `persona_created` and `server_error`, each with a few properties such as provider, step count, platform, version or the error reference. Downloads and update checks are counted under a fingerprint of the address and user agent, never the address itself, and create no person. The console reports pageviews, time on page, and on public pages the clicks it tags (download and call-to-action buttons, by name only). With `SLACK_OPS_WEBHOOK_SIGNUPS` or `SLACK_OPS_WEBHOOK_EVENTS` set, it posts one Slack line per signup, key created, desktop download, first desktop connection, playbook saved, CDP attach and server error. Visited URLs, page content, cookies, API keys, key labels, persona and playbook names never leave the server this way; the sign-up email goes to PostHog's identify call and the Slack lines, nowhere else. Sending is best effort and never delays a request. Unset the variables and it stops.

Health: `/livez` is liveness, `/readyz` is readiness. `/api/health` answers `ok`
unconditionally and is not a readiness probe.
