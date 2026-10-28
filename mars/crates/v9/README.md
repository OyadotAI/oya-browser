<!-- V9 is an independently selectable, source-owned Rust engine package. -->
# V9

V9 is Mars's own JavaScript engine: a bytecode VM over a bounded, serializable heap, the ECMAScript library, and a browser platform over the app's inert document projection. Use it without launching the Mars desktop app, Eye, Relay or sibling service processes. Nothing in V9 has native authority: the document is a projection the app supplies, network calls and navigations come back as requests for the app, and time is a virtual clock the app advances.

```toml
[dependencies]
mars-v9 = { path = "/path/to/agentchrome/mars/crates/v9" }
```

```rust
use mars_v9::{load, JsValue};

let execution = load("const count = [...'A😀B'].length", &[])?;
assert_eq!(execution.state.globals()["count"].value, Some(JsValue::Number(3.0)));
# Ok::<(), mars_v9::protocol::EngineError>(())
```

## The engine

Scripts are scanned, parsed into a syntax tree for the full classic-script grammar, and compiled to stack bytecode on every request. Code is identified by its page byte offset, so committed state never carries executable code: closures name a code site, and the state is validated against freshly compiled sources. Frames are explicit data, so generators and async functions suspend into the heap and resume in a later request (an `await fetch(...)` continues when the app delivers the response).

Language support includes classic and module scripts: classes with fields, private members, accessors and static blocks; destructuring, spread and rest; generators and `yield*`; async functions, async generators, `await` and `for await`; promises with a microtask queue; labels, `switch`, `try`/`catch`/`finally`; optional chaining, nullish coalescing and logical assignment; `using` and `await using` with reverse cleanup and suppressed errors; tagged templates. The library includes Object, Function, Array, String, Number, Boolean, Symbol, BigInt, Math, JSON, Date, RegExp, Map, Set, WeakMap, WeakSet, WeakRef, FinalizationRegistry, Promise, Proxy, Reflect, ArrayBuffer, the twelve typed array kinds, including Float16Array, DataView, structuredClone, TextEncoder/TextDecoder and the Error types. BigInts use immutable signed limbs beyond the inline 128-bit representation; arithmetic, division, shifts, bitwise operations and radix conversion retain the existing work and storage limits. Weak references use precise weak-edge tracing and per-checkpoint kept targets. Finalization registries retain holdings and schedule cleanup tasks after collection; unregister tokens stay weak. ArrayBuffer bytes count against `JS_ENVIRONMENT_UNITS` (two bytes per unit) and one buffer holds at most twice `JS_STRING_UNITS` bytes. `eval` and `new Function` compile with V9's own parser (live source units are kept in state, bounded by `VM_DYNAMIC_SOURCES`; collection reclaims unreachable dynamic code while retaining closures and suspended frames). Some V8 language features and host APIs remain unimplemented; executed conformance failures remain visible. Absent APIs are `undefined`, so libraries that feature-detect take their fallbacks; a feature V9 parses but cannot run throws a catchable `TypeError` and is listed in `Execution::effects.unsupported`.

Arrays store logical length separately from resident values, so large gaps do not allocate every hole. Array species constructors and Set algebra preserve callback order. String normalization uses ICU4X scalar algorithms while preserving lone UTF-16 surrogates. `Math.sumPrecise` accumulates binary inputs exactly before rounding once. Function source keeps the original grammar and trivia.

ArrayBuffer supports actual resizing, transfer and detachment. Fixed and length-tracking typed-array and DataView bounds follow the backing allocation, including shrink and regrowth. Buffer and resident-element limits remain enforced.

Iterator helpers support lazy transformations, eager consumers, concatenation and joint iteration with captured padding and reverse cleanup. Native disposal stacks and iterator disposal methods retain captured resources through suspension and saved-state restoration. Unicode regex property escapes use exact, case-sensitive ICU4X category, binary-property, script and script-extension data across the entire code-point range. Unicode regex matching uses simple case equivalence, complete surrogate-pair positions and backwards lookbehind captures; buffer-boundary assertions are an explicit environment opt-in. Uint8Array codecs support hexadecimal and base64 conversion, including partial writes and strict final-chunk validation. Array, ArrayBuffer and typed-array result creation observes actual constructors and species with live storage checks.

