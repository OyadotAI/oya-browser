# Oya on one Docker host

The control plane, Postgres and Caddy (automatic HTTPS) under Docker Compose. Every browser is its own container, started by the control plane on a private network.

```
 clients ──HTTPS──▶ Caddy :443 ──▶ control plane ──▶ Postgres        (network: internal)
                                        │
                                        │ docker run (Docker socket)
                                        ▼
                               browser containers ──▶ ws://control-plane:3100/ws   (network: oya-browsers)
```

## 🚀 Deploy

On a Linux host with Docker (and Compose v2), from a checkout of this repository:

```bash
cd deployments/docker
./deploy.sh deploy --domain oya.example.com
```

It:
1. generates the secrets into `.env`;
2. builds the image from the checkout and pins the browser image by digest;
3. starts the stack and waits until it's healthy;
4. prints the URL and the API key.

Ports 80 and 443 must reach the host, and `oya.example.com` must resolve to it, so Caddy can get a Let's Encrypt certificate.

Without `--domain`, it serves `https://localhost` with Caddy's own certificate. That's for trying it out on a laptop.

Then check that it works end to end:

```bash
./deploy.sh smoke
# ==> Smoke test passed in 11s: started, connected, loaded a page, stopped
```

## 🧰 Commands

| Command | What it does |
|:---|:---|
| `./deploy.sh deploy [--domain D] [--browser-image REF] [--server-image REF]` | Deploy or update. Secrets and data are kept. `--server-image` uses a published Oya server image instead of building the checkout. |
| `./deploy.sh smoke` | Start a browser, load a page, stop it. |
| `./deploy.sh status` | The stack and any running browser containers. |
| `./deploy.sh keys` | The API keys. |
| `./deploy.sh logs [service]` | Follow the logs. |
| `./deploy.sh destroy [--volumes] [--yes]` | Stop everything. `--volumes` also deletes the database and all data. |

## ⚙️ Settings

`.env` is written by `deploy.sh` (secrets, image references and the domain); don't edit it. For optional settings, such as an LLM key, a residential proxy or a CAPTCHA solver, create `oya.env` here from [`../oya.env.example`](../oya.env.example) and deploy again. [`../ENVIRONMENT.md`](../ENVIRONMENT.md) lists every variable.

## 🔒 Security

- **The Docker socket is host root.** The control plane starts browsers through the host's Docker socket, and anything that controls that socket controls the host. Run Oya on a host (or VM) of its own, with nothing else on it.
- **Browsers are isolated.** They're only on the `oya-browsers` network: they reach the control plane and the internet, never Postgres or Caddy. Each container runs with all capabilities dropped, no privilege escalation, and limits of 256 processes and 2 GB of memory. It's removed when it stops.
- **Secrets.** `.env` holds the API key, the profile secret and the database password, and is readable only by its owner. Back up `OYA_PROFILE_SECRET` and the `oya_postgres-data` volume; without the secret, stored credentials can't be decrypted.
- **Architecture.** The browser image is built for `linux/amd64`. On an arm64 host, `deploy.sh` runs browsers under emulation and says so; use an amd64 host for production.

## 💾 Backups

```bash
docker compose --project-directory . --env-file .env exec postgres pg_dump -U oya oya > oya-$(date +%F).sql
```
