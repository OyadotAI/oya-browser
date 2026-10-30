# Ladybird comparison and Mars compatibility work

This comparison uses the local Ladybird checkout at commit `ccadf944d59` on
2026-10-29. Source inspection establishes implementation differences, not a
successful Google Search run in Ladybird. Ladybird was not launched. Only Mars's
native runtime was used for these tests.

## Everyday browsing candidate, 2026-10-30

The current `ladybird-parity` working tree repairs the ordinary default-search
journey without a site-specific response replacement or a lite-page fallback.
This is a usability milestone, not a full Ladybird-parity result or a Google fix.

The frozen native test build `3e60dbacc49b1cef1b3cae65c37a3aa347188605659442869322dd27e93e87ec`
completed a plain DuckDuckGo query in 7.42 seconds, a real native mouse click to
rust-lang.org, native Back, and a second query in 5.00 seconds. Result links were
visible and enabled; the Rust destination reported no script degradation.
Evidence: `/private/tmp/mars-usability-journey-final/report.json` and its native
window screenshots. These observations cover those journeys, not every result
type or website.

The same build passed the isolated native navigation fixture: latest-address
replacement in 98 ms; five Stop acknowledgements in 37–39 ms, settling in
91–94 ms; failure Details/Dismiss; native link activation and Back; and a
background agent completion overlapping a human Reload followed by another
human address. The last case reproduced a permanently pending address on the
previous frozen build before its completion bookkeeping was repaired.
Evidence: `/private/tmp/mars-navigation-overlap-{before,after}.log`.

Compatibility repairs include JavaScript MIME aliases, regular expressions after
control headers, current SELECT/OPTION state and adoption, bounded root-style
coalescing, SVG name adoption, and classic-script resource completion. Ignored
vendor keyframe blocks no longer mark unrelated links noninteractive. Script
completion handlers enforce a separate bounded CSP attribute policy, including
fallback, policy intersection and explicitly permitted source hashes; allowing
an attribute does not enable eval, Function or string timers.

Focused checks pass: 46 native desktop tests, 39 native script/frame tests,
10 V9 policy tests and eight host CSP tests. The desktop suite includes stopping
at first paint before V9 admission, preserving every service PID, the committed
page identity and a mutated JavaScript value. Correctness, security and
reliability reviews closed their supported findings. Strict all-targets,
all-features release Clippy passes. The maintainer stopped the final broad
workspace run during compilation and explicitly skipped packaged QA and the
normal stage pipeline to prioritize parallel implementation. The interrupted
run is not a passing gate.

The subsequent code-first Canvas slice adds bounded decoded PNG/JPEG sources
for all drawImage overloads, source cropping/scaling, JPEG orientation and
origin-clean propagation. Loaded images outside the supported formats/512-pixel
source edge remain explicitly unsupported. Cross-origin sources are
conservatively tainted, including redirects away and back; CORS-authorized
clean cross-origin images remain unimplemented. Six V9 regressions and two
native fixtures were added but have not been run in this implementation batch.

The opacity-transition slice implements duration/delay/easing lists, negative
delay, interruption, reversal shortening and target removal. Runtime connects
the Eye timeline to recascade, native paint and background frame scheduling;
sampled values never rewrite authored opacity. The combined production build
passes. Eleven Eye regressions and a runtime paint-bridge regression were added
but not run in the code-first batch. Other properties, transition events,
discrete transitions and CSS animation API exposure remain unfinished.

HTTP(S) dedicated module workers now use the existing module engine for static
and dynamic imports and import.meta.url. Constructor credentials and CSP/CORS
admission persist across the graph. Queued messages wait for entry evaluation,
including top-level await, and failed initialization terminates the worker.
The combined production build passes; new native/V9 module-worker regression
targets have not been run in this code-first batch. Blob URLs, nested workers
and redirects remain unsupported in this slice.

Classic Blob workers now use actual immutable Blob/File bytes captured by the
constructor. Object URLs are context-owned and revocable; existing captured
workers survive revocation, while future loads of revoked URLs fail. Runtime
enforces creator origin and inherited CSP without network acquisition. Registry
storage is bounded to 64 URLs and 4 MiB, and captured commands retain the existing
worker queue limit. Seven V9 and five native regression cases were added but not
run in the code-first batch. Blob module workers, cross-context URL sharing and
Blob fetch/image consumers remain unfinished.

