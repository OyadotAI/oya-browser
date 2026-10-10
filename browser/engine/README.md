# Experimental macOS native passkey engine

**The full arm64 engine builds, and six native policy tests pass. The signed native
app is blocked by macOS provisioning; no real-credential verification yet.
Do not ship as a passkey fix.**

The desktop WebAuthn cancellation shim is removed separately. Stock Electron
supports device-local Touch ID, but does not wire Chromium's iCloud Keychain
factory to the owning NSWindow. This patch supplies that connection; it never
implements credential cryptography or weakens relying-party validation.

## Pinned source

- Electron `v44.5.1`, commit `19c601167d4ae75989938416c18ed81eb21c6020`
- Chromium `152.0.7977.130`, commit `2c592105bbcd9490a9894df48d0fe59b2c512651`,
  selected by Electron's `DEPS`
- Patch: `patches/macos-native-passkeys.patch`

Apply only **after** the unmodified checkout finishes `gclient sync`. Verify
`git rev-parse HEAD` in `src/electron` against the commit above, then run
`git apply --check /absolute/path/to/macos-native-passkeys.patch` followed by
`git apply /absolute/path/to/macos-native-passkeys.patch` from that directory.
Never use force/fuzzy application on another Electron version.

## Boundaries

The patch is macOS-only and requires the `--oya-native-passkeys` process switch,
macOS 13.5+, and Apple's browser public-key-credential signing entitlement.
It permits only explicit modal WebAuthn requests from visible content in a
focused, visible owning window. Conditional/autofill, immediate and payment
requests do not launch the new sheet. DevTools authenticator overrides remain
untouched. Existing Windows behavior is unchanged.

For opted-in modal requests, iCloud replaces the device-local Touch ID discovery
to avoid simultaneous biometric sheets. This prototype does **not** offer a
picker between those credential stores. Apple owns user consent, credential
selection, biometric verification and private keys. No credential data is
exposed through Oya's agent tools. This does not yet implement Chrome's QR,
security-key PIN or direct desktop Google Password Manager vault synchronization.

## Apple and Google credential sources

The target is Apple Passwords locally and compatible phone-held passkeys through
FIDO cross-device authentication, including Google Password Manager on Android.
These are separate from direct desktop Google Password Manager synchronization.
Chromium's `chrome/browser/webauthn/enclave_manager.h` depends on Chrome's primary
account, IdentityManager, trusted vault and enclave services; Electron does not
provide that stack. Do not read Chrome's private vault, import encryption keys,
reuse its OAuth identity, or claim a Gmail session enables vault synchronization.

`patches/macos-native-passkey-phone.patch` is a **second experimental patch** for
the pinned Chromium checkout (`src`, not `src/electron`). Apply it after sync,
using `git apply --check` first. It enables Apple's existing nearby-phone option
only behind `--oya-native-passkeys` and when the request permits hybrid transport.
It leaves credential IDs, RP checks, challenges and OS consent unchanged. Six
native policy tests cover opt-in and transport eligibility; the native test
executable builds and all six tests pass. The packaging hook adds a Bluetooth usage explanation only
for the opted-in native build, never a permission grant.

**Still unverified/incomplete:** Android Google Password Manager end-to-end,
Bluetooth denied/off, hybrid-only discovery (when the site's transport hints
exclude the internal transport), cross-device registration, and Windows phone UI.
The current phone prototype does not constitute complete support for both stores.
Do not label it as such until the signed macOS and Windows acceptance tests pass.

## Local build and signing

Use Electron's official build tools with a separate checkout, non-component testing arm64
configuration and no paid remote builder. Pin the revision before syncing.
Use `--no-history` with the Git cache disabled for a space-constrained local
checkout: build-tools' `--shallow` cache still fetches 10,000 commits of each
branch. If the Chromium Google host stalls, the official
`https://github.com/chromium/chromium.git` mirror has the same pinned commit;
verify the hash rather than changing versions.

The local test configuration imports `//electron/build/args/testing.gn`, keeps
DCHECKs, and sets `target_cpu="arm64"`, `symbol_level=0`, `use_thin_lto=false`,
`enable_dsyms=false` and `chrome_pgo_phase=0` to reduce disk and linking costs.
It is a local test configuration, not a replacement for release builds. Limit
local compilation to four jobs on the 16 GiB development machine.

This is a Chromium source build, not an ordinary `npm install`; allow substantial
disk space and do not fill the system volume. The completed local arm64 build retained more than 20 GiB of free space;
other configurations may require substantially more space.

After building the patched runtime, package Oya using electron-builder's
`electronDist` pointing to that runtime, the normal `APPLE_TEAM_ID`, and
`OYA_NATIVE_PASSKEYS=1`. The existing `beforePack` hook adds
`com.apple.developer.web-browser.public-key-credential` to **only the main app**.
The environment opt-in does not enable the engine switch by itself, and stock
release builds do not request this extra entitlement. Inspect the resulting
signature/entitlements before launching the signed Oya executable with
`--oya-native-passkeys`. Do not substitute unsigned stock Electron for this test.

## Required validation before enabling by default

1. Compile the patch against the pinned runtime (both macOS architectures for release).
2. Run all browser checks and native Oya identity tests using that runtime.
3. Verify virtual authenticator registration/assertion, cancellation, aborted
   requests, RP mismatch rejection and navigation cleanup.
4. Verify no Apple sheet opens for background tabs or conditional requests.
5. Run the native `OyaNativePasskeyPhone.*` policy tests.
6. In a signed Oya build, manually sign in to Gmail with an existing Apple
   Passwords/iCloud passkey. Only the user completes biometric/account consent.
7. Test an Android phone with an existing Google Password Manager passkey,
   including empty allow-list, internal+hybrid and hybrid-only transport hints.
8. Verify cancel, denied system permission, missing credential and window closure.
9. Verify Windows native WebAuthn remains unchanged before cross-platform release.

Do not treat the virtual-authenticator test as proof of iCloud or Gmail support.

## Initial source-compilation checkpoint (superseded by full build below)

- Synced the pinned Electron/Chromium sources and successfully generated 32,290 GN targets.
- Compiled all five changed translation units using the exact generated arm64
  testing-build compiler commands: both Electron delegates, the Chromium iCloud
  authenticator, the native Apple bridge, and its unit-test translation unit.
  Generated header prerequisites were built separately. This is **compilation**,
  not a linked executable, a native test-suite pass, or a real-device login.
- Browser: 1,756 unit tests plus integration suites pass; typecheck, lint,
  formatting and production app build pass. Those use the installed runtime,
  not a rebuilt native engine.
- A normal build of even these object targets pulls in 35,287 dependency steps.
  At that checkpoint the full runtime still required building, signing, and the manual
  Apple/Android/Windows acceptance checklist above. No long-running build was
  detached, and no experimental runtime has been released.

