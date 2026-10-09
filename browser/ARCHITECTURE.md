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

## Required native agent-control architecture

**Target requirement; not yet satisfied by the current implementation.**

External agents may speak CDP to Oya's front door. That compatibility adapter
must terminate the protocol and dispatch browser-owned native operations:

`agent → CDP front-door adapter → authorized Oya operations → native browser APIs`

There must be no internal CDP transport, Electron debugger attachment, upstream
Chromium debugging endpoint, or CDP fallback. Engine reuse and the V8 JavaScript
engine are separate concerns; this requirement governs the control architecture.

### Migration inventory

- Native private contexts reserve their quota and ownership immediately, but stay
  absent from discovery, session lookup and target visibility until asynchronous
  setup completes. Cancellation/disconnect is checked again before publication;
  late setup cannot resurrect a revoked id. Retirement revokes access first and
  attempts network, downloads, each exact-session tab, connections, storage and
  cache cleanup even when another step fails. Failures propagate rather than
  claiming successful disposal. Late setup is cleaned again, and private-session
  tombstones remain. Unit and real Oya context tests cover the readiness boundary
  and late native-cookie writes; this does not enable production persona mode.

- External `Page.createIsolatedWorld` terminates in native owner-scoped V8 worlds,
  separate from the internal analyzer/recorder. Context tokens, value arguments,
  lifecycle notifications and release groups preserve exact tab/frame/world
  ownership. Named reuse is document-local, universal-origin access is rejected,
  and old engines fail explicitly. This does not supply worker contexts or
  replace the remaining persona/worker migration.

- The external native adapter provides machine-readable `Oya.getCapabilities`
  from its dispatch/validation tables; it explicitly reports partial compatibility
  with native availability checked at execution, not universal CDP support.
- Native tab-history snapshots use connection-owned capabilities and actual native
  navigation history. Traversal verifies unchanged state and destination policy;
  serialized page state is never exported. These are Oya extension commands.
- Native inherited property inspection uses V8 prototype descriptors, not an
  inspector: bounded walks, shadowing/symbol identity, uncalled accessors and proxy
  rejection. The new opcode fails explicitly on engines lacking the patch.
- Native request failures map supported names to actual net errors on the original
  held continuation. Engine capability, exact frame ownership, human control and
  egress remain checked before consumption. This does not add response fulfillment.

- Native cookie management (`main/native-cookies/`) uses the cookie manager of an
  explicitly owned private session. Browser-level Storage commands and the
  URL/name deletion extension recheck ownership/control around native awaits.
  Engine-provided canonical metadata prevents fabricated priority/source fields.
  MacOS integration and live-app tests cover HttpOnly delivery, scope and isolated
  clearing. Unsupported partition semantics fail explicitly; Windows is unverified.

- External Runtime context routing now includes native cross-process child frames.
  `main/native/runtime-contexts.ts` binds opaque context IDs to exact native
  document tokens, while `runtime-values.ts` routes engine handles without a
  page-global registry. Frame identities are shared with the native frame-tree
  service. Native readiness/navigation/removal events drive context lifecycle;
  child replacement/removal preserves surviving siblings. The macOS front-door
  test verifies these behaviors with debugger access forbidden. Worker contexts,
  full Debugger semantics and Windows validation remain unfinished.

- Pointer input: `main/input/mouse.ts` and pointer dispatch in
  `main/actions/pointer-commands.ts` now use native `sendInputEvent`, with no
  debugger fallback. `npm run test:native-input` launches Oya and makes debugger
  access fatal while testing trusted clicks, double-clicks, held drags and scroll
  direction. This does not yet make the full application CDP-free.
- HTML drag/drop: `main/input/native-drag.ts` uses the patched engine's `_dragOya`
  operation, not a sequence that leaves an uncontrolled OS drag active. The engine
  owns renderer-filtered payloads and native drop acknowledgements. Focus loss,
  navigation, destruction and concurrent gestures fail closed. Unsupported
  child-frame/file drags are explicit errors; no CDP or DOM-event fallback exists.
  `test:native-drag` covers trusted events, payload delivery, refusal, cancellation,
  and ordinary slider dragging. Patched macOS runtime tested; Windows is unverified.
