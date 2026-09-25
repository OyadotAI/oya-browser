# Working in this repository

> Here to **use** Oya Browser rather than change it? Read the "For AI agents"
> section of `README.md`, or https://oyabrowser.com/llms.txt.

Guidance for AI coding agents and people alike. Read `ARCHITECTURE.md` first
for the map, then the ARCHITECTURE.md of the part you are changing.

## Parts and their commands

| Part | Lint | Types | Tests |
|:---|:---|:---|:---|
| `server/` | `npm run lint` · `npm run format:check` | `npm run typecheck` | `npm test` (unit + integration) · `npm run test:unit` · `npm run test:coverage` |
| `ui/` | `npm run lint` · `npm run format:check` | `npm run typecheck` | `npm test` (Vitest) · `npm run test:ui` (Playwright, needs a running stack) |
| `browser/` | `npm run lint` · `npm run format:check` | none (plain JavaScript) | `npm test` (unit + regressions) · `npm run test:shell` · `npm run test:control` · `npm run test:identity` · `npm run test:sync` (real Electron) |
| `packages/*` | `npm run lint` (root) | `npm run build:sdk` | `npm run test:packages` (root) |

Run the commands from the part's own folder. The root `npm test` runs the
server, CLI and package suites.

## Rules the linter will hold you to

- **Every file starts with a `/** … */` header.** Every function, class, method,
  field, type, interface member and route has a doc comment. Say what it is for
  and why, in plain words, not the name again.
- **Functions: at most 10 lines** of code. React components: at most 50, with
  their logic in hooks. **Classes: at most 200 lines.** Split long code into
  named steps and classes into collaborators.
- **No magic numbers.** Put each number in the folder's `constants.ts` (or
  `constants.js`) under a name. HTTP statuses come from the shared `Status`
  table. An env-tunable value reads the environment in one place, with a named
  default.

## Design conventions

- **Facades:** import another module through its `index.ts` or entry file,
  never its internals. Keep a module's exported names stable. When you move
  code, re-export it from where it was.
- **Command maps instead of `switch`:** dispatch on a type through
  `Record<type, handler>`. Guard the lookup with `Object.hasOwn`.
- **Strategies:** for interchangeable implementations (transports, providers,
  launchers), define an interface and pick the implementation in one place.
- **Repositories:** services never touch `fs` or the database directly.
- **Composition root:** services receive their dependencies through their
  constructors. `server/src/app/container.ts` builds them.
- **Errors:** a service throws `HttpError(Status.X, message)`. Routes stay
  thin.

## Tests

- **Placement:** unit tests live in `tests/unit/`, mirroring `src/`. Name
  each one after the file it tests: `src/a/b.ts` → `tests/unit/a/b.test.ts`.
- **Runner:** Node's built-in `node:test` with `node:assert/strict` (server,
  browser, packages), and Vitest with Testing Library (ui). Add no other test
  dependencies.
- **Isolation:** tests are hermetic. The server preload
  (`tests/support/hermetic.js`) stops `.env` from loading and points state at a
  scratch directory. Fake the seams with `tests/unit/support/fakes.ts`, and
  use `mock.timers` for anything time-based. Never call the network.
- **What to test:**
  - behavior through the public surface, not private helpers
  - one test per rule, named after the rule
  - the failure paths as well as the happy one
- **Coverage:** changes must keep coverage at or above the enforced minimum.

## Things that must not change casually

- `server/src/modules/control/egress.ts` and `browser/governance.js` share a
  host matcher that must stay byte-identical. A test compares the two.
- Scripts injected into web pages (`browser/scripts/analyzer.js`,
  `browser/anonymity/*`) are visible to the pages they run in, including
  their source text and shape. Change them only on purpose, and re-run the
  stealth checks.
- Persona fingerprints must be deterministic: the same persona gives the same
  device, forever.
- Leave commits to the maintainer unless asked.
