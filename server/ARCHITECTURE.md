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
  integration/         end-to-end routers/sockets and native Oya fixtures; legacy browser suites remain gated
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
  Its host matcher must stay byte-identical to
  `browser/src/main/identity/host-rules.ts`, and
  `tests/integration/egress-rules.test.js` compares the two.
- **`agent/placeholders.ts` `FILTERS.date`** sits inside a lint-disable block.
  Its source text is copied into the Playwright export, so it must not change.

## Desktop library tools

`agent/library-tools.ts` defines history, bookmark and closed-tab tools for the
agent loop and per-browser MCP. Handlers call the existing `sendCommand` path,
which owns authorization/control admission and capability reporting. The browser
owns persistence and profile isolation; no server library copy is introduced.
Tools are filtered against announced capabilities and are unsupported on generic
CDP browsers. Destructive history clearing requires an explicit confirmation
argument; library titles/URLs are untrusted data in the agent prompt.

The read-only `list_keyboard_shortcuts` agent/MCP tool obtains the connected
browser's live shortcut registry and platform through the normal command path.
It describes shell shortcuts; it does not execute them or grant shell control.

## Native service integration fixtures

`npm run test:native-services` runs automatic login, TOTP login, task-file upload,
challenge detection/MFA, native dialogs, MCP desktop lifecycle and fixture
failure-path checks against an explicit `OYA_NATIVE_ENGINE`. The fixture launcher
has no installed-browser search and no skip-on-missing-engine path. Its private
parent/child stdio channel has no network
listener and supports native main-world evaluation, loopback navigation, and
analysis, clicks and dialog answers through the production native command runner.
The Oya child uses the production NativeRuntime for exception-preserving,
document-scoped evaluation; accessing its debugger throws. External requests and
redirects are blocked by the native session. The parent owns the temporary
profile and removes it only after the engine exits.

These exercise the existing service scripts on real pages; they do not claim to
replace the native file-chooser or native trusted-keyboard suites. Existing
service assertions remain, and missing engines or unsupported fixture commands
fail explicitly. The anonymity suite now installs the production native persona
policy before creating its page and retains its 34 surface/isolation assertions.
The gateway suite also uses cold native Oya workers. The integration gate remains
enabled to prevent new stock-browser launchers from entering these suites.

The native MCP suite starts the real server, admits a private native fixture over
the authenticated Oya control socket, and exercises public MCP start/adopt,
analysis, trusted clicks, human-control handoff, cross-key refusal and stop.
The fixture advertises only its supported actions, with CDP disabled and engine
debugger access forbidden. It does not replace cloud provisioning or validate
persona application. Separately, `mcp-lifecycle.test.js` provisions a fresh Oya
Cloud worker through the real reservation, enrollment and MCP paths, installs
the authenticated native persona before navigation, and captures its profile
before destroying the worker. Only the cloud allocator is replaced with a local
native process. It retains the original MCP lifecycle assertions and checks
creation/release counts; it does not validate a Docker image or outbound CDP
provider compatibility.

The SDK login journey now uses disposable native Oya engines. It retains encrypted
profile/MFA reload and tenant-isolation checks, first-script cookie/storage
hydration, trusted typing/clicking, a returned public CDP gateway URL terminating
at the native front door, logout, stop/save and fresh-engine restore. No debugging
endpoint backs these operations. Explicit stop of a native client requests
`profile_capture`, waits for its correlated result after ordered state messages,
and drains encrypted persistence before disconnecting. Failed capture leaves the
browser connected. Borrowing a desktop finishes only the unused provisioning
reservation, preserving the desktop and avoiding a fictitious live fleet entry.

The external CDP connection transport accepts an explicit ephemeral bearer token
for Oya's native compatibility front door. Tokens are kept outside enumerable
connection state and endpoint URLs, are sent only over WSS or literal-loopback
WS, and are never forwarded through redirects. The native transport integration
checks unauthenticated/wrong-token refusal, target attachment and native runtime
evaluation with engine debugger access forbidden. The transport also verifies
native isolated-world creation and separation from page globals; this coverage
is supplemented by `cdp.test.js`, which retains driver navigation, input, analyzer,
tab and isolation assertions against the authenticated native adapter. Its
recording assertions now use the production native recorder, and persona setup
uses the production native session policy. Direct native sessions keep the host
timezone to match unproxied egress. This is not protocol parity for generic CDP
recording, profile replay or persona emulation; those provider paths retain unit
coverage. Gateway providers of type `oya-cloud` restore named cookie/localStorage
profiles through native persona enrollment before exposing a client connection.
The encrypted snapshot binds the same owner-checked persona across restarts;
recording and final profile capture use native commands. Legacy sessionStorage
snapshots are explicitly refused, and native snapshots cannot be replayed into
an external CDP provider.

Linux cloud workers require the complete checksum-pinned Oya distribution in
[native engine provisioning](../browser/engine/README.md#ci-engine-provisioning).
The cloud image and server integration fixtures use that same runtime; an npm
Electron download cannot supply the native storage and profile-capture contract.

Gateway profile hydration fails closed: missing pages, refused cookie replay,
or refused storage hooks prevent the client upgrade and release its lock and
provider slot. Cleanup never captures the partially restored browser over the
saved profile. Capture transport/evaluation failures preserve the prior saved
state. The native provider bypasses protocol replay entirely.
