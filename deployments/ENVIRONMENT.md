# Environment variables

Every setting the Oya server reads, for all the deployments in this folder.

- **Set for you:** each deployment generates or sets these itself. You never write them; they're listed so you know what's there.
- **Optional:** put these in an `oya.env` file (one `KEY=value` per line; copy [`oya.env.example`](oya.env.example)) and deploy again:

| Deployment | How `oya.env` reaches the server |
|:---|:---|
| [`docker`](docker/) | `deployments/docker/oya.env`, loaded by Compose |
| [`ecs`](ecs/) | `deployments/ecs/oya.env` (or `--env-file`), stored as the Secrets Manager secret `<stack>/settings` |
| [`k8s`](k8s/) | `deployments/k8s/oya.env` (or `--env-file`), stored as the Kubernetes Secret `oya-settings` |
| [`gcloud`](gcloud/) | `deployments/gcloud/oya.env` (or `--env-file`), same as k8s |

Treat `oya.env` as a secret: it holds API keys. It's ignored by git. A value you set in `oya.env` never overrides one the deployment sets itself.

---

## Set for you

| Variable | Set by | What it is |
|:---|:---|:---|
| `API_KEYS` | all (generated on first deploy) | Comma-separated API keys the server accepts. Each key is its own tenant. `./deploy.sh keys` prints them. |
| `OYA_PROFILE_SECRET` | all (generated once, never rotated) | Encrypts cookies, tokens, TOTP seeds and proxy credentials at rest. **If it's lost or changed, everything stored becomes unreadable.** Back it up. |
| `OYA_STORAGE` | all (`postgres`) | Where every table lives. The templates set `postgres`; the server refuses `DATABASE_URL` without it. |
| `DATABASE_URL` | all | Postgres connection string. The schema is applied on every start (idempotent). |
| `OYA_CLUSTER_SECRET` | ecs, k8s, gcloud (generated) | Shared by replicas to sign requests they route to each other. |
| `OYA_INSTANCE_URL` | ecs, k8s, gcloud (from the task or pod IP) | This replica's own address, for routing between replicas. |
| `PORT` | image default `3100` | The port the server listens on. |
| `OYA_UI_MODE` | image default `production` | Serves the built console. |
| `OYA_PUBLIC_WS_URL` | all | Where browsers connect back to: `ws://control-plane:3100/ws` (docker), the Cloud Map name (ecs), the Service name (k8s, gcloud). Only browsers need to reach it. |
| `OYA_CLOUD_RUNTIME` | all | What runs each browser: `docker`, `ecs` or `k8s`. |
| `OYA_BROWSER_PROVIDER` | all: `oya-cloud` | So `oya.browser.start()` with no provider starts a browser on this deployment. A key can still pick another provider for itself. |
| `OYA_CLOUD_IMAGE` | docker, k8s, gcloud | The browser image, pinned by digest. |
| `OYA_CLOUD_DOCKER_NETWORK` | docker | `oya-browsers`, the network browsers share with the control plane and nothing else. |
| `OYA_CLOUD_DOCKER_PLATFORM` | docker | Set only when the browser image has no build for the host's architecture; browsers then run emulated. |
| `OYA_K8S_NAMESPACE` | k8s, gcloud | The namespace browser pods run in. |
| `OYA_ECS_CLUSTER`, `OYA_ECS_TASK_DEFINITION`, `OYA_ECS_SUBNETS`, `OYA_ECS_SECURITY_GROUPS`, `AWS_REGION` | ecs | Where browser tasks run. `OYA_ECS_AUTH` stays unset, so the control plane uses its task role. |
| `OYA_CLOUD_SANDBOX_TTL_MINUTES` | all (default `60`) | A browser stops itself this many minutes plus 10 after it starts. Change it with the deployment's option, not in `oya.env`. |

---

## Optional

Defaults are what the server uses when the variable is unset. For limits and quotas, `0` turns the limit off.

### LLM (the `ask()` agent and chat)

| Variable | Default | What it does |
|:---|:---|:---|
| `OPENAI_API_KEY` | none | The deployment-wide LLM key. Any API key can set its own in the console or SDK, which overrides this. Without one, `ask()` and chat are off for keys that don't bring their own. |
| `OPENAI_BASE_URL` | `https://api.openai.com/v1` | Any OpenAI-compatible endpoint (Anthropic, Gemini, a local vLLM or Ollama). Private addresses work here but are refused when a tenant sets them. |
| `CHAT_MODEL` | `gpt-4o-mini` | The model the agent uses. |
| `CHAT_MAX_ITERATIONS` | `200` | Most steps one agent run takes. |
| `OYA_LLM_TIMEOUT_MS` | `180000` | How long one model call may take. |
| `OYA_CLAUDE_EFFORT` | `high` | Reasoning effort for Claude models. |
| `OYA_AGENT_VERIFY` | on | `0` skips the check before a run reports done (saves one short model call per run). |
| `OYA_AGENT_LOG` | off | `1` logs each tool call and the start of its result. |
| `OYA_AGENT_LOG_CHARS` | `300` | How much of each call the agent log prints. |
| `OYA_PAGE_FORMAT` | the browser's setting | How pages are shown to the agent: `markdown`, `toon` or `jsonl`. |

