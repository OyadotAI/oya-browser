# Mars: independent Rust browser and engines

Mars is being built as a complete replacement for Oya's Electron browser for people
and agents. Current implementation areas are under development and not yet at compatibility parity.
It has no route into the existing Electron product, server, persona store or published CLI.

## Independently reusable libraries

`Cargo.toml` is the Mars application manifest and the workspace root. The unpublished packages
own their implementations under `crates/`; an engine can be consumed as a path dependency
without importing the application or starting its service processes:

```text
mars (application, native shell, coordinator, MCP, secure profiles)
  ├── mars-eye   → mars-core + document dependencies + OS text primitives
  ├── mars-v9    → mars-core + serde/serde_json/ryu-js
  ├── mars-relay → mars-core + HTTP/TLS/URL dependencies
  ├── mars-mdp   → mars + CDP-compatible loopback front door (`mars-mdp` / `mars-cdp`)
  └── mars-core (shared bounded data, protocol/errors, limits)
```

Eye owns `crates/eye/src/document/` and `crates/eye/src/platform/`; V9 owns
`crates/v9/src/v9/`; Relay owns `crates/relay/src/network/`. Each exposes its public operations
at the crate root as well as its domain module. Mars keeps the existing `document`, `platform`,
`v9`, `network`, `constants` and `protocol` module paths as compatibility reexports. These
facades reuse the owning libraries and do not duplicate their engine implementations.

Core owns the canonical constants and protocol, inert script document/text-change contracts,
HTTP method enum, and pure profile failure/size contracts. It owns no secure storage, sockets,
native widgets, authority or service lifecycle. V9 consumes the inert script projection without
depending on Eye; Eye derives that projection from its validated document. Mars independently
validates V9's changes before applying and publishing them. Relay uses Core contracts without
depending on Keychain or the application. Eye's macOS text primitives measure and paint through
OS frameworks; they do not launch the desktop app. Non-macOS fallback limitations remain.

The executable is `mars`, and the feature-gated fault fixture is `mars-test-service`.
Private service identities remain `network`, `document` and `v9`; branding does not alter
wire routing, protocol versions, the MCP `serverInfo.name` (`oya-native`), ownership or
profile/Keychain identities. The existing saved store needs no migration. These path libraries
are development APIs, not a registry publication or a guarantee of full browser compatibility.

```text
AppKit native window       native agent NDJSON or MCP stdio
        \                    /
       bounded commands + local control epochs
                        |
          one worker / one Rust Runtime
          bounded tab states + shared engines and cookie profile
                        |
            supervised private pipes
               /        |        \
          network    document     V9
          process     process   process
             |           |
      OS sockets/TLS    owned layout + OS glyph metrics
```

## Ownership and transactions

Canvas 2D uses an injected `mars_core::canvas::CanvasHost`. V9 owns its serializable
context state; Eye supplies synchronous macOS text shaping/rasterization and PNG
encoding. Standalone hosts default to unavailable. Immutable pixel allocations are
shared between candidates, with admitted copy-on-write before drawing. Dirty,
projected canvases publish one final `ScriptChange::Canvas` after DOM changes.
Head-mounted canvases retain dirty pixels until body adoption. Detached DOM trees
retain wrapper state through actual heap/DOM reachability; DOM, listener and
weak-map edges are traced to a shared fixed point before collection releases
canvas quota. Listeners on unreachable node/object targets are swept rather than
rooting their own target through a captured callback.
Eye validates the tag and backing before publication; display items carry the same
pixels to desktop and agent painters. This reuses the page revision, candidate
commit and display cache invalidation rather than adding a separate asset store or
bitmap revision protocol. No graphics operation grants network or file authority.

