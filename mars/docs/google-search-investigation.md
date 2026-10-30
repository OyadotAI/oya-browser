# Google Search investigation - 2026-10-29

## Ladybird comparison follow-up

Final quiet packaged-app QA still fails search acceptance: homepage HTTP 200
in 4.98 seconds, followed by one query returning HTTP 429 at `/sorry/index`
through 20.62 seconds. No result was opened and no captcha interaction occurred.
The initial search script load completed in about 1.30 seconds. Worker load and
import succeeded, but its subsequent port message task hit the four-second
execution deadline. The identical captured task reproduces offline in 4007.61 ms
after a 189.04 ms bootstrap, with fresh native state/port identities mapped for
delivery. Sampling shows Promise and nested array callback execution, without a
proven specific implementation defect. This does not establish the cause of the
server's HTTP 429. See the [parity evidence report](ladybird-parity.md) for the
final build, test counts and local artifact paths.

The owned engines shut down within one second. Retaining the disabled desktop
window is documented behavior, so the earlier harness SIGTERM observations
below are not evidence of a shutdown defect.

Source comparison with local Ladybird `ccadf944d59` identified immediate-frame
directory/routing, Canvas drawing behavior and advancing script clocks as
concrete gaps. The implementation and remaining boundaries are tracked in
[Ladybird parity evidence](ladybird-parity.md). This is source comparison;
Ladybird was not run and its success on this network has not been verified.

The frame/worker follow-up build
`6c2e3a103af44e86275e38a9e82e483acfe570e7ca1fd394a2b49b6b02e9dd31`
loaded the homepage with HTTP 200. One `openai` submission still returned HTTP
429 at `/sorry/index`, observed through 20.48 seconds. Fixed-category worker
diagnostics show source loading and import succeeded, followed by an execution
limit around 4.02 seconds. The native screenshot renders the challenge checkbox
and refusal page; no challenge interaction was attempted. Artifacts are in
`/private/tmp/mars-ladybird-frame-live/`.

A separate hermetic worker task waiting for `performance.now()` to advance one
millisecond reproduced a frozen-clock defect and failed to return before its
execution limit. Native page and worker regressions now pass after repair.

The later corrected build `b5f467192c2cb64eb755b8af50037b0aae5eda7e34d9eb4204b924403f98e8cb`
received HTTP 200 at `/search`, but through 14.52 seconds showed only fallback
text. Inline scripts 4 and 5 were reported after the shared load execution
deadline expired; the fifth may never have begun. There were no frame/worker
errors in that response. Compilation ran concurrently, so quiet-machine QA is
still needed to separate execution cost from contention. No result was opened.
Artifacts: `/private/tmp/mars-ladybird-current-live/`. The earlier server refusal
remains unexplained; the response change alone does not establish causation.

## Continued work after the incomplete result

The maintainer's “still not done” makes successful search results and opening a
result the acceptance criterion. The earlier scoped QA verdict below is not
completion of that journey. Work continues across demonstrated compatibility
blockers. The continuation's single live submission is recorded below; it still
returned HTTP 429, so no repeat search was attempted.

An owned Oya Browser tab retrieved the exact public script version from the saved
error. At newline 457, column 229, the failing expression constructs `Worker`.
Mars previously exposed that interface through the generic illegal-constructor
binding without web DedicatedWorker execution. The call is unconditional;
hiding the interface would not repair it. The public worker bootstrap consists
of a synchronous `importScripts` of the same release script. Scratch sources are
`/private/tmp/mars-recaptcha-captured.js` and `/private/tmp/mars-worker-bootstrap.js`.
This identifies the exception's cause, not Google's HTTP 429 decision.

The capacity prerequisite now uses page-owned leases: seven ordinary contexts
globally and one replacement slot, with admission before acquisition and no live
state eviction. Parent and every admitted child complete two message exchanges
at saturation. New-tab saturation preserves other tabs; closing/removing contexts
makes room; failed navigation preserves the old tree and replacement headroom.
The final 14 frame tests plus four capacity integration tests pass, as do two
lease unit tests and six service tests. Workspace Clippy with all features/targets
and warnings denied passes. Evidence: `/private/tmp/mars-residency-tests.log`,
`mars-residency-unit.log` and `mars-residency-clippy.log` in the same directory.
The capacity reviewer found no blocking defects. Worker review remains pending.

Dedicated classic workers now use an isolated worker global, candidate-owned
resident state, synchronous `importScripts`, queued messages and transferred
MessagePorts. Imports suspend on the existing private V9 transport while the
coordinator enforces authority, deadlines, CSP, credentials and MIME rules.
The initial script is same-origin HTTP(S); imports may cross origins. Worker
fetches retain the top-level partition and enforce the worker's connect policy.
Module, blob and nested workers and unsupported transfer types fail explicitly.
Cross-origin imported errors are conservatively muted for that worker's lifetime;
precise per-script error provenance remains unfinished.

