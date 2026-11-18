@AGENTS.md

## The gate

A change is done when the gate for every part it touches is green. Run each part's gate from that
part's folder. Each one is the same set of checks CI runs (`.github/workflows/ci.yaml`).

| Part touched | Gate |
|:---|:---|
| `browser/` | `npm run lint && npm run format:check && npm run typecheck && npm run test:unit && npm run build` |
| `server/` | `npm run lint && npm run format:check && npm run typecheck && npm run test:coverage` (floors: 95% lines, 90% branches, 90% functions) |
| `ui/` | `npm run lint && npm run format:check && npm run typecheck && npm run test:coverage` |
| `packages/*`, `tooling/`, `scripts/` | from the root: `npm run lint && npm run format:check && npm run test:packages && npm run test:scripts` |
| `packages/python/` | from that folder: `uv sync --group dev && uv run ruff format --check src tests && uv run ruff check src tests && uv run mypy && uv run pytest -q` |

Integration suites need a real Electron or a running stack. Run the ones a change touches, under
`xvfb-run -a` on Linux:
- `browser/`: `npm run test:integration`, plus the named `test:*` suite for the area changed
  (`test:shell` for the strip and toolbar, `test:control` for human/agent control, etc.).
- `server/`: `npm run test:integration`.

Run `npm run setup` at the root once to install every part.

## Where things are

| Path | What lives there |
|:---|:---|
| `server/src/app/` | Composition root (`container.ts`), HTTP wiring (`http.ts`), API surface (`api.ts`) |
| `server/src/modules/<domain>/` | One folder per domain: `control`, `browsers`, `personas`, `playbooks`, `gateway`, `auth`, `billing`, … |
| `server/src/modules/browsers/connection/commands.ts` | `sendCommand`: the one path every command takes to a browser |
| `server/src/mcp/` | The MCP server's browser tools |
| `browser/src/main/main.ts` | The desktop app's composition root: builds the services and wires events |
| `browser/src/main/app/services.ts` | The `AppServices` type every main-process service takes its deps from |
| `browser/src/main/tabs/` | Tab manager (`tabs.ts`), strip order and commands (`tab-order.ts`), right-click menu (`tab-menu.ts`), session save and restore (`session-tabs.ts`) |
| `browser/src/main/ipc/` | Shell → main channel handlers, registered behind the shell-only guard in `index.ts` |
| `browser/src/shared/ipc.ts` | The typed channel tables (`ShellCalls`, `ShellEvents`, `CALL_CHANNELS`, `EVENT_CHANNELS`) both sides compile against |
| `browser/src/preload/` | The bridge the shell page calls |
| `browser/src/renderer/features/<feature>/` | Shell UI by feature (`tabs`, `toolbar`, `library`, …): `components/`, `hooks/`, `view-models/` |
| `browser/scripts/`, `browser/anonymity/` | Scripts injected into web pages. Pages can see them. |
| `ui/src/` | Next.js console and public site |
| `packages/sdk/`, `packages/cli/` | Public TypeScript client and install CLI |
| `tooling/eslint/` | The shared lint rules every part imports |
| `browser/tests/unit/`, `server/tests/unit/` | Mirror of `src/`; shared fakes in `tests/unit/support/` |

Each part's `ARCHITECTURE.md` goes into more detail. Read the one for the part you are changing.

## How a request flows

- **API, MCP or SDK → a browser:** a route or MCP tool in `server/` calls `sendCommand(browserId, action, params)`.
  `sendCommand` picks the transport: the control socket for Oya browsers, or the CDP driver for third-party ones.
  The Oya browser receives the command and runs it through native operations in `browser/src/main/`. Control
  ownership (human vs agent) is checked before anything runs.
- **The person in the desktop shell → main:** a renderer feature calls the preload bridge, which uses a channel
  in `CALL_CHANNELS`. That arrives at a handler in `browser/src/main/ipc/`, and the handler calls a main-process
  service. The service sends state back to the renderer over an `EVENT_CHANNELS` event (e.g. the tab list via
  `sendTabList`). The main process owns state; the shell only asks.

## Invariants and the tests that hold them

| Invariant | Guarded by |
|:---|:---|
| The egress host matcher is byte-identical in `server/src/modules/control/egress.ts` and `browser/src/main/identity/host-rules.ts` | `server/tests/integration/egress-rules.test.js` |
| No internal CDP. The only debugger use is `browser/src/main/app/cdp-passthrough.ts` | `browser/tests/unit/native-boundary.test.ts` |
| A persona's fingerprint is deterministic: the same persona gets the same device, forever | `browser/tests/unit/main/native-policy/persona.test.ts`, `browser/tests/unit/anonymity/fingerprint.test.js` |
| Scripts injected into pages keep their shape | `npm run stealth` (server), `npm run test:analyzer` and `test:recorder` (browser) |
| Restored tabs are web addresses only, restored once per launch, only into the same profile, never in a governed browser | `browser/tests/unit/main/tabs/session-tabs.test.ts` |
| Shell channels answer only the shell's own pages | `browser/tests/unit/main/ipc/handle.test.ts` |
| Server coverage stays at or above 95/90/90 | `npm run test:coverage` in `server/` |

## How to extend

- **A new shell → main action** (e.g. a tab command):
  1. Add the method to `ShellCalls` and its channel to `CALL_CHANNELS` in `browser/src/shared/ipc.ts`. Until a
     handler exists, it won't compile.
  2. Add the handler in the matching file in `browser/src/main/ipc/`, calling a service. Keep the logic out of
     the handler.
  3. Expose the call through `browser/src/preload/`, and call it from a hook or view model under
     `browser/src/renderer/features/<feature>/`.
  4. Add unit tests for the service and the handler under `browser/tests/unit/main/`.
