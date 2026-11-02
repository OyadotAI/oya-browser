# Handover: Mars fingerprint parity work (2026-10-29)

Written for Codex (or any agent) picking up this tree. Branch `blink-parity`, nothing committed;
the maintainer commits. Read `AGENTS.md` and `mars/README.md` first. Hard rules: never commit or
push, never add Claude attribution, no em-dashes in prose or code comments, every file opens with a
doc comment, Mars is its own engine (small leaf crates are allowed), run the whole Mars gate before
saying done and fix every failure it shows even if unrelated.

## 1. The goal and where it stands

Google serves Mars a JavaScript interstitial on every cookieless `/search` and then rejects the
botguard token V9 computes, redirecting to `/sorry/` (reCAPTCHA). The wire is already
Chrome-identical (TLS JA4 `t13d1516h2_8daaf6152771_d8a2da3f94cd`, HTTP/2 hash, headers) through
the vendored rustls fork in `mars/vendor/rustls` (notes in `vendor/rustls/MARS.md`). What is left
is JavaScript-environment fidelity: the engine must look like V8 and the platform like Chrome.

Diagnosis tools and recipes are in the memory file
`~/.claude/projects/-Users-mk-Dev-oya-agentchrome/memory/mars-page-diagnosis.md` and
`mars-fingerprint-findings.md`; the relevant bits are repeated below.

## 2. State of the working tree RIGHT NOW: it does not compile

Three parallel lanes were mid-flight when the session ended. Their edits are in the tree,
uncommitted and unfinished. `cargo build -p mars-v9` fails in `compile/pattern.rs`,
`syntax/expr.rs` and `vm/run.rs` (three-argument calls to a function that grew a fourth
parameter, an `Option<&str>` used as a bool, a `callee_at` type mismatch). Finish or back out each
lane before anything else. The lanes and their files:

- **Lane vm (runtime error texts, V8 callee rendering)**: `vm/call.rs`, `vm/error.rs`,
  `vm/error/render.rs` (new, `v8_value_text` and `v8_callsite_text`), `vm/iter.rs`,
  `vm/private_elements.rs` and `/brand.rs`, `vm/property.rs`, `vm/run.rs`,
  `compile/site_text.rs` (new), `compile/expr.rs`, `compile/function.rs`, `compile/mod.rs`,
  `compile/pattern.rs`, `compile/stmt.rs`, `compile/catch_bindings.rs`, new tests
  `tests/class_messages.rs`, `tests/iteration_messages.rs`, `tests/strict_property_messages.rs`.
  Design: render the callee expression at compile time (V8's CallPrinter rules: `o.f`, `o[0]`,
  `(intermediate value)`, `{}`, literals) into a side table per call/construct/iterate site, use it
  in `Vm::invoke` and the iterator path; render values V8-style (`#<Object>`, `[object Array]`,
  `function f() { [native code] }`) for property and receiver errors.
- **Lane syntax (SyntaxError texts)**: `syntax/*.rs`, `lexer.rs`, `lexer/number.rs`,
  `literals.rs`, `regexp.rs`, `regexp/class_set.rs`, `regexp/modifiers.rs`, `errors.rs`
  (the `V8_REASONS` mapping). Design: a token-class aware `unexpected()` producing
  `Unexpected token 'x'`, `Unexpected identifier 'b'`, `Unexpected number`, `Unexpected string`,
  `Unexpected end of input`, `Unexpected strict mode reserved word`; the specific texts from V8's
  `src/common/message-template.h`; regexp texts without the doubled prefix.
- **Lane builtins (property order, Math, builtin messages)**: `builtins/mod.rs`, `builtins/object.rs`,
  `builtins/reflect.rs`, `builtins/regexp.rs`, `builtins/math.rs`, new `builtins/math/{fdlibm,
  glibc_trig, hyperbolic, sincostab, stable}.rs`, new `builtins/v8_order.rs`, `crates/v9/Cargo.toml`
  and `Cargo.lock` (a `libm` dependency may have been added). Design: every intrinsic's
  `Object.getOwnPropertyNames` order equal to V8's; `NaN`, `Infinity`, `undefined` as own
  non-writable globals; Math bit-identical to V8 (fdlibm); builtin messages as V8 words them.