WebAssembly uses V9-owned binary validation and execution, including scalar instructions, SIMD, GC references, custom descriptor identity and prototypes, prototype configuration, exception tags, memory64 and synchronized shared memory with real atomic wait/notify. Shared GC records use an owned arena with cross-worker root handles, synchronized atomic fields, cycle collection and validated inert snapshots. Modules retain validated bytes and inert heap records across requests. JSPI suspends imported Promises into typed heap-owned call and control frames and resumes through real Promise jobs, including loops, conditional paths and direct/reference/indirect calls. Suspension through exception handlers, Wasm continuation effects and other unimplemented instructions still fail explicitly. The opt-in d8 host compiles promised byte sources through actual Promise jobs and enforces original module-size flags only as a lower limit on the existing byte ceiling.

Regular expressions run on V9's own backtracking engine (`regexp.rs`), bounded by `REGEXP_STEPS`, `REGEXP_NODES` and `REGEXP_GROUPS`.

## Bounds

Every bound in `mars_core::constants` still applies, now to live values: a precise collector runs between instructions, so garbage never counts, and a live count past its bound stops the script with `limit_exceeded` (scripts cannot catch it). `VM_OBJECTS`, `VM_FUNCTIONS`, `VM_PROPERTIES`, `JS_ENVIRONMENT_UNITS`, `VM_ENVIRONMENTS`, `VM_BINDINGS`, `VM_ARRAY_LENGTH`, `JS_STRING_UNITS` and `VM_STATE_BYTES` bound the heap and the committed state; `OPERATIONS` bounds work and the script deadline (`SCRIPT_DEADLINE_DEFAULT`, overridable through `MARS_V9_SCRIPT_DEADLINE_MS`) bounds wall-clock time; `SYNTAX_DEPTH`, `TOKENS`, `STATEMENTS` and `FUNCTION_NESTING` bound the parser, with `EXPRESSION_DEPTH` and `STATEMENT_DEPTH` bounding nesting inside each function body; `VM_CALL_DEPTH` bounds frames (a catchable `RangeError`); `VM_PENDING_JOBS`, `VM_TIMERS`, `VM_HOST_REQUESTS`, `VM_LISTENERS`, `VM_DETACHED_NODES` and `VM_CONSOLE_ENTRIES` bound the platform. The `measure` feature adds `measure::*_scaled` entry points (never used by the service) that multiply the heap bounds to show what a script needs.

## Pages

`load_page(PageInput)` loads a page's scripts the way a browser does: each script runs against what earlier scripts left; a script that fails to parse, throws, reaches an unsupported feature or exceeds a limit is rolled back alone and listed in `Execution::failed`. Microtasks drain after each script. `PageInput` carries the sources, the projection, storage candidates, the `DocumentHost` (languages and the fragment parser) and a `PageEnvironment` (URL, referrer, user agent, cookie string, viewport, time origin, timezone, random seed, script URLs, node text, and the change-log level).

Later requests take the committed `VmState`:

| Function | What it does |
|:--|:--|
| `page_event(input, state, PageEvent::DomContentLoaded \| Load)` | Moves `document.readyState` forward and fires the lifecycle events. |
| `click_page(input, state, local_id)` | Dispatches a trusted click (capture, target, bubble to document and window); an unprevented link click records a navigation. |
| `run_timers(input, state, now_ms, budget)` | Advances the virtual clock, running due timeouts, intervals and animation frames earliest first (at most `budget` callbacks, each followed by its microtasks). |
| `resume_requests(input, state, responses)` | Delivers `HostResponse`s: fetch promises settle, XMLHttpRequests fire their events, awaiting code resumes. |

`Execution::effects` lists what the app does next: `requests` to perform, the last `navigation` request, same-document `history` records, raw `cookies` written through `document.cookie`, `console` output, `next_timer_ms`, `unsupported` APIs and a changed `title`.