Offline replay of the actual captured bootstrap and release bundle now completes
without script errors and posts `recaptcha-setup` with a transferred port. This
is evidence of real script execution, not successful Google Search. The latest
public version seen in the live run (`bSuJ2xBC8cHT21-c68JXN31m`, 859330-byte bundle)
also passes the same offline replay. Evidence is
`/private/tmp/mars-worker-current-replay.txt`.
Nine V9 worker tests and twenty native worker tests now pass, including the four
review regressions: repeated CSP policy enforcement, response-policy restrictions
on dynamic code, later exception delivery with cancellation, and unsupported Blob
clone refusal before detaching ports. Clippy with all targets/features and denied
warnings passes. Follow-up review also corrected WorkerGlobalScope.onerror's
five-argument contract and cancellation on true, preserving Event listeners.
Reviewer clearance is LGTM; security, reliability and broader validation remain pending.

The later native takeover fixture also passes: changing the authority epoch
during an import aborts candidate navigation and preserves the committed page.
Four private callback transport tests and all 161 Relay unit tests pass. Seven surrounding native suites pass
(55 tests): frames and residency, CORS, service restart, script ordering and
isolation, and Canvas runtime behavior.

The wider V9 run exposed `frames::other_frames_have_no_document`: it expects a
null contentWindow where the existing implementation returns a cross-origin
window proxy. A standalone replay linked to the earlier baseline library
`libmars_v9-d5be06e9ec2d1e7d.rlib` returned the identical `,,[object Object]` result,
and HEAD already contains that proxy path. This is a confirmed baseline test
failure; it has not been hidden by changing the expectation. Scratch evidence is
`/private/tmp/mars-frame-baseline.rs` and `/private/tmp/mars-frame-baseline`.

The complete default-feature native release suite ran across 73 targets:
1065 passed, five failed, nine ignored. Four unit failures reproduce against the
earlier V9/Eye libraries: three legacy syntax-message expectations and a property
quota fixture that reaches the resident-string budget first. Scratch comparison
is `/private/tmp/mars-worker-baseline-unit-results.log`.
The fifth changes a serialized listener callback to object ID 999. A follow-up
using out-of-range u32::MAX also remains accepted. The earlier hypothesis that
999 had simply become a valid intrinsic was insufficient; this is an unresolved
validation defect under security investigation. The fixture retains the
out-of-range ID and all original assertions. Evidence is
`/private/tmp/mars-worker-invalid-callback-test.log`.