The first slice admits 512 pixels per edge, 16 contexts and 4 MiB of backing per
page, with 8 million pixel-work units per execution. Canonical pixel allocations
and admitted native scratch share a 96 MiB process ceiling, stricter than the
initial 128 MiB V9 design ceiling. Supported drawing includes solid fill/clear
rectangles, generic-family text, width measurement, affine transforms, saved
drawing state, global alpha, ImageData read/write, canvas-to-canvas copying,
bounded decoded PNG/JPEG image drawing, polygon fill/stroke and PNG export. Decoded sources retain
fetch/redirect origin provenance, share immutable pixels and have an independent
4 MiB per-page admission bound. Cross-origin drawing taints readback/export and
copied canvases; reset restores origin cleanliness. Core plans bounded raster work before mutation, uses premultiplied
bilinear sampling and preserves immutable self-copy snapshots. Eye transforms
native text before clipping. Current paths record construction-time transforms
independently of saved drawing state; strokes apply the draw-time pen transform.
Nonzero/evenodd filling and bounded cap/join outlines use 4-by-4 pixel coverage.
Admission caps paths at 1,024 points and derived outlines at 16,384 edges, with
work charged before bitmap mutation. StrokeRect leaves the current path intact.
Quadratic and cubic Bezier curves flatten adaptively in bitmap coordinates to a
one-eighth-pixel tolerance, under the existing path-point budget and a bounded
subdivision depth. Full point batches are admitted before altering the path;
empty and closed subpaths retain their Canvas construction semantics.
Arcs and ellipses use parametric geometry with the same pixel tolerance and path
budget, bounded angular segments and subdivision depth. Direction, whole-turn
normalization and connections are admitted atomically; degenerate ellipse axes
follow the source-guided center-line behavior.
ArcTo constructs the shortest tangent circle through the existing adaptive arc
path, with current-point inverse mapping and straight-line degeneracies. Singular
transforms requiring an inverse are explicitly refused. The source-guided
negative-radius empty-subpath side effect is retained; resource failures remain
atomic. CSS color and font parsing remain limited; roundRect, clipping paths, Path2D,
dashes, other decoded-image formats, full compositing and text metrics, WebGL
and WebGPU are unfinished. Exposed GPU discovery resolves to null.
Neither this Canvas implementation nor AppKit layer backing is a Mars GPU compositor.

Eye retains bounded CSS transition metadata separately from authored opacity.
Candidate-owned opacity timelines sample duration, delay, easing and interrupted
or reversed transitions for HTML, body and ordinary nodes. Runtime seeds the
initial cascade, updates timelines on recascade, and overlays transition values
at native painting with priority over other sampled opacity effects. Background
frames use the existing page clock and expensive-work throttle; continuing CSS
transitions do not extend the JavaScript timer wake window. Other transition
properties, events, discrete transitions and CSS animation API exposure remain
unfinished. CSS opacity keyframes also retain authored styles and use the same
native clock. Resolved tracks carry easing, delays, iterations, direction, fill
and play state; recascades match names from the end of each list to preserve
playback. Paused and completed fills remain visible without scheduling frames.
Paint priority is CSS animations, V9 effects, then CSS transitions. Unsupported
animated properties retain their earlier static fallback; full transform
interpolation, animation events and animation API exposure remain unfinished.
Translation-only transform transitions interpolate separate pixel and percentage
components with the shared timing, delay, interruption and reversal machinery.
Parser provenance excludes rotation, scale, skew, matrix, 3D and independent
transform properties even when their flattened matrix happens to be identity.
Runtime overlays sampled translations in ordinary and forced candidate painting;
visual NodeBox positions follow translations while separate layout positions
keep offsetLeft/offsetTop independent of transforms.

V9 retains genuine Blob/File bytes in a context-owned object URL registry: up to
64 URLs and 4 MiB including metadata. Classic and module worker construction capture an
immutable source, so later revocation releases the mapping without canceling an
already created worker. Runtime rechecks publication, origin and worker CSP,
inherits the creator's CSP, and executes captured bytes without a network fetch.
Captured source bytes count toward the existing worker command queue allowance.
Module entry bytes require a JavaScript MIME essence and enter the existing
module graph; absolute HTTP(S) dependencies retain its CSP/CORS and credential
checks. Inherited CSP host sources match against the creator origin while URL
resolution and import.meta retain the actual Blob address. Cross-context registry
sharing, imported Blob dependencies and Blob fetch/image consumers remain unavailable.

IntersectionObserver owns bounded, serialized viewport-root registrations in V9,
using actual client boxes, root margins, thresholds and a dedicated asynchronous
record queue. Limits are 64 observers, 256 aggregate targets, 512 queued records
and 128 thresholds. Callbacks and targets participate in GC and restored-state
validation; author timer IDs cannot cancel observer tasks. Runtime compares
native geometry at layout boundaries and invalidates on resize/scroll. A dedicated
Observe action delivers callbacks and their microtasks without firing unrelated
timers, extending the author wake window or polling idle observers. Element roots,
ancestor overflow clipping, occlusion/visibility tracking and nonzero scroll
margins/delay remain explicitly unsupported or unavailable from current geometry.

Static and inserted HTTP(S) iframes share scripted-frame admission: cross-origin
frames and same-origin sandboxes allowing both scripts and their origin may use
an independent child page. Runtime supplies bounded owner IDs to V9, which selects
the existing messaging proxy before consulting local document caches. Completion
answers remain URL-keyed, so an inert sibling sharing the same URL can still read
its local document. Refused scripted owners never fall back to that sibling's
document. Runtime tracks attempted owners separately from attached children, so a
later sibling cannot retry a refused load. Removed owners are pruned, and successful
scripted loads are not fetched again merely to complete an inert answer. Existing egress, framing, sandbox,
deadline and frame-count checks remain in force.

