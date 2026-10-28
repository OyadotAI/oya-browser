# Plan: a working browser (for Haiku and Codex builders)

Milestone: real pages render close to Chrome and behave like Chrome for reading, searching,
signing in and navigating. Wasm, d8 conformance and Temporal are out of scope until then.
Rules for every task: `AGENTS.md` at the repo root (file headers, doc comments, small
functions, no magic numbers, tests beside the change, no commits unless asked, no em-dashes).

## Ground truth on 2026-10-28

Measured on `mars/dist/Mars.app` built from `blink-parity` at `4748f142`.

| Signal | Value | How it was measured |
|---|---|---|
| mjsunit | 5473 pass / 139 fail / 3476 skipped (all skips d8-only) | `MARS_V9_SCRIPT_DEADLINE_MS=30000 CONF_TIMEOUT_MS=60000 cargo test --release --offline -p mars-v9 --features conformance --test test262_runner -- --ignored --nocapture mjsunit` |
| test262 | 43837 pass / 4225 fail (3674 Temporal and Intl402, 446 string bound) | same command with `test262` |
| V9 speed, browser build | arithmetic 200 ns, call 370 ns, property read 150 ns, `new` 900 ns, indexed read 500 ns per operation; about 10x slower than V8's interpreter | temporary test timing `common::eval` with `std::time::Instant` |
| YouTube home | renders; 17 s of scripts over 75 s, 11 MB bundle, no video | `MARS_PERF` phase log (below) and `mars/tools/inspect_page.py` |
| Google | home renders, typing works, search returns Google's bot-block page | `mars/tools/inspect_page.py` plus a `page.click` on the search button |
| Visual parity vs the Electron app | 62.7 / 100 on `qa/mars-compat/sites.txt` (older run) | `python3 qa/mars-visual/visual.py` |

## How to measure (use these before and after every task)

The `qa/` harnesses (`mars-compat`, `mars-visual`, `mars-bench`, `mars-parity`) live only on the
maintainer's machine (`qa/` is gitignored); ask for their output or run them there.

- Phase log: `MARS_PERF=/tmp/perf.tsv timeout 60 mars/dist/Mars.app/Contents/MacOS/mars desktop --ephemeral --url <url>`.
  One line per step: `page.*`, `script.step`, `rpc.total <op>`. Three `service.ready` lines are
  startup; more mean an engine restart. `MARS_PERF_DUMP=<dir>` also writes every request frame.
- Replay a dumped frame into a service to read the real error (service stderr is nulled in the app):
  `cat ready.json parse.json | mars _service document` (one JSON object per line).
- Inspect and screenshot: `python3 mars/tools/inspect_page.py <url> 40 out.png`. Prints title,
  status, `degraded`, the CSS and API diagnostics, and saves the screenshot.
- Engine unit gate: `cargo test --release --offline -p mars-v9 --no-fail-fast` and the same
  with `--features conformance`; eye: `-p mars-eye`; network: `-p mars-relay`.
  Known red before this plan: `regex_imports_snapshot_virtual_root_and_verified_absence`
  (browser configuration only), `a_stalled_server_times_out_before_the_callers_deadline` (flaky
  under parallel load), and workspace-wide `cargo fmt --check` and `cargo clippy -D warnings`.
- Rebuild the app a person can test: `bash mars/packaging/build-app.sh`, then `open mars/dist/Mars.app`.

Evidence to return with every task: the before and after numbers from the measurement that
matches the task, the test names added, and the diagnostics or screenshot that changed.

## Tasks

Sizes: S under a day, M a few days, L one to three weeks. "Haiku" tasks are bounded and
mechanical with a single file family and a checkable output; "Codex" tasks carry design
decisions across modules. A builder that finds its task needs a file outside the listed
family stops and reports instead of editing it.

### A. Visual fidelity (eye crate)

- A1 (Haiku, S). Rank CSS gaps by frequency. Run `qa/mars-visual/visual.py` on
  `qa/mars-compat/sites.txt`, and for each site run `mars/tools/inspect_page.py` and collect the "CSS property X
  is not supported" and "A CSS value for X" diagnostics. Produce `qa/mars-visual/css-gaps.md`:
  property, count of sites, example value, and the parity score of the sites it appears on.
  Evidence: the file, plus the raw diagnostics per site.
- A2 (Codex, M). Transform functions still dropped: `rotate(1turn)`, `translateZ`,
  `translate3d` with three values, `scaleX(-1)`, `matrix`. Implement in
  `crates/eye/src/document/style/paint.rs` beside the existing `translate`/`scalex` arms, with
  tests in `crates/eye/tests/` mirroring that file. Evidence: YouTube and GitHub diagnostics no
  longer list `transform`; screenshots before and after.
- A3 (Codex, M). `text-shadow` (paint), `scale`, `rotate`, `translate` as individual
  properties (compose into the transform), `transform-origin`. Same file family and evidence.
- A4 (Codex, L). `@container` queries and the pseudo-element selectors the inventory in A1
  shows are not `::-webkit-scrollbar`. Design note first: which selectors, how container size
  resolution fits the existing cascade in `crates/eye/src/document/style/`.
