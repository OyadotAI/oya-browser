# Browser architecture

Oya Browser is an Electron app. On a desktop, a person uses it as their own
browser. In a sandbox, the same code runs headless (under Xvfb) as a cloud
browser. Either way it dials the server's control socket and runs as one
_persona_: a fingerprint, a cookie jar and a proxy, bound together. It is
plain JavaScript (CommonJS, with a few `.mjs` tests), so there is no type check
and no build step besides electron-builder.

The shared standards (size limits, doc comments, no magic numbers) are in the
root [ARCHITECTURE.md](../ARCHITECTURE.md), and `npm run lint` enforces them.
This file covers what is specific to the browser.

## Processes

| Where it runs | Files                                                   | What it is                                                              |
| :------------ | :------------------------------------------------------ | :---------------------------------------------------------------------- |
| Main process  | `main.js`, `main/`, `*.cjs` and `*.js` at the top level | Windows, tabs, the control socket, recording, CDP                       |
| Shell page    | `renderer/`, `preload.js`                               | The toolbar, tab strip and dev panel (`window.oyaBrowser`)              |
| Visited pages | `scripts/analyzer.js`, `anonymity/`                     | The page reader and the fingerprint patches (page-side, see Exceptions) |
| Workers       | `scripts/workflow-worker.cjs`                           | Workflow validation, run in a utility process                           |

## Layout

```
main.js                 composition root: pre-ready flags, builds every service into ctx, wires Electron's events
preload.js              the shell's bridge: window.oyaBrowser → IPC channels (sandboxed; requires only electron)
launch.cjs              `npm start`: a branded dev copy of Electron on macOS
cdp-front-door.js       the CDP endpoint harnesses use (OYA_REMOTE_DEBUGGING_PORT); facade over front-door/
front-door/             the front door's HTTP routes (door.cjs) and its WebSocket bridge (bridge.cjs)
control-state.cjs       who drives: agent or person, the handoff protocol, the local automation gate
shell-layout.cjs        pure geometry: where the page and the dev panel go
login-state.js          localStorage transport, shared with the server's CDP driver (kept standalone)
governance.js           managed egress rules; its matcher is byte-identical with the server's
constants.cjs           numbers for the top-level files
main/
  app/                  the app itself
    config-store.cjs    repository: config.json, with environment overrides
    persona.cjs         the active profile, its session (partition) and its login state
    deep-links.cjs      oya:// sign-in links, from every route the OS delivers them by
    lifecycle.cjs       boot order once ready, and what must finish before quit
  shell/                the window
    window.cjs          the shell window, send() to its page, the dev log
    menu.cjs, shortcuts.cjs   the application menu and keyboard shortcuts (command maps)
    control-shield.cjs  the transparent view that keeps human input off the page while an agent drives
    layout.cjs          page bounds and the dev panel's slide and width
    overlays.cjs        overlays over the page, with a screenshot backdrop
  tabs/                 the tabs
    tabs.cjs            TabManager: open, close, switch, reload, browsing mode
    tab-events.cjs      the listeners a tab gets, and its bounded first-page protection
    protection.cjs      persona, dialog watcher, login state and isolated world on a tab or popup
    navigation.cjs      the address bar
    context-menu.cjs, page-source.cjs   the right-click menu and the source pane
  recording/            demonstrations recorded as playbooks
    recorder.cjs        Recorder: steps in, limits, stop, clear, the serialized task queue
    start.cjs           starting and resuming (the stages of a start)
    channels.cjs        one RecordingChannel per tab
    tab-names.cjs, publish.cjs   step tab names, and saving to the server
  connection/           the control socket
    socket.cjs          ControlSocket: auth, ordered message queue, heartbeat, backoff
    server-messages.cjs command map: server message type → handler
    commands.cjs        CommandRunner: runs `cmd`s, races a JS dialog, sends each result once
    tab-commands.cjs    command map: tab, recording and workflow commands
    result-summary.cjs, server-api.cjs   the dev-log view of a result, and HTTP calls to the server
  ipc/                  what the shell page may call
    index.cjs           facade: registers every table through handle.cjs (shell-only guard)
    navigation.cjs, session.cjs, recording.cjs, workspace.cjs, shell.cjs, dev.cjs   channel → handler tables
  page-actions.cjs      facade over actions/: the agent's page commands (createPageActions)
  actions/              PageDriver and its command maps (page, pointer, dev panel), page script text
  input.cjs             facade over input/: human-like keyboard and mouse over CDP
  cdp.cjs, world.cjs, dialogs.cjs   CDP on a view, the analyzer's isolated world, native JS dialogs
  cookie-sync.cjs, session.cjs      the cookie pool sync, and the Electron session's UA, hints and proxy
  cdp-relay.cjs, stream.cjs         CDP relayed over the control socket, and the live view
  pairing.cjs, auth-popup.cjs, updater.cjs
  constants.cjs         numbers for the top-level main/*.cjs files (each folder has its own)
renderer/               the shell page (plain browser scripts, see "How the shell page is built")
  index.html            loads the scripts below in order
  core/                 constants, Dom helpers, ShellState (shared state)
  shell/                icons, dialogs, layout, tab strip, commands, theme
  connection/           setup, status, reconnect, profile, updates, the connection toolbar
  panes/                the Agent panel's Ask (chat) and Inspect tools (actions, activity, source), resize
  studio/               the workflow studio: view, steps, editor, variables, run, actions
  control.js            ControlBar: who drives the browser, Take control, the watch-only guard
  ready.js              sets window.shellIcon last; the Electron tests wait for it
scripts/, anonymity/    page reader, workflow worker, fingerprint patches (see their headers)
tests/
  unit/                 node:test, hermetic; mirrors the source (main/tabs/tabs.cjs → tests/unit/main/tabs/tabs.test.cjs)
  integration/          regressions.js, control-state.cjs, release.cjs, workflow.cjs (in npm test);
                        shell.mjs, control.mjs, recording-electron.cjs, workflow-electron.cjs (real Electron);
                        cdp-front-door.mjs (against a running container, by hand)
  support/              fakes.cjs (debugger, webContents, view, electron module), main-ctx.cjs (a fake ctx), page.cjs
```

