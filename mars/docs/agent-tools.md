# Agent tools

These tools cut the number of round trips and tokens an agent spends on a page. Each one is a
native operation over NDJSON (`page.summary`, ...) and an MCP tool (`page_summary`, ...). The MCP
tool always requires `tab_id`, and `page_act` also requires `control_epoch`. They keep every rule
the single-step tools follow: human/agent ownership, `control_epoch`, `expected_revision`, tab
isolation and secret handling.

Reading tools (`page_summary`, `page_find`, `page_diff`, `page_wait`) work before a grant, like
`page_inspect`. `page_act` is a mutation.

## Typical loop

1. `page_summary` reads the page: about 1 KB for a sign-up form, where `page_inspect` returns
   10 KB or more.
2. `page_act` fills and submits in one call, with steps that target controls by `find` query, so
   the agent never copies node IDs.
3. `page_wait` (MCP) waits for the person or the site, for example `{"url":"/account"}`.
4. `page_diff` or the batch's `result:"diff"` shows what changed without reading the whole page
   again.

## page_summary / page.summary

Input: `{tab_id}`.

Output: `page_id`, `revision`, `url` (with the query and fragment removed on sensitive pages),
`title`, plus:

- `headings`: a tree of h1 to h6, `{level, node_id, text, children?}`. At most 64.
- `landmarks`: `{role, node_id, name?}` for nav, main, header, footer, aside, form, search and
  explicit ARIA landmark roles. At most 32.
- `controls`: numbered actionable controls in document order, `{n, node_id, role, name?, value?,
  checked?, placeholder?, options?, disabled?, invalid?}`. At most 150. The number of controls
  left out is in `controls_omitted`. Numbering is stable for one revision.
  - `value` is never present for password or hidden inputs.
  - A select lists up to 24 `options` as `{option_id, text}`, and `value` is the selected text.
  - `invalid` carries the control's validation message after a refused submission.
- `status`, only when it is not 200.

Each label, text or value is cut to 96 bytes.

## page_act / page.act

Input: `{tab_id, control_epoch, page_id, expected_revision, steps, result?}`.

`steps` is an ordered array of 1 to 16 steps. Each step is
`{"action": ..., <target>, <arguments>}`:

| action | target | arguments |
|:---|:---|:---|
| `type` | yes | `text` |
| `click` | yes | none |
| `set_checked` | yes | `checked` |
| `select` | yes | exactly one of `option_id`, or `option` (option text, ignoring case) |
| `submit` | yes | none |
| `scroll` | no | `y` |
| `navigate` | no | `url` |

A target is exactly one of `node_id` or `find: {role?, name?, label?, text?}`. A `find` query must
match exactly one node of the current revision. Otherwise the step fails with `invalid_target` or
`ambiguous_target`.

The rules:

- The batch must start on `page_id` and `expected_revision`, or nothing runs.
- Step k runs against the page and revision that step k-1 produced. The runtime fills in
  `page_id` and `expected_revision`, so the agent plans once.
- Before every step, the runtime checks the admission permit against the tab's current owner and
  epoch. If the person takes over mid-batch, the next step fails with `control_denied` and the
  batch stops. Each step also runs through the same native handler, profile and publication
  checks as the single-step tool.
- The first failing step stops the batch. Earlier steps stay committed and are reported.

`result` sets what comes back after the batch:

- `summary` (the default): the summary of the final page.
- `diff`: the changes since the starting revision. If the batch navigated away, you get the
  summary instead.
- `none`: only the IDs.

Output: `{ok, completed, total, steps:[{index, action, ok, page_id, revision} | {index, ok:false,
error}], page_id, revision, summary? | diff?}`.

## page_wait / page.wait

Input: `{tab_id, timeout_ms?, <exactly one condition>}`. The conditions are:

- `text`: some visible node contains this text (ignoring case).
- `role` / `name` / `label`: a matching node is present. Add `absent: true` to wait until no node
  matches.
- `url`: the page URL contains this.
- `after_revision`: the page revision is greater than this.
- `navigated_from`: the tab shows a page other than this `page_id`, meaning a navigation
  committed.