The root owns a bounded directory of immediate child contexts and queued frame
mail. Indexed and named parent access uses document order while delivery uses
never-reused window identities. Source navigation preserves the window identity
but replaces its document generation; detachment retires the window even when
the same element returns in one script task. Both ends of queued messages carry
document-generation fences. The router supplies source origin and proxy identity,
enforces target origin, and tracks transferred-port ownership across siblings.
Hidden frames still execute. Nested scripted frame trees and general same-origin
cross-realm DOM access remain unfinished. See [the parity evidence](docs/ladybird-parity.md).

The frame layout ceiling remains 16; scripted contexts share a smaller global
budget. Page-owned leases admit seven ordinary contexts across tabs and children,
with an eighth slot reserved for replacing a committed root. Incoming parents
reserve before loading children. Cloned candidates and temporarily moved frames
share their leases; abandoned candidates release them naturally. Full-capacity
navigation can replace its parent while refusing additional children. This trades
frame/tab admission for preservation of working contexts without raising memory
ceilings. Full same-origin frame DOM access remains unfinished.

Each resident V9 call carries a bounded, unique live-lease snapshot. The service
validates it and token ownership before synchronously dropping unleased contexts;
it never evicts a leased context to admit another. A closed tab's heap is reclaimed
on the next resident call or shutdown, not immediately. Obsolete state versions
also retire synchronously instead of on unbounded detached threads. The committed
child token remains protected alongside candidate base/result versions. Removed
candidate frames lose their images and delivery routes together; the original
tree retains its rollback ownership until publication.

MessagePort routing supports one movable external endpoint per channel: the original
retained peer stays in its realm until the channel reunites or closes. Moving that
original peer, moving an already linked Worker port, forwarding an external port
after posting on it in the same JavaScript turn, and moving a port with pending
local message tasks raise `NotSupportedError` before any transfer entry detaches.
These explicit limits avoid ambiguous ownership in the current document-pair router.
A frame endpoint may forward to a third realm or return to its original peer; return
restores local entanglement and drains bounded native queues to their correct endpoints.
Closure releases native routes and both VM registrations without callback dispatch.

Classic and HTTP(S) module dedicated web workers share those leases and run cooperative, bounded
tasks in isolated worker globals. They have no document or shell capabilities.
Their synchronous imports borrow the existing V9 service pipe: one correlated
source callback at a time, governed by the coordinator's current permit, absolute
deadline, partition and worker response policy. No worker opens its own network
connection or starts an unbounded thread. Message graphs, queues, imports and
transferred-port routes are bounded and owned by the candidate page; inspection
cannot execute worker effects. Termination and page disposal release ownership,
while unpublished candidates preserve the committed worker state for rollback.
Module workers reuse V9's module graph for static/dynamic imports and import.meta,
with immutable constructor credentials and governed CSP/CORS acquisition. Entry
evaluation readiness holds queued messages through top-level await while allowing
initialization timers and fetches to progress. Saved worker state binds its
classic/module kind. Blob entry sources use the captured registry described
above. Imported Blob dependencies, nested workers and worker redirects remain
explicitly unsupported.

Native page and worker environments opt into script-visible monotonic elapsed
time. A request-local `Instant` anchor advances `performance.now()` and
`Date.now()` within synchronous tasks; serialized observed time prevents
regression across restored requests. The virtual scheduler clock, timer due
times, coordinator advances and execution budgets remain separate and unchanged.
Standalone V9 environments remain deterministic unless explicitly opted in.
This hybrid clock does not establish full real-time scheduler compatibility.

The main thread owns NSWindow, widgets, immutable BrowserSnapshot and display. It cannot create
another document runtime. One worker owns Runtime and all three engines. A bounded queue combines
human and agent requests. Only the composition root chooses executables, fixture grants, initial
ownership and the optional capture directory.

