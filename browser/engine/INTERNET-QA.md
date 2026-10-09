<!-- Live native-operation acceptance evidence and remaining release gates. -->

# The Internet: native QA, 2026-10-09

**Not all green / not release-ready.** Latest complete macOS arm64 run: 42 of
45 behavioral checks passed, covering all 44 catalog entries. The suite exits
nonzero. Catalog coverage is not exhaustive coverage of every example variant.

## Remaining failures

| Check                      | Observed result                                                                                         | Ownership                   |
| -------------------------- | ------------------------------------------------------------------------------------------------------- | --------------------------- |
| Forgot-password submission | Server returns HTTP 500 / Internal Server Error                                                         | External demo endpoint      |
| Rich-text editing          | TinyMCE explicitly reports exhausted monthly editor quota and read-only mode                            | External demo configuration |
| Frames navigation          | `/frame_middle` and `/frame_right` returned HTTP 503; analyzer recorded Heroku application-error frames | External demo availability  |

No assertions were relaxed to turn these failures green. No DOM drag events,
FileList assignment, read-only removal, or alternate browser were used to fake
acceptance. HTML drag-and-drop now passes through the browser-owned native
controller, including the live A/B swap. The separate AppKit enum guard remains
crash prevention rather than the mechanism used to deliver that drop.

The first rerun exposed a native-evaluation hang during reload. A local pending
frame-removal test reproduced an unresolved promise. The document-lifetime patch
now cancels pending requests on removal/replacement/destruction and bounds
never-settling promises. Both the local regression and the final live
`challenging-dom-refresh` check pass. The earlier failed run was not counted as
passing or replaced with a skipped assertion.

## Verified scope

The live suite uses Oya native operations, a disposable audit profile, and the
production analyzer/input implementations. Debugger access throws. The final
run reused the HTTP cache of a previous disposable audit profile; it did not
mock site responses. The browser engine includes the patches documented in
README.md. It is not the stock installed runtime.

Confirmed behaviors include native select keys, dialogs, generated-file upload,
downloads, Basic/Digest authentication, native HTML drag/drop, sortable tables, nested frames, menus,
hover, and dynamic content. Geolocation tests denial without disclosing a real
location, not successful location retrieval.

Browser unit suite: 1,876 passed, zero skipped. Browser typecheck, lint,
formatting, source regressions, and build passed. Local Oya regressions cover
iframe editing independently of the live TinyMCE quota.

The production `PageDriver` now has a separate local command-path regression in
`test:native-input`. It resolves fresh analyzer IDs, refuses covered click/type
targets (including overlays within and above an iframe), verifies trusted form
submission, and replaces iframe contenteditable text. This reproduced a real
selection bug (`Old contenEdited` instead of `Edited`): selection now belongs to
the editor's document, not the top document. It does not bypass the public
TinyMCE read-only restriction or simulate a working forgot-password endpoint.
This fixture exercises the production command driver, not the authenticated
server/agent transport or the complete agent loop.

Native drag regressions verify trusted payload/event delivery, rejected drops,
concurrency, invalid coordinates, navigation, closure, focus loss, unsupported
child sources, real out-of-process target isolation, cancelled dragstart, and
slider input. Native frame regressions verify pending-request cancellation,
stale-global isolation and the ten-second execution deadline.

This does **not** validate the complete production agent loop or Windows.
Native drag is currently limited to non-file payloads from the owning main
document; other renderer widgets and child-frame sources fail explicitly.
The private native file-selection event still requires production authorization
and tool integration. The repository-wide aggregate suite is not green: legacy
non-Oya integration launchers are refused by the integration gate until migrated.
No commit, push, or release is justified by this report.

## Reproduce

From `browser/`, set `OYA_NATIVE_ENGINE` to the patched Oya executable and run
`npm run test:internet`. Each run prints its scratch evidence directory, including
`report.json`, `network.jsonl`, analyzer observations and per-check screenshots.
Without that explicit engine path the test refuses to fall back.