## How the main process is built

`main.js` builds one object, `ctx`, and puts every service on it by name:
`ctx.config`, `ctx.shell`, `ctx.tabs`, `ctx.persona`, `ctx.recorder`,
`ctx.socket`, `ctx.commands`, and so on. Each service class takes `ctx` in its
constructor and reaches its neighbours through it when it runs. This is the
composition root. It is a context object rather than constructor parameters
because the services call each other in a cycle: tabs join recordings, the
recorder reads tabs, the socket applies personas, and the persona closes tabs.
Two rules keep this readable:

- A service touches another only through that service's methods, never its
  internals.
- Electron comes in as `ctx.electron`, so a test hands over a fake instead
  (`tests/unit/support/main-ctx.cjs`).

The patterns in use:

- **Command maps** (`Object.hasOwn`-guarded):
  - server messages (`connection/server-messages.cjs`)
  - server commands (`connection/tab-commands.cjs`, `actions/*-commands.cjs`)
  - IPC channels (`ipc/*.cjs`)
  - workspace commands (`ipc/workspace.cjs`)
  - shortcuts (`shell/shortcuts.cjs`)
  - the front door's routes (`cdp-front-door.js`) and browser-level CDP (`front-door/bridge.cjs`)
- **Facades**: `page-actions.cjs`, `input.cjs`, `cdp-front-door.js` and
  `ipc/index.cjs`. The older factories (`createCookieSync`, `createWorld`,
  `createStream`, `createCdpRelay`, `createUpdater`, `createPageActions`)
  keep their signatures, with classes or small named steps behind them.
- **Repository**: `app/config-store.cjs` is the one place that reads or writes
  config.json. User-chosen exports go through `ipc/files.cjs`.
- **Stages**: boot (`app/lifecycle.cjs`), starting a recording
  (`recording/start.cjs`), and protecting a new tab (`tabs/tab-events.cjs`)
  are sequences of named steps.

## How the shell page is built

The shell page has no bundler. Each file is a classic `<script>` that defines
one global object (`Dom`, `ShellState`, `Chat`, `StudioView`, `ControlBar`,
…) and lists what it uses in a `/* global */` line. Load order in
`index.html` is the dependency order: `core/` first, then `shell/`,
`connection/`, `panes/`, `shell/theme.js` (it needs the panes), `studio/`,
`control.js`, and `ready.js` last. A new file goes after everything it uses.