Fetch → parse → V9 load/event → document validate/apply → native display → wire budget validation
→ authority recheck and atomic publication. Document operations are stateless over bounded
values. Eye keeps only the parser state after a page's sheet list (the two most recent per
thread), continuing it when a page's scripts append a sheet. V9 keeps compiled code by a SHA-256
of grammar, page offset and source text (CODE_CACHE_BYTES, least recently used first), a page's
classic sources and module input by digest, and each page's script state (heap, globals,
platform state) by token, so a step sends a token instead of the heap (25 MB on YouTube). Per
page it keeps only the committed state, the last step's base and that step's result, for at most
RESIDENT_PAGES pages; the coordinator keeps the state's summary (listeners, detached nodes,
images) and the token. A state V9 built itself skips re-validation; one decoded from a frame is
always validated. When V9 restarts or drops a page, the next step gets `state_missing` and the
coordinator reloads the page (a POST result is never replayed; its scripts stop). In the desktop, a
navigation shows its parsed and styled page in the window before its scripts run (`window.inspect`
reports it with `settling`); the committed page, history and the navigation's reply still wait
for the scripts to settle, and a provisional page is never a candidate. Networking owns one expiring prepared connection. Transmission has its own authority
admission after connection/TLS preparation; response cookies commit under current authority
independently of subsequent document parsing. POST responses carry an explicit no-replay history
policy. In the macOS desktop, one bounded host-only cookie jar is serialized as a versioned
Keychain item. A response candidate is stored before it becomes live; a stale authority permit
causes storage rollback, and failed rollback blocks further navigation. Session-only cookies
survive app restarts until deletion, expiry where present, or an explicit clear. Headless sessions
remain in memory only.
No candidate appears in the committed page before every step succeeds. Navigation advances page
generation/history; mutations advance revision; resizing changes layout generation independently.
Failed navigation keeps the page/history position. History refetches rather than storing DOMs.
Web Storage is supplied to V9 as a bounded candidate. Local values are keyed by canonical origin
in a distinct Keychain record; session values are keyed by tab and origin. A storage write is
persisted before publication, then rolled back if the exact tab permit is stale. An uncertain
write or failed rollback sets a separate Keychain recovery marker; startup checks it before reading
site values. Recovery deletes the origin map before the marker, so late writes cannot become trusted
on restart. Cookie clearing touches only the cookie item.

Desktop tabs each own page identity, V9 state, history cursor and saved scroll. They share one
network/document/V9 process trio, viewport and secure cookie profile. Agent page requests name an
exact, never-reused `tab_id`; background work temporarily loads that tab's state and restores the
human selection afterward. An ordinary launch restores a bounded ordered HTTP(S) URL list from a
third Keychain item after cookie and Web Storage recovery succeeds. Restored pages use GET, receive
fresh tab IDs and begin under human control; query and fragment data stay hidden from agent views.
An explicit startup URL skips session restoration and saving for that run. Malformed session data
falls back to a fresh tab. Concurrent app processes use last-complete-write-wins session snapshots.
Tab ownership epochs are independent, so takeover and close revoke
the relevant pending work; app close invalidates every queued tab mutation before the final save.
Activating a saved page rerenders its layout at the current
viewport and clamps its scroll. The native strip bounds the session to the configured tab limit;
multiple windows, tab history restoration and closed-tab recovery remain future work.

## Performance and footprint targets

The long-term target is at least a 10× improvement in memory use, elapsed time for comparable
work, and runtime size for V9 relative to the pinned V8 revision. The whole native browser targets
at most one tenth the process-tree memory, elapsed time for equivalent work, and installed size
of the current Electron browser. These targets have not been demonstrated. Browser-level results
count only after both products complete the same workload with equivalent outcomes.

Comparisons must use equivalent release builds on the same hardware and OS, with the same
optimization and instrumentation settings. Count the complete process trees, including the
native coordinator and all three services. Predefine startup-to-first-render, fixed-resource
navigation-to-render, and V9 parse-and-execute workloads. Report cold and warm latency percentiles,
peak and steady-state memory under the same OS metric, and installed and compressed-download
bytes separately. V9 comparisons use only compatible scripts with matching observable results;
browser comparisons exclude user data and caches and use the same page/resource set.

Authority is an owner/epoch mutex with per-tab owner epochs. Admissions capture a Permit. Human
takeover, tab close and agent EOF immediately invalidate relevant work, including during network
I/O. Publication checks the exact tab identity and owner epoch under the same brief lock as commit,
without engine I/O. Agents also supply control_epoch and exact page/node/revision. Human edits
preserve native order and can rebase revision only within their originally observed page generation.
Deleted identities are not recycled within a page.

Native controls use the engine boxes; custom painting uses its premeasured lines. Links and
labels get their native widget once they come within a viewport of the visible region (a settled
scroll republishes the snapshot and creates the rest); form controls always have theirs. Inspection
comes from that same display. AppKit does not parse HTML, execute JS or choose page flow. Native
captures read the actual root view after display completion and convert its monitor-profile
bitmap to sRGB before PNG encoding. A capture repository writes create-new PNGs beneath a
startup-only directory grant. Color conversion uses the existing AppKit binding; no page is
rerendered for capture.