- Keyboard: `main/input/keyboard.ts` now uses native key events, text composition
  and select-all editing. Real Oya tests cover Unicode, Enter submission and
  focused iframe input with debugger access forbidden. Windows runtime validation
  is still required.
- Page execution and inspection: `main/native/world.ts` now uses native isolated
  execution for the main-frame analyzer. The old `main/cdp/world.ts` is only a
  compatibility export, not a protocol implementation. Tests verify that the real
  analyzer is hidden from the page's global and reinstalls after navigation.
  Explicit main-world evaluation and PNG/JPEG capture are native as well.
  The patched engine now provides native isolated execution in cross-process
  frames through `main/native/frames.ts`; its real Oya regression forbids debugger
  access and covers duplicate frame URLs, isolation and disposed-frame rejection.
  Pending native evaluations are document-scoped too: removal, replacement and
  tab destruction cancel them, and unresolved promises have a ten-second engine
  deadline. Native lifecycle errors remain distinguishable from page exceptions.
  This is a prerequisite, not a migrated recording transport.
  Cross-origin frame recording and pre-script worker protection remain separate,
  unmigrated capabilities; main-frame isolation does not replace them.
- Shell motion: background/container animation control now uses a local UI policy
  and native execution, not debugger media emulation. First-paint styles and reload
  handling preserve the policy without changing system or website media queries.
- Native keyboard provenance: the production shortcut fence distinguishes a
  synchronous browser-owned key dispatch from human input on the exact target
  contents. The grant is revoked before native dispatch returns, including
  exceptions and nested calls; it never spans an async typing run. Native keys
  do not trigger shell shortcuts. The native input suite now uses a layered
  BrowserView, shell focus, the real shortcut fence, and an overlaid shield;
  it verifies agent field entry and that unmarked keys remain blocked. Earlier
  single-window tests did not cover this production ownership interaction.
- Element commands: production click/type refuse covered targets rather than
  using synthetic click replay or typing into an overlay. Hit testing checks the
  target document and containing frames; iframe contenteditable selection uses
  the editor's own document. The native input suite exercises these production
  `PageDriver` commands with fresh analyzer IDs and debugger access forbidden.
  This local command-path fixture is not the authenticated end-to-end agent loop.
- Native dialogs: production startup now installs `main/dialogs/DesktopDialogs`
  on tabs and sign-in popups using browser-owned callbacks, not debugger events.
  Alerts become notifications; confirmations, prompts and before-unload decisions
  route to the live human/agent owner. Human sheets use a sandboxed, isolated
  private session and exact-frame IPC, with literal text, safe keyboard defaults
  and no network. Takeovers transfer pending decisions without answering them.
  Replacement navigation declines obsolete prompts before their modal sheets can
  block it; same-document navigation and before-unload warnings are preserved.
  Real Oya tests cover the production router/UI, trusted keyboard input, IPC
  refusal, navigation, cancellation and unsaved changes. Native tab retry also
  avoids debugger access. Windows validation and other legacy identity/recording
  paths remain release gates; this does not make the whole default app CDP-free.
- Recording transport: production tabs and popups install a sandboxed preload with a bounded,
  fixed-channel sender only in the agent isolated world. `NativeRecordingInbox`
  checks owned web contents, recording epochs and explicitly authorized isolated
  preload document identities. Immutable URL/owner-path snapshots keep final unload
  batches attributed to the original document; unarmed replacements are refused.
  Real Oya tests verify that the
  analyzer flushes final typing from cross-process frames during navigation.
  Engine-owned local/remote frame tokens now resolve ordinary and open-shadow
  owner paths without index or URL guesses, with token checks across the entire
  asynchronous ancestry walk. `NativeDocumentRecorder` now owns document-guarded
  start/drain/stop, bounded readiness and cancellation before DOM-ready. Its stop
  returns final typing directly, avoiding a final IPC race with inbox teardown.
  Production RecordingChannels now uses NativeRecordingChannel, with native
  frame readiness and removal subscriptions for navigation and dynamically added
  cross-process frames. Final drain retains the originating tab identity after
  removal. Admission retains at most 1,024 document authorizations per recording;
  overflow reports an explicit capture issue rather than evicting unload attribution.
  The bridge loads before website scripts; analyzer capture starts at DOM-ready,
  not before website execution. The complete desktop recording integration
  fixture now exercises native input, analyzer coexistence, secret masking,
  clear/discard, hidden controls, native file selection and same-origin frames.
  Tab creation no longer reads a debugger to track recording frames, and
  no-persona setup no longer auto-attaches targets just for recording. Native
  multi-window tests forbid debugger access on every application renderer.
  The legacy server recording channel remains migration debt.