How to tell a lane's leftover from finished work: run the differential harness (section 4) and
read the compile errors. If a lane is too broken to finish quickly, `git diff` its files and keep
only what compiles; do not `git checkout` whole files blindly, because earlier finished work (the
fmt/clippy sweep, the platform additions) lives in the same files.

## 3. Finished this session (compiles on its own, not yet gate-run)

- `platform/chrome_shapes.rs` + `platform/chrome_shapes.txt` + `tools/chrome_shapes.py`:
  every window interface Chrome exposes (932, from Blink's stable
  `global-interface-listing-expected.txt` at Chromium 2c592105bb plus the platform-specific
  listing, constants from IDL). Interfaces Mars lacks are created whole (parent chain,
  members, constants, toStringTag, Illegal constructor); interfaces Mars has get the members
  they miss. Members on a prototype throw `Illegal invocation` (Chrome's behaviour without an
  instance); on an instance they return `undefined` (handlers `null`) and setters store an own
  property so the event dispatcher and round-trips keep working. The installer runs last in
  `platform/install.rs::install`. Verified: all 932 names present, `webkitURL === URL` etc.
- `platform/chrome_env.rs::navigator_objects`: `navigator.mediaDevices` (enumerateDevices
  empty, getUserMedia/getDisplayMedia reject NotFoundError, getSupportedConstraints),
  `storage` (estimate, persisted), `userActivation`, `getBattery`, and 23 more member objects of
  the right interfaces. Not yet built (the tree was broken by the lanes); expect small compile
  fixes (`PromiseState` import path `crate::v9::vm::data::PromiseState`, `vm.set_proto`).
- `vm/realm.rs`: `Builder::host_method` (enumerable, as Chrome lists WebIDL operations),
  `Builder::host_constant`, and `Builder::accessor` now names getters `get x` / `set x`.
  All `b.method(` in `platform/` became `b.host_method(`; Node constants are enumerable;
  the animation handler attributes moved from `EventTarget.prototype` to HTMLElement,
  Document and Window prototypes (`install.rs::event_handler_attributes`).
- Earlier in the session, already green: the TLS fork, Trusted Types, plugins, Chrome env,
  the Eye font panic fix, the runtime revision-conflict fix, and 181 fmt/clippy fixes.

## 4. How to measure (the differential harness)

`node` on this machine is V8 14.x; for pure JavaScript its output is what Chrome prints.
Scratchpad (may be gone; recreate from this description):
`/private/tmp/claude-501/-Users-mk-Dev-oya-agentchrome/b332133e-ade1-4c58-9648-eb26d9fe2390/scratchpad/`
holds `v8diff.sh`, `diff1.js` (the probe), `diff1.diff` (240 differing lines at session start),
`gen_ifaces.py`, `blink/` (the listings), `members.js`, `winprops.js`, `lanes/*/probe.js`.

