# Graphics and fingerprint baseline — 2026-10-29

Measured from `0968fdfc` on macOS arm64 before the Canvas implementation.
This is a baseline, not a browser compatibility or release verdict.

## Scope and review

The PM scoped a first real Canvas 2D slice, language consistency, accurate GPU
absence results and rendering measurements. Full Canvas, WebGL/WebGPU, a GPU
compositor, remaining TLS parity and application signing were excluded. The
designer specified a local drawing/readback/export fixture and visible capability
results. The principal required bounded, serializable bitmap ownership shared by
both painters, with allocation and publication failures preserving committed state.

Implementation reuses page revisions and candidate transactions instead of adding
a separate bitmap asset store/revision protocol; the reason and stricter shared
96 MiB allocation ceiling are recorded in `../ARCHITECTURE.md`. That reservation
ceiling is not a total process RSS bound. No PM scope override was made.

Changed areas are `core/src/canvas*` and `languages.rs`; V9 Canvas, reflection,
GC and GPU capability bindings; Eye native drawing/layout/publication; desktop
and agent painters; Relay request-language/cache shaping; runtime host injection;
and the fixture, regression tests, changelog and architecture/evidence documents.
Review findings covering publication, dimensions and reachability were repaired.
Security reported zero findings. Reliability's listener-cycle finding was repaired
and independently rechecked with zero remaining findings.

QA's verdict is **ship with known issues for this bounded macOS graphics slice**.
On macOS 26.6.2 arm64 at 1100 × 760, native drawing and PNG output matched;
three repeat draws produced identical PNG hashes. Changed drawing changed the
pixels, export and native display; valid-size recovery restored the original
hash. Both language/runtime integration tests passed independently. Owned native
processes and fixture ports were cleaned up.

QA also observed one fresh capture refusal, “stale visible page or layout”, after
intentional stale-revision/wrong-epoch refusals; cause and baseline attribution
are unresolved. Dark/narrow layouts, keyboard, cancellation/disconnect, service
restart, remote detectors and GPU profiling remain unverified. The known direct
devtools async-drawing limitation was not independently retested. This is not a
full-release verdict: the repository gate failures described below remain open.

## Graphics

The V9 `measure` example returned:

```text
canvas.getContext('2d'): null
canvas.getContext('webgl'): null
canvas.getContext('webgl2'): null
canvas.getContext('bitmaprenderer'): null
canvas.toDataURL(): undefined
navigator.gpu.requestAdapter(): undefined
typeof document.createElement('div').getContext: function
typeof Path2D: function
```

`getContext` is installed on `HTMLElement.prototype` in
`crates/v9/src/v9/platform/components/install.rs`. Its implementation always returns
null. The `Path2D` methods in `platform/install.rs` are no-ops. The GPU objects in
`platform/chrome_env.rs` and `platform/chrome_shapes.rs` expose interface shapes
without a graphics backend. Those observations explain missing graphics results;
they do not establish why any remote site refuses a request.

Desktop `src/desktop/canvas.rs` uses an AppKit layer-backed view and replays the
Eye display list through CoreGraphics in `canvas/paint.rs`. Layer backing allows
AppKit to cache presentation; Mars has no explicit Metal device, command queue or
GPU compositor. Source inspection cannot establish how much GPU work macOS does
underneath AppKit. `src/runtime/agent/raster.rs` paints headless screenshots into a
CPU-owned pixel buffer. CPU/GPU profiling has not yet been performed.

## Network capture

Command:

```sh
cargo run --offline --manifest-path mars/Cargo.toml -p mars-relay \
  --example fetch -- https://tls.peet.ws/api/all
```

The initial sandboxed connection failed. An approved network-enabled rerun
returned HTTP 200 over HTTP/2. The echo reported:

| Field | Observed value |
|---|---|
| JA4 | `t13d1516h2_8daaf6152771_d8a2da3f94cd` |
| HTTP/2 fingerprint | `1:65536;2:0;4:6291456;6:262144\|15663105\|0\|m,a,s,p` |
| Reduced UA engine | `Chrome/152.0.0.0` |
| Client-hint brands | `Not?A_Brand` 24, `Chromium` 152, `Mars` 152 |
| Accept-Language | `en-US,en;q=0.9` |

The capture contains no paired current Oya reference. It confirms Relay's current
signature, not complete Chrome equivalence. In particular, the advertised but
unimplemented TLS CBC suites and ALPS handling documented in `vendor/rustls/MARS.md`
remain limitations. `D1-fingerprint.md` predates this TLS patch; its older JA4 is
not the result of the current build.

Both targeted Relay library tests passed (one selected test per command):
`the_ja4_is_chromes` and `the_preface_carries_chromes_settings_and_window`.

At this baseline, JavaScript languages came from the coordinator's configured list;
Relay's default language header was hardcoded in `network/web/fields.rs`.
Live detector completion, Google acceptance, graphics parity, and a green full
workspace gate have not been demonstrated by these checks.