The default `timeout_ms` is 5000 and the maximum is 20000.

Output: `{satisfied, page_id, revision, url, timed_out?}`.

The native operation checks the condition once and returns at once. Mars has no timers, so a page
is settled when each command returns, and nothing can change it until the next command. The NDJSON
connection has a single client, so one check is the complete answer there.

Over MCP the person can act between agent calls. The session keeps an unsatisfied wait parked
without holding the native worker. It runs the check again every 50 ms, between the person's
actions, until the condition holds or the deadline passes. At the deadline it answers once with
the last observation and `timed_out: true`. Cancelling the request drops the wait.

## page_diff / page.diff

Input: `{tab_id, since_revision}`.

Output: `{page_id, from_revision, to_revision, added:[{node_id, role, name?, text?}],
removed:[{node_id, role, name?}], changed:[{node_id, role, name?, changes:{text?, name?, value?,
checked?, selected?, enabled?, invalid?}}]}`.

- `changes` holds new values only.
- At most 100 entries are listed, and the rest are counted in `changes_omitted`.
- The runtime keeps the last 8 revisions per tab. A revision it no longer has, or one from an
  earlier page, fails with `revision_unavailable`.
- Password and hidden input values are never captured, so they cannot appear in a diff.

## page_find / page.find

Input: `{tab_id, role?, name?, label?, text?, limit?, visible_only?}`. At least one criterion is
required, and all the criteria you give must hold.

- `role`: exact match on the role `page_inspect` reports.
- `name`: the accessible name equals this, ignoring case and surrounding spaces.
- `label`: the accessible name or the placeholder contains this.
- `text`: the node's text contains this.
- `limit`: 1 to 50, with a default of 10.

Output: `{page_id, revision, total, matches:[{node_id, role, name?, text?, disabled?,
bounds?}]}`. `bounds` is in logical points on the desktop. Use it in place of guessing a CSS
selector.

## page_inspect options

`page_inspect` accepts these optional fields:

- `fields`: up to 32 node field names to keep. `node_id` is always kept.
- `visible_only`: keep only nodes inside the viewport.
- `compact`: omit fields that hold their default value (false, empty, null, `enabled:true`,
  `visible:true`).

Setting `fields` or `visible_only` also compacts the output. Without options, `page_inspect`
returns exactly what it returned before.

## Cost on a typical form task

Take a sign-up form with name, password, a checkbox, a select and a submit button, followed by a
check of the result.

| | Round trips | Approximate response tokens |
|:---|:---|:---|
| inspect, then act, then inspect for each of the 5 controls | 11 calls (1 + 5 × (act + inspect)) | 11 full inspections, about 30k to 40k |
| Mars agent tools | 2 calls: `page_summary`, then `page_act` with `result:"summary"` | 2 summaries plus step results, about 1k to 1.5k |

Mutation responses already return the full inspection, so a careful agent can skip the
re-inspects and get down to about 6 calls. That still costs about 6 full inspections.

## Mars DevTools Protocol (MDP / CDP-compatible) front door

`mars-mdp [--port 9222]` (with backward-compatible alias `mars-cdp [--port 9222]`) serves the
Chrome DevTools Protocol on loopback for Playwright
(`connectOverCDP`), Puppeteer (`connect`), browser-use and Stagehand, translating every method
into these native operations. [cdp.md](cdp.md) is the generated conformance matrix: which
commands are implemented, accepted without effect, or not applicable and why, and where Mars
differs from Chrome.

## Attaching to the running browser

`mars mcp` joins a browser that is already running for this user instead of opening a new window.
`mars agent --attach` does the same for native NDJSON. `mars serve [--agent-control]` runs a
windowless browser that listens the same way.

- The browser listens on a Unix domain socket, `agent.sock`, in a private directory:
  `$MARS_RUNTIME_DIR`, else `$XDG_RUNTIME_DIR/mars`, else `<temp>/mars-<uid>`. The directory is
  0700 and the socket 0600.