## Full build and provisioning checkpoint (2026-10-08)

- Completed the full pinned arm64 engine build (44,677 initial steps). The Node
  config generator needed `src/buildtools/mac` on PATH; resuming with that PATH
  and the pinned SDK reused the completed objects. No source workaround was needed.
- Built `device_unittests` and executed `OyaNativePasskeyPhone.*`: all six pass.
- The custom engine passes Oya's hermetic identity and virtual-WebAuthn regression
  in a separately signed QA copy **without restricted credentials entitlements**.
  This verifies the engine, not Apple/iCloud access.
- Browser checks: 1,772 unit tests plus integrations, types, lint and build pass.
- Created `dist/native-passkey-test/Oya Browser Native.app` with Developer ID
  team `68267DX5UC`, bundle ID `ai.oya.browser.native-test`, hardened runtime,
  the browser public-key-credential entitlement and the stable WebAuthn group.
  Deep/strict signature verification passes, and the renderer helper has neither
  restricted entitlement. **That app cannot currently launch.**
- macOS kills it with SIGKILL. `taskgated-helper` reports no eligible provisioning
  profile; AMFI reports `No matching profile found` (code -413). No matching-team
  profile is installed in the standard user provisioning-profile directories.

### Required external input

Supply an Apple provisioning profile matching the signing certificate, bundle ID,
keychain access group `68267DX5UC.ai.oya.browser.webauthn`, and requested browser
credential capability. The local native test app currently uses
`ai.oya.browser.native-test`; production packaging uses `ai.oya.browser` and
requires a matching profile for that identifier instead. No private signing key
or Apple account password should be copied into this repository.

`OYA_WEBAUTHN_PROFILE` now supplies the profile path to both build configuration
and packaging. Without it, stock builds omit restricted WebAuthn entitlements
and do not advertise signing-bound Touch ID configuration. `OYA_NATIVE_PASSKEYS=1`
without a profile fails before producing a native-capability release. Supplying
a file is not proof that its capabilities are valid: inspect the profile, sign,
verify, and actually launch the artifact before real-device acceptance tests.

Do not disable AMFI, remove platform security checks, or present the unrestricted
QA copy as a native-passkey fix. No experimental release has been published.

## Native isolated frame execution (in progress)

`patches/native-agent-frames.patch` adds a browser-process-only
`WebFrameMain._executeJavaScriptInOyaWorld(code, userGesture?)` method. It uses
native RenderFrameHost execution in fixed isolated world 1004, including
out-of-process frames. It does not attach a debugger, dispatch CDP, or expose
an arbitrary world selector. JavaScript content settings remain honored.
The TypeScript native facade fails closed when the capability is unavailable
or the frame is detached, and never retries against another document.

Built locally against Electron 44.5.1 commit
`19c601167d4ae75989938416c18ed81eb21c6020`. The patch is platform-independent;
Windows compilation and runtime verification remain required. This capability
is not yet wired into recording: reliable navigation-time event delivery and
frame-owner identity must be preserved before replacing that transport.

Set `OYA_NATIVE_ENGINE` to the executable of a separately branded Oya engine
containing this patch, then run `npm run test:native-frames` from `browser/`. The hermetic test uses temporary profiles and
local cross-origin frames, forbids debugger access, and checks isolation,
duplicate frame URLs, promise/error propagation and detached-frame refusal.
Stock-engine runs must fail rather than silently skip this capability.

`patches/native-frame-execution-lifetime.patch` applies in `src/electron` after
the native frame patches. A live reload timeout led to a reproducible local bug:
removing a frame left its pending isolated-world execution unresolved. Each
request now observes its exact document, cancels on replacement/removal/tab
destruction or lost native replies, and has a ten-second deadline for unresolved
promises. Late replies cannot settle a replacement document's request. Page-thrown
values remain distinct from native lifecycle errors; a lost document raises an
Error that the existing bounded World retry can recognize. The regression tests
pending evaluations, not merely calls made after a frame is already detached.
The deadline bounds caller waiting; it does not interrupt JavaScript that has
already started executing in the page.

## Native dialog callbacks and application ownership

`patches/native-agent-dialogs.patch` introduces a browser-process-only
`webContents._setOyaDialogHandler(handler | null)` capability. The handler receives
native frame metadata and a single-use reply callback for alerts, confirmations
and prompts. It runs after the engine's disabled-dialog checks. Navigation and
destruction invalidate pending replies and emit `-oya-dialog-cancelled` with the
exact callback identity. Pending dialogs prevent handler replacement; completed
subscriptions can be explicitly removed to restore ordinary human dialog handling.

The patch also removes Electron's page-visible throwing `prompt()` override,
allowing Blink's native prompt to reach the browser handler. Without a registered
handler, Electron's existing prompt cancellation behavior remains; a full human
prompt UI is not implemented by this patch. No page-level dialog shim is installed.
`patches/native-before-unload.patch` extends that registration to Blink's native
before-unload callback (apply after `native-agent-dialogs.patch`). Navigation,
reload and window close wait for an explicit decision; refusing a `loadURL`
rejects its promise without leaving the document. Replies retain the same
single-use/cancellation protection. Removing the registration restores the
original `will-prevent-unload` behavior. The browser adapter and packaging probe
require the new `_oyaBeforeUnloadDialogs` marker, rather than silently using an
older engine that bypasses unload decisions. The application routing and human presentation described below now consume this
capability on actual tabs and popups.

Run `npm run test:native-dialogs` with `OYA_NATIVE_ENGINE` pointing to a separately
branded, patched Oya executable. macOS native-engine tests cover explicit decisions,
prompt text, alerts, repeated answers, navigation/destruction cancellation and
subscription teardown with debugger access forbidden. The unload regression uses
trusted native pointer input and covers navigation/reload/close accept and cancel,
stale replies, destruction, and restoration of the original human event path. Windows runtime verification remains a release gate.

### Command-facing native dialog service

`main/dialogs/NativeDialogs` consumes the native callbacks and shares a
transport-independent decision queue with existing command handling. Simultaneous
tab decisions are retained in arrival order; cancellation removes only its exact
source. Alerts are reported to the notification callback without replacing a
pending confirmation. The real engine regression also drives this service through
two independent Oya windows.

Production startup now uses `main/dialogs/DesktopDialogs` and installs native
subscriptions on tabs and sign-in popups before setup succeeds. It does not use
`main/cdp/dialogs.ts` as a backend or fallback. The legacy adapter remains only for
unmigrated identity fixtures; other default-path identity/recording dependencies
still prevent a full internally-CDP-free claim.

The production router reads live human/agent control ownership. A handoff closes
an obsolete human sheet or removes an obsolete agent queue entry, then routes the
same pending native decision to its new owner without answering. Late UI events,
agent answers after human takeover and engine-cancelled callbacks are fenced.
Alerts retain the notification-first behavior.