Canvas paths now rasterize real polygon fills and strokes with nonzero/evenodd
winding, line width/caps/joins/miter and strokeRect. Construction-time path
transforms and draw-time pen transforms are distinct; save/restore leaves the
current path intact and resize clears it. Storage, geometry and raster work are
bounded before mutation. Seven Core and nine V9 regressions were added but not
run in this code-first batch. Curves, arcs, clipping, Path2D and dashes remain
unfinished.

CSS opacity keyframes now advance on the native page clock, with easing,
positive/negative delay, fractional/infinite iterations, playback direction,
fill and pause/resume. Recascade preserves matched animation times. Paused and
completed fills do not keep background work pending; sampled values stay separate
from authored opacity. Ten Eye regressions and a runtime paint/scheduling
regression were added but not run. Other animated properties retain their
existing static fallback; animation events and full API exposure remain gaps.

The combined production binary for classic Blob workers, Canvas paths and CSS
opacity keyframes builds successfully. Its SHA-256 is
`096ffde044f7832ad11fc51c36079fe0e441ec9cb17a4c3e063c1c5c1304b96b`;
build evidence is `/private/tmp/mars-parity-path-blob-keyframes-build-resumed.log`.
The first build exhausted disk space; clearing only the regenerable debug
incremental cache allowed it to complete. No QA or per-change tests were run.

The next Blob slice admits module entry scripts from captured bytes. Module MIME
must be JavaScript; the existing graph handles absolute HTTP(S) imports, dynamic
imports, top-level await and early-message gating. Inherited CSP matching now
uses the Blob creator's origin for scheme-less host sources while retaining the
actual Blob URL for resolution and import.meta. Nine focused regressions were
added but not run; the combined production build for this next batch is pending.
Imported Blob dependencies and cross-context registry sharing remain unfinished.

Canvas quadraticCurveTo and bezierCurveTo now produce actual fill/stroke geometry
through adaptive de Casteljau subdivision in bitmap coordinates. Flattening uses
a one-eighth-pixel tolerance, bounded depth and the existing path-point allowance;
the whole generated batch is admitted before path mutation. Six Core and seven
V9 regressions were added but not run. The combined production build for this
next batch is pending; arcs and ellipses remain unfinished.

CSS transform transitions now sample two-dimensional translations in pixel and
percentage components, including none-to-translate, delays, easing, interruption
and reversal. Native painting and visual geometry use the sample while CSSOM
layout offsets retain untransformed positions. Parser provenance prevents
rotation/scale/skew/matrix/3D values from masquerading as supported translations.
Ten Eye, two runtime and one V9 layout-position regressions were added but not
run. The combined production build for this batch is pending. General transform
interpolation and other animated properties remain unfinished.

Canvas arc and ellipse now append real transformed curve geometry with directed
sweep/full-turn normalization and current-path connections. Adaptive subdivision
retains the existing path quota and one-eighth-pixel approximation; admission is
atomic. Seven Core and eight V9 regressions were added but not run. Degenerate
ellipse axes follow local Ladybird's center-line behavior. The combined build
for this batch remains pending; arcTo, roundRect and clipping are unfinished.

Canvas arcTo now constructs actual tangent-circle geometry with straight-line
degeneracies, construction transforms and the existing bounded arc approximation.
It preserves local Ladybird's negative-radius empty-subpath side effect while
keeping resource failures atomic. Singular transforms requiring inverse mapping
remain explicit refusals. Four Core and five V9 regressions were added but not
run; the combined production build for this batch is pending.

IntersectionObserver now reports measured viewport intersections instead of
fabricated visibility. Persistent registrations retain px/% root margins,
threshold changes and asynchronous records; unobserve/disconnect stop future
sampling while preserving already queued records. Native layout, resize and
scroll invalidations schedule dedicated observer-only turns, including after
ordinary timers go idle. Seven V9, two runtime state and two native integration
regressions were added but not run. State and queues are bounded and validated.
Element roots, ancestor clipping, visibility tracking and nonzero scroll
margins/delay remain unfinished. The combined production build is pending.

Canvas image sources now include full-canvas GIF first frames and static WebP
on macOS through actual ImageIO/CoreGraphics pixels. Container, decoder and
declared dimensions are checked before output allocation; input and scratch
memory are bounded. Output uses straight-alpha sRGB and preserves the existing
immutable source and taint path. Twelve regression functions were added (eleven
applicable on macOS) but not run. Animated WebP, partial-first-frame GIF and
non-macOS native decoding remain unsupported. The combined build is pending.