Flex layout uses content-unit base/min/max sizes and allocates border boxes once.
Render-local intrinsic caches and a shared finite work allowance cover measurement,
dry runs, allocation and final placement. Row wrapping uses hypothetical outer sizes;
the bounded freeze loop distributes growth/shrinkage before cross-axis placement.
Columns retain content-derived automatic minimums. All final geometry is validated
before Runtime can publish. CSS shorthand expansion precedes cascade selection.
Unsupported mixed direct text and table/flex structures fail explicitly.
HTML and BODY formatting records retain parsed attributes and computed styles outside the
node-ID space. Selector ancestry passes through those records before real nodes. Cascade
selection operates per expanded longhand; font size and offered color scheme resolve before
relative box values, line height and light-dark colors. Unitless line-height factors remain
inherited factors, while percentages become fixed computed lengths. Authored BODY box rules
replace the legacy reading inset once; normal horizontal auto margins are resolved after
content-box constraints. Solid root/body canvas paint and separate non-target BODY decoration
share the display's color/geometry facts with inspection. Unsupported root box geometry,
flex-item/table auto margins and other unsupported structural styles fail explicitly.

One `RenderContext` holds checked page width, page height and effective appearance. A page
retains acquired stylesheet sources; navigation preparation, resizing, appearance changes and
tab activation use the same presentation helper over the current edited document. It prepares
styles, measured display and bounded output before committing context/page/scroll together
under the captured authority. Presentation does not fetch, execute scripts, alter history or
change page/revision identity; it advances layout generation. Inactive tabs recompute with the
current context on activation. A failed candidate leaves the prior committed presentation.

Static readable text admits 16 KiB of UTF-8 per node and 512 KiB (a quarter of HTML_BYTES) in
aggregate, independently
of the unchanged 2,048-byte editable/control/metadata/document-write limits. Content validation
also checks forged documents before style, projection, mutation and rendering. Nodes are never
split to bypass a text cap. The request-local layout context charges native shaping inputs,
retains bounded exact-font measurements/wrapped runs and shares its 5,242,880-work allowance (128 per node plus 8 per stored text byte, so a page at
both the node and text limits fits)
across intrinsic sizing, dry runs and placement. Uncached native measurements are capped at
16,384 per layout, since each is a fixed native call the per-scalar work undercounts.
Inspection names are cut to 2,048 bytes per node. Line placement consumes measured widths
rather than repeatedly shaping a growing string. The private frame is bounded at 512 MiB,
with compile-time assertions for the node, source and text budgets; the VM state bound
is 64 MiB. These are admission ceilings, not typical resident usage. Classic scripts are
fetched in document order, then run blocking, then defer, then async, each in document order;
script errors still name the document position. `<meta charset>` and http-equiv content-type
are checked against the transport charset, X-UA-Compatible is inert, and every other
http-equiv value is refused.

The native error banner uses actual label measurement below the complete toolbar/tab stack.
Its action row stays separate; a tall message scrolls inside a capped text region while at
least 120 logical points remain for the page. The resulting actual scroll viewport feeds the
same render context. The shell banner has no document identity or agent target.

Test-support builds expose actual AppKit widget frames through window inspection,
without values or additional mutation authority, to compare native placement with
the exact agent-visible display list. Production builds omit this observation.

## Modules

| Module | Responsibility |
| --- | --- |
| crates/core/src/protocol/ | Strict envelopes, errors and bounded NDJSON |
| src/mcp/ | Pinned MCP 2025-11-25 lifecycle, typed correlation and fixed native tool translation |
| crates/relay/src/network/ | Relay: loopback client, desktop policy, exact-IP sockets, Rustls and owned HTTP framing |
| crates/eye/src/document/ | Eye: bounded tree, atomic text/value changes, image metadata and native-point layout |
| crates/v9/src/v9/ | V9: tokenizer, located AST, source code/scope registry, captured lexical cells, bounded value arenas and interpretation |
| src/runtime/supervisor.rs | Readiness, pipe workers, deadlines, cancellation and reaping |
| src/runtime/navigation.rs | Candidates, final redirected URL and atomic history |
| src/runtime/interaction.rs | Revision-scoped type/click/set-text and V9 events |
| src/runtime/page.rs, src/runtime/images.rs | Sole committed page, bounded image assets, snapshots and output budgets |
| src/runtime/presentation.rs, src/runtime/presentation/context.rs | Transactional retained-CSS presentation, appearance, viewport and scroll |
| src/runtime/authority.rs | Owner epochs and admission/publication validation |
| src/profile.rs | Versioned cookie record and macOS Keychain repository; no plaintext fallback |
| src/desktop/broker.rs, src/desktop/io.rs | Bounded queues and cancellable agent pipes |
| src/desktop/mcp_io.rs | Pollable MCP input, bounded output, UI-acknowledged replies and disconnect lifetime |
| src/desktop/window/ | Native intentions, ordered updates, OS-only tools, menus |
| src/desktop/canvas.rs, src/desktop/mars.rs | Native controls, painting and shell |
| crates/eye/src/platform/ | OS font measurement/drawing primitives reused by the Mars shell |