Human confirmations and prompts use Oya-owned modal windows, an ephemeral private
session, denied network/permissions, a sandboxed context-isolated preload and
exact-main-frame IPC. Site text is never markup; URL credentials/paths/query
strings are not displayed. The complete form is populated before the window is
shown. Prompt text is selected on open; Escape cancels; non-prompt Enter defaults
to Cancel/Stay. Long messages scroll without hiding controls. Navigation away
closes and declines an old confirmation/prompt, while same-document navigation
and actual before-unload decisions are not auto-answered.

`test:native-dialogs` now builds the private preload before testing. On the local
macOS arm64 Oya engine it covers real production UI, native typing and Enter,
Escape/default cancellation, literal markup, isolated sessions, forged IPC,
human/agent handoffs, long-message layout, unsaved-work decisions and same/different
URL cancellation with debugger access forbidden. Windows and the full application
release gates still require separate verification.

## Native recording transport and focused text insertion

The optional `out/preload/recording.js` entry exposes a fixed-channel, size-bounded
emitter only in isolated world 1004 through the native context bridge. It does not
install an API in the page main world. It is **not enabled in production tabs**.
The transport test enables subframe preloads while retaining sandboxing,
context isolation, and disabled page Node integration, and verifies those
boundaries in real cross-process frames.

`npm run test:native-recording-transport` uses `OYA_NATIVE_ENGINE` and tests the
actual analyzer's final typing flush during navigation, not just a synthetic
message. The native inbox scopes messages to their owning web contents, rotates epochs on
restart, and bounds payload size. Recording lifecycle integration, durable document
identity and stable frame-owner locator paths still need completion before replacing the
existing channel.

That test exposed a null active-input-controller crash in the renderer-based
`webContents.insertText` implementation when a cross-process child owns focus.
`patches/native-focused-text.patch` routes the same public operation to native
IME delivery on the browser's focused frame widget instead. No debugger input
or page-level value assignment is used. The promise acknowledges dispatch;
callers requiring rendered completion must observe the resulting page state.
Windows compilation/runtime verification remains a release gate.

### Native frame-owner identity

`patches/native-frame-owners.patch` adds browser-owned parent-renderer frame tokens
and renderer-native selector-to-frame-token lookup. Cross-process children use the
parent renderer's remote proxy token; same-process children use their local token.
`nativeFramePath` compares these identities through a narrow isolated preload
capability, never `window.frames` indices, frame URLs or website-provided ids.

Real Oya tests cover ordinary, reordered, nested and open-shadow-root iframe
owners, including identical URLs. Ambiguous selectors, changed topology, changed
tokens, detached frames and missing engine capabilities fail explicitly. The
page main world receives no lookup API. Closed shadow roots cannot currently
supply a replayable selector chain through the analyzer and remain unsupported.

This resolves the earlier shadow-frame index mismatch but does not itself switch
production recording to the new transport. Lifecycle integration, durable document
identity and navigation re-arming still need completion before that switch.

## Native select menus and file selection

`patches/native-popup-tracking.patch` applies to the pinned Chromium `src` checkout;
`patches/native-popup-keyboard.patch` applies to `src/electron`. Together they route
keyboard input to a tracking macOS HTML-select menu only when its exact renderer
view owns that menu and its window has keyboard focus. AppKit receives ordinary
native key events; there is no global OS keystroke injection, CDP command, DOM
selection replacement, or routing to another tab's menu. Separate renderer `char`
events are consumed while tracking to avoid duplicate text. Ordinary renderer
input is unchanged when no menu belongs to the target view.

`patches/native-file-chooser.patch` applies to `src/electron`. It adds the private
browser-process `-oya-file-chooser` event on the owning web contents for open-file
and multiple-file choosers. A handler must call `event.preventDefault()` to claim
the request. It receives native URL/process/routing metadata and a one-shot reply
accepting absolute paths; an empty list cancels. Replies are document-scoped,
relative paths and multiple paths for a single-file chooser are refused, and an
unanswered claimed request cancels after 30 seconds. Files use the existing native
selection pipeline, including its frame-destruction handling. Folder/save dialogs
and unclaimed requests retain ordinary human UI.

The event is **not an authorization policy**. Before production agent wiring,
the application must validate control ownership, exact originating frame/document,
and an explicit allowlist of user-authorized files. It must not accept arbitrary
paths from a website or an unauthenticated agent. This event is not exposed to page
JavaScript. The live test replies only with its own generated scratch fixture.

`OYA_NATIVE_ENGINE=/path/to/patched/Oya npm run test:native-choosers` exercises
native select/Enter, Escape cancellation, trusted file-change events and file
contents, cancellation, invalid paths, single-file limits, and stale replies after
navigation. These regressions have run on macOS arm64. Windows compilation/runtime,
production agent upload integration, and application-level authorization remain
release gates; this patch is not a complete file-upload tool implementation.

`patches/native-drag-end-validation.patch` applies to the Chromium `src` checkout.
It normalizes unsupported AppKit drag-operation values to cancellation before
serializing a Mojo enum. A live HTML drag exposed an invalid-enum message that
disconnected the renderer. This guard is a crash-prevention fix, **not** a working
HTML drag-and-drop automation implementation on its own.

## Scoped native drag transport

Apply `patches/native-drag-handler.patch` in the Chromium `src` checkout, then
`patches/native-drag-controller.patch` in `src/electron`, at the pinned revisions
above. The content hook runs **after** source URL and file-access filtering. It
passes the browser-owned native controller the sanitized renderer payload;
it never configures DevTools interception or sends a protocol command.

The private browser-process `_dragOya(from, to)` operation scopes one gesture to
one focused main document. It uses native hit testing, acknowledged pointer input,
and native drag-enter/over/drop callbacks. The renderer's accepted operation and
`document_is_handling_drag` response are preserved for drop delivery; an ordinary
pointer gesture is distinguished from an accepted HTML drop. Drop data is not
manufactured by the agent, and no page `DragEvent` or `DataTransfer` is synthesized.

Concurrent operations in one tab, out-of-viewport coordinates, focus loss,
navigation and destruction reject the operation. A ten-second deadline cancels
stalled requests. Unsupported child-frame sources, other renderer-widget targets,
and file payloads fail explicitly, with no OS-drag or CDP fallback. This limited
same-document implementation is not cross-window or external-file drag support.
As with other private engine APIs, the application command boundary still owns
agent authorization; the engine method is not exposed to web pages.

`OYA_NATIVE_ENGINE=/path/to/patched/Oya npm run test:native-drag` verifies trusted
drag events and payload delivery, refused drops, concurrent-request rejection,
invalid coordinates, navigation/destruction cancellation, unsupported child-frame
sources, actual out-of-process target isolation, focus isolation, page-cancelled
dragstart, and native slider input.
`test:native-input` now also requires that explicit patched engine. These tests
have run on macOS arm64; Windows compilation/runtime remain unverified.