- A5 (Haiku, S). Fonts: `font-variation-settings`, `font-stretch`, `font-display`,
  `unicode-range` and `src` inside `@font-face` are reported as unsupported properties. Make
  the `@font-face` descriptors parse silently (they are descriptors, not properties) and record
  `font-variation-settings` as an accepted no-op with a diagnostic once per page.

### B. Scripts behaving like Chrome (V9 platform and runtime)

- B1 (Codex, L). DOM events delivered to scripts: `input`, `change`, `keydown`/`keyup`,
  `submit`, `focus`/`blur`, `mouse*`, `click` on any element. Row WP-05 in
  `qa/mars-parity/PARITY.md`. Evidence: Google's search box submits on Enter through Google's
  own handler (today Enter does nothing because the keydown never reaches the page), and a
  React form validates as you type on a fixture under `mars/fixtures/`.
- B2 (Codex, L). `fetch` and `XMLHttpRequest` with CORS and credentials, through Relay with
  cookies attached. Row WP-06; depends on the same-origin rules in WP-07. Evidence: a fixture
  page that fetches JSON from its own origin and from a second fixture origin, and
  `inspect.py` on Hacker News showing comments load after a script fetch.
- B3 (Codex, M). History API: `pushState`, `replaceState`, `popstate`, `hashchange`, same
  document entries in the tab history. Row WP-14. Evidence: a fixture SPA router changes the
  address bar and Back returns without a reload.
- B4 (Codex, M). Cookies: `Domain=` cookies, `document.cookie`, a jar sized for Google
  (more than 16 per host). Row WP-08. Evidence: sign-in on a fixture that sets a domain cookie
  on a subdomain, and Google no longer resets its consent state between pages.

### C. V9 speed

- C1 (Haiku, S). Regexp class membership is a linear scan over the class's ranges per
  character (`in_set` in `crates/v9/src/v9/regexp.rs`). Sort and merge ranges at compile time
  and binary search. Evidence: `/^\p{Alphabetic}+$/u` over 300k astral characters finishes
  under 200 ms in a `tests/regexp_long_loops.rs` test; existing regexp suites green.
- C2 (Haiku, S). The `DEBUG` style leftovers and any `eprintln!` in `crates/v9/src` hot paths:
  list and remove, evidence is `grep -rn eprintln crates/v9/src` output before and after.
- C3 (Codex, L, design note before code). Value representation and operand stack: the
  profile shows time in `JsValue` clone and drop on push and peek, `Arc::make_mut` on shared
  storage, and string hashing on inline-cache misses. Proposal expected: NaN-boxed or tagged
  values with strings and bigints behind a handle, a register file instead of a push and pop
  operand stack for arithmetic, and interned property keys. Gate: the micro-benchmark above
  halves on every row, every V9 suite stays green in both configurations, mjsunit and test262
  numbers do not drop.

### D. Network

- D1 (Codex, L, research first). Google and other sites serve a bot-block page to Mars.
  Compare the TLS ClientHello (rustls) and the HTTP/2 SETTINGS and header order with Chrome's
  using a local capture, and list every difference. Then decide with the maintainer what rustls
  can be configured to match and what needs more. Evidence: the capture diff and a Google
  search that returns results from an ephemeral window.
- D2 (Haiku, S). The restart banner says "Browser network service stopped" for both a crashed
  engine and one that passed its RPC deadline. Make the banner name the operation and say
  "did not answer within N seconds" for the deadline case (`src/desktop/window/events.rs`,
  `src/runtime/restart.rs`). Evidence: a unit test on the banner text for each error shape.
- D3 (Haiku, S). Legacy text encodings in Relay: row WP-24. Evidence: a Shift_JIS fixture
  renders its text, `crates/relay/tests/encoding.rs` extended.

### E. Housekeeping

- E1 (Haiku, S). Refresh `qa/mars-parity/PARITY.md` and `parity.json`: for every WP row, check
  the cited Mars source today and set the status with a one-line evidence pointer. Rows known
  stale: WP-01, WP-02, WP-03, WP-13, WP-15, WP-24, WP-25 are largely done now.
- E2 (Haiku, S). The two pre-existing red tests named above: fix
  `regex_imports_snapshot_virtual_root_and_verified_absence` in `crates/v9/tests/support/mjsunit.rs`
  (browser configuration), and make the stalled-server test tolerant of parallel load or run
  it serially.
- E3 (Haiku, M). The remaining test262 language rows: 6 in `language/global-code`, 5 typed
  array cases that need `$262.createRealm` in the runner prelude, and the `other` bucket
  (Number.toExponential rounding and -0, Date negative-year formatting, decodeURI malformed
  percent escapes, Object.entries ordering with getters). Each is one engine file plus one
  test file.

## Order

Week 1: A1, A5, C1, C2, D2, E1, E2 (Haiku, in parallel on disjoint files); A2 and B1 start
(Codex). Week 2 and 3: A3, B2, B3, B4. Then C3 and D1 as design notes reviewed by the
maintainer before code, A4 and E3 as they fit.