The platform provides `window`/`self`/`globalThis`, `document`, `navigator`, `location`, `history`, `screen`, `devicePixelRatio`, `innerWidth`/`innerHeight`, `console`, `performance`, `localStorage`/`sessionStorage`, timers, `queueMicrotask`, `fetch`, `XMLHttpRequest`, `sendBeacon`, `URL`, `URLSearchParams`, `Headers`, `Event`/`CustomEvent`, `atob`/`btoa`, `matchMedia`, `getComputedStyle` (inline values over CSS defaults; V9 has no layout), `document.fonts` (a FontFaceSet with every font already loaded), `CSS.supports`/`CSS.escape`, `AbortController`/`AbortSignal`, `requestIdleCallback` (with an IdleDeadline), IntersectionObserver and ResizeObserver (each observed target is reported once, asynchronously, as intersecting and zero-sized, since V9 has no layout; MutationObserver records nothing yet), and `crypto.randomUUID`/`crypto.getRandomValues` (from the operating system's CSPRNG; only `Math.random` is seeded). `Element.animate` is absent so Web Animations polyfills install their own.

## The DOM and its change logs

The DOM is the app's projection plus nodes scripts create (kept in state until inserted) and virtual `html`, `head` and `body` elements; each page script gets a `<script>` element in the head so `document.currentScript` works. Queries use V9's selector engine (type, id, class, attribute operators, combinators, structural, `:not`, `:is`, `:has`). Mutations of connected nodes go to the projection and to a change log the app's document engine replays; `Execution::structure` is the digest the replayed document must reproduce.

With `DomLevel::Legacy` (the default, and what the existing `*_document_scripts*` entry points use) V9 emits only `DomChange` (text by id, fragment insertion, moves, `lang`/`dir`); a script that needs more fails alone as before, except that a node inserted and removed in the same run leaves no trace. With `DomLevel::Extended`, V9 emits `Execution::script_changes` (`mars_core::dom::ScriptChange`): node removal, any attribute `script_attribute_allowed` admits (never `on*` handlers, `srcdoc` or `http-equiv`), attribute removal, node text and field values. `replay` applies both logs to a projection for callers without Eye, and `SimpleFragmentParser` is a standalone fragment parser on V9's own HTML reader.

## Compatibility entry points

`check`, `execute`, `load`, `load_scripts*`, `event*`, `load_document_scripts*` and `click_document_scripts*` keep their signatures and run on the same engine. Run `cargo run -p mars-v9 --example check -- FILES` to parse files, `--run FILES` to load them as one page, and `cargo run -p mars-v9 --features measure --example js -- [--scale N] FILES` or `--example page -- [--scale N] [--extended] [--click TAG] FILES` to measure real scripts.

See [Mars's supported features](../../README.md), [upstream research](../../UPSTREAM.md) and [third-party notices](../../THIRD_PARTY_NOTICES.md). V9's dependencies are Mars Core's shared contracts plus the pinned serde, serde_json, ryu-js, ICU4X Unicode data, url and ring crates already in the workspace. This package is an unpublished path dependency with no API stability guarantee.

## Upstream conformance runs

Run V8's unchanged `test/mjsunit` sources from the Mars workspace:

```sh
CONF_OUT=/tmp/mars-v9-mjsunit CONF_REQUIRE_PASS=1 cargo test --release --offline \
  -p mars-v9 --features conformance --test test262_runner -- \
  --ignored --nocapture --exact mjsunit
```

`MJSUNIT_ROOT` selects the checkout (default `~/Dev/upstream/v8/test/mjsunit`),
and `MJSUNIT` optionally selects comma-separated path prefixes. Without a filter,
the runner retains its existing exclusions for V8 native syntax and the d8,
Wasm, shared-memory, sandbox and tools directories. Report skipped cases separately;
they do not count as passes. `CONF_REQUIRE_PASS=1` makes any executed failure fail
the command after writing the complete JSON report.

To inspect previously excluded top-level directories, explicitly admit them and use
`MJSUNIT` to select cases, for example:

```sh
MJSUNIT_INCLUDE_DIRS=wasm,shared-memory,d8 MJSUNIT=wasm/memory-size.js,shared-memory/value-serializer.js,d8/files/exists.js CONF_OUT=/tmp/v9-extra CONF_REQUIRE_PASS=1 cargo test --release --offline -p mars-v9 --features conformance --test test262_runner -- --ignored --nocapture --exact mjsunit
```

Opt-in discovery records each excluded path and reason in the JSON report. Set
`MJSUNIT_INCLUDE_INERT_NATIVE_FLAG=1` to admit tests that request native syntax but contain
no percent character and use only other supported flags. Set `MJSUNIT_INCLUDE_NATIVE_INTRINSICS=1`
to also admit tests whose percent tokens name only the implemented `%AbortJS`,
`%PerformMicrotaskCheckpoint`, `%ArrayBufferDetach`, `%Is64Bit`, `%ToLength`, and
`%AtomicsSynchronizationPrimitiveNumWaitersForTesting`, `%IsJSReceiver`, `%IsArray`, and
`%ToBoolean`, `%Typeof`, `%ToName`, `%CreateIterResultObject`, `%GetWeakCollectionSize`,
`%AtomicsNumWaitersForTesting`, `%AtomicsNumUnresolvedAsyncPromisesForTesting`, and
`%SetAllowAtomicsWait`, `%IsAtomicsWaitAllowed`, `%IsSharedString`, `%IsSameHeapObject`,
`%SharedGC`, `%GetInitializerFunction`, `%ArrayBufferDetachForceWasm`,
`%GetWasmExceptionTagId`, `%GetWasmExceptionValues`, `%ThrowStackOverflow`, and `%StringMaxLength`
operations. `--noincremental-marking` is supported because V9's collector already completes
marking in one stop-the-world pass.
`--shared-string-table` enables the bounded shared literal table used by worker transfers.
The equivalent `--no-incremental-marking` spelling, `--expose-async-hooks`,
`--ignore-unhandled-promises`, `--throws`, `--harmony-shadow-realm`, and
`--enable-queue-microtask` use their
corresponding implemented host behavior.
`%SharedGC` coordinates live workers at safepoints; it reports unsupported if a concurrent
collection or a native path cannot reach one within two seconds.
Every discovered test is excluded when it requires an unsupported flag, including default-suite
tests; exclusions record the full flag line so the reported pass rate does not imply coverage.
The scan is conservative, so percent signs in comments or ordinary modulo expressions can keep
a case excluded; unknown native intrinsics are never treated as successful operations. Original
source is parsed directly under an opt-in shell grammar with a separate code-cache namespace.
The same opt-in applies to the exact upstream Wasm exception helper fixture; other fixture paths
do not gain native syntax.
Unconditional upstream skips and tests whose flags the host has not verified stay
excluded. An empty strict selection fails. Workers verify the parent’s case
identity and are terminated and reaped when their response deadline expires. Literal relative
module fixtures are snapshotted recursively; an unsupplied computed fixture fails explicitly.

The opt-in `conformance` feature supplies test-host `d8.file.execute/read/exists`, `load`
and binary `readbuffer` over host-provided fixtures, synchronous and scheduled major/minor collection,
and uncatchable host termination. The runner opts into ordinary shell globals,
so browser properties such as `Window.top` do not interfere with test bindings.
Realm and ShadowRealm contexts use separate intrinsics, globals and module state;
callable wrapping and structured Error stacks retain their ownership boundaries.
Workers use explicitly scoped Rust threads and isolated V9 heaps. Typed message
graphs preserve cycles and sharing; transferred buffers detach, and shared buffers
retain synchronized native backing stores. Session teardown cancels and joins
child isolates. `performance.measureMemory` schedules real collection and reports
conservative resident-data estimates for permitted contexts. It counts live object
headers, property/binding storage, captured values and deduplicated backing data;
process code/shape caches, allocator bookkeeping and RSS are excluded.
File execution reports original ordinary exceptions and throws a new Error wrapper,
matching d8; native failures and termination retain their severity. The runner
preserves each source as a separate global Script, honors `// Files`, `// NO HARNESS`
and native `--bundle` Script/module containers, and delivers actual module fixtures, timers and microtasks
before scoring. Fixture reads stay beneath the upstream checkout, including
through symlinks. These test-host APIs are absent from default builds. Missing
capabilities remain executed failures; the runner does not emulate success.
Declared `TZ` metadata supplies bounded system TZif transitions and future rules;
on macOS, the test host obtains localized zone names from the system ICU4C library.


Validation snapshot, 2026-10-25, against V8 revision
`fedd021b538607860f9c56e5c193ad8dcd4f2bb1`: the isolated V9 commit candidate
passed 1,014 default-build Rust tests and 1,205 conformance-feature Rust tests,
with five existing ignored tests in each configuration. The unfiltered mjsunit
run passed 3,411 of 3,742 executed cases (91.15%); 331 cases failed and the
original 5,345 exclusions remained separate. The strict upstream gate failed as
required. Full V8 compatibility is unfinished; implementation gaps, resource
bounds and incompatible upstream configuration expectations remain visible.