- Focused text: the patched engine routes `insertText` to the focused native
  widget, fixing a renderer crash with cross-process frame focus. The regression
  records real native text input without debugger access.
- Other observation: dialogs, telemetry and before-unload integration still need
  their remaining native lifecycle migrations.
- The experimental native engine now offers an immutable session user-agent string
  with worker-startup and network-context ownership; `test:native-user-agent`
  exercises first-script values and real headers without a debugger. This does
  not activate complete native personas. The metadata prerequisite now binds
  native low/high-entropy `navigator.userAgentData` to the session; native HTTP
  client-hint negotiation and `navigator.platform` remain separate work.
- Page protection: persona application, worker coverage and tab startup currently
  rely on debugger commands. Preserve existing isolation and egress guarantees;
  capabilities absent from public Electron APIs may require native engine work.
- Workflow target selection now uses native pre-dispatch mouse/key interception
  and a dedicated isolated world, without debugger attachment. Real Oya tests
  cover hover, selection without click-through, Escape cleanup and 200% zoom.
  The highlight intentionally adds a temporary DOM element while the user picks;
  frame targets remain explicitly unsupported, as before. Validation automation
  workers still need migration to the authorized native operation surface.
- Front door: the current implementation proxies an upstream debug endpoint.
  Replace it with an explicit supported-method adapter, Oya-owned target/session
  identifiers and native events. Reject unsupported methods rather than forwarding.