Latest local evidence directory:
`/var/folders/t6/qxxbbwn904301xxb03nyknhh0000gn/T/oya-internet-audit-WwutJ5`.
The earlier complete run at `oya-internet-audit-aOf2jQ` passed 43/45;
its frame-navigation pass does not replace the latest failed assertion.
The latest `network.jsonl` records both frame HTTP 503 responses and
`analysis.jsonl` records the corresponding Heroku application-error documents.
These scratch files are machine-local and not release artifacts.

## Amazon search performance follow-up

A native production-command diagnostic reproduced slow analysis on Amazon:
a home-page read took 7.1 seconds, and the results-page read hit the native
execution timeout. Profiling showed `scopedText` repeatedly scanning the full
DOM for every control to count exact-text duplicates.

The analyzer now builds one exact-text index per synchronous analysis and drops
it immediately afterwards. Visibility lookup reuses live element references.
The local 500-control regression asserts one index pass, unchanged duplicate
scope selectors, and invalidation after both removal and insertion of duplicate
text. No timeout was extended, controls dropped, or repeated labels ignored.

The live command diagnostic completed navigate → analyze → type `jordans` →
Enter → analyze results in approximately 12 seconds after the index fix, with
182 controls on the home page and 1,122 on the results page. It used a separate
profile, native input and the production shortcut ownership fence. This is
**not** the LLM agent loop or a guarantee of its end-to-end latency.

To repeat from `browser/`, run the explicitly patched Oya executable with
`tests/integration/native-search-electron.cjs`. Optional `OYA_PROFILE_ANALYZER=1`
prints isolated-world function timings. The diagnostic only searches; it does
not sign in or purchase. The initial console callback signature error was a
diagnostic bug, corrected before the successful run.

## Live external-CDP selector regression — 2026-10-09

Fixed `__acFindElement` parsing the first digits anywhere in a reference. For
example, `[data-f9c3c8b3="3"]` selected element **9**, not **3**. The parser now
accepts a complete positive numeric reference or a quoted numeric attribute
value, checks safe integer bounds, and rejects malformed references. It retains
legacy `data-ac-id` references without falling back to arbitrary CSS.

Verification used the running native Oya desktop, its authenticated external CDP
front door and production native Oya action extensions. Actions used the actual
analyzer-generated attribute selectors, **not numeric-ID workarounds**. Fresh
test tabs were closed afterwards; existing user tabs were not operated on.
These are command-path smoke flows, not a full LLM agent-loop benchmark.

| Site         | Verified flow                                                 |  Time | Analyses |
| ------------ | ------------------------------------------------------------- | ----: | -------: |
| Amazon       | Type “jordans”, Enter, verify result URL and product content  |  9.3s |        2 |
| Wikipedia    | Search “JavaScript”, read the article                         |  6.7s |        2 |
| eBay         | Search “jordans”, verify result URL and results               | 16.9s |        2 |
| MDN          | Open search, type “Array”, Enter, read Array reference        |  4.2s |        3 |
| npm          | Type “lodash”, click Search, verify package results           |  3.4s |        3 |
| DuckDuckGo   | Search “jordans”, verify readable results                     |  4.7s |        2 |
| Hacker News  | Click “new”, verify newest stories                            |  1.6s |        2 |
| The Internet | Open checkboxes, click an unchecked box, verify checked state | 31.6s |        3 |
| Python docs  | Search “asyncio”, verify results                              |  2.6s |        2 |
| W3Schools    | Follow tutorial navigation to JavaScript Introduction         |  3.9s |        2 |

**Limitations and unsuccessful attempts retained:** GitHub redirected the public
repository request to sign-in and was not counted as a pass; W3Schools was an
additional site, not a successful GitHub retest. npm did not submit on Enter in
this run; its explicit Search button completed the flow. The Internet initially
returned an empty analysis and needed one same-URL native navigation. MDN's first
assertion incorrectly required a search-results URL, although its autocomplete
opened the correct Array reference; a corrected article-or-results assertion was
rerun successfully. No access challenges were bypassed and no login credentials
were entered.