- Beside the socket, `agent.token` (0600) holds 32 random bytes in hex, new on every start.
- A client's first line is `{"token":"...","protocol":"mcp"|"native"}`. The browser checks that
  the peer process runs as the same user, then checks the token. The answer is
  `{"ok":true,"lane":"agent-N"}`, or `{"ok":false,"error":...}` before the connection closes.
- After that the connection speaks plain MCP or native NDJSON, exactly as stdio does.
- At most 8 agents attach at once. Attached agents cannot call `runtime.shutdown`.

## Lanes and agent tabs

Each attached connection is one lane. The browser stamps the lane on every request, and anything
the client puts in a `lane` field is overwritten.

- `tab_open` (`tab.open` with an agent permit) opens a background tab under your control. The
  person's selected tab does not change. Input `{url?}`. Output `{tab_id, owner:"agent",
  control_epoch, lane, page? | navigation_error?}`. A lane holds at most 8 open tabs.
- `tab_close` (`tab.close`) closes a tab your lane opened: `{tab_id, control_epoch}`. Tabs the
  person granted you are never closed by an agent.
- The first lane to change a granted tab holds it. Any other lane's mutation there fails with
  `control_denied` ("Another agent drives this tab").
- When a lane disconnects, its tabs, and only its tabs, go back to the person and their epochs
  advance, so its queued work is refused. Tabs it opened stay open for the person.
- The person can still take over any agent tab from the toolbar. That advances the epoch, and
  the agent's next mutation is refused.
- Reads (`page_summary`, `page_screenshot`, ...) of other tabs work as before. Lanes fence
  mutations only.

## Screenshots, pointer and keyboard

| Tool / operation | Input | What it does |
|:---|:---|:---|
| `page_screenshot` / `page.screenshot` | `{tab_id, node_id?}` | Returns a PNG of the viewport, or of one element's box, as an MCP image content block. The JSON copy has no image data. Headless and attached runtimes paint it from the display list: boxes, borders, image boxes and text runs drawn as bars (`render:"wireframe"`). The desktop window answers with real pixels. Reads need no grant. |
| `page_click_at` / `page.click_at` | `{tab_id, control_epoch, page_id, expected_revision, x, y}` | Clicks whatever is painted at that viewport point, through the same path as `page_click`. The result includes `hit`. A text field has no click action: it comes back as `focused`, ready for `page_press`. |
| `page_hover` / `page.hover` | `{tab_id, x, y}` | Reads the node under a point and the node a click there would reach (`clickable`). Pages get no hover events (`events:"none"`). |
| `page_scroll_at` / `page.scroll_at` | `{tab_id, control_epoch, page_id, expected_revision, x, y, delta_y}` | A wheel turn: scrolls the document by `delta_y`. |
| `page_press` / `page.press` | `{tab_id, control_epoch, page_id, expected_revision, node_id?, key? \| text?, modifiers?}` | See below. |

What `page_press` does with each key:

- Enter: submits a text field's form, adds a newline in a textarea, and activates a link or button.
- Space: toggles a checkbox, selects a radio button, and activates a button.
- Backspace: erases one character. With Alt it erases a word; with Control or Meta it clears the
  field.
- PageDown, PageUp, Home and End scroll the page. These need no `node_id`.
- One printable character, or `text`, is appended to the field. Typed values never appear in a
  result, and password values stay hidden as before.
- Escape, Tab, the arrow keys and Delete return `unsupported_operation`. Mars has no focus or
  caret model yet.

## Console and network

- `page_console` / `page.console` `{tab_id}`: `{entries:[{level, source:"engine", text}],
  degraded, page_console:"unavailable"}`. The entries are the engine's messages about the page:
  skipped scripts, removed handlers and refused resources. V9 has no `console` object yet, so page
  code cannot write here.
- `page_network` / `page.network` `{tab_id}`: `{entries:[{kind:"document", method, url, status},
  {kind:"image", mime, bytes}...], images, scripts, stylesheets, degraded}`. At most 50 entries.
  There are no headers, cookies or bodies, and a sensitive URL loses its query.

Not yet available: file upload (the engine renders file inputs but cannot submit them) and
alert/confirm/prompt dialogs (V9 has no dialog functions).

## MDP / CDP front door

`mars-mdp [--port 9222]` (crate `crates/mdp`, alias `mars-cdp`) serves CDP on 127.0.0.1 for the
browser running for
this user. It works like Chrome's remote debugging port: `/json/version`, `/json/list`, and
WebSocket at `/devtools/browser/<random id>` and `/devtools/page/<tab_id>`.

- It refuses any request with an Origin header, and any Host other than a loopback name on its
  port.
- Each CDP connection attaches through the endpoint above as its own lane. Every method is
  translated into a native operation, with the tab's control epoch observed when the client
  created or attached the target.
- The epoch is never refreshed. A person's takeover therefore fails the client's next mutation
  with `control_denied: ...`.
- Mars has no debugger, and nothing is passed through to one.

Supported methods (`Mdp.getCapabilities` and `Mars.getCapabilities` list them):

- Browser: `getVersion`, and `close` (ends this connection only; the browser keeps running).
- Target: `setDiscoverTargets`, `getTargets`, `getTargetInfo`, `setAutoAttach`,
  `attachToTarget` (flatten only), `detachFromTarget`, `createTarget` (a lane tab),
  `closeTarget` (own tabs only), `getBrowserContexts`.
- Page: `navigate`, `reload`, `getFrameTree`, `captureScreenshot` (PNG of the viewport; no
  `clip` or `format`), `createIsolatedWorld`, `addScriptToEvaluateOnNewDocument` (an empty
  source only). A committed navigation emits `frameStartedLoading`, `frameNavigated`, the
  execution context events, `lifecycleEvent` from init through networkIdle, `domContentEventFired`,
  `loadEventFired` and `frameStoppedLoading`.
- Runtime: `enable`. `evaluate` and `callFunctionOn` answer only argument-free read-only
  expressions, from native state: `document.title`, `location.href`, `window.location.href`,
  `document.URL`, `document.location.href`, `document.readyState`. Anything else is an
  explicit error: page JavaScript is not exposed to agents (AG-17).
- Input: in `dispatchMouseEvent`, a left `mouseReleased` becomes `page.click_at` and
  `mouseWheel` becomes `page.scroll_at`. In `dispatchKeyEvent`, keyDown becomes `page.press` on
  the focused field. `insertText` is supported too.
- DOM: `getDocument`, `querySelector` and `querySelectorAll` (selectors `tag`, `#id` and
  `tag#id`), `describeNode`, `getBoxModel`, `getContentQuads`, `focus`.