Canvas roundRect now builds actual corner arcs, supports numeric/point/iterable
radii, normalizes one to four corners, scales overlaps and handles negative
dimensions. Conversion order and iterator cleanup are preserved under bounded
input and atomic path admission. Seven Core and nine V9 regressions were added
but not run; this batch's combined production build is in progress.

Current substantial gaps remain in broader CSS transition/animation properties,
Canvas clipping/compositing and broader image sources, nested workers,
nested scripted frames and broader DOM/JavaScript compatibility. The local
Test262 and WPT harnesses can measure slices, but no matched current Mars and
Ladybird run establishes a percentage of parity. Google Search's recorded
challenge/worker failure remains unresolved below.

## What Ladybird supplies

Ladybird's `Libraries/LibWeb/HTML/WindowProxy.cpp` resolves active child windows
and named properties through browsing contexts. Its `HTML/Worker.cpp` and
`DedicatedWorkerGlobalScope.cpp` connect real worker execution through message
ports. `HTML/Canvas/` defines drawing state, affine transforms and the overloads
for image drawing and pixel access. `HighResolutionTime/Performance.cpp` reads
the current high-resolution clock.

Mars had narrower implementations in these paths: parent/child-only frame
routing, incomplete drawing operations, and script clocks fixed at a scheduler
timestamp during execution. Merely exposing interface names cannot provide these
behaviors. The implementation work below closes specific differences without
claiming equivalence across either browser.

## Scope and evidence

PM scoped the first milestone to immediate frame identity/routing and worker
diagnostics, followed by Canvas state/transforms and ImageData/canvas copying.
Designer was skipped because no product UI or copy was changed. Principal kept
coordinator-owned routing and origin checks, shared bounded raster operations,
and native text rendering. Module/blob/nested workers, nested scripted frames,
general cross-realm DOM, full Canvas paths/compositing, decoded image provenance,
WebGL/WebGPU and broad web-platform conformance remain outside these milestones.

| Area | Implemented and tested | Remaining limits |
| --- | --- | --- |
| Frame directory | Indexed/named lookup, self identity, duplicate-name precedence, static/dynamic hidden frames | Immediate children; broader WindowProxy reflection remains incomplete |
| Frame lifecycle | Stable context on navigation, retirement on detach/reinsert, queued-document fencing | Full same-origin cross-realm DOM and nested trees |
| Frame messages | Direct sibling replies, target-origin refusal, bidirectional ports and forwarded endpoints | Full structured clone; frame/worker cross-domain moves explicitly unsupported |
| Workers | Classic worker source/import execution, bounded capacity, cleanup and fixed-category diagnostics | Module/blob/nested workers, already-linked worker-port retransfers; full worker platform |
| Canvas state | Save/restore/reset, affine transforms, global alpha, smoothing, transformed native text | General DOMMatrix and advanced text metrics |
| Canvas pixels | ImageData constructors/create/put, dirty rectangles, canvas drawImage, self-copy, nearest/bilinear samples | Decoded image sources, paths/clip, gradients/patterns, other composite modes; unrepresentable geometry gives catchable NotSupportedError |
| Script clock | Native page and worker one-millisecond loops complete; saved observations cannot rewind time; deterministic default preserved | Hybrid virtual scheduler; full real-time scheduling remains separate |
| Google Search | Homepage loads; captured real worker bundle completes offline setup | Final quiet query returned HTTP 429; subsequent worker message task times out, also reproduced offline; opening a result not achieved |

Focused evidence lives under `/private/tmp/`: `mars-ladybird-siblings-lifecycle.log`
(8 native frame tests), `mars-worker-runtime-validation.log` (23 native worker
tests plus 2 diagnostics tests), `mars-canvas-images-tests.log` (52 V9 Canvas
tests), `mars-canvas-core-tests.log` (13 raster and 5 existing Core tests), and
`mars-canvas-native-transform-tests.log` (2 Eye text tests). These counts overlap
other suites and must not be added as a unique-test total.

The native worker clock regression fails before repair: a task waiting for
`performance.now()` to advance by one millisecond never sends its result before
the execution limit. Evidence: `mars-ladybird-worker-clock-before.log`.
After repair, both native page and worker regressions pass. Nine focused V9 clock
tests cover timer ordering, restored high-water time, resume, rollback anchors,
input validation and old-state defaults. Evidence: `mars-live-clock-v9.log` and
`mars-live-clock-native.log`. The latter also exposed a port-route validation
failure under concurrent repair; that initial log alone is not a complete
worker-suite pass. The final combined gate below covers the repaired worker suite.

