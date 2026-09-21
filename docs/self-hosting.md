# Self-hosting Oya Browser

Everything the wizard asks about, and everything it writes. For day-two operations,
multi-replica, governance, session lifecycle, roles and recovery, see
[control-plane.md](control-plane.md).

```bash
git clone https://github.com/OyadotAI/oya-browser.git
cd oya-browser
make wizard
```

```bash
git clone https://github.com/OyadotAI/oya-browser.git
cd oya-browser
make wizard
```

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

`make wizard ARGS="--dry-run"` shows the plan and writes nothing. Every answer is saved
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
| Control plane | Docker. Kubernetes and ECS are not wired into the wizard yet, [`k8s/`](../k8s) has manifests you can apply by hand. |
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
| `OPENAI_API_KEY` / `OPENAI_BASE_URL` / `CHAT_MODEL` | The deployment-wide LLM default. Any API key that sets its own overrides it. A private or `http://` base URL works here but is rejected from the dashboard, because tenants can set that field too. |
| `OYA_FLEET_RUNTIME` | `docker` or `k8s`, what starts a governed browser. |
| `OYA_MANAGED_*` | The governed runtime: network or NetworkPolicy, image, control URL, egress proxy. On Kubernetes the image must be digest-pinned; a tag can move between verification and scheduling. |
| `OYA_CLOUD_API_KEY` / `OYA_CLOUD_SNAPSHOT` / `OYA_PUBLIC_WS_URL` | Oya Cloud sandboxes, and the public URL they dial back to. |
| `OYA_RESIDENTIAL_PROXY_URL` | A residential vendor gateway every Oya Cloud browser uses unless its persona has its own proxy. Never sent to desktop browsers, which could extract the credentials. `{session}` and `{geo}` in the username become a sticky per-persona session and its country. Traffic is counted in the sandbox, both directions, and metered per key as `residential_proxy_bytes`. |

Health: `/livez` is liveness, `/readyz` is readiness. `/api/health` answers `ok`
unconditionally and is not a readiness probe.