Recipe: a probe collects lines into a global array `L` with
`function p(label, f) { try { L.push(label + " = " + String(f())) } catch (e) { L.push(label + " ! " + e.name + ": " + e.message) } }`
and ends with
`var W = (typeof WIN === "undefined") ? [0, 1e9] : WIN; L.slice(W[0], W[1]).forEach(function (l) { console.log(l.slice(0, 1000).replace(/\n/g, "\\n")); });`
Run under node, and under V9 with
`cargo build --release -p mars-v9 --features measure --example js` then
`target/release/examples/js win.js probe.js` where `win.js` is `var WIN=[i,i+30];` for each
window of 30 (V9's console is capped at 32 entries of 1 KB, `VM_CONSOLE_ENTRIES`). Diff the two.
Things node does that Chrome does not (module wrapper `arguments`, `global`, `process`,
`Error.prepareStackTrace` set) are not gaps.

Reference sources, never copied: `~/Dev/upstream/v8` (`src/common/message-template.h`,
`src/ast/prettyprinter.cc`, `src/init/bootstrapper.cc`, `src/base/ieee754.cc`) and
`~/Dev/upstream/chromium` (partial clone; `git show HEAD:path` fetches a blob but can hang,
`https://chromium.googlesource.com/chromium/src/+/<commit>/<path>?format=TEXT` base64 works).

## 5. Gate and app verification

Mars gate, from `mars/`: `cargo fmt --all --check`,
`cargo clippy --offline --workspace --all-targets --all-features -- -D warnings`,
`cargo test --offline --workspace --all-features`. The lanes used their own
`CARGO_TARGET_DIR`s under the scratchpad; the main `target/` is warm. Tests that pinned old
error wording must be updated to V8's wording, never deleted. `inner_scroll` and `spread` tests
are load-sensitive; rerun them alone before calling them broken.

App check: rebuild `mars/dist/Mars.app` (the packaging script the README names), quit the running
Mars.app first (`pkill -9 -f target/release/mars`; a stale instance holds the agent socket), then
`python3 tools/inspect_page.py` style readout or the scratchpad `inspect_site.py`
(`desktop --agent --ephemeral --capture-dir ... --url URL`, then `window.inspect {}` and
`page.screenshot`). Note `--agent-control` makes the startup navigation report
"Navigation blocked" for some sites; use the plain window for those.
Sites: `https://www.google.com/search?q=test` (the goal: results, not the interstitial),
`https://bot.sannysoft.com`, `https://abrahamjuliot.github.io/creepjs/`,
`https://browserleaks.com/ip`, `https://deviceandbrowserinfo.com/are_you_a_bot`,
`https://www.browserscan.net/bot-detection`, `https://tls.peet.ws/api/all`.
Google's interstitial can be replayed offline: `MARS_PERF_DUMP` frame dumps and
`V9_TRACE_MISSING=all target/release/examples/replay <dump>` print every property miss and
every throw the script hits (see the diagnosis memory file).

Two experiments the permission classifier refused for me, which the maintainer can run:
presenting the "Google Chrome" brand instead of "Mars" end to end (`crates/relay/.../hints.rs`
and the V9 userAgentData brands), and Chrome's `x-browser-validation` / `x-client-data` headers.

## 6. What remains, in order

1. Make the tree compile: finish or trim the three lanes (section 2). Then the gate.
2. Rebuild Mars.app and check Google plus the detector sites; compare what Google's script
   now reads with `V9_TRACE_MISSING=all` on a fresh dump.
3. Window object shape: Chrome's window has its [Global] members as own accessors (document,
   navigator, location, the `on*` handlers); Mars has data properties and members on
   `Window.prototype`. `Object.getOwnPropertyNames(window)` order also differs.
4. Member gaps on existing interfaces that stubs do not cover well: MouseEvent, KeyboardEvent,
   PointerEvent, WheelEvent, TouchEvent members live on instances in Mars, on `UIEvent`-chained
   prototypes in Chrome; `Document` and `HTMLElement` handler accessors are now stubs that store
   own properties.
5. Canvas 2D with `toDataURL`, a WebGL context stub, `AudioContext`, `OffscreenCanvas`,
   `document.write`.
6. `Accept-Language` derived from the same list as `navigator.languages` (macOS gives
   `en-US,ar-US`; the header is static `en-US,en;q=0.9`).
7. Intl: `supportedValuesOf` lists one calendar and one time zone, `localeCompare` ignores
   invalid tags and Swedish collation, `toLocaleUpperCase("tr")`.
8. The four CBC cipher suites in `crates/relay/src/network/web/legacy.rs` (ECDHE + AES-CBC +
   HMAC-SHA1; advertised, not implemented; the client retries without them).
9. Date in the `js` example harness runs in UTC; the app passes the host time zone, so only
   the harness differs.