- **A tab strip command:** write it as a function in `browser/src/main/tabs/tab-order.ts` (or a sibling file),
  then add it to `TabMenu` in `tab-menu.ts`. Every item runs only while a person holds control (`item()`).
- **Something saved across launches:** go through `JsonFile` (`browser/src/main/app/json-file.ts`), constructed
  in `main.ts`. Treat what you read back as untrusted input and validate it. `SessionTabs` shows the pattern.
- **A new server domain:** create `server/src/modules/<domain>/` with an `index.ts` facade, build the service in
  `server/src/app/container.ts`, and keep its routes thin.
- **A new browser command reachable from the API:** add a server route or MCP tool that calls `sendCommand`, and
  a handler in the browser that runs it through native operations. Never add a CDP fallback.

## How to run

- Desktop browser: `cd browser && npm run dev` (builds, then launches a branded development copy of Electron via `src/dev/launch.ts`).
- Server and console together: `npm run dev` at the root (reads `server/.env` if present).
- Console alone: `cd ui && npm run dev`.

## Production

The production checklist the reviewers enforce is in `docs/PRODUCTION.md`. It applies.

## How a request becomes a change

Every request that changes the product runs this pipeline, in this order, and the person sees one
report at the end of it — not a stream of half-decisions along the way. The stages are the agents
in `.claude/agents/`. They run **one after another**, never in parallel, because each one reads
what the last one returned; and what a stage returned is handed to the next **verbatim**, because
a subagent starts with an empty context and a paraphrase is where a scope quietly grows.

1. **`pm`** — given the ask in the person's own words. Returns build / change / cut / not yet,
   and the smallest version worth shipping with its out-list. *Build* and *change* carry on, to
   the scope the PM set. **`cut` and `not yet` stop the pipeline and go back to the person**, with
   the reasoning. They can overrule it in one word.
2. **`designer`** — given the ask and the PM's scope, whenever anything a person reads changes: a
   page, a label, an error message, CLI output, an API response a client renders. Returns the
   states table, the layout, the copy. Skipped — and said to be skipped — when nothing visible
   moves.
3. **`principal`** — given the ask, the scope and the design. Reviews both for what they missed,
   then returns the technical design: the modules touched, what is reused, the invariants at risk,
   and the tests that will pin it. If the scope or the design cannot stand, that goes back through
   the stage that owns it once, not around it.
4. **Implementation** — the main loop, not a subagent, so the work is visible as it is written.
   It follows the principal's design; a deviation is written down with its reason, not made
   silently, and it keeps to "Engineering rules" below. Done means the gate is green, then
   `reviewer` on the diff — and `security` or
   `reliability` when their descriptions say the change is theirs.
5. **`qa`** — given the ask, the scope and what changed. *Do not ship* means fix and run `qa`
   again, **twice at most**; after that the person gets the change and the open bug list rather
   than a loop with no end.

**What the person is shown** is one report: what was asked, what the PM scoped in and out, what
the designer and the principal decided in a line each, the files changed, the gate's result, QA's
verdict with what it covered and did not, and anything any stage was overruled on. A stage that
could not run is named as not run, never implied.

**What does not go through it:** a question, an explanation, running something, git and release
chores, edits to docs and to these agents, and a change with no decision in it — a typo, a rename,
a version bump. Say in one line that it was skipped. "Just do it" and "skip the pipeline" skip it;
naming stages ("no PM", "QA only") runs the ones named.


## Engineering rules

Defaults Keel starts every project with. Where the linter or another section of this file says
otherwise, that wins — edit these to match rather than keeping two answers. `principal` holds
changes to them; say which rule a deviation breaks and why it is worth it.

**Code quality**

- **Every file opens with a comment** saying what it is for. Every public function, type and
  route has a doc comment that says what it is for and why — what the decision was and what breaks
  without it — never the name again.
- **Small units.** Functions around 10 lines of logic, UI components around 50 with their logic in
  hooks or a view model, classes and modules around 200. Long code is steps that want names.
- **No magic numbers.** Each number gets a name in the folder's constants file; HTTP statuses come
  from one shared table; a value the environment can tune is read in one place with a named
  default.
- **No new dependency** for something a few lines of the standard library cover.
- **In JavaScript and TypeScript the linter holds these.** `tooling/eslint/index.js` has the rule
  sets (file header, function and class length, no magic numbers, doc comments); import them into
  the project's ESLint config rather than restating them.

**Design**

- **One folder per domain, one job per file.**
- **Facades.** Use another module through its entry file (`index.ts`, `mod.rs`, `__init__.py`),
  never its internals. Keep exported names stable; when code moves, re-export it from where it was.
- **Command maps instead of `switch` chains** — dispatch on a type through a map of handlers, and
  guard the lookup.
- **Strategies** for interchangeable implementations (providers, transports, stores): one
  interface, and the implementation picked in one place.
- **Repositories.** Services never touch the filesystem or the database directly.
- **Composition root.** Services receive their dependencies through their constructors, and one
  place builds them.
- **Thin edges.** A service raises a typed error carrying its status; routes, handlers and CLI
  commands translate and stay thin.

**Tests**

- Every change leaves tests behind. Unit tests mirror the source tree and are named after the file
  they test; an integration test when the change is a flow across modules.
- Test behaviour through the public surface, one test per rule, named after the rule — and the
  failure paths as well as the happy one.
- Tests are hermetic: no network, no real `.env`, state in a scratch directory, fakes at the seams,
  fake timers for anything time-based.
- Coverage never goes down.