### Access and operations

| Variable | Default | What it does |
|:---|:---|:---|
| `OYA_OPERATOR_TOKEN` | none | Bearer token for `/metrics`, `POST /api/operator/drain` and fleet provisioning. No API key can do these. Generate with `openssl rand -hex 32`. |
| `OYA_METRICS_TOKEN` | none | Lets a Prometheus scraper read `/metrics` without the operator token. |
| `FLEET_TOKEN` | none | One shared token any browser can enrol with; for fleets of desktop browsers. |
| `OYA_ALLOW_LEGACY_QUERY_KEYS` | allowed | `false` refuses API keys passed as `?token=` on WebSocket URLs (headers only). |
| `OYA_PAIRING_TTL_MS` | `300000` | How long a desktop pairing code lives. |

### Other browser providers and the CDP gateway

| Variable | Default | What it does |
|:---|:---|:---|
| `ANCHOR_API_KEY` | none | Enables Anchor as a provider. |
| `BROWSERBASE_API_KEY`, `BROWSERBASE_PROJECT_ID` | none | Enables Browserbase. |
| `STEEL_API_KEY` | none | Enables Steel. |
| `BROWSERUSE_API_KEY` | none | Enables Browser Use Cloud. |
| `OYA_CDP_WS_URL` | none | Your own Chrome's DevTools URL, for the `cdp` provider. |
| `OYA_BROWSER_PROVIDERS` | none | JSON overriding a provider's endpoint or response shape without a code change. |
| `OYA_PROVIDERS` | `[]` | JSON list of CDP backends the `/connect` gateway routes to. |
| `OYA_ROUTING_STRATEGY` | `priority` | How the gateway picks among them: `priority`, `round-robin`, `least-connections`, `latency`, `weighted`. |
| `OYA_ALLOW_PRIVATE_TARGETS` | `false` | Allows provider URLs on private addresses. Only where every API key is yours; cloud metadata stays blocked either way. |
| `OYA_SESSION_GRACE_MS` | `60000` | How long a session waits for a dropped client to come back. |
| `OYA_COMMAND_TIMEOUT_MS` | `60000` | How long one browser command may take. |
| `OYA_STUCK_COMMAND_MS` | `60000` | When a gateway command counts as stuck. |

### Personas and proxies

| Variable | Default | What it does |
|:---|:---|:---|
| `OYA_PERSONA_MAX_CONCURRENT` | `2` | Browsers one named persona may run at once. One persona is one device. |
| `OYA_DEFAULT_PERSONA_MAX_CONCURRENT` | unlimited | The same cap for each key's default persona. |
| `OYA_RESIDENTIAL_PROXY_URL` | none | A residential proxy gateway every cloud browser uses unless its persona has its own. `{session}` becomes a sticky per-persona id and `{geo}` its country. Metered per key. HTTP(S), not SOCKS5. |
| `OYA_RESIDENTIAL_PROXY_GEO` | none | The country when a persona has no hint, e.g. `US`. |
| `OYA_PROXY_CHECK_URL` | `https://api.ipify.org?format=json` | What a proxy check fetches to learn the exit IP. |
| `OYA_PROXY_CHECK_TIMEOUT_MS` | `10000` | How long a proxy check may take. |

### CAPTCHA

| Variable | Default | What it does |
|:---|:---|:---|
| `OYA_CAPTCHA_PROVIDER` | none | `capsolver` or `2captcha`, for browsers whose provider can't solve CAPTCHAs itself (Oya Cloud included). |
| `OYA_CAPTCHA_API_KEY` | none | That service's key. Without one, a CAPTCHA is reported as unsolved instead of blocking the run. |
| `OYA_CAPTCHA_TIMEOUT_MS` | `120000` | How long a solve may take. |

### Rate limits and quotas (per API key)

| Variable | Default | What it does |
|:---|:---|:---|
| `OYA_LIMIT_COMMANDS_PER_MIN`, `OYA_LIMIT_COMMANDS_BURST` | `600`, `120` | Browser commands. |
| `OYA_LIMIT_CHAT_PER_MIN`, `OYA_LIMIT_CHAT_BURST` | `60`, `10` | Chat and `ask()` requests. |
| `OYA_LIMIT_PROVISION_PER_MIN`, `OYA_LIMIT_PROVISION_BURST` | `20`, `20` | Browser starts. |
| `OYA_LIMIT_CONNECT_PER_MIN`, `OYA_LIMIT_CONNECT_BURST` | `120`, `60` | CDP gateway connections. |
| `OYA_LIMIT_AGENT_SIGNUPS_PER_DAY` | `3` | Self-service agent keys per caller address per day. |
| `OYA_QUOTA_MAX_BROWSERS` | `5000` | Browsers one key may hold at once. |
| `OYA_QUOTA_SANDBOXES_HOUR` | `500` | Cloud browsers one key may start per hour. |
| `OYA_QUOTA_CHAT_TOKENS_HOUR` | `2000000` | LLM tokens one key may use per hour; `false` turns it off. |