## Implementation verification

The subsequent working change supplies a macOS Canvas host and a bounded 2D
slice. The native `/compat/canvas` fixture completed through its **Draw sample**
button: actual rectangle pixels were `204,51,68,255`, cleared pixels were
`0,0,0,0`, text measurement returned `98.935546875`, and the exported PNG decoded
at 300 × 150. Desktop captures showed matching canvas and PNG content. Resize,
zero-size, excessive-size refusal and valid-size recovery completed. WebGL
returned null; WebGPU returned a Promise resolving to null. These are absence
results, not GPU implementations.

Public-surface checks passed: 17 V9 Canvas tests, 22 DOM tests, five native
Canvas tests, five core storage/serialization tests, one agent raster pixel test,
two real-process integration tests and all 160 Relay library tests. The language
integration used `fr-CA,fr` and `ja`, checking both JavaScript and received HTTP
headers. Workspace Clippy passed across all targets/features with warnings denied.
The root TypeScript check passed. The npm gate had 3,632 passing unit tests and
one skipped test; integration reported 26 passing and 13 failing tests, including
missing `OYA_NATIVE_ENGINE` configuration. The initial sandboxed runs also hit
loopback listener permission errors; the reported Relay/npm results came from
approved reruns with socket access. This is not a green full repository gate.

Subsequent review added regressions for head-to-body publication, detached
expando collection, reachable detached trees, WeakMap reachability and shared
dimension parsing. An expanded debug GC stress test exceeded its fixed script
time limit; its release build passed. The `section.align` reflection failure
was independently reproduced on the starting commit `0968fdfc` in an isolated
checkout. The final selected release run passed 77 tests, including 19 DOM-event
tests, one GC stress test and ten reflected-attribute tests; the independently
reproduced baseline attribute failure was explicitly filtered in that run.
Neither result is hidden by the focused Canvas checks.
Reliability review then reproduced a detached listener cycle retaining Canvas
quota. Listener edges now follow target reachability in the same collection
fixed point; unreachable registrations are swept. Both node and generic
EventTarget cycles recover over three serialized requests, while a retained
detached canvas keeps its callback, pixels and abort behavior. The 58-test
Canvas/DOM/event-dispatch repair run passed.
The final release verification after that repair passed 66 tests across native
Canvas, real-process Canvas/languages, V9 Canvas, DOM, event dispatch and GC
stress. Workspace Clippy (all targets/features, warnings denied), formatting
and whitespace checks passed on the repaired change.

### Supported first slice

| Surface | Implemented behavior |
|---|---|
| Context ownership | `HTMLCanvasElement.getContext('2d')` returns the same real context; other context names return null |
| Pixels | Solid `fillRect`, `clearRect`, separate clamped-array `getImageData` |
| Text | Native `fillText` and `measureText().width`, using serif, sans-serif or monospace |
| Export | Actual PNG `toDataURL`; the desktop and screenshot painters consume the same source pixels |
| Reset | Width/height writes clear drawing state and pixels while retaining context identity |
| Limits | 512 pixels per edge, 16 contexts, 4 MiB backing per page, 8 million pixel-work units per execution |
| Recovery | Zero/excessive surfaces have no backing; valid dimensions restore drawing |
| Languages | Configured JavaScript languages and default HTTP language header agree; explicit header overrides remain intact |
| GPU discovery | `navigator.gpu.requestAdapter()` returns a Promise resolving to null |

Named colors are limited to black, white, red, green, blue and transparent,
plus three- and six-digit hexadecimal colors. Full CSS font/color parsing,
paths, transforms, image drawing, complete text metrics, WebGL/WebGPU and a
GPU compositor are not implemented by this slice. The host is macOS-only;
standalone and non-macOS hosts do not claim a working 2D context.

Two separate release-build workloads on this Mac were measured:

- Native fixture: 20 alternating scroll operations, with an inspection before each,
  took 764.73 ms total in a repeat run (407.25 ms in the initial run). The existing
  `ui.display` spans over that repeat interval totaled 1.69 ms across 41 updates;
  `ui.update` totaled 30.99 ms. Those spans do not measure
  all window-server/GPU activity or isolate CPU raster time.
- Headless native runtime: 20 viewport screenshots of the completed fixture took
  59.23 ms total in the final release run (55.88–66.48 ms in earlier runs), including CPU rasterization
  and PNG encoding. This workload is
  different from scrolling and is not a speed comparison between CPU and GPU.

These measurements do not establish a rendering bottleneck or GPU utilization.
Full Canvas, WebGL/WebGPU, a GPU compositor and detector equivalence remain open.
A separate observed limitation remains: invoking the fixture's asynchronous
`drawSample()` directly through `page.devtools` left `img.decode()` pending;
the native click path completed. No baseline attribution was made for that issue.
