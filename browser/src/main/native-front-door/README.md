# Native external CDP compatibility

**Partial compatibility. Not a complete CDP implementation.**

The listener terminates external CDP requests and invokes Oya-owned native APIs.
It never forwards to a debugging endpoint, attaches Electron's debugger, or falls
back to internal CDP. Retired adapters are excluded from the shipped dependency graph.

## Activation

The adapter uses the production native session lifecycle. The patched engine must
install and verify immutable persona policy and page/worker pre-script sources
before any tab or private context becomes visible. A missing capability fails
startup; there is no legacy fallback. Native workflow validation uses the same
protected sessions and human-control admission checks.

Set `OYA_NATIVE_CDP_PORT` (or the legacy `OYA_REMOTE_DEBUGGING_PORT` alias) and
`OYA_NATIVE_CDP_TOKEN` (at least 32 characters; generate a random secret).
Legacy port `0` disables the listener; explicit native port `0` requests an
ephemeral authenticated listener. Managed cloud browsers use the authenticated
control-socket relay and require no local listener.
Conflicting port aliases and explicit hosts other than `127.0.0.1` fail at startup.
The legacy port selects this native adapter, never the old upstream proxy.
The listener binds only `127.0.0.1`. Both HTTP discovery and WebSocket upgrades
require `Authorization: Bearer <token>`. Browser Origin headers are rejected;
credentials cannot be provided in URLs. Never publish a token in logs or screenshots.

Discovery: `/json/version`, `/json/list`. WebSockets: `/devtools/browser` and
`/devtools/page/<targetId>`. A browser connection attaches using
`Target.attachToTarget` with `flatten:true`; page commands then include its
returned `sessionId`. This adapter never requires or accesses a Chromium debugging
endpoint. Release evidence includes production dependency enforcement and actual
patched-engine integration, not merely adapter-level checks.

## Implemented subset

- `Oya.getCapabilities`: read-only adapter method names, allowed parameter names
  and browser/target/connection scope derived from the live dispatch and validation
  tables. The response explicitly declares partial compatibility and execution-time
  availability checks; it is not a promise of full CDP or old-engine support.
- `Browser.getVersion`.
- `Target.getTargets`, `getTargetInfo`, `createTarget`, `closeTarget`,
  `attachToTarget`, `detachFromTarget`. Private contexts must be explicitly owned.
- `Target.setDiscoverTargets`: native page creation, metadata changes and destruction;
  `Target.activateTarget` and `Page.bringToFront`: exact native tab/window activation.
- `Target.setAutoAttach`: explicitly page-only, flattened, unpaused sessions.
  Enable with `{autoAttach:true,flatten:true,waitForDebuggerOnStart:false,filter:[{type:"page"}]}`.
  Existing and future native pages emit attachment events; removal revokes sessions.
  Disable revokes automatic sessions, not manual ones. Worker/frame discovery and
  renderer pausing are rejected. Asynchronous capacity failure closes the connection.
- `Page.enable` / `disable`: native main-document `domContentEventFired` and
  `loadEventFired`, with monotonic timestamps and per-delivery control checks.
  No fabricated frame, network-idle or SPA-render-complete events.
- `Page.reload` (optional boolean `ignoreCache`) and `Page.stopLoading`: exact
  native target only. Reload returns command acceptance; await load events.
- `Page.navigate`, `captureScreenshot` (PNG/JPEG, integer JPEG quality 0–100), `getFrameTree`.
  Activate a background tab before capture; invisible capture is not guaranteed.
- `DOM.getDocument`, `querySelector`, `querySelectorAll`, `describeNode`,
  `getAttributes`, `getOuterHTML`: read-only, main-document nodes. Depth 0–32;
  no `pierce:true`, mutation stream, backend-node lookup or Runtime object handles.
  Native frame trees include cross-origin children, but DOM frame sessions do not.