## Failure, egress and lifetime

Each engine has one bounded transport worker. The supervisor retains Child handles outside
blocking I/O, so deadline/cancellation kills the process to release both pipes, then joins the
worker. Transport/protocol/type failure or child exit retires the runtime. Ordinary unsupported
input/navigation errors leave it healthy. No automatic restart or replay occurs.

Desktop requests resolve once, reject any nonpublic DNS answer (including mixed sets), then
connect to an exact accepted address. Host/SNI remain the URL hostname. Every redirect repeats
policy, with no TLS downgrade. Only the owned demo's exact origin gets a startup loopback grant.
Headless policy remains literal-loopback HTTP. Proxy environment is not inherited.

Keychain reads/writes/deletes run on one guarded helper thread with a three-second result
deadline. A timeout retires profile access rather than blocking the browser worker. Failed clear
or rollback writes a bounded recovery marker so restart remains blocked. If Keychain rejects both
the operation and marker, the current runtime stays blocked and explicitly tells the person to
keep the browser open until a clear succeeds; restart safety cannot be guaranteed while every
secure-store write is failing.

The window owns engine and demo-site lifetime. Close signals cancellation, kills/reaps engines
and joins the worker, transport and pollable agent I/O tasks. NDJSON EOF revokes ownership while
the window remains open. MCP EOF or terminal transport/session failure also closes its launched
window through noninteractive cleanup. Other windows are unaffected. Blocked stdout cannot block the main thread; reply saturation revokes
control. Every service also has an expected-parent watchdog: reparenting exits the process even
while its main dispatch is blocked in OS DNS. Event-stream disconnection revokes ownership,
clears Loading, disables stale controls and shows explicit restart instructions. Private pipes establish local ownership, not an OS sandbox or remote authentication.

MCP uses the same Broker, authority permits, Runtime and supervised engines. Its pure
Session state machine retains bounded correlation/cancellation metadata rather than
typed argument payloads. String and safe-integer request IDs are unique for the entire
connection; internal native IDs are generated separately. Initialize/initialized
sequence gates the fixed 16-tool catalog. Native errors are preserved in tool results;
framing, lifecycle and malformed wrappers use JSON-RPC errors. There is no arbitrary
native-operation dispatch or control-grant tool.

Three owned transport threads separate input, UI-acknowledged native completion and
output. Session locks never cover I/O, queue admission or authority changes. Pending
calls and output queues each hold at most 16 entries; MCP frames cap at one MiB,
including their newline. Initialization and admitted native calls have 30-second
absolute deadlines; a dequeued output frame has two seconds to finish, regardless
of partial progress. Ready idle sessions do not expire. Cancellation suppresses a
reply and revokes only the matching tab's current agent epoch; it never revokes a
later grant or claims to undo committed work. Descriptor failure, queue overflow or
terminal correlation failure revokes publication before bounded owned cleanup.
Cancellation also binds to the actual captured admission permit, so a guessed future
epoch cannot revoke a later human grant. MCP windows disable and guard the two modal
profile-clear actions; ordinary and NDJSON windows retain confirmed profile clearing.

## Dependencies and adaptations