## Manual macOS development app and deep links

Do not register a bare diagnostic engine bundle for manual sign-in. macOS starts
its bundle entry, not the repository-path arguments supplied to a terminal launch;
that can open Electron's `path-to-app` screen instead of Oya.

After building `browser/`, prepare a **separate** manual app:

```sh
node scripts/prepare-native-dev.mjs '/absolute/path/to/patched/Oya Browser.app'
open 'node_modules/.cache/oya-native-manual/Oya Browser.app'
```

The preparation script clones the explicit patched engine, embeds a local entry
point referencing this checkout, declares `oya://` in Info.plist, and ad-hoc signs
it with the distinct `ai.oya.browser.native-development` bundle identity. Both
ordinary launches and OS deep links use native browsing mode and the same
`oya-native-manual-profile` test profile. HTTP/HTTPS default-browser associations
are not changed. The QA source bundle is untouched.

This is local development tooling, not a portable/release app or a full native
agent-loop implementation. It refuses to overwrite an existing manual bundle:
close and move that generated bundle before preparing an updated engine copy.
JavaScript rebuilds load directly from this checkout after restarting the app.

## Acknowledged native text composition

Apply `patches/native-text-acknowledgement.patch` after
`native-focused-text.patch` and the existing native input patches. It updates
native `insertText` and exposes the explicit browser-process-only
`_insertTextOya` capability used by the compatibility adapter.

The previous implementation queued IME text without focusing the renderer widget
or waiting for its acknowledgement. Under the production control shield, OS focus
intentionally belongs to the shell; text could silently disappear. The new native
controller scopes renderer-widget focus to composition, waits for the native
acknowledgement, and restores focus without focusing an OS window, removing the
shield, or allowing unmarked human keys through. Concurrent insertion in the same
frame is rejected. Document/frame replacement, process loss, channel closure and
a ten-second response deadline reject pending calls rather than returning success.
A timeout is not permission to automatically retry an input that might have begun.

Built and tested on macOS arm64. The front-door regression verifies hidden-page
Unicode immediately after acknowledgement, stale/foreign DOM rejection and
native load events. The native-input regression verifies composed Unicode in a
focused iframe behind the real control shield, preserved shell focus, and blocked
unmarked keyboard input. The running full Oya app also passed this path. Windows
compilation/runtime validation remains outstanding. No CDP backend is involved.

## Native main-world Runtime

Apply `patches/native-runtime.patch` after the native frame execution/lifetime and
text-acknowledgement patches. It adds a dedicated frame-associated native Mojo
service and `_runOyaRuntime` on the browser-owned frame. The service uses Blink's
main-world script execution and V8 value APIs directly. It does not open a debugger,
use an inspector session, forward protocol commands or install a page-global
object registry. The external compatibility adapter translates supported Runtime
requests into fixed native operations.

Each connection gets an opaque owner namespace; documents and values get native
unguessable identities. Navigation clears values and rebinds replaced frame
channels. Disconnect marks owners closed so late promise callbacks cannot recreate
handles. Pending calls use the existing exact-document execution lease. Own-property
inspection rejects proxies and does not invoke accessors; JSON-by-value serialization
has ordinary JSON semantics and may execute page `toJSON` methods/getters.

Bounds: 64 owners per frame, 2,048 handles per owner, 512 properties per inspection,
128 call arguments and 1 MiB source/string results. Promise/native response deadlines
are ten seconds, not a guarantee of terminating infinite synchronous page scripts.
Only default main-world contexts are exposed. Full debugger semantics, preview,
side-effect detection, raw-evaluation exception stacks and worker contexts
are not implemented. Child frames require the additional lifecycle patch below. See the front-door README for accepted options.

The macOS real-engine regression checks main/isolated-world separation, primitives,
object ownership, safe descriptors, function calls, promise results, release groups,
navigation cancellation/recovery and suppression during human control. Windows
compilation and runtime verification remain required before cross-platform release.

## Native response capture

Apply `patches/native-network.patch` after the native Runtime patches. It adds
an engine-owned Mojo data-pipe tee and `_setOyaBodyListener`, not a debugging
transport. Original bytes are forwarded with backpressure; capture is bounded
to 512 KiB and eight streams per session. Overflow disables capture, not delivery.
Disabling the listener invalidates in-flight callbacks by epoch. Completion is
posted to the owning task runner to avoid reentrant JavaScript during response
forwarding. Native request headers are exposed from the actual request.

The external adapter uses native webRequest continuations only on exclusively
owned private sessions; it never forwards CDP requests to another endpoint.
macOS native integration covers binary bytes, a one-MiB nontruncated response,
request pause/continue/fail and authorization. Windows remains unverified.

## Native child-frame Runtime lifecycle

Apply `patches/native-runtime-frames.patch` after the native Runtime and network
patches. It exposes an explicit lifecycle capability marker and a browser-owned
frame-tree removal signal after the native wrapper has been revoked. The adapter
combines that signal with existing native frame creation, document readiness and
navigation events. No debugger or polling backend is involved.

Main-world contexts now include cross-process descendants. Their IDs agree with
`Page.getFrameTree`, and handles route only to the native document that allocated
them. Native document tokens reject replacements even when a wrapper survives.
Cross-frame argument handles are rejected before function invocation. Group
release covers the selected target's documents; siblings survive removal or
navigation of another child. Older engines explicitly reject Runtime observation
rather than advertising incomplete child lifecycle support.

The real macOS front-door regression covers identical iframe URLs, cross-process
context selection, handles, property inspection, promises, group release,
foreign-socket rejection, cross-frame argument rejection without page execution,
navigation replacement, removal cancellation and sibling preservation. Worker
contexts, Debugger and Windows verification are still separate outstanding work.

## Native cookie metadata

Apply `patches/native-cookies.patch` after the existing native patches. The native
cookie converter exposes actual canonical priority, source scheme, source port
and partitioning presence to the browser process. The external Storage adapter
reads those attributes from the native session cookie manager rather than using
an inspector or inventing protocol defaults. Old-engine metadata and unsupported
partitioned exports fail explicitly.

Real macOS tests cover HttpOnly delivery, server-provided High priority, session
and persistent cookies, path matching, private-context isolation, targeted native
URL/name deletion, cookie-only clearing and human-control refusal. Batch writes
are validated up front but native write failures are not transactional rollback.
Windows compilation/runtime validation is still outstanding.

## Native inherited property inspection

Apply `patches/native-runtime-properties.patch` after the existing Runtime patches.
A distinct native `inspectChain` operation walks actual V8 prototypes; older engines
reject it rather than silently pretending an own-only result is complete. Own-only
inspection remains unchanged. No inspector session or CDP transport is used.