Local, private evidence (not checked into the repository):
`/private/tmp/oya-cdp-selector-fixed/report.json` and
`/private/tmp/oya-cdp-selector-fixed/followup/report.json`, with command timings,
analyses and failure screenshots alongside them. The original failed reports are
preserved, rather than replaced with the follow-up outcomes.

Checks: 1,917 browser unit tests passed, including 18 selector cases. Real native
analyzer tests verified exact input values and a trusted click using a fixed
digit-bearing attribute name, with debugger access forbidden. Source regressions,
typecheck, lint, format and build passed. Windows runtime verification remains
outstanding; the tested macOS app is still running with this fix.

## 2026-10-09 — complex flows and native target lifecycle

External authenticated CDP was used only as Oya's front door, not as an internal
backend. The built macOS app passed:

- Wikipedia JavaScript → ECMAScript navigation while another tab remained intact,
  stale DOM handle rejection, explicit activation, and screenshot capture (6.0s).
- The Internet's publicly advertised demo login: invalid credentials rejected,
  valid demo login, secure-area verification, logout (8.0s).
- Two-tab local fixture: trusted field entry/click in the background tab,
  unchanged second tab, foreign-client DOM rejection, scoped viewport override
  and restoration (3.1s).
- Native target discovery plus page-only automatic attachment on the running
  app: create a disposable tab, use its flat session to read actual DOM, close,
  receive exactly one detach plus destruction, reject the stale session.

npm's autocomplete-to-package flow passed separately (3.4s), but its combined
rerun exposed a URL/content mismatch: the package URL appeared while the home
page title remained. A bounded content-readiness retry also failed; the underlying
SPA completion problem is not yet resolved.
Earlier pointer attempts hit a real covering autocomplete, correctly refused by
the native hit test. No synthetic-click bypass was added. This remains a
timing-sensitive flow, not an unconditional final-suite pass.

Hidden-tab capture initially returned UnknownVizError. Explicit native
Page.bringToFront made capture pass; invisible background capture is not claimed.

Evidence is preserved privately at:

- /private/tmp/oya-complex-flows-rL2bsh/report.json (combined rerun, including npm failure)
- /private/tmp/oya-complex-flows-oMtjtQ/report.json (npm keyboard flow pass)
- /private/tmp/oya-complex-flows-re0TYK/report.json (bounded content retry failure)
- /private/tmp/oya-complex-flows-v9t295/ (covered-element screenshot and analyses)

Added native Target.setDiscoverTargets, Target.activateTarget, Page.bringToFront
and explicitly page-only Target.setAutoAttach. Automatic sessions require
flatten:true, waitForDebuggerOnStart:false and filter:[{type:"page"}]; unsupported
worker/frame and pause semantics are rejected, not silently ignored. Session
capacity, disconnect cleanup, manual-session preservation and independent socket
ownership have regression coverage.

Checks: 1,929 unit tests passed; typecheck, lint, format, source regressions and
build passed. Real Oya engine integration passed with debugger access forbidden,
including automatic session DOM reads and disable/revocation. No Windows runtime
test or complete Runtime/Network/Debugger compatibility is claimed.

## 2026-10-09 — readiness events and acknowledged native input

Added native-backed Page.enable/disable (main-document DOM-ready and load
events), Page.reload/stopLoading, DOM.focus/scrollIntoViewIfNeeded, and
Input.insertText. Domain subscriptions are connection/session scoped, idempotent,
and cleaned up independently; disabling Page does not disable Log.

A real regression was found while testing text insertion: the previous native
IME implementation resolved immediately and could drop Unicode when OS focus was
held by Oya's control shield. Merely bringing the tab forward did not fix the
production app. The engine now performs document-scoped, acknowledged composition
with temporary renderer-widget focus, without moving OS focus or weakening the
shield. Missing engine capability fails explicitly; there is no debugging or
page-script fallback. Native concurrent insertion is refused.

Verification on the rebuilt macOS engine:

- Hidden-page DOM focus and trusted Unicode insertion, observed immediately after
  acknowledgement, passed; concurrent insertion was rejected.