Locked serde crates supply serialization; url supplies syntax; Rustls/ring and webpki roots
supply verified TLS (Rustls is a vendored copy, `vendor/rustls`, with one addition that shapes the
ClientHello like Chrome's; see its `MARS.md`); png and jpeg-decoder validate static image resources; security-framework
binds the macOS Keychain; objc2 supplies AppKit bindings; libc supplies cancellable pipe polling.
No HTTP client, JS VM, document/render engine or browser framework dependency is used.
The pinned ryu-js numeric primitive supplies ECMAScript number formatting; it owns no VM state.
The scanner records whether trivia before each token, including End, contains a line
terminator. It records this after the preceding literal has finished scanning, so literal
contents cannot authorize a statement boundary. One parser helper consumes explicit
statement-tail semicolons or accepts EOF, right brace and permitted trivia line breaks
without consuming a token or allocating an empty statement. Ordinary expression parsing
finishes first; valid multiline continuations remain intact. Return/jump restrictions,
postfix updates and arrow heads share that metadata. Required for-header punctuation and
inline named-handler token/punctuation policy remain separate and unchanged. Strict
directives depend on complete parsed statements and exact spelling. The source-only bit
never enters private VM records or grants authority; all sources and unreachable bodies
still validate before any candidate publishes. Existing source/token/AST/statement/depth,
execution and retained-state limits remain unchanged. This is automatic semicolon insertion
in the supported classic-script grammar, not a complete V8 grammar/scanner port.
V9's scanner/source cursor, typed UTF-16/binary64 values, precedence AST, flat object arena and
evaluation remain Rust modules owned here. Functions pair parsed code-site IDs with defining
environment IDs; activations follow lexical parents and per-iteration let records preserve
captures. Private root var/function cells stay separate from lexical globals. Source schemas validate
every retained graph record before events; a finish-only iterative tracer reclaims unreachable
objects, functions and environments, including cycles, without moving identities. This is an owned
bounded collector, not V8's GC. Within-request allocation caps remain explicit failures.
Ordinary/arrow kind is AST/registry metadata; wire functions cannot choose it. Arrow
heads are recognized at assignment precedence using a bounded parenthesis table,
and concise bodies become budgeted returns. Both forms reuse the lexical graph,
activation and collector. Each parsed classic Program owns strictness; function code
inherits its defining mode and exact raw directive spelling can raise it. Prepared member
calls retain receivers before argument callbacks run. Arrows serialize a lexical receiver;
invocation environments retain effective receivers. Both are validated and traced across
all arenas. One always-rooted page-global object holds own data; global var/function properties
route to the existing root function cells, while lexical globals stay separate. Protected
global names fail explicitly; no native authority is exposed. Primitive wrappers and implicit
arguments objects remain unimplemented; explicit arguments cells retain lexical lookup.
Callable records distinguish source closures, bound targets and sixteen canonical owned
intrinsics. All retain bounded own properties; fixed function-prototype lookup keeps
method identity and shadowing coherent. Invocation charges the same depth/work budget
for source and intrinsic calls. Bound chains remain flat IDs with explicit cycle/depth,
argument and metadata validation. Function properties and bound receiver/argument edges
participate in validation, quotas and finish-only tracing; canonical intrinsics remain
roots. The fixed 512-function allowance includes all sixteen canonical roots; some/find
append IDs 14/15 without shifting IDs 0–13. Validation rejects missing/substituted/duplicate
canonical selectors, including unreachable hostile records. Private VM snapshots are same-build
values; no saved-profile migration or interpretation of old closure IDs as intrinsics occurs.
Source display names stay separate from private recursive expression names.
String methods operate directly on UTF-16 units; literal splitting uses an owned linear matcher
and charges bounded scan work before allocation. Array callbacks snapshot length but read
current values at visitation. Map/filter/some use presence checks; find visits absent indices
as undefined through the complete original range, even after truncation. Some/find return
immediately on a truthy callback, with find returning its pre-callback captured value, and
allocate no result array. Map/filter result arrays retain ordinary heap/property/environment
bounds. Every traversed index consumes the shared work allowance, including holes. Canonical
methods use the existing callable dispatcher and optional receiver rules, so bound/extracted
methods, nested invocation limits and atomic failure need no second path. Array own properties
shadow supplied methods; primitive string stores retain strict/sloppy behavior without creating
wrapper objects.

Array literal elements are source-only Value or Spread records with the ellipsis location;
strict validation and the source registry walk every operand, including unreachable functions.
They introduce no executable wire form, intrinsic root or native operation. Construction
allocates one fresh empty destination before the first operand, evaluates each once in order
and completely emits each spread before proceeding. This preserves the existing bounded
allocation policy; reviewed V8 leading-spread specialization allocates differently, so exact
allocation-failure ordering is deliberately not claimed.

Actual owned arrays copy a length captured after operand evaluation, materialize absent
positions as present Undefined and shallow-copy existing value identities. With no getters,
prototypes or iterator callbacks on this path, no author code can interleave with that copy;
its equivalence to default iteration is a documented inference, not a generic live-length
iterator implementation. Primitive strings use a borrowed monotonic UTF-16 cursor: valid
surrogate pairs form one two-unit element, other units (including lone surrogates) form one.
It allocates no all-elements intermediate; indexed strings and literal split remain code-unit
operations. Each emitted value charges shared work before allocation/copy and preflights
destination length, combined heap/function property capacity and environment units. Existing
set/environment validation follows each append. No cap is raised and finish-only collection
retains the same temporary-root safety. A failed expansion rejects the entire candidate.
Unsupported iteration never consults length/index fields or performs ToPrimitive; generic
iterators, custom Symbol.iterator, NodeList, literal elisions, other spread sites and rest
remain outside this path. The existing Choose an option model is genuinely spread-composed
with unchanged visible controls and states; external required scripts are not rewritten.

Abstract equality is a pure value operation over the existing closed variants, reusing strict
identity/Number equality, UTF-16 strings and numeric-string conversion. Boolean normalization is
structurally bounded. Object/function/host variants share a language object category but retain
distinct stable identities; comparisons between them need no heap inspection or conversion.
Only a primitive/object comparison requiring ToPrimitive is refused, including Boolean/object
after normalization. No conversion callbacks, heap lookups, new state tags or authority are
introduced. The evaluator retains once-only left-to-right operands, and != negates only success.
The reviewed primary-source branches and explicit terminal-false inference are recorded in
UPSTREAM.md; this is not a complete V8 equality-core or current-spec port. Equality or predicate
failure rejects the existing whole candidate and cannot partially publish Storage or DOM edits.
The separate synthetic `/compat/choose-option` case exercises these ordinary language paths
without replacing an external page's required scripts or changing native control permissions.
A shared source walker collects var declarations across supported statements but stops
at nested functions. Script instantiation initializes object-environment cells only
for the script being reached; function activation initializes shared parameter,
var and direct-function cells before statements. Lexical blocks retain their own TDZ
schemas. Source validation requires initialized Var cells, including retained
activations, and rejects duplicate physical global mirrors. Initializers execute
in order; redeclarations without an initializer do not overwrite existing values.
Expression assignments, compounds and numeric updates share one synchronous
prepared reference over a resolved cell or retained receiver/key. No executable
reference is serialized. Ordinary stores preserve their existing error order;
compounds read before their RHS, while postfix returns the converted old Number.
Conditional branches are lazy and `typeof` handles only unresolved ordinary
identifiers specially. The parser and registry still inspect every source child;
call argument nesting contributes to the same expression depth bound. Named
handler calls and legacy literal DOM text-write statements keep their existing seam.
Projected page execution receives a bounded document-owned node projection on each
load/click. Host document, element and event values are private data identities;
they confer no native authority. New service operations keep the old source-only
facade behavior separate. V9 owns listener/event records and callback collector
roots, while the real document independently replays the ordered `DomChange` log
(text, fragment insert with a fixed `first_id`, subtree move, `lang`/`dir` attribute)
and validates
the returned state before Storage persistence and publication. Candidate identity
combines the active tab and next global page generation; navigation/reload replace it.
V9 parses inserted fragments through the core `FragmentParser` trait, which Eye
implements and the app supplies, so neither engine depends on the other. Both trees
place nodes with the same core `place`/`relocate` helpers, and the coordinator fails
the navigation or click unless V9's returned structure digest equals the digest of
Eye's applied projection.
Click dispatch snapshots registrations before the inline handler and shares one
candidate/work allowance across all callbacks. Additions wait; removals suppress
pending calls. Event currentTarget is transient interpreter state, never serialized
authority. Text replacement updates the projection immediately; after collection,
retained references to removed children explicitly reject the candidate. Document
rendering, ancestry and header lookups resolve stable local IDs after removals.
Typed private state preserves numeric bits, including
nonfinite values and negative zero, plus stable object identity through page events. Script
locations refer to original UTF-8 source using UTF-16 columns. See
[UPSTREAM.md](UPSTREAM.md) for pinned reference code, observed routines, adaptations and limits.

Repository adaptations:

- Cargo tests Rust public surfaces. Unit tests mirror source domains; integration tests use
  real services. Synthetic loopback fixtures are the integration exception to no-network unit
  tests. An explicitly ignored GUI test opens only our native app.
- Rust constants hold limits. Exhaustive matches handle AST/syntax/control enums; operation
  dispatch remains explicit. Helpers target short logical steps, although rustfmt and FFI
  signatures add physical lines. Native adapter setup and atomic transactions remain coherent.
- NSAttributedString exact font metrics and primitive drawing replace the proposed direct
  CoreText calls. Measurement and painting share fixed attributes; Oya owns line breaks and
  positions. This avoids a second text stack without delegating page layout.
- The AppKit event loop polls worker updates at 16 ms. Agent descriptors are nonblocking, poll
  at 25 ms, restore their flags on drop, and never execute page code on the UI thread.

No release, existing engine replacement, schema migration or cloud deployment is performed.
The default Keychain item remains until cleared from Mars; uninstalling the package does not
remove that saved session.