- Accepted with no effect, because Mars already behaves that way: the `enable` and `disable`
  methods of Page, Network, Log, DOM, CSS, Performance and Inspector, plus
  `setLifecycleEventsEnabled`, `runIfWaitingForDebugger`, `releaseObject` and
  `setFocusEmulationEnabled`.
- Everything else returns `Mars does not support <method>`. That includes Emulation overrides,
  Network interception and `Debugger`.

Which clients work: raw CDP scripts and Puppeteer-style flows built from the methods above
(`goto`, `title`, `screenshot`, mouse clicks by coordinates, keyboard typing). Playwright's
`connectOverCDP` and Puppeteer's `page.$`/`page.evaluate` with page functions need page
JavaScript evaluation, which waits for AG-17.

## Measured cost on three scripted tasks

`tests/agent2_cost.rs` runs each task both ways through the real Runtime against a loopback page
that has 30 nav links and 40 paragraphs. Bytes are the serialized results the model reads. At
about 4 bytes per token, the inspect loop's sign-up costs about 64k tokens; the agent tools cost
about 0.9k.

| Task | Inspect loop: calls | Bytes | Agent tools: calls | Bytes | Byte ratio |
|:---|---:|---:|---:|---:|---:|
| Sign-up form (name, password, checkbox, submit) | 9 | 257,099 | 2 (`page_summary`, `page_act`) | 3,592 | 71.6x |
| Search and submit | 5 | 103,640 | 3 (`page_find`, `page_press` text, `page_press` Enter) | 958 | 108.2x |
| Follow a link | 3 | 52,397 | 2 (`page_wait`, `page_act` with a find target) | 462 | 113.4x |

The test fails if any task falls below 10x on bytes, or above about half the calls.