- The production shielded iframe test passed Unicode insertion while shell focus
  remained intact and unmarked keyboard input remained blocked.
- The running Oya app passed DOM focus, trusted Unicode, scroll, event-driven
  reload, stale-node rejection, and cleanup through its external endpoint.
- The final complex-flow run passed Wikipedia multi-tab/stale handles/capture
  (6.0s), npm autocomplete-to-package (3.5s), public demo invalid/valid login and
  logout (7.8s), and background input/cross-client/viewport isolation (3.2s).

Latest complex-flow evidence:
`/private/tmp/oya-complex-flows-NNJ3Ew/report.json`.
Earlier npm failures remain preserved. Native console diagnostics also observed
npm's own `onChangeActiveElement is not a function` exception, including during
a successful run; that alone does not establish the cause of every previous SPA
failure. One successful final run is not a claim that all npm timing issues are
permanently resolved.

Checks: 1,936 unit tests, source regressions, typecheck, lint, format and app build
passed. The native engine compiled successfully. Native-front-door and
native-input integration suites passed with debugger access forbidden. Windows
runtime, full Runtime object/context lifecycle, Network interception/body capture,
Debugger, browser contexts and download control remain unfinished.

## 2026-10-09 — native main-world Runtime and value lifetimes

Added Runtime.enable/disable, evaluate, callFunctionOn, getProperties (explicit
own-properties only), awaitPromise and releaseObject/releaseObjectGroup. The
external protocol is terminated in Oya. A new native frame-associated Mojo service
uses Blink main-world execution and V8 values, with no inspector/debugger backend
and no page-visible value registry. The isolated analyzer world is unchanged.

The first real-engine navigation test found a disconnected native channel retained
across frame replacement. Resetting/rebinding that channel fixed both context
notifications and direct evaluation after reload. Additional descriptor tests
caught the need to ignore inherited descriptor getters; inspection now checks
own flags before reading them.

Verification on macOS arm64:

- Real Oya tests passed main-world/isolated-world separation, primitives including
  bigint/nonfinite/negative-zero, nested JSON, cyclic-value refusal, symbol keys,
  accessor-safe descriptors (including poisoned Object.prototype), exact function
  receivers/arguments, foreign handles/contexts, promises and page exceptions.
- Release groups invalidate derived values; navigation cancels pending evaluation,
  recreates contexts and rejects stale identities. Human-held context changes are
  suppressed. Unit tests cover late/out-of-order event replies and disposal.
- The rebuilt production app passed Runtime mutation, native promises, foreign
  handle refusal, trusted Unicode with a main-world value oracle, group release
  and reload recovery through the authenticated external front door.
- Four complex-flow regressions passed again: Wikipedia multi-tab/stale handles
  (6.0s), npm autocomplete/package documentation (3.4s), demo invalid/valid login
  and logout (8.0s), and background native input/client/viewport isolation (3.1s).
  Evidence: `/private/tmp/oya-complex-flows-9jdAka/report.json`.
- 1,943 unit tests passed. Native-front-door and native-input integration passed
  with debugger access forbidden. Engine/app builds, source regressions,
  typecheck, lint and formatting passed.

This is bounded default-main-world Runtime support, not complete CDP compatibility.
Inspector previews, side-effect guarantees, custom timeouts, raw-evaluation stack
traces and child/worker contexts remain unsupported. A response timeout does not
terminate infinite synchronous JavaScript and must not trigger automatic retries.
Network interception/body capture, Debugger, browser contexts, download control
and Windows compilation/runtime validation remain outstanding. The legacy
Playwright recorder suite was not run; native Oya suites were used instead.

## Native private sessions and network — 2026-10-09

Implemented native private-context lifetime/storage isolation, scoped GUID downloads,
and original-response capture with native request-stage pause/continue/fail. CDP
is only the external command format; no upstream debugging socket, Electron
debugger or inspector session backs these capabilities. The native response tee
was compiled into the macOS Oya engine; its checked-in patch passes reverse-apply
validation against the tested engine source.

Verification for this stage:

- 1,961 browser unit tests passed (zero skipped), including continuation of a held
  native request without releasing/reacquiring its command admission gate.
- Typecheck, lint, build, touched-module formatting and diff whitespace passed.
- Source regression, control-state, release guard and workflow suites passed.
- Real Oya `test:native-contexts` passed: separate cookies/storage, foreign-context
  refusal, actual download bytes and cancellation, original binary response bytes
  with exactly one server request, full one-MiB page delivery despite capture
  overflow, request pause/continue/fail and observation revocation.
- Real Oya `test:native-front-door` passed, including native Runtime, DOM, main/child
  frame trees, trusted Unicode, navigation cancellation and ownership. These tests
  forbid Electron debugger access.

Evidence: `/private/tmp/oya-network-native/{unit-escalated-final,integration-final,front-door-final,lint-final,build-final}.log`.
This stage uses disposable localhost fixtures, not a new public-site benchmark.
The default user profile is not intercepted. Full standard Network events,
Debugger, child/worker Runtime sessions and Windows validation remain incomplete.

## Native child-frame Runtime — 2026-10-09

The compiled `native-runtime-frames.patch` adds browser-owned frame removal
notifications. Runtime now advertises main-world contexts for cross-process
children with identities shared by `Page.getFrameTree`; receiver/promise/property
handles route to their exact allocating document. Cross-frame argument handles
are rejected before invocation. Document removal cancels pending work without
revoking surviving siblings. No debugger backend or frame polling is involved.

Verified on macOS:

- 1,970 browser unit tests passed, zero skipped; typecheck, lint, app build,
  touched-file formatting and diff whitespace passed.
- Real Oya native-front-door, native-contexts, native-frames and native-input
  suites passed; source, control-state, release-guard and workflow checks passed.
- New actual-engine checks cover duplicate iframe URLs, cross-process context
  selection, handles, properties, promises, group release, foreign-socket refusal,
  cross-frame argument refusal, navigation replacement and removal cancellation.
- The local full Oya application was restarted with the tested engine. The same
  child-frame checks passed over its authenticated external front door using
  disposable localhost tabs. Existing tabs/profile were not test targets. The
  production Unicode/main-world/promise/reload smoke also passed.
- The engine patch passes reverse-apply validation against the compiled source.

Evidence: `/private/tmp/oya-runtime-frames/` (unit, lint, build, front-door,
contexts, frames and input logs, plus the production smoke script).
Not a public-site benchmark or a Windows result. Worker/isolated contexts, full
Debugger, standard Network parity and Windows verification remain outstanding;
this is not a claim of complete CDP compatibility or release readiness.

## Native child-frame network control — 2026-10-09

Network observation and request-stage interception now use the exact native frame
graph, including cross-process descendants. Network and Fetch events share frame
IDs with Page/Runtime. Held requests snapshot their native frame and renderer token;
committed navigation, removal, request failure, policy loss and teardown revoke
continuations. Requests without native frame attribution are not assigned a guessed
parent. Old engines fail capability preflight before capture/listener installation.

Verified: 1,977 browser unit tests (zero skipped), typecheck, lint, build, touched-file
formatting and diff checks. Native context/network and front-door suites passed,
as did source/control-state/release-guard/workflow checks. The network fixture
forces site isolation and asserts that child renderer processes differ from the
main process before checking binary capture without refetch, urgent continuation,
removal cancellation and preservation of a separately paused sibling.

The local Oya app was restarted and the same network flow passed through its
production command gate in a disposable private context; existing signed-in tabs
were not test targets. Evidence: `/private/tmp/oya-network-children-{unit,lint,build,integration,front-door}.log`
and `/private/tmp/oya-network-children-live.cjs`.

This does not add worker attribution, response fulfillment, full standard Network
or Debugger parity. Windows remains unverified. No CDP backend or passthrough was
introduced, and this is not a release-readiness claim.

## Native private-context cookies — 2026-10-09

