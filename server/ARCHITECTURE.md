# Server architecture

The server is the control plane. It serves the REST API, the browsers' control
sockets, MCP, the CDP gateway and the console. It runs its TypeScript directly
(Node strips the types; there is no build), so only erasable syntax is
allowed: no `enum`, no `namespace`, no constructor parameter properties.

The standards every part shares (size limits, documentation, no magic numbers)
are in the root [ARCHITECTURE.md](../ARCHITECTURE.md) and enforced by
`npm run lint`. This file covers what is specific to the server.

## Layout

```
src/
  index.ts             entry point: boot order, listeners, graceful shutdown
  app/                 the HTTP app
    api.ts             mounts every module's routes under /api, plus the error handler
    http.ts            route helpers: getKey, canAccess, requireBrowser, longJson, validData
    container.ts       composition root: builds each layered service once
    frontend.ts        runs and proxies the Next.js console
  platform/            infrastructure every module uses
    errors.ts          HttpError: an error that carries its HTTP answer
    http-status.ts     Status: HTTP codes by name
    paths.ts           where data, the browser scripts and the console live on disk
    db.ts, metrics.ts, audit.ts, usage.ts, limits.ts, llm.ts, secrets.ts, net-guard.ts, runtime-config.ts
    llm/               one provider per API (anthropic, gemini, openai) behind llm.ts, with
                       their shared transport (retries, timeout, one error shape)
  drivers/             how browsers are reached: cdp.ts (CDP driver), providers.ts (vendors), sandbox.ts (Oya Cloud, a facade over sandbox/workers/: daytona, docker, k8s, ecs)
  mcp/                 the per-browser and pool MCP servers (facade: server.ts); the browser
                       tools, and the agent's own tools re-served (agent-tools.ts)
  modules/             one folder per domain
    browsers/          connected browsers: control sockets, commands, lifecycle, routes
    personas/          identities: fingerprint + cookie jar + proxy
    gateway/           the CDP gateway: sessions, provider routing, recordings, profiles
    control/           the durable control plane: sessions, projects, members, workers, cluster
    playbooks/         recorded and replayed workflows, runs, Playwright export
    agent/             the LLM agent that drives a browser (chat, tools, guards, verifier,
                       page tools, per-site notes)
    challenges/        CAPTCHA, site login, MFA
    auth/              accounts, API keys, the auth middleware
    proxies/           the proxy pool and assignment
    config/            per-key settings
    slack/, pairing/, fleet/
  types/               ambient types (Express request fields)
tests/
  unit/                unit tests; mirrors src/ (src/a/b.ts → tests/unit/a/b.test.ts)
  integration/         end-to-end suites that boot routers, sockets and real Chrome
  support/             hermetic.js (test preload), fakes, cluster helpers
```

## How a module is built

A module's **facade** is its public surface: an `index.ts`, or its original
entry file (`service.ts`, `routes.ts`, `socket.ts`). Other code imports the
facade only. Behind it the work is split by responsibility, one job per file.
`browsers/connection/` shows every pattern at small scale:

| File                                                                        | Role                                                                                       |
| :-------------------------------------------------------------------------- | :----------------------------------------------------------------------------------------- |
| `browsers/socket.ts`                                                        | Facade: `handleConnection`, `sendCommand`, `takeDialogNote`                                |
| `connection/browser-connection.ts`                                          | Runs a connection's stages in order: admission, registration, welcome, heartbeat           |
| `connection/admission.ts`, `registration.ts`, `identity.ts`, `heartbeat.ts` | One stage each                                                                             |
| `connection/handlers/`                                                      | Command map: message type → handler, grouped by topic                                      |
| `connection/transports/`                                                    | Strategy: `CommandTransport`, with `DriverTransport` and `SocketTransport` implementations |
| `connection/commands.ts`                                                    | Facade over the transports: slot, dispatch, outcome                                        |
| `connection/constants.ts`                                                   | Every number, by name                                                                      |

The other patterns in use:

- **Layered module** (`personas/`): `model.ts` (types and pure rules),
  `repository.ts` (a storage interface with file, Supabase and fallback
  implementations), `service.ts` (a `PersonaService` class that gets its
  dependencies through its constructor), and `routes.ts` (a routes factory
  that takes the service). `app/container.ts` wires them.
- **Stages:** the CDP gateway's upgrade (`gateway/upgrade*.ts`) and browser
  start and stop (`browsers/lifecycle/`) are sequences of named steps.
- **Command maps:**
  - CDP actions (`drivers/cdp/handlers/`)
  - agent tools (`agent/tool-handlers.ts`, `agent/page-tool-handlers.ts`), which the MCP
    servers re-serve from the same definitions (`mcp/agent-tools.ts`)
  - playbook replay and Playwright lines (`playbooks/replay.ts`, `playwright.ts`)
  - routing strategies (`gateway/strategies.ts`)

## Requests and errors

A route validates input, calls a service and answers. Services throw
`new HttpError(Status.X, message)`, and the API error handler (`app/api.ts`)
turns uncaught errors into `{ error, code }`. Some routes answer their own
errors because their response bodies predate the handler. New routes should
throw instead. `requireBrowser` guards every `/:browserId` route.

## Configuration

- **Per folder:** each folder's numbers live in its `constants.ts`.
- **Environment:** a value an operator may tune reads the environment once,
  next to a named default:
  `export const X = Number(process.env.OYA_X) || DEFAULT_X;`.
- **`server/.env`:** loaded by `index.ts` through `dotenv`, never by tests.
- **Disk paths:** come from `platform/paths.ts`.

## Tests

| Command                    | Runs                                  |
| :------------------------- | :------------------------------------ |
| `npm test`                 | unit, then integration                |
| `npm run test:unit`        | `tests/unit/**/*.test.ts` in parallel |
| `npm run test:integration` | the integration suites, one at a time |
| `npm run test:coverage`    | unit tests with a coverage report     |

All of these load `tests/support/hermetic.js` first. It makes a run hermetic:
`.env` is never read, and state goes to a fresh scratch directory. Unit tests
use `node:test` with `mock.timers` for anything time-based, plus the fakes in
`tests/unit/support/fakes.ts`. Suites that need a real service stay outside
`npm test`. They are listed in CONTRIBUTING.md and run with `OYA_TEST_LIVE=1`.

## Exceptions

- **`modules/control/egress.ts`** is exempt from the size and number rules.
  Its host matcher must stay byte-identical to `browser/governance.js`, and
  `tests/integration/egress-rules.test.js` compares the two.
- **`agent/placeholders.ts` `FILTERS.date`** sits inside a lint-disable block.
  Its source text is copied into the Playwright export, so it must not change.