- `DOM.focus` and `DOM.scrollIntoViewIfNeeded`: explicit operations on this
  connection's live main-document node IDs. Alternate handles and custom scroll
  rectangles are rejected.
- `Input.insertText`: acknowledged native composition through the exact page's
  focused frame, including Unicode behind Oya's control shield. Requires the
  `native-text-acknowledgement.patch` engine capability; no legacy fallback.
  It does not generate keydown/keyup events or turn arbitrary nodes into editors.
- `Input.dispatchMouseEvent`: native press/release/move/wheel, CSS coordinates
  adjusted for the exact page's zoom, left/right/middle buttons and explicit
  held-button/modifier masks. Wheel deltas retain CSS direction and magnitude.
  Pen/touch, back/forward buttons, timestamps and pressure metadata are rejected.
- `Input.dispatchKeyEvent`: native keyDown/rawKeyDown/keyUp/char for ASCII
  letters/digits and supported editing/navigation keys. Physical code metadata
  must match the key. A keyDown with text also commits one character; rawKeyDown
  and keyUp reject text. Character events accept one printable ASCII character
  or carriage return; use `Input.insertText` for Unicode/composed text.
  Layout overrides, repeat, system/keypad flags and arbitrary physical keys are
  not supported. Dispatch completion is not proof a website accepted the input.
  Both input methods retain exact-target ownership and human-control admission;
  neither activates another tab nor grants access to browser shell shortcuts.
- `Runtime.enable` / `disable`: native default main-world context identities for the main frame
  and cross-process child frames. Contexts share frame IDs with `Page.getFrameTree`;
  native readiness/removal events drive creation and destruction notifications.
  No URL-based frame matching or polling is used.
- `Runtime.evaluate`, `callFunctionOn`, `awaitPromise`: actual main-world V8 values,
  JSON-by-value results, undefined/nonfinite/bigint primitives and opaque native
  handles. Optional promise waiting has a ten-second response deadline. A deadline
  does not terminate synchronous JavaScript or authorize an automatic retry.
- `Runtime.getProperties`: bounded own descriptors (`ownProperties:true`) or
  inherited descriptors (omitted/false), including symbols/accessors without
  invoking getters. Shadowed keys appear only once; symbol identity is preserved.
  Proxy objects/prototypes are rejected without invoking traps. Prototype walks
  visit at most 32 objects and 512 keys, including shadowed keys. Inherited reads
  require `native-runtime-properties.patch`; old engines fail explicitly.
- `Runtime.releaseObject` / `releaseObjectGroup`: connection-owned native cleanup;
  navigation and disconnect invalidate values. Foreign/stale handles are rejected.
  Requires `native-runtime.patch` and `native-runtime-frames.patch`; no debugger fallback. Context IDs and object handles route to the exact native
  document, and cross-frame handle arguments are rejected before invocation. Group
  release covers known documents on the selected target, not other tabs.
  Inspector previews, side-effect checks, user gestures, worker contexts,
  console argument events and custom timeouts remain unsupported.
  Raw script exceptions report generic failure metadata rather than fabricated
  exception objects or stack traces; function/promise thrown values are preserved.
- `Page.createIsolatedWorld`: native connection-owned worlds in an issued frame,
  requiring `native-runtime-worlds.patch`. Named worlds are reused only inside the
  same connection and exact document; unnamed worlds are distinct. Names are
  bounded to 1,024 characters and at most 32 worlds may be retained per connection.
  `grantUniveralAccess` must be absent or false: origin protections are not relaxed.
  Context selection, values, exceptions, invocation, promises and group release
  use the exact native world, never Oya's internal recorder/analyzer world.
  Enabled Runtime observers receive isolated context creation/destruction events;
  disable stops events without deleting worlds, and disconnect revokes resources.
  Old engines fail explicitly rather than executing the request in the main world.
- `Log.enable` / `Log.disable`: native console text as `Log.entryAdded`, not
  `Runtime.consoleAPICalled` or structured JavaScript argument objects.