The wider all-features gate also exposed legacy helper handlers retaining code
constants without retaining their source for the next restored VM. A follow-up
uses the existing bounded dynamic-source ledger for immutable handler sites.
Five restoration/validation/location regressions and 37 conformance identity
tests pass in `mars-legacy-handler-v9.log`; the affected restoration failures
also pass in the broad native unit rerun.
This is an implementation expansion prompted by reproducible gate evidence,
not a relaxation of graph validation or an increase in execution budgets.

## Live Google observation

The frame/worker build `6c2e3a103af44e86275e38a9e82e483acfe570e7ca1fd394a2b49b6b02e9dd31`
loaded the homepage with HTTP 200. A single `openai` submission returned HTTP 429
at `/sorry/index`, still present at 7.61, 15.47 and 20.48 seconds. Worker source
loading and imports completed, but worker execution stopped around 4.02 seconds.
This local execution defect does not prove the cause of Google's server refusal.
No captcha was solved or repeated submissions attempted. Artifacts are in
`/private/tmp/mars-ladybird-frame-live/`.

The corrected native build
`b5f467192c2cb64eb755b8af50037b0aae5eda7e34d9eb4204b924403f98e8cb`
loaded the homepage in 6.37 seconds and received HTTP 200 at `/search`. At 5.49,
9.50 and 14.52 seconds the visible page contained only Google's fallback text.
Inline scripts 4 and 5 were reported as not run after the shared load execution
deadline expired; this does not establish two independent slow scripts. There
were no frame or worker diagnostic errors in this response. No result could be opened. This run
overlapped heavy local compilation, so a quiet-machine QA run must distinguish
execution cost from build contention. The server-response change is an
observation, not proof that a particular patch fixed its earlier refusal.
Artifacts: `/private/tmp/mars-ladybird-current-live/`. The harness terminated
the retained desktop window after shutdown. This is not evidence of a shutdown
defect: the documented desktop contract keeps the disabled last window open
after its engines stop.

Final quiet QA used the packaged default-feature binary
`380faedf3c5e772176b604394473d7f22f52e8753cb01b73e7c7647176269461`.
The homepage returned HTTP 200 in 4.98 seconds. One `openai` query returned HTTP
429 at `/sorry/index`, still present at 7.76, 15.60 and 20.62 seconds. Native
screenshots show the challenge checkbox and no search results. No captcha was
interacted with and no repeat submission was made. Initial search script loading
completed in about 1.30 seconds: the earlier inline-script timeout did not recur.
Worker loading (27.10 ms) and import (1.96 ms) succeeded; the subsequent port
message task hit its deadline at 4029.56 ms. Both worker requests enabled the
live clock. Evidence: `/private/tmp/mars-ladybird-qa-live/`.

The same packaged service reproduces that worker failure offline. Setup completes
in 189.04 ms; replaying the captured message takes 4007.61 ms and returns the
script time-limit error. The imported source is the exact captured URL/source
(859,260 UTF-8 bytes); the replay maps the fresh resident-state token and port
identity before delivery. Native sampling shows execution in Promise jobs and
nested array callbacks. It does not identify a specific semantic defect or prove
that correcting execution would remove Google's server refusal. Evidence:
`/private/tmp/mars-ladybird-worker-final-replay.log` and
`/private/tmp/mars-ladybird-worker-final-replay/worker-sample.txt`.
Inspection of the sampled Promise executor found a separate concrete error-path
defect: it silently swallowed fatal engine failures while handling ordinary
JavaScript throws. Two deterministic host-failure regressions fail before the
repair; explicit fatal propagation preserves engine failure while ordinary
throws still follow promise settlement. This improves error correctness, not
the measured workload's execution speed. Captures contain session data and
screenshots include network identifiers; raw artifacts remain local. No
execution deadline was increased.

After that repair, the final packaged binary
`bd7520c21fd8b9b06c38b7be5c30e3e0a3499aa5bcefeb0618e78e5a11ec5a15`
still times out on the offline task (4020.77 ms). It now preserves the execution
location, line 563/column 517, rather than replacing it with an unlocated timeout.
The exact workload remains unresolved. Evidence:
`/private/tmp/mars-ladybird-worker-final-replay-v2.log`. No additional Google
submission was made for this error-propagation-only change; the preceding live
observation belongs to the earlier hash stated above.

QA verified the documented shutdown contract: all three owned engine children
were gone after one second; the disabled desktop window remained until the
harness terminated its own process. Existing user sessions were preserved.

