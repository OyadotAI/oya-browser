# Architecture

Oya Browser is a control plane for AI-driven browsers: one API over Oya's own
browsers (desktop and cloud) and third-party providers (Browserbase, Steel,
Anchor, Browser Use, your own Chrome). This document maps the system. Each
part has its own `ARCHITECTURE.md` with the detail.

## The parts

| Part | Path | Runtime | What it is |
|:---|:---|:---|:---|
| Server | [`server/`](server/ARCHITECTURE.md) | Node 24, TypeScript run directly | REST API, browser WebSockets, MCP, the CDP gateway, the control plane |
| Console | [`ui/`](ui/ARCHITECTURE.md) | Next.js 16, React 19 | The dashboard and public site, served by the server |
| Oya Browser | [`browser/`](browser/ARCHITECTURE.md) | Electron | The desktop browser, and the image cloud browsers run |
| SDK | [`packages/sdk/`](packages/ARCHITECTURE.md) | Node 20+, browsers | `@oya-ai/browser`, the public TypeScript client (MIT) |
| CLI | [`packages/cli/`](packages/ARCHITECTURE.md) | Node 20+ | `@oya-ai/cli`, install and setup wizard (MIT) |

Around them: `tooling/eslint/` (the shared lint rules), `docs/` (operator
guides), `examples/` (runnable SDK and Playwright samples), `k8s/` and the
`Dockerfile` (deployment), `supabase/` and `server/migrations/` (schema).

## How they talk

```
 SDK / CLI / MCP clients / Playwright ──HTTP, WS, MCP, CDP──▶ server ◀──HTTP── console (ui)
                                                               │
                        ┌──────────────────────────────────────┼─────────────────────────────┐
                        ▼                                      ▼                             ▼
          Oya Browser (desktop or cloud)          third-party providers (CDP)         Supabase / Postgres
          control socket: commands, frames,        driven by the server's                / SQLite (local)
          cookies, relayed CDP                     CDP driver
```

- **Browsers connect to the server**, never the other way round. An Oya
  Browser dials the control socket (`/ws`), authenticates with its key, and
  runs as a *persona* (fingerprint, cookie jar and proxy bound together).
  Third-party browsers are dialled by the server over CDP.
- **Every command takes one path**: `sendCommand` in the server picks the
  transport for that browser (socket or CDP driver). So REST, MCP, chat
  and playbooks drive every kind of browser the same way.
- **The console is a client like any other**: it calls the REST API with a
  project credential. It does not share code with the server.
- **Playwright and Puppeteer** reach a browser through the server's CDP
  gateway (`/connect`), which relays to the browser.

## Engineering standards (all parts)

The same rules apply everywhere. The linter enforces them: each part's ESLint
config imports `tooling/eslint`, and CI runs lint, format check, type check and
tests on every pull request.

- **Structure**
  - One folder per domain.
  - One job per file.
  - A module is used through its facade: an `index.ts` or a named entry file.
- **Design patterns**, where they fit the problem:
  - Strategy for interchangeable implementations.
  - Command maps instead of `switch` chains.
  - Repositories for storage.
  - Service classes wired in a composition root.
  - Thin HTTP routes.
- **Size**
  - Functions: at most 10 lines.
  - React components: at most 50 lines. Their logic lives in hooks.
  - Classes: at most 200 lines.
- **No magic numbers**: every number is a named constant, and HTTP statuses use a shared table.
- **Documentation**: every file, function, class, field and route has a doc
  comment saying what it is for and why.
- **Tests**: unit tests beside the code's structure (`tests/unit/` mirrors
  `src/`), integration tests for flows, all hermetic (no network, no live
  credentials, no shared state).
- **Formatting**: Prettier with one shared config (`.prettierrc.json`).

Deliberate exceptions are listed in each part's ARCHITECTURE.md with the
reason. Examples: scripts injected into web pages, whose shape is visible to
the page, and code whose text is copied into generated output.