Source-loading behavior was checked through Oya against the primary
[classic imported-script algorithm](https://html.spec.whatwg.org/multipage/webappapis.html#fetch-a-classic-worker-imported-script)
and [Fetch request credentials](https://fetch.spec.whatwg.org/#concept-request-credentials-mode).
The callback executes a global Script synchronously and does not drain
microtasks between nested imports. Source text is UTF-8 decoded independently
of response charset, and cross-origin imports omit cookies.

The continuation's tested build was
`08d1a4ace2a5e6c8e92e0c52ce8ab6aad93314468e48063eb99c1b41dfe3a286`.
Its homepage loaded with HTTP 200 in 6.37 seconds; one `openai` submission returned
HTTP 429 at `/sorry/index` at 5.41, 13.09 and 18.12 seconds. The frame still exposed
14 nodes. `Illegal constructor` was gone; the frame console instead reported
`The worker script could not be loaded.` The parent still lacked
`solveSimpleChallenge`. Artifacts are in `/private/tmp/mars-google-workers-retest/`.
The owned desktop acknowledged shutdown but required SIGTERM after eight seconds.

The later review fixes are not part of that live build. A primary-source check
also identified a worker CSP fallback defect: script-src `'strict-dynamic'`
permits these non-parser script-like requests. Mars now follows the
[CSP script pre-request algorithm](https://www.w3.org/TR/CSP3/#script-pre-request),
while explicit worker-src/child-src restrictions still take precedence. A fixture
adapted from the pinned WPT strict-dynamic worker case covers this behavior.
This could explain the loading refusal, but the last run did not capture enough
policy detail to establish causation. A separate Oya capture of the current
public bootstrap returned HTTP 200 and JavaScript MIME; that does not prove its
response to Mars. Google Search acceptance remains incomplete.

This work follows the native Google Search failure on `blink-parity`, based on
`0968fdfc` plus the uncommitted graphics slice. It does not establish browser
fingerprint equivalence or a release verdict.

## Observed failure and demonstrated defect

The first native run loaded Google's homepage, typed `openai`, and submitted it.
Google returned HTTP 429 at `/sorry/index`; observations through 12.17 seconds
showed the same refusal and a blank challenge frame. The console included
`ReferenceError: solveSimpleChallenge is not defined`. That exception does not
establish the cause of Google's refusal, and no missing definition was proven.
The implementation leaves it unchanged.

A local fixture demonstrated a separate compatibility defect: a same-origin
sandbox frame with `allow-scripts allow-same-origin` executed and exchanged
messages when present in markup, but the identical dynamically inserted frame
did not. Dynamic loading answered the inert-document request before considering
the existing scripted-frame route. This is relevant to the dynamically inserted
frame in the earlier challenge capture, without proving it caused the HTTP 429.

## Repair and boundaries

Runtime now selects the existing scripted route first, using the same admission
predicate as static frames. A bounded, validated set of owner IDs tells V9 which
frames use messaging proxies. URL-keyed answers remain available to ordinary
inert siblings sharing the same URL. Refused scripted owners do not gain access
to those siblings' documents. Removed owners are pruned; empty and whitespace
sources retain their blank documents.

Runtime also retains attempted owner IDs separately from successful attachments,
so inserting a sibling cannot retry an earlier refused load. The set is local to
the page, pruned with route metadata, and recreated on navigation; it adds no V9
permission or serialized environment field. A counted fixture reproduced three
acquisitions for two refused owners before this correction.

Changed files: `src/runtime/{frame_pages,frames,navigation,scripting}.rs`,
`src/runtime/devtools/evaluate.rs`, V9's `v9/mod.rs` and
`v9/platform/{mod,frames}.rs`, `tests/frame_pages.rs`,
`crates/v9/tests/frame_routes.rs`, architecture and changelog notes.

The PM scoped a proven general compatibility repair and one bounded live retest.
Designer was skipped because no owned UI, copy or API shape changed. Principal
required per-owner routing and reuse of existing sandbox, authority, framing,
deadline and admission checks. No Google-specific stub, challenge bypass,
identity change or new graphics feature is included.
No stage's scope decision was overruled.

## Original capacity limitation (subsequently repaired above)

The existing scripted-frame admission ceiling is 16, while the V9 service holds
eight resident page states. A 17-frame fixture admitted 16 fetches but lost the
parent's script state (`state_missing`). The admission test checks fetch count
only; it does not claim that 16 contexts operate together. This separate capacity
mismatch was repaired by the lease admission change above. Full same-origin frame DOM access and general
iframe compatibility remain unfinished.

## Evidence

Local artifacts are transient and are not committed:

- `/private/tmp/mars-google-current/report.json`: initial Google failure.
- `/private/tmp/mars-google-frame-before.log`: static control passed; dynamic
  execution failed before the routing repair.
- `/private/tmp/mars-google-regressions.log`: 71 selected release tests passed,
  covering frames, script ordering/isolation, Canvas/languages and GC.
- `/private/tmp/mars-google-frame-cap.log`: capacity diagnostic above.
- `/private/tmp/mars-google-refusal-before.log`: two refused owners caused three
  acquisitions before the attempt-tracking correction.
- `/private/tmp/mars-google-frame-final.log`: all 14 final frame tests passed,
  including blank sources and refused-owner idempotency.
- `/private/tmp/mars-google-clippy-final.log`: final workspace Clippy passed with
  all targets/features and warnings denied. Formatting and diff checks passed.

Reviewer cleared the blank-source correction; security reported zero supported
findings from source and artifact review. Reliability caught the duplicate retry
above and cleared its correction with zero remaining findings.

QA's verdict is **ship with known issues for the bounded frame-routing repair**,
and **do not ship as working Google Search**. On macOS 26.6.2 arm64, QA independently
ran all 14 native frame tests: 14 passed in 1.17 seconds. Coverage includes dynamic
and static routing, ready/echo messaging, owner isolation, sandbox/framing
refusals, child failures, blank sources, repeat navigation, removal before
acquisition and exactly two fetches for two refused owners. QA visually confirmed
the saved challenge screenshot without making additional Google requests.

QA evidence is `/private/tmp/mars-google-qa-verification.md` and
`/private/tmp/mars-google-frame-qa.log`. Measured deadline exhaustion, removal
after attachment, cancellation, disconnect and service restart remain unverified.
This is a focused Rust gate, not a full repository gate rerun. Previously recorded
repository integration failures and baseline limitations remain documented in
[the graphics report](graphics-fingerprint-baseline.md).

## Single live retest

The release binary SHA-256 was
`8224598ebae39c3cb556f6f9a7da857c961e9ce4227546c20b6d4513f223fd18`.
This run preceded the later refused-owner attempt-tracking correction; no live
retest of that correction was performed, preserving the single-submission scope.
The isolated native window loaded Google's homepage with HTTP 200 in 4.94 seconds,
typed `openai`, and submitted once. Google returned HTTP 429 at `/sorry/index`
at 3.29, 7.29 and 12.31 seconds. No second submission or challenge interaction
was attempted.

Unlike the earlier blank frame, the final screenshot now shows the reCAPTCHA
checkbox and logo, and the native inspection exposes 14 frame nodes. The child
script now reports `TypeError: Illegal constructor` in script 3 (inline), line
457, column 229. The parent still reports the missing `solveSimpleChallenge`.
The constructor responsible has not been isolated; these observations do not
identify the cause of Google's HTTP 429 or establish working challenge behavior.

Artifacts: `/private/tmp/mars-google-retest/report.json`, `search.png` and
`perf.log`. The JSON removes IP-address lines and URL query parameters; the raw
local screenshot contains the page as displayed and is not committed.
`runtime.shutdown` acknowledged without error, but the owned desktop process
remained for the harness's eight-second wait and was then terminated with SIGTERM
(exit -15). No fixture processes remained after QA. A separate Mars.app instance
that had been running for over 18 hours was preserved.
