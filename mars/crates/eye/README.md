<!-- Eye owns independently reusable document parsing, cascade and bounded render data. -->
# Eye

Eye is Mars's Rust-owned document and rendering library. Parse HTML, compute the supported CSS subset and produce bounded display geometry without launching the Mars desktop app, V9, Relay or service processes.

```toml
[dependencies]
mars-eye = { path = "/path/to/agentchrome/mars/crates/eye" }
```

```rust
use mars_eye::{compute_styles, parse, render};

let document = parse("<h1>Hello Eye</h1><p>Rust-owned layout.</p>")?;
let document = compute_styles(&document, &[])?;
let display = render(&document, 640.0)?;
assert_eq!(display.unit, "logical_points");
# Ok::<(), mars_eye::protocol::EngineError>(())
```

From the repository root, run:

```sh
cargo run --manifest-path mars/Cargo.toml -p mars-eye --example render
```

`parse` builds a validated owned document. `compute_styles` applies inline/embedded CSS and explicit stylesheet strings. `render` returns caller-owned display data; it neither creates a window nor paints a native browser surface. Render context can explicitly supply appearance and viewport height. Unsupported layout rules return typed errors rather than falling back to another engine.

Eye uses shared Mars Core contracts, including inert script projections and text proposals. `script_document` derives a validated projection from the actual document and omits protected input values. Projections and geometry do not grant native control authority. A host remains responsible for authorization, networking, script execution, committing candidates and presenting a surface.

Script structure changes arrive as a core `DomChange` log. `apply_changes` replays it on one candidate through the core placement helpers, so Eye and V9 build the same tree, and any refused change leaves the input committed. `Fragments` implements the core `FragmentParser` seam with Eye's own parser in fragment mode. That mode refuses scripts, resources, controls, tables and handlers, and accepts an inline SVG subset (`svg`, `g`, `path` with M/L/H/V/Q/Z, `use`, `rotate`). The SVG subset renders as one `image` item carrying absolute `vector` paths.

On macOS Eye uses OS text/font primitives for shaping and measurement without launching the Mars application shell. Other platforms retain the documented limited deterministic text fallback. This is a bounded development engine, not full Blink or web-platform compatibility. See [Mars's supported subset](../../README.md), [upstream research](../../UPSTREAM.md) and [third-party notices](../../THIRD_PARTY_NOTICES.md). Eye is an unpublished path package; no stable crates.io API is promised.
