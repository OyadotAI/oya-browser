# Contributing

Thanks for looking. Bug reports, fixes and new backends are all welcome.

## Before you start

- **Security problems** go through GitHub's private advisory form
  ([Report a vulnerability](https://github.com/OyadotAI/oya-browser/security/advisories/new)), never a public issue.
- **Large changes** (a new provider, a new storage backend, a change to the
  persona model): open an issue first so we can agree on the shape.
- **Licensing.** The SDK (`packages/sdk`) and CLI (`packages/cli`) are MIT.
  Everything else is under the [Sustainable Use License](LICENSE.md). By
  opening a pull request you agree your contribution is licensed the same way
  as the directory it lands in.

## Setup

Node 22+ and npm 10+.

```bash
git clone https://github.com/OyadotAI/oya-browser.git
cd oya-browser
npm run setup          # root, server, ui and browser dependencies
npm test               # server suite + CLI suite, no credentials needed
```

The test suite is hermetic: it runs with no database, no cloud keys and no
network: `server/tests/support/hermetic.js` keeps `.env` from loading and points
state at a scratch directory. If a test of yours needs a service, fake the seam
(`server/tests/unit/support/fakes.ts`); `server/tests/integration/providers.test.js`
shows mocking `fetch`.

Some tracked tests are deliberately *not* reachable from `npm test`, because
they need something CI does not have. Run them by hand when you touch that area:

| Test | Needs |
|:---|:---|
| `server/tests/integration/stealth.test.js` | a real Chrome, and the public detector sites |
| `server/tests/integration/control-postgres.test.js` | a Postgres at `DATABASE_URL` |
| `server/tests/integration/control-supabase.test.js` | a live Supabase project |
| `browser/tests/integration/cdp-front-door.mjs` | a running browser container and `playwright-core` |
| `ui/tests/*.spec.ts` | Playwright and a running stack (`npm run test:ui`) |

Run the live-service suites with `OYA_TEST_LIVE=1` so the hermetic preload lets
`server/.env` through.

`browser/tests/integration/regressions.js` and the browser's unit tests
(`browser/tests/unit/`) need neither Electron nor a display and do run in CI
(`npm test --prefix browser`).

Running the stack locally:

```bash
make dev               # API + Next.js console on http://localhost:3100
make wizard            # the guided self-host install, if you want the full stack
```

`server/.env` is loaded from the server's working directory. Copy
`server/.env.example` to start.

## House style

Read [ARCHITECTURE.md](ARCHITECTURE.md) and the one for the part you touch;
[AGENTS.md](AGENTS.md) is the short version. `npm run lint` enforces most of it:

- **Small units.** Functions at most 10 lines (React components 50, with their
  logic in hooks); classes at most 200. Long code is steps that want names.
- **Clear structure.** One job per file; modules used through their facade;
  strategies, command maps, repositories and a composition root where they fit
  the problem. See how `server/src/modules/browsers/connection/` is built.
- **No magic numbers.** Named constants in the folder's `constants.ts`; HTTP
  statuses from `Status`; env-tunable values read once with a named default.
- **Documented.** Every file, function, class, field and route has a doc
  comment that says what it is for and why. The tone to match is the security
  reasoning in `server/src/platform/net-guard.ts` and `platform/secrets.ts`:
  what the decision was and what breaks without it.
- **No new runtime dependency** for something a few lines of standard library
  covers. The SDK has zero runtime dependencies and stays that way.
- **Every change leaves tests behind.** Unit tests in `tests/unit/`, mirroring
  `src/`, on `node:test` (Vitest in `ui/`); an integration suite when the change
  is a flow across modules. Test behavior through the public surface, name each
  test after the rule it checks, and cover the failure paths.

## Pull requests

1. Branch off `main`.
2. In each part you touched: `npm run lint`, `npm run format:check`, the type
   check and `npm test` all green, and coverage no lower than before.
3. One logical change per PR, with a description of what breaks without it.
4. If you touched anything under `server/src/modules/control/`, say in the PR how you
   tested tenant isolation: that is the boundary most likely to regress.

## Project layout

| Path | What lives there |
|:---|:---|
| `server/` | Control plane: REST + MCP + WebSocket gateway, admission, personas, challenges |
| `server/src/modules/` | One folder per domain: browsers, personas, gateway, control, playbooks… |
| `server/src/modules/control/` | Durable control plane: store, cluster, fleet drivers (docker, k8s) |
| `browser/` | Electron desktop app and the containerized browser runtime |
| `ui/` | Next.js console, live viewer and docs |
| `packages/sdk` | `@oya-ai/browser`, TypeScript SDK (MIT) |
| `packages/cli` | `@oya-ai/cli`, fleet CLI and the install wizard (MIT) |
| `examples/` | One runnable script per capability |
| `k8s/`, `docker-compose.yml` | Deployment manifests |
| `tooling/eslint/` | The lint rules every part shares |

Each part has an `ARCHITECTURE.md`; start from the [root one](ARCHITECTURE.md).