- `Emulation.setDeviceMetricsOverride` / `clearDeviceMetricsOverride`: width,
  height, deviceScaleFactor, mobile only. Dimensions 0–10000; pixel ratio 0–4.
  This does not spoof user agents, install touch handlers, or emulate sensors.
  Overrides are exclusive to one connection and restored when it disconnects.
- `Oya.getNavigationHistory`: this tab's native back/forward entries as indexes,
  URLs and titles, plus current index and a connection-local snapshot token.
  `Oya.navigateToHistoryEntry` requires that token and an index from the latest
  unchanged snapshot. It consumes the token and checks destination policy before
  native traversal; await page readiness after command acceptance. Native page
  state is never exported. These are Oya extensions, not fabricated standard CDP
  entry IDs/transition metadata. This is not the global browsing-history database.
- `Oya.analyze`, `click`, `type`, `pressKey`, `scroll`, `readElements`: Oya
  extensions mapped to the production native PageDriver. Element commands use
  the analyzer's selector handles; these are not standard CDP commands.

Unknown methods and unsupported parameters return explicit errors. Do not claim
unmodified general-purpose CDP clients work with this subset.

## Native private contexts, downloads and network

- `Target.createBrowserContext` requires `disposeOnDetach:true`. Contexts use
  separate in-memory sessions, belong to one connection and are destroyed on
  disconnect. Cross-connection discovery and access are denied. Native policy
  setup must succeed first; governed/proxied persona contexts are currently
  rejected explicitly rather than bypassed.
- `Target.createTarget` in an owned context activates/mounts the new tab before
  readiness checks, like ordinary automation tab creation. This avoids a zero-size
  initial viewport; no unsupported background-rendering guarantee is implied.
- `Browser.setDownloadBehavior` supports `deny` (default) and `allowAndName`
  only in owned contexts. Destinations must be existing absolute canonical
  directories. Files are exclusively reserved under random GUID names, never
  page-controlled filenames. Download events and cancellation remain scoped to
  the owner and current control permission.
- `Oya.enableNetwork` emits `Oya.networkEvent` from actual native request metadata.
  This is deliberately not `Network.enable`: inspector timings, connection IDs
  and stack traces are not fabricated.
- `Network.getResponseBody` reads captured original bytes, base64 encoded, without
  refetching. Native capture tees the response stream with backpressure, at most
  512 KiB per response and eight concurrent streams per session. Overflow reports
  an error but never truncates the page's response. Retention is bounded and
  disabling observation revokes pending readers and handles.
- `Fetch.enable` supports native-frame-attributed request-stage interception: omitted
  patterns match all attributed requests; `patterns:[]` matches none. Rules may
  select `urlPattern`, a supported native `resourceType`, and `requestStage:"Request"`.
  URL wildcards support `*` (zero or more), `?` (exactly one), and backslash escaping.
  `continueRequest`
  consumes the original native continuation; URL/header/body overrides are
  unsupported. `failRequest` supports Failed, Aborted, TimedOut, AccessDenied,
  ConnectionClosed, ConnectionReset, ConnectionRefused, ConnectionAborted,
  ConnectionFailed, NameNotResolved, InternetDisconnected, AddressUnreachable,
  BlockedByClient and BlockedByResponse. Nondefault reasons require
  `native-request-errors.patch`; older engines reject them without consuming the
  pending continuation. Each reason completes the original request with its actual
  native net error, not a synthesized response or second request. Held requests cancel
  after ten seconds, on observer removal or on lost permission. Continuations
  recheck exact target ownership, native frame membership, renderer identity and
  egress authorization. Child requests carry the same frame IDs as
  `Page.getFrameTree` and Runtime contexts. Committed navigation, removal and
  native request failure cancel held work; surviving siblings stay independent.
  Worker attribution and requests without an exact native frame are not exposed.

