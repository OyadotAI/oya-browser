# Browser architecture

Oya Browser is an Electron app. On a desktop, a person uses it as their own
browser. In a sandbox, the same code runs headless (under Xvfb) as a cloud
browser. Either way it dials the server's control socket and runs as one
_persona_: a fingerprint, a cookie jar and a proxy, bound together.

It is TypeScript under `src/`, built by electron-vite into `out/`: the main
process, the preload, the Playwright worker and the two shell pages. The only
JavaScript left is on purpose: the page-side files web pages can see
(`scripts/analyzer.js`, `anonymity/{stealth,fingerprint,inject}.js`) and
`src/renderer/public/first-paint.js` (runs before any style). The folder's
root holds only config and build files besides those. See Exceptions.

The shared standards (size limits, doc comments, no magic numbers) are in the
root [ARCHITECTURE.md](../ARCHITECTURE.md), and `npm run lint` enforces them.
This file covers what is specific to the browser.

## Code conventions (TypeScript)

**Files and modules**

- `src/main/` (the main process), `src/preload/`, `src/renderer/` (the shell
  pages), `src/worker/` (the validation worker), `src/shared/` (types and the
  IPC contract every side imports), and the modules the server imports too:
  `src/workflow/`, `src/page/`, `src/anonymity/`. `src/dev/launch.ts` is the
  dev launcher Node runs directly; it is not built.
- ES modules, named exports only. Imports name the file with its `.ts`
  extension: Node runs the sources directly in tests (and in the server) by
  stripping types, and electron-vite bundles them for the app.
  `src/package.json` says `"type": "module"` so Node reads them as ES modules.
- Erasable syntax only (`erasableSyntaxOnly`): no `enum` (an `as const` object
  instead), no parameter properties (declare the field, assign it in the
  constructor), no `namespace`.
- File names are kebab-case; one main class per file, named for it.

**Objects**

- Anything with state or a lifetime is a class: a service, a session, a
  channel, a timer owner. Pure helpers are exported functions.
- No module-level mutable state. A value that changes lives on an instance.
- No factory that only calls `new`, and no wrapper that re-binds a class's
  methods onto a plain object. Callers hold the instance.
- A class gets what it uses through its constructor, as one `deps` object.
  Main-process services type it as `Pick<AppServices, 'tabs' | 'shell'>`
  (`src/main/app/services.ts`); a collaborator with a few needs declares a
  small interface of its own.
- Only `src/main/main.ts` imports `electron` at runtime; the lint rule
  `no-restricted-imports` holds it. Everything else takes the Electron objects
  it needs from its deps; `import type` from `electron` is fine. So a test
  hands in a fake instead of patching `require`.
- Fields are `private readonly` unless they must change. No `any` in `src/`;
  `unknown` and a check at the edge instead.

**Patterns**

- Command maps, not `switch`: `const HANDLERS: Record<Name, Handler>`, looked
  up behind `Object.hasOwn`.
- Numbers and fixed text live in the folder's `constants.ts`, `as const`.
- IPC: channels and payloads come from `src/shared/ipc.ts`. The preload builds
  `window.oyaBrowser` from its tables and the main handlers are typed against
  it, so a channel missing on either side is a compile error.
- Errors are `Error`s with a message for a person; nothing throws strings.

**The shell page: MVVM**

- `src/renderer/features/<feature>/` holds one feature, split by role:
  `components/` (its views), `hooks/` (hooks only it uses), `view-models/`
  (its ViewModels, their collaborators and state types) and `model/` (plain
  types, pure helpers, `constants.ts`). Its `index.ts` is its public entry:
  `app/` and other features import a feature only through it.
- `ui/` is the design system: the presentational pieces features draw with
  (`Button`, `IconButton`, `Icon`, `Orb`, `Dialog`, `TabList`/`Tab`,
  `TextField`, `StatusLine`, `TimeAgo`). Each renders exactly the markup the
  stylesheets and the Electron tests know, and passes native attributes
  through. A piece goes here once two features use it, or it is plainly
  generic UI.
- `hooks/` holds the hooks features share (`useViewModel`, `useDialog`,
  `useClickOutside`, `useEscapeKey`, `useStickToBottom`, `useRovingFocus`,
  `reducedMotion`). Import both through their `index.ts`.