### Session recording (opt in per session with `?record=1`)

| Variable | Default | What it does |
|:---|:---|:---|
| `OYA_RECORD_MAX_FRAMES` | `1800` | Frames kept per recording. |
| `OYA_RECORD_MAX_BYTES` | `104857600` | Size cap per recording (100 MB). |
| `OYA_RECORD_QUALITY` | `40` | JPEG quality of recorded frames. |
| `OYA_RECORD_EVERY_NTH` | `5` | Keep one frame in this many. |
| `OYA_RECORDING_BUCKET` | none | A private Supabase Storage bucket for recordings. Needed for recordings once there's more than one replica. |

### Accounts and sign-in (Supabase)

Without these, sign-in is by API key and the console asks for one. The control plane's state stays in the `DATABASE_URL` Postgres either way; see [`docs/self-hosting.md`](../docs/self-hosting.md#databases).

| Variable | Default | What it does |
|:---|:---|:---|
| `SUPABASE_URL`, `SUPABASE_SERVICE_KEY` | none | A Supabase project for email, Google and GitHub sign-in. Sign-in only; requires `OYA_STORAGE=postgres` and `DATABASE_URL`, which holds all data. |
| `TURNSTILE_SITE_KEY`, `TURNSTILE_SECRET_KEY` | none | Cloudflare Turnstile on sign-in and sign-up. |

### Slack

| Variable | Default | What it does |
|:---|:---|:---|
| `SLACK_CLIENT_ID`, `SLACK_CLIENT_SECRET`, `SLACK_SIGNING_SECRET` | none | Your Slack app, so customers can connect alerts in one click. Without them, customers paste a bot token of their own. |
| `OYA_CONSOLE_URL` | from `OYA_PUBLIC_WS_URL` | The console's public address, for links in Slack messages. **Set this** in these deployments: `OYA_PUBLIC_WS_URL` is a private address. |

### Telemetry (all off unless set)

| Variable | What it does |
|:---|:---|
| `POSTHOG_KEY`, `POSTHOG_HOST` | Product analytics; both are needed. |
| `SLACK_OPS_WEBHOOK_SIGNUPS`, `SLACK_OPS_WEBHOOK_EVENTS` | One-line ops messages to Slack webhooks. |
| `SLACK_OPS_WEBHOOK_PRODUCT` | A multi-line card per product event worth reading (sign-ups with their method, a key's first browser, sessions of a minute or more with their length, playbooks, desktop installs and updates, server errors). |
| `RB2B_ID` | RB2B visitor identification on the public pages only. |

### Tuning

| Variable | Default | What it does |
|:---|:---|:---|
| `DATABASE_POOL_MAX` | `10` | Postgres connections per replica. Keep replicas × this under the database's limit. |
| `OYA_USAGE_FLUSH_MS` | `60000` | How often usage counters are written. |
| `OYA_INSTANCE_ID` | random per process | A stable replica id, if you need one; must be unique per process. |

### Other browser runtimes

These deployments run browsers on the platform they deploy to, so these usually stay unset. See [`docs/self-hosting.md`](../docs/self-hosting.md#cloud-browser-runtimes).

| Variable | What it does |
|:---|:---|
| `OYA_CLOUD_API_KEY`, `OYA_CLOUD_SNAPSHOT`, `OYA_CLOUD_API_URL`, `OYA_CLOUD_TARGET` | Daytona, with `OYA_CLOUD_RUNTIME=daytona`. |
| `OYA_ECS_AUTH`, `OYA_ECS_ROLE_ARN`, `OYA_ECS_EXTERNAL_ID`, `OYA_ECS_SSO_PROFILE`, `OYA_ECS_CONTAINER`, `OYA_ECS_ASSIGN_PUBLIC_IP` | ECS sign-in other than the task role (`iam`, `role`, `sso`) and task details. |
| `OYA_K8S_CONTEXT` | A kubeconfig context, when the control plane runs outside the cluster. |
| `OYA_FLEET_RUNTIME`, `OYA_MANAGED_*`, `OYA_EGRESS_*` | The governed fleet (`oya-selfhosted`, one browser per session behind an egress proxy). See [`docs/control-plane.md`](../docs/control-plane.md). |

### Do not change

| Variable | Why |
|:---|:---|
| `OYA_PROFILE_SALT` | Part of the key that decrypts stored credentials. Changing it makes them unreadable. |
| `OYA_DATA_DIR` | Local data directory; with `DATABASE_URL` it holds only caches. |