The collector rejects proxy objects/prototypes before enumeration, never invokes
JavaScript accessors, preserves symbol identity and the nearest shadowing descriptor,
and reports actual `isOwn` values. Work is bounded to 32 prototype objects and 512
visited keys, counting duplicates before shadow filtering. Derived handles inherit
the receiver's release group and existing document/connection isolation.

The macOS engine builds and real Oya tests cover inheritance, shadowing, distinct
same-description symbols, accessor filtering, zero getter/proxy side effects,
limit errors and release-group cleanup. Windows verification remains outstanding.

## Native request failure reasons

Apply `patches/native-request-errors.patch` after `native-network.patch`. It adds
an explicit capability marker and a browser-process-only named error selection to
the original request-stage continuation. The engine maps the supported names to
native net errors; unspecified cancellation remains ERR_BLOCKED_BY_CLIENT. No
headers, URL, replacement response, request refetch or debugging transport is added.

The adapter validates all names, checks engine support before consuming a pause,
and retains current tab/frame/control/egress authorization. MacOS integration tests
exercise all 14 advertised reasons, asserting the actual native error notification,
zero server delivery, invalid-reason preservation and one-shot callback behavior.
Windows compilation and runtime verification remain outstanding.

## Fail-closed desktop packaging

Desktop packaging now requires `OYA_NATIVE_DISTRIBUTIONS`, with unpacked
Electron-compatible distribution directories named `darwin-arm64`, `darwin-x64`,
`win32-x64`, and `linux-x64`. A macOS directory contains `Electron.app`; Windows
contains `electron.exe`; Linux contains `electron`. These must be built patched
Oya engines, not renamed stock downloads. Do not point the directory at the whole
compiler output tree, which contains build intermediates unrelated to shipping.

The before-pack hook launches the selected executable in a disposable profile,
requires native frame/runtime/text/network capabilities, and executes a native
isolated-world calculation with debugger access forbidden. It checks the actual
process architecture against the target before calling the existing signing hook.
The distribution hook selects the identical platform/architecture directory for
unpacking. Validation deliberately runs in before-pack: the installed builder
catches errors from its distribution hook and otherwise downloads stock Electron.
Missing distributions, cross-OS packaging, failed probes, and wrong architectures
must fail before that fallback can occur.

This is a packaging capability gate, not release certification. It does not
replace full native suites, signed/notarized artifact tests, macOS entitlement
approval, native-mode rollout validation, or Windows/Linux execution tests. CI
still needs the corresponding pinned native engine artifacts and migration of
legacy integration fixtures before a release tag is safe. The local arm64
Testing build is not a universal production artifact.

## Native localStorage engine primitives (experimental)

`patches/native-local-storage.patch` adds session-owned read, restore and change
observation through the storage service's Mojo interfaces, without a debugger,
renderer injection or a CDP transport. Apply it to the same pinned Electron
checkout as the other native engine patches, using `git apply --check` first.

- `_readOyaLocalStorage(origin)` returns key/value pairs from that session's
  first-party storage key.
- `_restoreOyaLocalStorage(origin, pairs)` initializes an empty store; a nonempty
  store is returned unchanged. **Use only before exposing the partition to page
  navigation.** The empty check and writes are not a transaction against other
  writers. The production integration must serialize restoration and exclude
  active pages; partial write failures are explicit, not rolled back.
- `_watchOyaLocalStorage(origin)` resolves after native observation starts.
  `oya-local-storage-changed` carries the origin and an availability boolean, not
  stored values. A false value reports loss of the native observer.
- `_unwatchOyaLocalStorage(origin)` closes the observer. Cancelling an unready
  watch rejects its readiness promise.

Only canonical HTTP(S) origins are accepted; partitioned third-party and opaque
storage keys are not supported. Snapshots/imports are limited to 4,096 entries
and 1,048,576 UTF-16 code units, with no lossy UTF-8 round trip. Operations and
watch initialization have 15-second deadlines; a session has at most 1,024
watches. These are privileged main-process APIs, not renderer exports.

`OYA_NATIVE_ENGINE=/path/to/patched/Oya npm run test:native-storage` checks real
pre-script hydration, Unicode/NUL/unpaired-surrogate preservation, profile
isolation, rejected imports, unchanged existing data, native change/clear events
and watcher cancellation. It runs in a disposable parent-owned profile with
debugger access forbidden.

`src/main/sync/native-storage.ts` now provides a tested, session-bound lifecycle:
complete import validation before mutation, serialized initialization, observer
readiness, coalesced snapshots, offline/refused-send retention (including empty
logout state), bounded native waits, and disposal fencing. The native storage
integration also exercises this service against the real engine. Its `flush()`
means transport acceptance, **not durable server persistence**. The caller must
exclusively own the partition during initialization, register discovered origins,
and arrange periodic/reconnect/final flushing; observer events capture state but
do not themselves send it.

The native-browsing path now wires this through `PersonaStorage`: cold partitions
are initialized before tab creation, already exposed partitions are never restored
late, native main-frame navigations discover origins (including popups), and
periodic delivery uses captured changes instead of rescanning unchanged stores.
Reconnect and manual save flush the native state. A server-requested final capture
uses a correlated reply after ordered cookie/storage messages; server Stop awaits
that reply and durable encrypted persistence before dropping the connection.
A failed capture leaves the browser connected unless the operator explicitly
forces stop. Borrowed SDK desktop handles remain non-destructive.

The migrated `server/tests/integration/login-flow.test.js` exercises encrypted
login/MFA reload, cold first-script hydration, native trusted input, public gateway
discovery terminating at the native front door, logout, refused final capture,
durable save, and a fresh-engine restart. The packaging probe now also executes
native storage read/restore/watch operations; a stock engine cannot satisfy it.

**This does not complete the overall native migration or enable production
releases.** The legacy CDP driver, outbound-provider MCP lifecycle, gateway and
anonymity integration suites remain gated. The default-path migration, native
persona/worker protection and cross-platform release validation remain separate
requirements. Current engine evidence is local macOS arm64, not a Windows or
universal macOS production distribution.

## Native isolated runtime worlds

Apply `patches/native-runtime-worlds.patch` after the runtime property/frame
patches. This adds browser-process-only `isolatedContext` creation to
`_runOyaRuntime`; it does not attach a debugger or expose a renderer API.
An opaque owner/world pair receives its own V8 context identity and handle
registry. Numeric world IDs are allocated only inside the renderer from a
reserved monotonic range, excluding Electron preload and Oya's internal world.
No universal-access security origin or preload bridge is installed.

Calls in a world must carry its exact native context token. Different owners
using the same world key do not share globals, intrinsic prototypes or handles.
Contexts still share the page DOM subject to normal origin checks. Closing an isolated world revokes
its value handles and leaves a bounded tombstone until navigation, so delayed calls
cannot resurrect the context. It does not cancel arbitrary timers or DOM effects
previously installed by agent scripts. Native document replacement invalidates
all its worlds. Allocation limits fail explicitly rather than reusing identities.