- `core/` holds the shared code with no UI (the ViewModel base, the bridge,
  markdown, constants); `app/` holds the composition root and the shared
  ViewModels (`ShellViewModel`, and `app/panel/` for the workspace panel).
- **A ViewModel** extends `ViewModel<State>` (`core/view-model.ts`). Its state
  is one immutable snapshot changed only through `set`; its public methods are
  the intents a view calls (`closeTab(id)`, `send(text)`). It takes what it
  uses in its constructor: `Pick<RendererServices, 'bridge' | 'shell'>`, plus a
  small interface for any other feature it calls (declared by the caller, wired
  by the root, so features never import each other's ViewModels). It owns its
  subscriptions and timers through `own()`, and never touches the DOM, so its
  tests are plain `node:test` with the fake bridge
  (`tests/unit/renderer/support/bridge.ts`) and `mock.timers`.
- **Where state lives.** Feature and IPC state, and the rules around it, live
  in ViewModels. Purely local UI state that nothing else reads (which menu is
  open, hover, text not yet submitted, a focus target) stays in `useState` in
  the view or a small custom hook beside it. There is no global store and no
  query library: the main process pushes its state over IPC, and each
  ViewModel subscribes to what it shows.
- **A view** is a React function component, at most 50 lines, that reads a
  ViewModel through `useViewModel(vm)` and calls its intents. DOM-only concerns
  (focus, measuring, scrolling into view) live in the view or a small hook next
  to it. An effect never returns a value: Chromium's `scrollIntoView` returns a
  promise, which React would then call as a cleanup. Views keep the ids, classes, `data-*` and ARIA attributes the
  stylesheets and the Electron tests rely on.
- HTML built as text is only ever the escaped markdown (`core/markdown.ts`) and
  the icon set; everything else is JSX.
- CSS lives beside what renders it (`ui/button.css` imported by `ui/button.tsx`,
  `features/<f>/components/*.css` by its components); `app/main.tsx` loads
  `core/tokens.css` and the page-wide `app/shell.css` first, so import order is
  cascade order.

Page-side code is the exception to all of this; see Exceptions.

## Processes

| Where it runs  | Built from → to                                                           | What it is                                                              |
| :------------- | :------------------------------------------------------------------------ | :---------------------------------------------------------------------- |
| Main process   | `src/main/main.ts` → `out/main/index.js`                                  | Windows, tabs, the control socket, recording, CDP, the studio           |
| Shell page     | `src/renderer/index.html` → `out/renderer/index.html`                     | The tab strip, toolbar and workspace panel (React, `window.oyaBrowser`) |
| Control shield | `src/renderer/control-shield/index.html` → `out/renderer/control-shield/` | The view over the page while an agent drives (`window.oyaShield`)       |
| Preload        | `src/preload/index.ts` → `out/preload/index.js`                           | The shell page's bridge, built from `src/shared/ipc.ts`                 |
| Worker         | `src/worker/index.ts` → `out/main/worker.js`                              | Workflow validation with Playwright, in a utility process               |
| Visited pages  | `scripts/analyzer.js`, `anonymity/` (read as text)                        | The page reader and the fingerprint patches (page-side, see Exceptions) |

## Layout

```
electron.vite.config.ts  the build: main (+ worker), preload, renderer (two pages), all into out/
tsconfig.json            type-check only; electron-vite emits
scripts/analyzer.js      the page reader, read as text and injected into every page
anonymity/               stealth.js, fingerprint.js, inject.js: the fingerprint patches, page-side
src/
  dev/                   launch.ts: `npm start` (Node runs it directly), a branded dev copy of Electron on macOS
  shared/                ipc.ts (the contract: every call and event, with its channel), within-time.ts
  preload/               index.ts and bridge.ts: window.oyaBrowser from the contract, one IPC listener per event, fanned out
  main/
    main.ts              composition root: pre-ready switches, every service built once into ctx, Electron's events
    app/                 services.ts (AppServices), config-store, persona, deep-links, boot (start-up stages), lifecycle (quit), updater
    ipc/                 the shell's channels: one handler class per area, typed against shared/ipc.ts, behind the shell-only guard (handle.ts)
    shell/               the window, menu, shortcuts, panel layout (layout.ts, shell-layout.ts), overlays, the control shield and its narration
    tabs/                TabManager, TabEvents, TabProtector (a tab's bounded first-page protection), TabWindows, Protection, AddressBar, context menu, page source, tab order and menu, favicons, workers
    recording/           Recorder, RecordingStart (the stages of a start), channels, moves, PageChecks, FrameSessions, tab names, RecordingPublisher
    connection/          ControlSocket, ServerMessages (the server message map), CommandRunner and TAB_COMMANDS, ServerApi, Chat (Ask), CdpRelay, LiveStream, pairing
    actions/             PageDriver and its command maps (page, pointer, history, dev), the page scripts (byte for byte), the action vocabulary, page-format
    workflow/            the studio's main-process side: Workspace, edits, runs, DraftStore, validation (forks the worker), target picker
    control/             DesktopControl: who drives, the handoff protocol, the local automation gate
    front-door/          the CDP endpoint harnesses use (OYA_REMOTE_DEBUGGING_PORT): routes, door, bridge, guard
    cdp/                 CDP on a view's debugger, World (the analyzer's isolated world), Dialogs (native JS dialogs)
    input/               Keyboard and Mouse: human-like input over CDP, and its timing
    identity/            the browser a persona says it is: identity, client hints, permissions, session setup, the exit zone's timezone; Governance (managed egress rules) and host-rules.ts (its matcher, byte-identical with the server's)
    sync/                CookieSync: the persona's cookie traffic with the server pool
    mirror/              logins imported from the person's real browsers
    observe/             Observer: what pages said and fetched, collected in main
    routines/            scheduled routines, claimed from the server and run as an Ask
  worker/                WorkflowWorker: the Playwright validation worker
  workflow/              the shared workflow model: rules, locators, handles, normalize, issues, generate, Chrome Recorder import (imported by the server)
  page/                  page helpers shared with the server: page-script text, rendering (markdown, TOON, JSONL), queries, dates, dialog text, the recorder channel, login-state.ts (the localStorage transport, also the server's CDP driver's)
  anonymity/             the Node side of the persona: apply, proxy, telemetry, the profile store (apply.ts is imported by the server)
  renderer/
    index.html           the shell page: the Oya mark sprite, #root, the module entry
    public/first-paint.js  the one classic script: the theme from the address, before the first paint
    app/                 main.tsx (entry), view-models.ts (composition root), shell-root.tsx, shell-view-model.ts, use-root-effects.ts, shell.css (page-wide), panel/
    core/                view-model.ts, bridge.ts, constants.ts, markdown.ts, tokens.css (every design token)
    ui/                  the design system: Button, IconButton, Icon, Orb, Dialog, Tabs, TextField, StatusLine, TimeAgo; type.css and fonts/
    hooks/               shared hooks: useViewModel, useDialog, useClickOutside, useEscapeKey, useStickToBottom, useRovingFocus, reducedMotion
    features/            ask, chrome (theme, activity, backdrop, launch), connection, control, inspect, routines, start, studio, tabs, toolbar
    control-shield/      the second page: Shield, Show, ShowClock, Veil and VeilWindows, Stage, Companion, Actor
tests/
  unit/                  node:test, hermetic; mirrors src/ (src/main/tabs/tabs.ts → tests/unit/main/tabs/tabs.test.ts)
  integration/           see Tests
  support/               fakes.cjs (debugger, webContents, view), main-ctx.cjs (a fake ctx), page.cjs, stores.cjs, fake-dom.cjs
```

## How the main process is built

`main.ts` sets the switches that must precede `ready`, then builds one object,
`ctx: AppServices`, and puts every service on it by name, each a class
instance: `new ConfigStore`, `new ShellWindow`, `new TabManager`,
`new Dialogs`, `new World`, `new DesktopControl`, `new CookieSync`,
`new CdpRelay`, `new Mirror`, `new LiveStream`, `new PageDriver` (with its own
`Keyboard` and `Mouse`), `new Updater`, and so on. There are no factories. Each
service takes `deps: Pick<AppServices, ...>` in its constructor and reaches its
neighbours through it when it runs. It is one shared object rather than
separate constructor arguments because the services call each other in a
cycle: tabs join recordings, the recorder reads tabs, the socket applies
personas, and the persona closes tabs. Then `registerIpc(ctx)` installs the
shell's channels, `new Boot(ctx).run()` starts the app once ready, and
`new Lifecycle(ctx).install()` holds the quit until recordings and the jar are
written.

Two rules keep this readable:

- A service touches another only through that service's methods, never its
  internals.
- Electron comes in as `ctx.electron`, so a test hands over a fake instead
  (`tests/unit/support/main-ctx.cjs`).

The patterns in use:

- **Command maps** (`Object.hasOwn`-guarded): server messages
  (`connection/server-messages.ts`), server commands
  (`connection/tab-commands.ts`, `actions/*-commands.ts`), IPC channels
  (`ipc/*.ts`), workspace commands (`ipc/workspace.ts`), shortcuts
  (`shell/shortcuts.ts`), and the front door's routes and browser-level CDP
  (`front-door/`).
- **Repository**: `app/config-store.ts` is the one place that reads or writes
  config.json, and `workflow/draft-store.ts` owns the drafts. User-chosen
  exports go through `ipc/files.ts`.
- **Stages**: boot (`app/boot.ts`), starting a recording (`recording/start.ts`)
  and protecting a new tab (`tabs/tab-protector.ts`) are sequences of named
  steps.

The app finds its files through `ctx.appDir` (`app.getAppPath()`: the package
root in development, the asar when packaged), never `__dirname`, because main
runs bundled from `out/main/`.

## How the shell page is built

`src/renderer/index.html` holds the Oya mark sprite and a `#root`.
`app/main.tsx` builds `ShellViewModels` (`app/view-models.ts`) over
`window.oyaBrowser`, `requestAnimationFrame`, the clipboard and the platform,
renders `ShellRoot`, and puts the ViewModels on `window.oyaShell` for the
Electron tests, which drive the page through them. `html[data-ready]` marks
the first render. `ShellRoot` lays the regions out in page order: the setup
screen, the tab bar and toolbar, the page backdrop, the start page, the
launch, the workspace panel with its panes, the reconnect overlay and the
account dialog.

- The page talks to the main process only through `window.oyaBrowser`
  (`src/preload/`), typed by `src/shared/ipc.ts`.
- Text and numbers live in each feature's `model/constants.ts` or in
  `core/constants.ts`.
- The control shield is the second page: plain TypeScript classes, no React,
  because it is imperative animation. Main drives it through
  `executeJavaScript('window.oyaShield?.(update)')`.

## Behavior worth knowing before you change it

- **A tab is protected before its first page.** It loads about:blank, sets up
  CDP (persona, stealth, dialog watcher, login state), and only then loads the
  real URL. The setup is raced against `CDP_SETUP_TIMEOUT`. A tab whose setup
  hangs still loads its page, and the log says loudly that it is unprotected.
- **`Page.enable` always comes with `dialogs.watch(dbg)`.** Without the
  watcher, the first `alert()` blocks that surface for good. There is one
  `Dialogs` (`ctx.dialogs`), shared by the tabs and the command runner.
- **Every result goes through `CommandRunner.sendResult`.** A command
  interrupted by a confirm() is answered straight away with the dialog. Its
  late answer is then dropped.
- **A persona switch drops the queued cookie changes.** It does not flush
  them. Then it closes every tab with `keepOne: false`.
- **Recording tasks run one at a time.** `Recorder.queueRecording` serializes
  them. `RecordingStart` remembers where a pause left each tab, so
  'start-recording' after a stop resumes the draft rather than starting fresh.
- **Only `ControlSocket.send` writes to the socket.** It never throws.
- **"Save as playbook" in Ask saves the server's copy of the run.** The server
  keeps each browser's latest agent run (`server/src/modules/agent/recorder.ts`),
  so the renderer sends only a name and offers the button on the newest reply
  alone, when the server's `replayable` says the run acted on a page.
  `REPLAYABLE_TOOLS` in `src/renderer/features/ask/model/constants.ts` mirrors
  the server's `RECORDED` list (the fallback for an older server); a unit test
  keeps them equal.
- **Ask lends control to the agent.** `send-chat` (`src/main/ipc/dev.ts`,
  through `connection/chat.ts`) returns control to the agent for the run and
  takes it back after if a person held it; otherwise every agent command is
  refused as a human takeover.
- **Start recording takes control.** Under agent control the control feature
  refuses page actions, except Start recording, which goes through the
  studio's `RecordingGate` and acquires control first: recording needs a
  person's hands on the page.
- **Browsing begins on the start page, with the panel closed.** `oya:home` is
  drawn by the shell, so there is no page view until the first address, and
  `layout.reveal()` opens the panel only when browsing starts on a real page.
- **The shell's own pages are never agent targets.** The front door's `isUi`
  hides `out/renderer/index.html` and `out/renderer/control-shield/index.html`
  from CDP harnesses; a regression check pins both addresses.

## Tests

| Command                                                       | Runs                                                                                     |
| :------------------------------------------------------------ | :--------------------------------------------------------------------------------------- |
| `npm test`                                                    | unit, then the node-only integration suites                                              |
| `npm run test:unit`                                           | `tests/unit/**/*.test.{js,cjs,mjs,ts}`                                                   |
| `npm run test:integration`                                    | regressions, control state, release guard, workflow model, the analyzer and recorder DOM |
| `npm run test:coverage`                                       | unit tests with a coverage report                                                        |
| `npm run typecheck`, `npm run build`                          | `tsc --noEmit`; electron-vite into `out/`                                                |
| `npm run test:shell`, `test:control`                          | the shell and the control handoff in real Electron; `test:shell` saves screenshots       |
| `npm run test:recording`, `test:workflow`                     | recording and workflow validation in real Electron                                       |
| `npm run test:identity`                                       | page identity vs request headers, passkeys, permissions, in real Electron                |
| `npm run test:agent`, `test:model-sync`, `test:routines-sync` | server commands, model settings and routines against a local or fake server              |
| `npm run test:sync`                                           | a login survives a server outage: real app and real local server                         |

The Electron suites that launch the app build first (`npm run build`) and
launch the browser folder, so they run what ships. Unit tests use `node:test`
with `node:assert/strict`: ViewModel tests use the fake bridge and
`mock.timers`, and main-process tests fake the Electron seams. They never touch
the network. `tests/integration/regressions.js` reads every
`src/main/**/*.ts` as text and checks that the guards above are still in
place. When you move one of those guards, update the pattern to follow the
move. Never loosen it.

## Exceptions

- **Page-side code** keeps its exact bytes: `scripts/analyzer.js`,
  `anonymity/stealth.js`, `fingerprint.js`, `inject.js`. The page can see it.
  So can the page scripts inside `src/main/actions/scripts.ts`,
  `src/main/tabs/context-menu.ts` (the inspector), `src/main/tabs/tab-events.ts`
  (the view-source theme), `src/main/mirror/device.ts` (the device probe) and
  `src/page/`. Those are kept byte for byte, with values slotted into marked
  holes; changing them is a stealth change.
- **`src/main/front-door/cdp-front-door.ts`** keeps the literals in
  `send(403` and `send(405` (with a lint-disable comment):
  `tests/integration/regressions.js` checks those exact guard lines.
- **`src/main/identity/host-rules.ts`** holds governance's host matcher, which
  must stay byte-identical with `server/src/modules/control/egress.ts`
  (`server/tests/integration/egress-rules.test.js` compares the two). That
  copy is untyped, so the file is `@ts-nocheck`, Prettier skips it, and the
  size and number lint rules are off for it. `Governance` (`governance.ts`)
  around it is ordinary typed code, built once in `main.ts` from
  `OYA_GOVERNANCE` and reached as `ctx.governance`.
- **The preload** runs sandboxed. It imports only `electron`; the contract is
  bundled in.
- **`src/page/login-state.ts`** is imported by the server's CDP driver too, so,
  like the rest of `src/page/`, it imports nothing.
- **The server image** copies `src/workflow/`, `src/page/`, `src/anonymity/`
  and `src/package.json` (root `Dockerfile`). Those folders import nothing from
  `src/main/` and no Electron or DOM types; `tests/unit/image-contents.test.cjs`
  and `server/tests/unit/drivers/vocabulary.test.ts` hold them to it.
- **The control shield** keeps its styles inline: its CSP allows only inline
  CSS.
- **Integration tests** under `tests/` are exempt from the size and number
  rules, as tests are everywhere. They still carry headers and doc comments.