Filters are bounded to 32 rules, 256 UTF-16 units per pattern, 32,768 UTF-16 units per URL,
and a shared 262,144-transition budget per request. Oversized or pathological
matches fail closed rather than blocking the event loop or silently bypassing
interception. Repeating `Fetch.enable` atomically replaces this owner's compiled
rules; invalid updates leave the previous policy intact. Existing pauses retain
their original one-shot continuations until explicitly answered or revoked.

Resource filters use actual native categories: Document, Stylesheet, Image, Media,
Font, Script, XHR, Ping, CSPViolationReport and Other. Native `xhr` includes
fetch-like requests; a separate Fetch/EventSource distinction is not exposed.
Unsupported category distinctions and response-stage rules are rejected. Filtering
never bypasses egress, frame ownership, human-control or disconnect checks.

Network capabilities are limited to these exclusively owned sessions. They never
replace the normal profile's session-wide protection hooks. Fetch continuation
messages may resolve an already-held native callback while a navigation/evaluation
awaits it; the original command retains the admission gate throughout. This is
not an alternative route for executing page commands.

## Native cookie management

`Storage.getCookies`, `Storage.setCookies` and `Storage.clearCookies` require an
explicit `browserContextId` owned by this connection. They are browser-endpoint
commands: page sockets and flattened page sessions cannot use them. The default
user profile is never an implicit destination. `Oya.deleteCookie` takes that same
context ID plus an explicit HTTP(S) `url` and `name`; it uses native URL/name
matching, not a script assignment or a promise of exact path-only deletion.

Writes accept bounded batches with explicit URL, name, value, domain, path,
secure, httpOnly, sameSite and expires attributes. Unsupported attributes fail
before any write. Native validation remains authoritative; a native failure partway
through a valid batch is surfaced and stops later writes, not claimed to roll back
completed ones. Ownership and human control are rechecked around each native await.

Cookie reads require `native-cookies.patch` for actual priority, source scheme
and source port metadata. Unknown metadata is never filled with guessed defaults.
Partitioned cookie export and partition/priority/source overrides on writes are
currently rejected explicitly. Clearing cookies preserves local storage and other
contexts. None of these operations use CDP internally.

## Still required

- Full standard Network event semantics, response interception and fulfillment.
- Debugger events, frame-specific sessions and worker/frame automatic attachment.
- Runtime worker contexts and the unsupported options listed above.
- Governed/proxied private contexts with complete native policy composition.
- Windows compilation/runtime verification of the new engine capabilities.

## Isolation and verification

Each socket owns its DOM registries, subscriptions and viewport overrides. Opaque
node-id ranges are not reused after navigation, across tabs or between sockets.
DOM and analyzer execution use Oya's reserved world; Runtime intentionally uses
the real main world through a separate native V8 registry. Explicit
DOM focus/scroll operations do not invoke page-world helpers. DOM reads
are bounded by node, text and depth limits. Console events recheck human control
and exact live-tab protection before delivery. Slow event consumers are closed.
Commands are serialized under the existing human/agent admission gate; disconnect
cancels queued work but does not prematurely release a running native command.

Run `npm run test:unit`, then set `OYA_NATIVE_ENGINE` to the patched Oya executable
and run `npm run test:native-front-door`. The real-engine test uses disposable
localhost pages and an authenticated WebSocket client; accessing the internal
debugger throws. It checks DOM values, stale/foreign nodes, document/frame
identities, actual viewport/pixel ratio, override ownership/restoration, console
events and suppression during human control, native target discovery/activation,
automatic flat-session access/revocation, main-world values, native handle ownership,
promises, getter-safe inspection and cancellation across navigation. macOS runtime and external public-site
flows are recorded in `engine/INTERNET-QA.md`; Windows and full-agent benchmarking
remain outstanding.

`npm run test:native-contexts` additionally checks actual isolated storage,
connection privacy, download bytes and scoped cancellation, binary response
capture without refetch, nontruncating capture overflow, native pause/continue/
fail, held evaluation unblocking and observer revocation. It uses the patched
Oya engine only, with debugger access forbidden.