Run `OYA_NATIVE_ENGINE=/absolute/patched/executable npm run test:native-worlds`
from `browser/`. The fixture forbids debugger access and checks stable owned
identities, page/recorder/agent-global separation, DOM sharing, child-frame and
handle isolation, native invocation/promises/exceptions, close and navigation.

The external `Page.createIsolatedWorld` adapter now uses this primitive through
connection-owned context/value routing, lifecycle events and disposal. It rejects
universal-origin access and never uses the internal recorder world. Run
`test:native-front-door` for the public adapter regression. This is still partial
compatibility: worker contexts, persona/worker migration and other release gates
remain; the primitive is not proof of full CDP driver parity.

## Native session timezone prerequisite

`patches/native-session-timezone.patch` adds the experimental main-process-only
`session._setOyaTimeZone(zone)` primitive to the pinned Electron checkout. Apply
it after the existing engine patches. It is not wired into the default persona
path yet and does not replace the complete persona/worker policy.

The zone must be a bounded, ASCII ICU system timezone. It is immutable once set;
repeating the identical setting is allowed. First renderer launch locks an
unconfigured session too, so late installation fails instead of letting early
page or worker scripts observe another zone. A different persona must use a
different session partition. No clear/reset operation is exposed.

The browser supplies the zone from the exact renderer's BrowserContext after
processing web preferences, not from page-selected renderer arguments. Configured
sessions refuse spare renderer reuse. At renderer startup, before any document
script, Oya acquires Blink's native timezone-controller override for that process's
lifetime. This updates actual ICU/V8 Date behavior and worker isolates; it is not
a JavaScript `Intl` shim or an inspector-protocol call. Native installation failure
terminates that renderer rather than allowing unprotected execution.

`OYA_NATIVE_ENGINE=/absolute/patched/executable npm run test:native-timezone`
checks first-script Date/Intl values in three simultaneous session partitions,
cross-origin frames, dedicated/shared/service workers, navigation, invalid/late
configuration and renderer-argument override refusal. The launcher owns cleanup
after Electron exits. Full persona migration still requires native locale,
hardware/UA metadata and pre-document/pre-worker protection, plus production
platform validation; this prerequisite alone must not enable native mode by default.

## Native session hardware-concurrency prerequisite

After the timezone prerequisite, apply `patches/native-session-hardware-blink.patch`
from the pinned Chromium `src` checkout, then `patches/native-session-hardware.patch`
from `src/electron`. Both patches are required; they do not introduce a CDP
transport or a page-world navigator shim.

The experimental main-process-only `session._setOyaHardwareConcurrency(count)`
accepts integer counts from 1 through 256. It shares the timezone policy's
first-renderer lock: configure before any renderer, never change an installed
count, and use a separate session for another persona. Repeating the same count
is idempotent. Invalid input is rejected before mutation. Hardware-only sessions
also refuse spare renderer reuse, and renderer arguments cannot supply or replace
the browser-owned value.

At renderer startup, a native atomic value is installed before document or worker
scripts. Blink's Navigator/WorkerNavigator getter reads it directly, with native
session policy taking precedence over inspector emulation. Unconfigured sessions
retain the engine's host processor count. This changes the web-exposed logical
processor count, not OS scheduling, CPU quotas, GPU identity or device memory.

Run `test:native-hardware` with an explicit `OYA_NATIVE_ENGINE`. It reuses the
timezone first-script fixture with hardware checks enabled, including independent
counts across partitions, invalid/fractional/non-numeric input, late/conflicting
configuration, argument injection, a hardware-only partition, and a service-worker
restart with its owning window closed. It does not enable full native persona
mode; native locale, UA metadata and remaining pre-script policy are still required.

## Native session ICU locale prerequisite

Apply `patches/native-session-locale.patch` from the pinned Electron checkout
after the timezone and hardware patches, and apply
`patches/native-session-locale-v8.patch` from the Chromium `src` checkout.
The V8 patch routes omitted-locale case operations through the native runtime
so Turkish/Azeri casing uses the isolate default instead of root-locale casing.
Explicit fast-path locales remain unchanged. Both patches are required.
The experimental main-process-only
`session._setOyaLocale(tag)` installs a canonical BCP47 locale before any session
renderer starts. Empty, malformed, partially parsed, non-ASCII, overlong and
language-less tags are rejected. An identical canonical tag is idempotent;
changing a configured locale or installing one after renderer startup fails.
Syntactically valid languages may still use ICU's ordinary fallback behavior.

The browser strips caller-supplied locale switches and supplies only the owning
BrowserContext's policy. Locale-only sessions also refuse spare renderer reuse.
Renderer startup sets ICU's process default and invalidates native V8 locale/date
caches before document or worker execution. Workers inherit that native default;
this is neither a JavaScript shim nor an inspector helper/protocol operation.

Run `test:native-locale` with an explicit `OYA_NATIVE_ENGINE`. It checks default
Intl formatting and locale-sensitive casing in first scripts of pages,
cross-origin frames, dedicated/shared/service workers, three simultaneous
partitions, replacement navigation and a cold service-worker restart. It also
checks argument injection, invalid/late/conflicting configuration and a
locale-only partition that retains host timezone and hardware behavior.

The locale patches alone control ICU/Intl defaults. The language-policy extension
below binds those defaults to native navigator values and default request headers.
UA metadata and remaining pre-script protections still block production persona
adoption. Native mode remains disabled by default; these prerequisites are not
completion of the persona migration or macOS/Windows release validation.

## Native session language-policy prerequisite

Apply `patches/native-session-languages.patch` from the pinned Electron checkout
**after** the locale prerequisite. `_setOyaLocale(tag)` now also owns the preferred
language list: the canonical tag followed by its primary language when different
(for example, `de-DE,de`; a bare `de` is not duplicated). Native language-reduction
features remain untouched. This is not an arbitrary independently mutable list.

BrowserContext supplies document renderer preferences, worker preferences even
without an owning window, and initial network-context parameters. Installing the
policy also updates already-created network contexts before the first renderer.
Network-context recreation reads the same immutable session value. No page-world
getter replacement, request-interception workaround or internal CDP is involved.

`session.setUserAgent()` preserves native language policy. A conflicting optional
language argument fails before either the user agent or headers are changed;
an omitted language or the exact policy list is accepted. Unconfigured sessions
retain their existing behavior, including platform-dependent differences between
page and worker language lists. Explicit per-request headers remain subject to
normal browser rules; this controls defaults, not request interception. This does
not implement native UA metadata.

`test:native-locale` additionally checks first-script navigator language/list and
actual HTTP request headers for pages, cross-origin frames and all worker script
types; renderer fetches; browser-owned requests with a pre-existing network
context; conflicting/idempotent user-agent updates before and after startup;
and primary-language-only configuration. The cold service-worker restart and
independent-partition tests still apply. Production activation and platform
release validation remain separate gates.