## Validation limits

Reviewer found and verified corrections for numeric drawing overflow, immediate
port messages, local endpoint reunion, and retirement on rejection/termination.
Scoped source clearance is recorded. Security review found no supported boundary
regression (read-only source review, not fuzzing). Reliability found that sender
closure discarded already-posted port replies. The repaired worker and frame
paths preserve accepted outgoing messages and discard work addressed to closed
receivers; reliability's follow-up has zero remaining findings. First packaged
QA passes 11/11 scoped smoke assertions but says **do not ship as Ladybird
parity or a Google Search fix**. It covers Canvas/PNG, transforms/alpha,
ImageData/copy, live clocks, frame identity/detach, catchable overflow, stale
authority refusal, navigation recovery and shutdown. Port lifecycle fixtures,
full conformance, crash recovery and persistent profiles were not independently
retested by QA. Evidence: `/private/tmp/mars-ladybird-qa-verification.md`.
Final packaged QA repeats those checks and adds committed Promise rejection
identity and settlement: **13/13 assertions pass**, including signature/hash
verification and owned-process cleanup. Its initial supplemental Promise probe
used ephemeral evaluation incorrectly; the corrected committing-API probe passes,
and the original evidence is retained. The verdict remains **do not ship as
Ladybird parity or a Google Search fix**. Evidence:
`/private/tmp/mars-ladybird-qa-verification-v2.md`. No further Google requests
were made in this second and final QA pass.
The final combined focused gate passes 521 tests with one ignored in
`mars-ladybird-focused-final.log`. It includes 69 native frame/worker tests,
nine close-order regressions and the 80-transfer cleanup regression. The separate
detached-node identity regression also passes in `mars-detached-node-final.log`.

Final formatting and strict workspace Clippy with all targets/features and
warnings denied pass in `mars-ladybird-format-final.log` and
`mars-ladybird-clippy-final.log`, after the closure-order repairs.
The final Promise follow-up passes three deterministic regressions and 19
adjacent worker/clock/resolver tests in `mars-promise-fatal-final.log`; its narrow
review found no supported defects. Formatting and strict Clippy were rerun after
that repair as well. No stage overruled the unmet Google or full-parity criteria.
The stable broad run in `mars-ladybird-stable-workspace.log` reports 5,765 passed,
27 failed and 30 ignored across 597 test/doc-test reports. Canvas images (22),
Canvas state (15), native Canvas publication (5), frame directory (14), native
page clocks (1), V9 clocks (9) and saved handlers (5) all passed. The late pending
transfer removal test linked a library compiled before its fix; the final focused
rebuild passes. An older test that expected removed elements to become invalid
was corrected to assert detached-node identity and local attribute mutation,
and its focused rerun passes.
The other failures include syntax-message expectations, resource-limit fixtures,
embedded-content expectations, platform presence and conformance/serialization
cases. Their presence means the whole workspace is not a green release gate.

An earlier broad run is not a valid native gate because
concurrent builds replaced its service executable with a different feature
configuration; those schema failures must not be described as product failures.
The wider
JavaScript repository typecheck passed and all 3,632 server unit tests passed;
13 integration failures include missing patched-engine configuration and CDP
connection timeouts. Those environment-dependent checks are not a clean release
gate. No full Ladybird or web-platform parity percentage has been measured.

## Local deliverable and remaining acceptance

The local app is `mars/dist/Mars.app`, with `mars/dist/Mars-0.1.0.dmg` alongside
it. The packaging script completed, and the app passed strict code-signature
verification. Packaging does not publish or upload a release. The maintainer
subsequently requested committing this work and merging it to main with the
documented Google Search and broader conformance failures still open.

This work changes native routing in `src/runtime/frame_tree.rs`,
`frame_router.rs`, `frame_pages.rs`, and the worker modules; V9 frame/channel
bindings, clock observations, detached-node preservation and legacy-handler
restoration; Core Canvas raster operations; Eye native text rendering; V9 Canvas
state/image bindings; and their regression suites. The architecture and this
evidence report describe the supported subset.

Full parity remains an unmet objective. The next demonstrated execution blocker
is the captured worker message task. A justified repair needs a reduced
interpreter/API regression, followed by the unchanged offline task and native
QA. Search acceptance still requires actual results, opening a result, and a
second successful query. Broader parity also requires the missing capabilities
in the matrix and a clean conformance gate; neither interface counts nor HTTP
200 alone establish it.