- The page talks to the main process only through `window.oyaBrowser`
  (`preload.js`).
- Text and numbers live in named tables at the top of a file
  (`CONTROL_LABELS`, `STAGE_TEXT`) or in `core/constants.js`.
- Views redraw a part only when its input changed (see `Studio.signatures`).

## Behavior worth knowing before you change it

- **A tab is protected before its first page.** It loads about:blank, sets up
  CDP (persona, stealth, dialog watcher, login state), and only then loads the
  real URL. The setup is raced against `CDP_SETUP_TIMEOUT`. A tab whose setup
  hangs still loads its page, and the log says loudly that it is unprotected.
- **`Page.enable` always comes with `attachDialogWatcher`.** Without the
  watcher, the first `alert()` blocks that surface for good.
- **Every result goes through `CommandRunner.sendResult`.** A command
  interrupted by a confirm() is answered straight away with the dialog. Its
  late answer is then dropped.
- **A persona switch drops the queued cookie changes.** It does not flush
  them. Then it closes every tab with `keepOne: false`.
- **Recording tasks run one at a time.** `Recorder.queueRecording` serializes
  them. Each task is passed the previous task's result, so 'start-recording'
  after a stop resumes the draft rather than starting fresh. This has always
  been the case; keep it unless you change it on purpose.
- **Only `ControlSocket.send` writes to the socket.** It never throws.
- **"Save as playbook" in Ask saves the server's copy of the run.** The server
  keeps each browser's latest agent run (`server/src/modules/agent/recorder.ts`),
  so the renderer sends only a name and offers the button on the newest reply
  alone, when the server's `replayable` says the run acted on a page.
  `REPLAYABLE_TOOLS` in `renderer/panes/chat-playbook.js` mirrors the server's
  `RECORDED` list (the fallback for an older server); a unit test keeps them equal.
- **Ask lends control to the agent.** `send-chat` (`main/ipc/dev.cjs`) returns
  control to the agent for the run and takes it back after if a person held it;
  otherwise every agent command is refused as a human takeover.
- **Start recording takes control.** Under agent control the guard in
  `renderer/control.js` refuses page actions, except Start recording, which
  acquires control first: recording needs a person's hands on the page.

## Tests

| Command                                   | Runs                                                            |
| :---------------------------------------- | :-------------------------------------------------------------- |
| `npm test`                                | unit, then the node-only integration suites                     |
| `npm run test:unit`                       | `tests/unit/**/*.test.{js,cjs,mjs}`                             |
| `npm run test:integration`                | regressions, control state, release guard, workflow model       |
| `npm run test:coverage`                   | unit tests with a coverage report                               |
| `npm run test:shell`, `test:control`      | the shell and the control handoff in real Electron (Playwright) |
| `npm run test:recording`, `test:workflow` | recording and workflow validation in real Electron              |

Unit tests use `node:test` with `node:assert/strict`. They fake the Electron
seams and use `mock.timers` for anything time-based. They never touch the
network: `fetch` is mocked, and sockets are fakes. `tests/integration/regressions.js` reads
`main.js` and every `main/**/*.cjs` as text and checks that the guards above are
still in place. When you move one of those guards, update the pattern to
follow the move. Never loosen it.

## Exceptions

- **Page-side code** keeps its shape: `scripts/analyzer.js`,
  `anonymity/stealth.js`, `fingerprint.js`, `inject.js`. The page can see it.
  So can the page scripts inside `main/actions/scripts.cjs`,
  `tabs/context-menu.cjs` (the inspector) and `tabs/tab-events.cjs` (the
  view-source theme). Those are kept byte for byte, with values slotted into
  marked holes.
- **`cdp-front-door.js`** keeps the literals in `send(403` and `send(405`
  (with a lint-disable comment): `tests/integration/regressions.js` checks
  those exact guard lines.
- **`governance.js`** is listed as page-side in the lint config. Its matcher
  must stay byte-identical with `server/src/modules/control/egress.ts`.
- **`preload.js`** runs sandboxed. It may require only `electron`.
- **`login-state.js`** is required by the server's CDP driver and copied
  alone into the server image. It stays self-contained.
- **Integration tests** under `tests/` are exempt from the size and number
  rules, as tests are everywhere. They still carry headers and doc comments.