## Native session policy readback and preparation

Apply `patches/native-session-policy-state.patch` after the language-policy patch.
The main-process-only `_getOyaSessionPolicy()` returns a fresh snapshot containing
contract `version: 1`, `rendererStarted`, `timeZone`, `locale`,
`hardwareConcurrency` and comma-separated `acceptLanguages`. Empty strings and a
zero processor count denote unconfigured fields. Mutating this snapshot cannot
change browser-owned state; it offers no reset or unlock operation.

`src/main/native-policy/` validates the entire supported subset before mutation,
requires the versioned readback contract and every setter, rejects warm sessions,
and verifies native results before publishing an installed binding. Partial
native failure permanently quarantines that session **within its lifecycle
owner**; it is not an engine rollback, nor permission to reuse it through another
owner. All future consumers must use the same owner and check its exposure guard.

Run `test:native-policy` against the explicit patched engine. It exercises the
coordinator in the same real first-script page/worker and HTTP fixtures, and
checks late setup, invalid-input non-mutation, native snapshot isolation and
partial-failure quarantine. The application default is unchanged: full persona
migration, remaining legacy fixtures and platform release validation still block
production activation. A successful subset is never a complete protection verdict.

## Native session user-agent string prerequisite

Apply `patches/native-session-user-agent-content.patch` from the pinned Chromium
`src` checkout, and `patches/native-session-user-agent.patch` from its Electron
checkout after the policy-state patch. The main-process-only
`session._setOyaUserAgent(value)` installs one immutable, nonempty printable-ASCII
user-agent string (maximum 1,024 bytes) before any session renderer starts.
Identical values are idempotent, including after startup; invalid, late or
conflicting configuration fails without changing the policy. The version-1
readback adds `userAgent`; an empty string means unconfigured. This is an additive
field, not a certification of a complete native persona.

The policy belongs to BrowserContext, not the application-wide fallback. Shared
and service-worker startup reads the owning context directly, including cold
service-worker restart without an owning page. Documents and dedicated workers
inherit the session's native WebContents override. Renderer initialization and
navigation defaults also read the owning context, so cross-process iframes do not
fall back to application-wide identity. Initial and already-created
network contexts use the same session value. Spare renderers are refused for a
configured session. No debugger, protocol backend, page shim or request-header
interceptor implements this behavior.

Conflicting public session/WebContents user-agent setters and `loadURL` user-agent
options fail; history restoration preserves the installed identity. Application
fallback changes cannot rewrite configured sessions. Unconfigured sessions retain
legacy behavior. Explicit per-request headers remain subject to normal browser
rules: this is a default identity policy, not an outbound-header allowlist.

Run `test:native-user-agent` with an explicit patched `OYA_NATIVE_ENGINE`. It
checks first-script navigator values, actual script-request and fetch headers,
parallel partitions, cross-origin frames, dedicated/shared/service workers,
already-created network contexts, invalid/late/conflicting changes, snapshot
isolation, history restoration and cold service-worker restart.

**Scope:** this installs the user-agent **string only**. UA client-hint metadata,
`navigator.platform`, full persona consistency and the remaining native protection
lifecycle are not implemented by this patch. The existing native-policy
coordinator still certifies only timezone, locale/languages and hardware count.
Default persona protection remains unchanged. Do not activate this as a complete
persona, claim internal-CDP-free production, or release on this prerequisite alone.

## Native session observable UA metadata prerequisite

Apply `patches/native-session-ua-metadata-content.patch` from Chromium `src` and
`patches/native-session-ua-metadata.patch` from Electron, after the user-agent
string patches. `_setOyaUserAgentMetadata(value)` installs copied, immutable
metadata after `_setOyaUserAgent()` and before any session renderer. Identical
reinstallation is allowed; late installation or conflicting values fail.

The required fields are `brands`, `fullVersionList`, `fullVersion`, `platform`,
`platformVersion`, `architecture`, `model`, `mobile`, `bitness`, `wow64`, and
`formFactors`. Strings are printable ASCII, at most 128 bytes. Brand lists contain
one to eight unique names, matching in order between major/full-version lists.
Both flags must be booleans; form factors must be unique native-supported values.
Malformed input is rejected before mutation. Native readback adds a fresh
`userAgentMetadata` object only when installed; changing the input or snapshot
does not mutate the policy. This validates shape and bounds, not the semantic
consistency of an entire device persona or its version claims.

Renderer startup, WebContents overrides, and dedicated/shared/service-worker
startup use BrowserContext-owned native metadata, never application-wide metadata
for a configured session. Native `navigator.userAgentData` exposes the configured
low- and high-entropy values. Navigation, history restoration and worker restart
retain the same policy. Unconfigured sessions preserve existing behavior.

`test:native-ua-metadata` extends the real user-agent fixture with distinct
synthetic Oya test identities, first-script low/high-entropy snapshots in pages,
cross-origin frames and all three worker types, malformed/late/conflicting input,
input/readback mutation isolation and a cold service-worker restart. The ordinary
`test:native-user-agent` still runs independently without installing metadata.

**Scope of this patch alone:** it supplies observable metadata, not HTTP
client-hint negotiation. The subsequent controller patch below adds native
navigation negotiation and its own tests; the legacy header interceptor has not
been replaced on the default path. The later platform prerequisite covers
`navigator.platform`; full native persona activation and macOS/Windows release
validation remain outstanding. The
native-policy coordinator still certifies only its existing explicit subset.

## Native UA client-hint negotiation prerequisite

Apply `patches/native-session-client-hints.patch` from the pinned Electron
checkout after the UA metadata patches. A BrowserContext-owned controller is
created only for sessions with installed native metadata; unconfigured sessions
retain their previous behavior. It supplies the immutable native metadata to the
engine's ordinary client-hint negotiation rather than rewriting requests or using
a debugger.

Low-entropy UA headers use the configured metadata. High-entropy UA hints require
origin opt-in through `Accept-CH`; the engine retains its trustworthy-origin,
JavaScript/WebPreferences, permissions-policy and navigation checks. The controller
remembers only UA-family opt-ins, keyed by exact origin, in memory for that
BrowserContext's lifetime. It keeps at most 256 origins and refuses additional
new opt-ins at the bound; replacing or revoking an existing entry still works.
Temporary additional-hint lists are deduplicated and restricted to UA hints.
This is not a disk-backed preference store or a general device/network-hint
implementation.

An empty `Accept-CH` or native `Clear-Site-Data: "clientHints"` revokes that origin's
opt-in. Session `clearCache`, `clearStorageData` and valid `clearData` calls also
forget the entire session's hint cache without changing its immutable identity.
For these session APIs this deliberately over-clears opt-ins even when their
storage filters select only some origins or data types. Hint state is not shared
with other sessions and is not restored after browser restart.