- Tests: `test:analyzer` now runs real DOM checks in Oya, including nested frameset
  reading and a trusted click through the production page command. The opt-in
  `OYA_NATIVE_ENGINE=/path/to/Oya npm run test:internet` exercises the public
  the-internet.herokuapp.com site, saves native screenshots and reports, inventories
  every example, and fails on missing behavioral coverage as well as failed checks.
  Behavioral cases now exist for all 44 catalog entries; defined coverage is not a
  passing live result. Reports retain completion status, screenshots, and native
  network statuses so upstream failures are visible. `OYA_AUDIT_PROFILE` may reuse
  only a previous disposable audit profile (never the person's normal profile).
  It is not a substitute for full agent-loop or Windows testing. Remaining
  Chrome/Playwright-backed browser fixtures still need migration. Keep compatibility
  protocol tests at the front door, separate from native core integration tests.
  The server's aggregate integration command now fails its preflight while its
  configured fixtures contain known non-Oya executable launchers. This is an
  explicit migration blocker, not a skip or proof of complete boundary enforcement.

### Acceptance gates

Exercise normal browsing and the agent loop with debugger attachment and debugger
command dispatch made fatal in tests. Cover input, isolated page inspection,
recording, frames/workers, navigation, dialogs, downloads, screenshots, permissions,
profile isolation and human-control handoff. Run the native desktop integration
matrix on macOS and Windows. Add static dependency enforcement against internal
CDP and tests proving that the front door has no upstream debugging connection.
Existing behavior must not silently disappear during this migration. Passing the
old CDP-backed tests alone does not satisfy these gates.

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
    library/             local history and bookmarks, scoped by persona partition, plus the native Library menu
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

## Everyday browsing

- Closing the selected tab returns to the most recently selected still-open tab,
  not its neighbor in strip order. Closing a background tab does not move focus.
- Selecting a new start-page tab focuses the shell and address bar; the agent
  task box no longer takes focus on arrival.
- **Library** in the toolbar or native menu opens local bookmarks and recent
  history. Cmd/Ctrl+D toggles the active page's bookmark; Cmd/Ctrl+Y opens the
  library. Selecting a saved page opens a new tab.
- `library/BrowsingLibrary` persists through `ConfigStore`, keyed by the persona's
  session partition. There is no background library sync; authorized agent tool
  calls return only the requested results through the normal command transport. History keeps the latest
  2,000 distinct HTTP(S) addresses; bookmarks remain until explicitly removed.
  Internal pages and URLs containing credentials are excluded. Clearing history
  requires confirmation and does not clear bookmarks or website logins.
- This first native library has paginated menus, not full-text search, folders,
  or import/export. Passkey behavior is unchanged by these browsing changes.

### Agent access to browser features

The agent loop and per-browser MCP expose `search_history`, `list_bookmarks`,
`add_bookmark`, `remove_bookmark`, `clear_history`, `list_closed_tabs`, and
`reopen_closed_tab`. Existing tab and navigation tools open returned URLs.
Library searches accept `query`, `limit` (default 25, maximum 100), and `offset`,
and return `entries`, `total`, and `next_offset`. Bookmark adds/removes are
idempotent; `clear_history` requires `confirm: true` after an explicit user
request, and leaves bookmarks, cookies and the separate closed-tab stack alone.

`connection/library-commands.ts` dispatches through the same command runner and
control gate as other tools, never through the human-only native menu. Queries
resolve the active persona at execution time; there is no cross-profile selector.
Reopening a closed tab cannot bypass remote restrictions on local files. Library
results are website data, not instructions, and are not recorded as replayable
page actions. Generic CDP browsers do not support these tools. The round-robin
pool MCP endpoint deliberately omits profile-local library tools: use the
per-browser endpoint to make the target unambiguous.

## Browser workspace additions

- The Playbooks pane uses `playbooks` IPC to list, rename, delete, transfer and
  replay the current project's saved playbooks. Runs target this desktop and
  use the existing server API. The main-process replay supervisor owns the
  Ask/routine exclusion until the run ends, even when the pane is closed.
- Models remain project settings. The command menu opens the same editor as
  Ask, including the optional API endpoint; saved credentials are never returned.
- First-login import is an explicit invitation. The `ui.importOffered`
  preference remembers acceptance or dismissal without reconnecting. Discovery
  prefers the default supported browser, including Windows App Paths and system
  installation directories. Partial profile failures remain visible.
- The control shield watches human clicks, keyboard input and substantial pointer
  motion. It offers a native takeover confirmation, deduplicated while open and
  rate-limited for pointer motion. Blocked input is never replayed after takeover.

## Tests

| Command                                                       | Runs                                                                                             |
| :------------------------------------------------------------ | :----------------------------------------------------------------------------------------------- |
| `npm test`                                                    | unit, then the node-only integration suites                                                      |
| `npm run test:unit`                                           | `tests/unit/**/*.test.{js,cjs,mjs,ts}`                                                           |
| `npm run test:integration`                                    | regressions, control state, release guard, workflow model, the analyzer and recorder DOM         |
| `npm run test:coverage`                                       | unit tests with a coverage report                                                                |
| `npm run typecheck`, `npm run build`                          | `tsc --noEmit`; electron-vite into `out/`                                                        |
| `npm run test:shell`, `test:control`                          | the shell and the control handoff in real Electron; `test:shell` saves screenshots               |
| `npm run test:recording`, `test:workflow`                     | recording requires explicit Oya native engine; legacy workflow validation remains migration debt |
| `npm run test:identity`                                       | page identity vs request headers, passkeys, permissions, in real Electron                        |
| `npm run test:agent`, `test:model-sync`, `test:routines-sync` | server commands, model settings and routines against a local or fake server                      |
| `npm run test:sync`                                           | a login survives a server outage: real app and real local server                                 |

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

### Keyboard-first shell

`src/shared/shortcuts.ts` is the binding registry used by native keyboard
handling, palette hints and the searchable Keyboard shortcuts guide. Keep new
bindings there, with a matching main-process or palette action. Modifiers match
exactly; physical aliases keep Option chords and tab selection layout-safe.
The footer opens Commands or Keyboard shortcuts (Cmd/Ctrl+/ or F1); the guide
uses the existing modal focus trap and hides native page views while open.
`FOOTER_HEIGHT` in shared constants reserves the same space in native page
bounds and renderer CSS, including the compact workspace layout.

The read-only `list_keyboard_shortcuts` agent/MCP tool obtains the connected
browser's live shortcut registry and platform through the normal command path.
It describes shell shortcuts; it does not execute them or grant shell control.

### Default browser opt-in

The welcome screen, command palette and Help menu expose Make Oya Browser
default. Shell-only IPC invokes `app/default-browser.ts`; it refuses development
builds, checks both HTTP and HTTPS, and leaves Windows choice to Default Apps.
Installation advertises support but never writes Windows UserChoice. The NSIS
include registers Oya-owned capabilities and removes those on uninstall. Linux
AppImage users still need an installed desktop entry. OS link delivery (macOS
open-url and Windows/Linux argv) uses DeepLinks, including startup queuing;
web links open new protected tabs and cannot interrupt agent control.

### Address completion

The shell-only `address-suggestions` IPC reads the current profile's local
library; it never contacts a search provider. `AddressCompletion` owns result
ordering fences, selection and the dedicated `address` overlay. Overlay calls
are serialized so closing a pending screenshot-backed overlay cannot hide the
native page permanently. The input is an ARIA combobox with a listbox, keeps
focus while arrow keys select rows, and ignores Enter during IME composition.

### Session notifications

`main/notifications/` retains a bounded, memory-only alert inbox for the current
profile. CDP alerts are accepted and copied here, while confirm/prompt behavior
and agent dialog notes remain unchanged. Profile changes discard prior entries.
The shell-only IPC exposes list/read/dismiss; change events carry no private text.
The toolbar bell never steals focus. Its explicit inbox uses a separate native
overlay owner and the shared focus-trapped Dialog. Cmd/Ctrl+Alt+J opens it and
appears in both the keyboard guide and the agent-readable shortcut registry.

The same inbox is available through `list_notifications` (bounded pagination,
unread filtering, no read side effects), `mark_notifications_read` (explicit IDs),
and `dismiss_notification` (one ID plus confirmation). These commands use normal
socket control admission; neither reads another profile nor opens the inbox UI.

### External meeting app links

`app/external-apps.ts` handles only validated `zoommtg://*.zoom.us/join` links,
including the apex host. Tab/frame redirects and window-open requests are stopped
before opening a broken web tab; shell address-bar input uses the same handler.
A native Cancel-default confirmation is required for every launch. Only the
active, human-controlled surface in an unmanaged browser can hand off, with
control and source rechecked after the prompt. The normal remote navigate/open_tab
HTTP(S) allowlist is unchanged. No meeting credentials enter logs or prompt text.

### Call media permissions

Microphone/camera requests use `app/media-permissions.ts`: explicit native site
consent, then macOS device consent. Grants are in-memory and document-scoped;
checks reject other origins, navigation, agent control and governed runtimes.
Only active HTTPS same-origin requesters can ask. Media capture is never globally
granted. Existing streams remain under the site's stop/mute controls; these
checks govern new access, not forced termination of existing calls. Ordinary
playback/autoplay policy is unchanged. Direct unmanaged desktop personas retain
native WebRTC ICE; proxies and managed sessions keep the leak-prevention wrapper.

### Native passkeys

Desktop page protection no longer installs the synthetic WebAuthn cancellation
shim. Chromium owns RP validation, registration, assertion and abort handling.
`app/passkeys.ts` supplies a Cancel-default native account chooser for Electron's
`select-webauthn-account` event. Choices are limited to the current human-controlled
surface and cancelled on navigation, destruction or timeout. Credentials and
private keys are never exposed through renderer IPC, notifications or agent tools.

Provisioned macOS builds use `APPLE_TEAM_ID` and `OYA_WEBAUTHN_PROFILE` to configure one stable keychain access group.
The build embeds it for `app.configureWebAuthn` and the packaging hook adds the
matching main-app entitlement without granting it to helper processes. Builds without a supplied provisioning profile do not configure signing-bound Touch ID. Electron owns the per-session
metadata secret, so credentials stay partition-scoped and device-bound.

**Not Chrome parity:** Stock Electron 44's native implementation does not expose iCloud
Keychain or phone/QR authentication, and security-key PIN entry remains unsupported.
The native identity test uses a disposable CDP virtual authenticator in Oya for
registration and assertion; it does not certify physical Touch ID, iCloud or Gmail.
Those require a signed build and real-device verification before claiming a fix.

`engine/` carries an opt-in, version-pinned macOS iCloud discovery patch and its
build/verification checklist. The full native engine now builds and is signed, but macOS rejects the restricted-entitlement test app without a matching provisioning profile; real-credential verification remains blocked. `OYA_NATIVE_PASSKEYS=1` adds the main-app browser credential
entitlement at packaging time; it does not activate a stock runtime. The patched
engine separately requires `--oya-native-passkeys`. Release defaults stay off.

All six native phone-policy tests pass; this is not physical-device verification.
The second experimental Chromium patch permits the native nearby-phone option
only for hybrid-eligible assertions, with six native policy tests. This targets
phone-held Google Password Manager credentials, not desktop vault synchronization.
Opted-in macOS packaging includes a Bluetooth usage explanation; permission stays
with the OS. Hybrid-only discovery, real-device success and Windows phone behavior
remain unverified; see the engine checklist before enabling either patch.

### Isolated native sign-in diagnostic (development only)

`--oya-native-signin-test` starts a human-only Oya window at Gmail, not the
normal desktop boot. It uses a fresh temporary user-data directory and an
in-memory session. No persona, user-agent override, page injection, CDP port,
agent connection, renderer IPC, cookie sync, or recording is installed. The
window is sandboxed, denies permissions and popups, and displays the actual
origin in its title. Closing it discards the in-memory login; this is not a
way to import Gmail cookies into the normal browser.

Packaged, governed, Docker, and explicitly debug-enabled launches refuse this
mode. It is an experiment to separate Google browser rejection from Oya's
identity emulation, **not a Gmail fix or native passkey validation**. The normal
browser and experimental engine patches remain unchanged by the flag. Google
rejected the normal development build on 2026-10-08 before a passkey ceremony;
its support article (https://support.google.com/accounts/answer/7675428) lists
embedded and automated browsers among possible causes, not an exact diagnosis.

The separate development flag `--oya-native-browsing` exercises the **normal
Oya shell and persistent profile** with native engine identity. It skips eager
CDP attachment, persona/worker emulation, main-world injection and automatic
analyzer startup. Explicit agent analysis still uses the existing control gate
and isolated-world analyzer on demand. The browser session keeps its permission
checks and ordinary profile-local storage; no diagnostic cookies are copied.
Governed, Docker, debug-port and packaged launches refuse this trial, and a
proxied persona fails session setup rather than silently exposing native identity.

This flag is not a default or a Gmail acceptance claim. The user successfully
signed in through an authenticator alternative in the isolated diagnostic;
normal-shell login and session persistence need separate verification. Native
mode does not eagerly install the CDP dialog-to-notification observer or remote
localStorage mirroring. Agent/recording use can attach instrumentation later;
repeat sign-in after agent use needs testing before production enablement.

### Multiple browser windows and live tab transfer

`main/windows/BrowserWindows` owns a per-window dependency graph: a concrete
ShellWindow, TabManager, PanelLayout, Overlays, Shortcuts and ControlShield.
`WindowContext` forwards shared task/session writes to the application root;
there is still one profile, cookie jar, recorder, control socket and control gate.
The application-level service router is an explicit multi-window dispatch boundary:
agent tab enumeration is global, id-based selection/close finds the owning window,
shared shell events/control state broadcast, and other commands use the selected
application target. Window-scoped IPC never uses a mutable global focus variable.
Only a registered shell's exact main frame can invoke shell commands.

A tab keeps its id, BrowserView, webContents and once-installed event listeners.
Transfer removes/adds that same view, without navigating, detaching CDP or forgetting
recording. Tab callbacks and keyboard input resolve the current owner after a move.
Browser windows have independent tab selection, overlays and panel layout. A native
close cannot destroy an agent-controlled window; application quit still uses the
existing flush lifecycle. Closing a window closes only its own tabs and shield.
Moving its last tab closes the empty source window. OAuth popups remain real windows
and cannot be torn off during verification.

Dragging out of the strip creates a full browser window. Dropping on another visible
Oya tab strip appends the live tab there. Screen coordinates come from Electron, in
DIPs, and new windows are clamped to the destination display's work area. Escape
cancels the gesture. Transfers are refused while either window has an open overlay
or the agent owns control. Cmd/Ctrl+N opens a window; Cmd/Ctrl+Alt+W moves the selected
tab to a new window. Both are in the footer guide and `list_keyboard_shortcuts`.
`list_tabs` adds `window_id` while existing tab ids and switch/close tools keep working.

`npm run test:windows` launches only the Oya runtime with a disposable local profile
and localhost pages. It exercises real pointer tear-off, Cmd/Ctrl+N, form/session
storage/history preservation, transfer back, independent close and IPC isolation.
The native test has been run on macOS; Windows GUI/mixed-DPI smoke testing remains
required before claiming Windows end-to-end verification. Window layouts are not
persisted/restored across application restarts by this change.

Window presentation is transactional: a new destination remains hidden while its
real shell fonts, selected tab, and URL field become measurable and cross a paint
boundary. A metadata-only staged tab never becomes a second owner of the page.
The source retains its mounted BrowserView until readiness succeeds; failure
removes the hidden destination instead of consuming the original tab.

Tab tear-out has a separate sandboxed, non-focusable native preview surface. It
uses no preload or IPC bridge, shows a local in-memory page thumbnail, follows
native screen coordinates across displays, and is excluded from agent targets.
Cancel/reorder removes it immediately; drop keeps it until the destination is
presented. The real Oya windows regression checks preview focus/cleanup and chrome
readiness at the destination's first native show call, alongside live page preservation.

### Experimental native CDP compatibility listener

`main/native-front-door/` is a separate, explicitly enabled native-only external
adapter (`OYA_NATIVE_CDP_PORT` plus a bearer credential). `app/native-cdp.ts`
composes it with existing protected tabs and control admission. It does not use
the legacy front-door proxy or require an engine debugging port. DOM inspection,
frame-tree snapshots, native console logs and device metrics are connection-scoped;
DOM ids cannot alias replacement documents, subscriptions stop on disconnect,
and viewport overrides restore native defaults. This is partial compatibility,
not full CDP parity or proof that the entire application is CDP-free. See its
README for supported methods and outstanding native-engine work.

### Native persona storage and final capture

In the native-browsing path, `main/sync/PersonaStorage` owns one immutable native
session binding per authenticated persona. It validates all imports before
mutation and only hydrates a cold partition before any surface is exposed. Warm
partitions (even empty ones) keep their local state rather than resurrecting a
logged-out token. Application-owned native navigation events discover first-party
origins across windows and popups; subframe origins are not exported as
first-party storage. Switching persona retires observers and fences late reads.

Native mutations are coalesced and delivered in batches without page scanning.
Offline/refused sends retain the newest snapshot, including empty logout state.
Reconnect and manual save await capture before requesting profile persistence.
Native clients advertise `profile_sync`; the authenticated server may request a
correlated `profile_capture`. It receives ordered cookie and storage updates
before the result. Server Stop waits for that result and encrypted persistence;
failure does not silently stop the browser. This protocol is storage-only and
never grants agent control of a page. The packaging probe verifies actual native
storage operations, not just the presence of function names.

This is integrated in the native-browsing path, not evidence that the default
legacy path, native worker/persona protection, or platform release matrix is done.