Implemented browser-scoped Storage.getCookies/setCookies/clearCookies plus
Oya.deleteCookie (native URL/name matching). Every operation requires an explicit
connection-owned private context; default-profile access and page/session-scoped
calls are rejected. Reads and each batch step recheck context ownership and human
control around native awaits. All write attributes are validated before the first
mutation; native mid-batch failures stop later writes but are not atomic rollback.

The compiled `native-cookies.patch` exposes actual canonical priority, source
scheme/port and partitioning presence. Tests verify a server-provided High priority
rather than accepting a hard-coded Medium. Partitioned export and unsupported
write attributes fail explicitly instead of silently discarding semantics.

Verified on macOS: 1,985 unit tests (zero skipped), typecheck, lint, build, touched
formatting and whitespace checks. Real Oya context/network/cookie, front-door,
frame and trusted-input suites passed. Source regression initially caught a helper
name collision; it was renamed and source/control-state/release-guard/workflow
checks then passed. The engine patch passes reverse-apply validation.

The local Oya app was restarted with the native-cookie engine. Production tests
used synthetic cookies in a disposable private context and verified metadata,
HttpOnly network delivery, foreign-owner refusal, targeted deletion and clearing.
Existing signed-in tabs and the normal cookie jar were not test targets.
Evidence: `/private/tmp/oya-native-cookies/` (unit, lint, build, engine, integration,
front-door, frames and input logs, plus `live.cjs`).

No CDP backend was introduced. Workers, full Debugger/Network parity and Windows
validation remain unfinished; this is not a release-readiness claim.

## Native selective request interception — 2026-10-09

Added Fetch request-stage URL wildcard and native resource-category filters.
Omitted patterns intercept all attributed requests; empty patterns intercept none.
Repeated enable atomically replaces only the authenticated owner's policy while
preserving already-held native continuations. Invalid updates preserve the prior
policy; revoked and stale observer handles cannot alter a replacement owner.
Matching has bounded input, rule count and shared state-transition fuel, with no
regex backtracking or CDP backend. Unsupported categories/stages fail explicitly.

Verified on macOS: all 1,994 browser unit tests passed, zero skipped; typecheck,
lint, build, touched-source formatting and whitespace checks passed. The source
regression check caught a helper-name collision, fixed before rerunning the source,
control-state, release-guard and workflow checks successfully. Real Oya native
front-door and private-context/network suites passed. Tests cover nonmatching
requests, real server hit counts, held callbacks across updates, empty rules,
Image versus XHR attribution and invalid-update preservation.

Restarted the production development app and repeated the selective-filter flow
through its authenticated external front door using a disposable private context
and localhost server. It passed without using the signed-in profile as a target.
Evidence: `/private/tmp/oya-native-patterns-{unit,integration,front-door,lint,build}.log`
and `/private/tmp/oya-native-patterns-live.cjs`.

Workers, full standard Debugger/Network compatibility and Windows validation
remain unfinished. This is not a full compatibility or release-readiness claim.

## Native tab history and inherited properties — 2026-10-09

Added tab-local native history snapshots and guarded back/forward traversal through
Oya extensions. Tokens belong to one connection and exact tab, newer reads revoke
old tokens, native history changes invalidate the snapshot, and traversal consumes
it after destination-policy checks. Serialized native page state never leaves the
browser. Real Oya tests cover back/forward, foreign/stale/consumed tokens and denied
destinations; a disposable production-app context also passed back navigation.

Added compiled `native-runtime-properties.patch` for bounded V8 prototype property
inspection. Real Oya tests verify nearest shadowing, symbol identity, actual isOwn,
accessor filtering without getter execution, proxy rejection without traps, depth
and key-count limits, and inherited handle release groups. Old engines reject the
new native opcode instead of silently dropping inherited descriptors.

All 2,001 browser unit tests passed with zero skipped. Typecheck, lint and build
passed. The native front-door integration suite passed both additions with internal
debugger access forbidden. History regression/control-state/release-guard/workflow
checks and production history smoke passed. Evidence: `/private/tmp/oya-history-*.log`,
`/private/tmp/oya-history-live.cjs`, `/private/tmp/oya-runtime-properties/`.

