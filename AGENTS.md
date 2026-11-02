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
| `browser/` | `npm run lint` · `npm run format:check` | `npm run typecheck` · `npm run build` (electron-vite, into `out/`) | `npm test` (unit + regressions) · `npm run test:shell` · `npm run test:control` · `npm run test:recording` · `npm run test:identity` · `npm run test:workflow` · `npm run test:sync` (real Electron) |
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

- `server/src/modules/control/egress.ts` and
  `browser/src/main/identity/host-rules.ts` share a host matcher that must
  stay byte-identical. A test compares the two.
- Scripts injected into web pages (`browser/scripts/analyzer.js`,
  `browser/anonymity/{stealth,fingerprint,inject}.js`, and the page-script
  strings in `browser/src/main/actions/scripts.ts`, `src/main/tabs/` and
  `src/page/`) are visible to the pages they run in, including
  their source text and shape. Change them only on purpose, and re-run the
  stealth checks.
- Persona fingerprints must be deterministic: the same persona gives the same
  device, forever.
- Leave commits to the maintainer unless asked.
- **Never mention Claude in commits or pull requests.** No `Co-Authored-By: Claude`
  trailer, no "Generated with Claude Code" line, no claude.ai session link, and
  never commit as `Claude <noreply@anthropic.com>`. Commits are authored by the
  maintainer alone.

## Native agent-control boundary (required)

- Oya's internal browser and agent operations must use browser-owned native APIs, not CDP.
- The patched engine is the Linux cloud runtime only. Desktop apps (macOS, Windows) ship stock Electron: use stock non-CDP Electron APIs there, and return the unsupported-capability error where only the patched engine has one.
- CDP is permitted only as an external front-door compatibility protocol for agents connecting to Oya.
- The front door must translate supported requests into native Oya operations. It must not proxy requests to a Chromium debugging endpoint or use Electron's debugger as its backend.
- Do not rename or wrap CDP helpers and describe them as native. Do not fall back to CDP when a native capability is unavailable; return an explicit unsupported-capability error.
- Retain authorization, human/agent control ownership, tab/frame isolation, persona and egress protections throughout migration. Do not remove safety checks to eliminate CDP calls.
- Test through Oya and its native APIs. Do not launch Chrome, Chromium, Playwright, or another browser as a substitute.
- **Exception (maintainer decision, 2026-10-12):** the external front door passes standard page-session CDP methods it has no native answer for to that tab's own debugger, so Playwright, Puppeteer and Stagehand clients work. It lives only in `browser/src/main/app/cdp-passthrough.ts` (the one file `tests/unit/native-boundary.test.ts` exempts) and keeps ownership, Human Mode, persona and egress refusals. Do not remove it as a boundary violation, and do not extend debugger use beyond it; replace passed-through methods with native ones as they land.
- Existing internal CDP dependencies are migration debt, not an exception to this requirement. Do not claim Oya is internally CDP-free or release it on that basis until enforcement and native integration tests prove it.

## Production

The production checklist the reviewers enforce is in `docs/PRODUCTION.md`. It applies.

## How a request becomes a change

Every request that changes the product runs this pipeline, in this order, and the person sees one
report at the end of it — not a stream of half-decisions along the way. The stages are the team's
subagents (`.claude/agents/`, `.agents/agents/`, `.codex/agents/` — one per coding agent). They run **one after another**, never in parallel, because each one reads
what the last one returned; and what a stage returned is handed to the next **verbatim**, because
a subagent starts with an empty context and a paraphrase is where a scope quietly grows.

**In Codex**, run a stage with `spawn_agent`: `task_name` is the stage's name (`pm`, `designer`,
…) and the message says to follow the `developer_instructions` in `.codex/agents/<stage>.toml`,
followed by the brief: the person's ask in their own words and what the previous stage returned,
verbatim. Wait for it to finish before starting the next stage. Do not do a stage's work yourself
instead of spawning it.

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

**Direct mentions:** When the person tags a member, give that member the requested task directly;
do not require unrelated pipeline stages. `oya-browser` is the browser specialist for research,
web interaction, screenshots and browser QA. Use Oya exclusively for all browser work; if its
connection is unavailable, report the blocker and never substitute another provider. Inside Keel,
Keel's own browser (the `browser_*` tools on server `keel`, the lane's Browser tab) is the browser
and Oya's tools are withheld on purpose; use it the same way. The team
ships role playbooks, specialist engineering skills, and Oya research with supporting assets.
Use `/handoff` when a session transfer is requested. Load `writing-for-agents` when editing
agent instructions or skills; these workflows do not change a member's permissions.

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
