# Packages architecture

Two published packages live here, both MIT and both built with tsup:

| Package | Path | What it is |
|:---|:---|:---|
| `@oya-ai/browser` | `sdk/` | The TypeScript client for the control plane. Zero runtime dependencies; runs in Node 20+ and in browsers. |
| `@oya-ai/cli` | `cli/` | The `oya` command: login, onboarding, browsers, personas and their cookies, the control plane, and the `oya install` wizard. The SDK is bundled into its single file. |

The standards every part shares (size limits, documentation, no magic numbers)
are in the root [ARCHITECTURE.md](../ARCHITECTURE.md). The rules are enforced
by the root ESLint config (`npx eslint packages` from the repo root).

## What must not change

Both packages are published, so their surfaces are contracts:

- **SDK:** the exported names, the classes and their public members, method
  signatures and option shapes. `npm run check --workspace=@oya-ai/browser`
  (publint and attw) must stay clean, and the package must keep zero runtime
  dependencies.
- **CLI:** every command, alias, flag, prompt and line of output, and the exit
  codes.

## SDK layout (`sdk/src/`)

```
index.ts           facade: the Oya client, and every export
client.ts          Http, the one HTTP path; createHttp builds it from options and the environment
errors.ts          OyaError
constants.ts       every number by name: time budgets, polling, file limits, the Status table
browser.ts         Browser: one running browser (commands, CAPTCHA, MFA, agent runs, links)
browser-shapes.ts  the shapes Browser's methods take and return
run.ts             Run: a task submitted with browser.submit()
run-watch.ts       the polling loop behind a Run: callbacks, retries, settling
file.ts            file(): a file as a task value
api/               one file per client namespace
  browsers.ts      oya.browser: start, get, list, stop
  ready.ts         waiting for a starting cloud browser to dial in
  control.ts       oya.control, built from one group of calls per topic
  personas.ts      oya.personas (and its alias oya.profiles)
  playbooks.ts, proxies.ts, config.ts
  shapes.ts        the shapes the namespaces take and return
types/             the exported types, by topic, behind types/index.ts
```

How it fits together:

- **One HTTP path.** Every call goes through `Http.request`, which adds the
  bearer key, encodes the body, applies the timeout and turns a non-2xx answer
  into an `OyaError`.
- **Namespaces are factories.** `oya.browser`, `oya.control` and the rest are
  object literals built by `api/*.ts`. They take the client's Http as a getter
  (`() => this.http`), because class fields are initialised before the
  constructor sets `http`. Keeping them object literals of arrow functions keeps
  their emitted types identical to before, and lets callers destructure them.
- **Private members stay.** `Oya.waitUntilConnected`, `Browser.command`,
  `Browser.elementId` and `Run.watch` remain private methods, because private
  members are part of a class's emitted type. Their bodies delegate to the
  module functions that do the work.
- **Named shapes.** Every property needs a doc comment, including those inside
  inline object types, so return and option shapes that were inline are now
  named interfaces (`browser-shapes.ts`, `api/shapes.ts`). They are not
  exported: the published list of names is unchanged, and the shapes are
  structurally identical.

## CLI layout (`cli/src/`)

```
index.ts           the bin entry: parse argv, dispatch, explain a failure, close prompts
args.ts            argv parsing: command, positionals, --flags
context.ts         what every command needs: the SDK client, the target browser, JSON or human output
config.ts          the saved key (~/.oya/config.json, mode 600)
constants.ts       the CLI's numbers and the Status table
help.ts            the help text
commands/
  index.ts         the command map: name → handler, aliases, unknown commands
  control.ts       the control-plane commands, a command map with its own subcommand maps
  login.ts, init.ts, browsers.ts, personas.ts, cookies.ts, settings.ts, stealth.ts
prompt.ts          facade over prompt/
prompt/
  style.ts         colour and cursor control (plain off a TTY)
  frame.ts         banners, numbered steps, notes, spinners
  lines.ts         the one readline and its line queue (what makes piped input work)
  ask.ts           text questions that re-prompt instead of failing
  menu.ts          menus: arrow keys on a TTY (a key map), numbered lists elsewhere
install.ts         facade for `oya install`: runs the stages in order
install/
  types.ts         the saved answers, secrets and preflight checks
  constants.ts     the wizard's numbers
  repo.ts          finding (or cloning) the checkout
  interview.ts     the questions, one function per step
  llm.ts           the LLM step and key verification
  preflight.ts     what the machine needs
  build-env.ts     answers → .env values, with a command map per fleet
  env-file.ts      reading, rendering and writing .env
  kek.ts           guarding OYA_PROFILE_SECRET against sealed data from an earlier install
  migrate.ts, k8s.ts, docker.ts, shell.ts, report.ts
```

Dispatch is a command map at every level: `commands/index.ts` for commands,
`commands/control.ts` for control-plane commands and their subcommands,
`commands/personas.ts` for persona subcommands, `prompt/menu.ts` for keys, and
`install/build-env.ts` for fleets. Each lookup is guarded with `Object.hasOwn`,
so `oya constructor` is an unknown command rather than a crash.

CLI source imports use `.ts` extensions (`allowImportingTsExtensions`), so the
tests run the source directly on Node's type stripping. That means erasable
syntax only in `cli/src/`: no enums, namespaces or parameter properties.

## Tests

| Command (repo root) | Runs |
|:---|:---|
| `npm run test:packages` | builds both packages, then every `packages/*/tests/unit/**/*.test.ts` |
| `npm run test:cli` | builds the CLI, then the integration suites in `cli/tests/integration/` |

- **Unit tests** use `node:test` and `node:assert/strict`, and mirror `src/`.
  They are hermetic:
  - The SDK tests run against the built `dist/` (what users get) with a fake
    `fetch` (`sdk/tests/unit/support/fake-fetch.ts`), and use `mock.timers`
    for polling.
  - The CLI tests import the source directly. `cli/tests/unit/support/harness.ts`
    points the config at a scratch directory, fakes `fetch`, captures console
    output and traps `process.exit`.
- **Integration tests** (`cli/tests/integration/`):
  - `prompt.mjs` drives the arrow-key menu with synthetic keys against a faked
    TTY, and the piped path in a child process.
  - `install.mjs` runs the built `oya install --dry-run` against a throwaway
    checkout, and checks that no secret is printed and an existing `.env`
    survives.

## Exceptions

- **`sdk/src/browser.ts`, class `Browser`** is over the 200-line class limit,
  with a lint-disable comment saying why. Its 31 public methods are the
  published API. Each is a documented one- to three-line call, and the
  reasoning in comments has moved to the helper functions below the class.
  Splitting the class would change the emitted types (a base class, or an
  interface merge).