Workers, full Debugger/Network compatibility and Windows verification remain open.

## Native failure reasons and capability discovery — 2026-10-09

Added native request-stage error selection backed by actual net errors, not CDP.
The adapter rejects invalid names and unsupported old engines before consuming a
held request. All 14 supported reasons passed real-engine assertions for actual
error text, no server delivery, invalid-command preservation and one-shot completion.
Human revocation still cancels through the normal fail-closed path.

`Oya.getCapabilities` derives method names and accepted parameter names from the
actual dispatch/validation tables, sharing the enforced scope classification. It
reports partial adapter compatibility and execution-time availability checks rather
than claiming universal engine or protocol support. Tests verify safe copies,
unknown-parameter rejection and absence of unimplemented methods.

Verified 2,007 passing browser unit tests, zero skipped; typecheck, lint, build,
touched formatting, whitespace, source/control-state/release-guard/workflow checks.
Both new engine patches pass reverse-apply verification. The native context and
front-door suites passed, then the restarted production app passed selective
filters, all 14 failures, inherited properties, capability discovery and native
history in disposable private contexts. Evidence: `/private/tmp/oya-native-errors/`,
`/private/tmp/oya-capabilities-*.log`, `/private/tmp/oya-history-live.cjs`.

The public-site audit is recorded separately; these hermetic passes are not a
claim of full public-site coverage, full CDP compatibility or Windows validation.

## Input-triggered navigation readiness — 2026-10-09

Replaced the action driver's `isLoading()` heuristic with native main-document
navigation observation installed before dispatch. Existing advertisements and
subresources no longer turn Enter into a 30-second wait. New document navigation
waits for native DOM readiness; child/same-document navigation does not. Renderer
loss, load failure and deadline expiry are explicit errors, with listener cleanup.
Ten new hermetic tests cover readiness, failure, timeout and cleanup; all 2,021
browser unit tests pass. Typecheck, lint, build and the source/control-state/
release-guard/workflow checks pass. No debugger backend was added.

Production Oya Amazon search now passed with trusted keyboard input, exact
`jordans` field value, matching search URL and readable result content: 8.525s
including target creation and two analyses; Enter took 1.984s rather than the
previous observed 30.47s. This is one measured run, not a latency guarantee.
Evidence: `/private/tmp/oya-amazon-navigation-fixed.log`,
`/private/tmp/oya-ten-sites-navigation-fixed.log`, and
`/private/tmp/oya-navigation-all-unit-native.log`.

The preceding isolated Internet catalog audit passed 41/45. Remaining observations:
upstream HTTP 503 in a nested frame, the site's forgot-password server error,
the site's exhausted TinyMCE quota, and a dynamic-content test that accepted a
transient empty document. The latter readiness predicate has been corrected;
it still needs a fresh public-site run. Do not report all catalog cases passing.

### Public-site follow-up observations

The ten-site batch reported 7/10; original failures are retained in
`/private/tmp/oya-native-ten-sites-WbYeGu/report.json`, not overwritten.
Follow-up verified MDN's actual first-result behavior (Enter opens the Array
reference, not a `/search` URL), npm's native Search-button submission, and
GitHub's Issues route after its asynchronous transition. GitHub's assumed Releases
link was not discovered; that specific original flow is not claimed as passing.
Evidence: `/private/tmp/oya-sites-followup.log`, `/private/tmp/oya-npm-submit.log`,
`/private/tmp/oya-github-issues-settled.log`.

Thus a successful native interaction was observed on each of the ten sites across
runs, not ten unchanged Enter-driven flows passing in one batch. npm receives
trusted Enter keydown/keypress with key code 13 but does not submit in the observed
flow; its visible Search button does. No synthetic submit or CDP fallback was used.

The corrected dynamic-content predicate was subsequently verified through the
running production Oya front door: a native click reached a different nonempty
Dynamic Content document. The initial page itself needed a bounded readiness
wait. Evidence: `/private/tmp/oya-dynamic-refresh-ready.log`. This targeted pass
does not rewrite the earlier 41/45 full-audit result or clear upstream failures.
