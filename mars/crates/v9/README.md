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

Language support covers ES2023 classic scripts: classes with fields, private members, accessors and static blocks; destructuring, spread and rest; generators and `yield*`; async functions and `await`; promises with a microtask queue; labels, `switch`, `try`/`catch`/`finally`; optional chaining, nullish coalescing and logical assignment; tagged templates. The library includes Object, Function, Array, String, Number, Boolean, Symbol, BigInt, Math, JSON, Date, RegExp, Map, Set, WeakMap, WeakSet, WeakRef, FinalizationRegistry, Promise, Proxy, Reflect, ArrayBuffer, the eleven typed array kinds, DataView, structuredClone, TextEncoder/TextDecoder and the Error types. BigInts are bounded to 128 bits: a result past that throws a catchable RangeError. A WeakRef holds its target strongly and finalization callbacks never run, both behaviours the specification allows. ArrayBuffer bytes count against `JS_ENVIRONMENT_UNITS` (two bytes per unit) and one buffer holds at most twice `JS_STRING_UNITS` bytes. `eval` and `new Function` compile with V9's own parser (dynamic sources are kept in state, bounded by `VM_DYNAMIC_SOURCES`). Not supported: modules, `with`, async generators and `for await`, SharedArrayBuffer and Atomics, Intl. Absent APIs are `undefined`, so libraries that feature-detect take their fallbacks; a feature V9 parses but cannot run throws a catchable `TypeError` and is listed in `Execution::effects.unsupported`.

Regular expressions run on V9's own backtracking engine (`regexp.rs`), bounded by `REGEXP_STEPS`, `REGEXP_NODES` and `REGEXP_GROUPS`.

## Bounds

Every bound in `mars_core::constants` still applies, now to live values: a precise collector runs between instructions, so garbage never counts, and a live count past its bound stops the script with `limit_exceeded` (scripts cannot catch it). `VM_OBJECTS`, `VM_FUNCTIONS`, `VM_PROPERTIES`, `JS_ENVIRONMENT_UNITS`, `VM_ENVIRONMENTS`, `VM_BINDINGS`, `VM_ARRAY_LENGTH`, `JS_STRING_UNITS` and `VM_STATE_BYTES` bound the heap and the committed state; `OPERATIONS` and `SCRIPT_DEADLINE` bound work; `SYNTAX_DEPTH`, `TOKENS`, `STATEMENTS` and `FUNCTION_NESTING` bound the parser, with `EXPRESSION_DEPTH` and `STATEMENT_DEPTH` bounding nesting inside each function body; `VM_CALL_DEPTH` bounds frames (a catchable `RangeError`); `VM_PENDING_JOBS`, `VM_TIMERS`, `VM_HOST_REQUESTS`, `VM_LISTENERS`, `VM_DETACHED_NODES` and `VM_CONSOLE_ENTRIES` bound the platform. The `measure` feature adds `measure::*_scaled` entry points (never used by the service) that multiply the heap bounds to show what a script needs.

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

See [Mars's supported features](../../README.md), [upstream research](../../UPSTREAM.md) and [third-party notices](../../THIRD_PARTY_NOTICES.md). V9's dependencies are Mars Core's shared contracts plus the pinned serde, serde_json, ryu-js, url and ring crates already in the workspace. This package is an unpublished path dependency with no API stability guarantee.