Run `test:native-client-hints` with an explicit patched `OYA_NATIVE_ENGINE`. It
uses actual HTTP response opt-in and records actual incoming request headers on
local servers. It covers low/high metadata values, host/port/session isolation,
same-session reuse, permission-policy denial and cross-origin delegation,
insecure-origin refusal, redirect recomputation, native critical-hint retry,
JavaScript-disabled navigation, empty opt-in, Clear-Site-Data and session data
clearing. Debugger access throws in every test window; there is no header
interceptor in this fixture.

This is still a prerequisite, not production persona activation. The legacy
header interceptor remains on the default protection path until the complete
native identity/lifecycle migration is ready. The platform prerequisite below
covers the remaining legacy navigator string; other native persona protections,
the legacy integration fixtures and the platform release matrix remain separate
gates. Worker request negotiation, browser-owned `session.fetch`, persistence
across restart and every non-UA client hint are not certified by this
navigation/fetch fixture; separate worker privacy coverage is described below.

## Worker request privacy coverage

`test:native-worker-client-hints` records real script-request and same-/cross-origin
fetch headers for classic and module dedicated, shared and service workers. It
checks first-script native identity in distinct sessions, parent opt-in and denial,
cold service-worker restart without an owning window, and storage clearing.

The pinned engine intentionally gives worker requests an all-blocking permissions
policy while worker permission-policy propagation is unsupported. Its worker fetch
context does not append UA client hints. The test therefore requires **no UA hint
headers**, rather than manufacturing headers or relaxing that protection. Native
user-agent strings and `navigator.userAgentData` still belong to the correct
session. This verifies the current guarded behavior; it does not implement worker
client-hint negotiation or certify browser-owned `session.fetch` hint behavior.

## Native legacy navigator platform prerequisite

Apply `patches/native-session-platform-blink.patch` from Chromium `src` and
`patches/native-session-platform.patch` from Electron after the preceding native
identity prerequisites. `_setOyaPlatform(value)` accepts the desktop profile
values `MacIntel`, `Win32` and `Linux x86_64`. It installs before any session
renderer; identical reuse is allowed, but late installation, malformed values and
conflicting changes fail. Readback adds `platform` only when installed.

BrowserContext owns the immutable value. Child launch sanitizes duplicate/raw
platform switches before forwarding only the owning context's value, and a
platform-only session refuses spare renderer reuse. Renderer startup installs a
one-time native atomic index before website/worker execution. Native Navigator
and WorkerNavigator getters read that index, preserving the original getter and
unconfigured host behavior; no page shim or debugger implements it. A native
policy takes precedence over inspector/page-setting platform overrides.

Run `test:native-platform` against the explicit patched engine. It checks all
supported platform-only values, forged launch arguments, unconfigured host
behavior, invalid/late/conflicting installation, snapshot isolation, first-script
page/cross-origin-frame/worker values, independent sessions, navigation and cold
service-worker restart. This changes the web-exposed legacy string, not the OS,
keyboard conventions, UA metadata, renderer architecture or hardware.

Full persona consistency still requires the application lifecycle to install all
native fields together before exposure; the native-policy coordinator now
requires platform, UA and metadata alongside timezone, locale/languages and
processor count, and verifies the native readback before publication. It validates
identity shape and bounds but does not yet certify semantic persona coherence or
full persona coverage. Default persona protection
is not migrated by these prerequisites. Remaining native protections, legacy test
migration and the signed macOS/Windows release matrix are still outstanding.

## Native session pre-script policy

`patches/native-session-pre-scripts.patch` applies from the pinned Chromium `src`
root after the identity prerequisites above. It includes the Electron and Blink
changes together. Apply it with
`sh browser/engine/tools/apply-native-pre-scripts.sh /path/to/oya-electron/src`.
Build the existing `PasskeyTesting` target with at most four jobs; put the pinned
`buildtools/mac` directory on PATH so Node's generated config can find GN.

The synchronous `_setOyaPreScriptPolicy({page, worker})` session API accepts two
source strings, each bounded to one MiB. The page source must be nonempty; an
empty worker source explicitly requests no additional worker script (the
no-persona lifecycle still installs native scalar identity). Both compile successfully
before the session commits either. A different policy or installation after any
renderer starts is rejected; identical installed sources can be reused.
`_getOyaSessionPolicy()` exposes `preScriptPolicyVersion: 1` and, once configured,
a copied `preScriptPolicy: {page, worker}` snapshot. This capability must be
required before enabling the native persona lifecycle.

Sources remain owned by the exact BrowserContext. A renderer bootstrap Mojo
interface resolves the requesting RenderProcessHost's context, never a
renderer-selected partition, and synchronously returns its immutable sources.
Configured sessions refuse spare renderer reuse. Blink installs the page source
in each main-world context after its native interfaces are initialized, and the
worker source in `PrepareForEvaluation` for dedicated, shared and service workers
before their author scripts. Worklets are not worker-navigator contexts and are
excluded. Isolated worlds are not modified. A failed bootstrap or runtime source
exception stops the renderer before unprotected author code can run.

This executes the existing persona injection sources through a browser-owned
engine hook. It does not convert their fingerprint getter wrappers into native
Blink getters, change those source bytes, or claim that injected wrappers are
undetectable. Native scalar policy remains responsible for UA/metadata, platform,
timezone, locale/languages and hardware concurrency.

`test:native-pre-scripts` explicitly selects `OYA_NATIVE_ENGINE` and checks the
unchanged production builders against first-author-script page, cross-process
frame, popup, classic/module dedicated and shared workers, service workers,
parallel partitions, reload and cold service restart. The fixture throws on
Electron debugger access. Passing macOS coverage does not certify a Windows
runtime, signed distribution, or real passkey credential.

### Native cross-process input routing

`patches/native-pointer-routing.patch` routes onscreen mouse and wheel input
through Chromium's browser-owned hit-test router for the exact WebContents.
Keyboard input follows its focused render widget, including an out-of-process
child frame, after native popup handling. Offscreen input retains its existing
native path. Apply it with `tools/apply-native-pointer-routing.sh /path/to/src`
after the existing Oya engine patches. The native workflow fixture exercises
nested cross-process pointer focus and typing; no debugger transport is involved.

### Opaque-origin WebAuthn availability

`patches/native-webauthn-opaque-origin.patch` rejects opaque-origin platform
and conditional authenticator availability with `false` before consulting
origin-bound delegates or request proxies. The proxy invariant assertion and
credential creation/assertion origin validation stay intact. Apply with
`tools/apply-native-webauthn-opaque-origin.sh /path/to/src`. The isolated native
`tests/integration/native-webauthn-opaque.mjs` regression covers secure sandboxed
opaque frames, all three capability probes, and ordinary-origin availability.
