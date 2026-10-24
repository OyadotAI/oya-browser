# Mars

Mars is Oya's Rust browser, being built for people and agents. Relay networking, Eye
documents/layout/rendering, the V9 JavaScript runtime, native window and agent tools are
implemented in Rust, with the engines running as supervised services. It uses no Chromium,
Blink, V8, CDP, WebView, injected agent scripts or existing browser engine.

The replacement target is a complete everyday browser with native human controls and native
agent tools over the same page. This build is in development and does not yet match Electron's
browser compatibility. The implementation-status sections below describe what works today and
what remains to build; they do not narrow the product goal.

## Packages

| Package | Display name | Use it for |
| --- | --- | --- |
| `mars` | Mars | Compose the browser, native shell and agent runtime |
| `mars-eye` | Eye | Parse documents and produce bounded layout/render data |
| `mars-v9` | V9 | Evaluate the supported JavaScript subset through explicit host data |
| `mars-relay` | Relay | Perform bounded networking through explicit request/policy APIs |
| `mars-mdp` | MDP | Serve the Mars DevTools Protocol (CDP-compatible) front door over native operations |
| `mars-core` | Mars Core | Shared bounded data contracts and errors |

Use each engine as a Rust library without launching the Mars desktop app or sibling
service processes. The packages are unpublished path dependencies; their development APIs
do not promise compatibility across releases. The examples below assume your consumer
project is beside the `agentchrome` checkout. Adjust the dependency path for another layout.

### Use Eye

Add this dependency to your consumer's `Cargo.toml`:

```toml
[dependencies]
mars-eye = { path = "../agentchrome/mars/crates/eye" }
```

```rust
use mars_eye::{parse, render};

fn main() {
    let document = parse("<p>Hello from Eye.</p>").expect("supported HTML");
    let display = render(&document, 800.0).expect("bounded layout");
    assert!(!display.items.is_empty());
}
```

Eye implements the bounded document/CSS/layout subset below. Its macOS font and drawing
primitives use OS frameworks without creating a window or event loop; other platforms retain
the documented limited fallback. It does not depend on V9, Relay or the Mars application.

### Use V9

```toml
[dependencies]
mars-v9 = { path = "../agentchrome/mars/crates/v9" }
```

```rust
use mars_v9::load;

fn main() {
    let execution = load("let answer = [...[40], 2].join(',')", &[])
        .expect("supported bounded JavaScript");
    assert!(execution.state.globals().contains_key("answer"));
}
```

V9 accepts explicit inputs and returns candidate VM state, storage and document changes.
Callers own validation and publication; these values grant no native browser authority.
It implements the supported language subset below without requiring a renderer or network.

### Use Relay

```toml
[dependencies]
mars-relay = { path = "../agentchrome/mars/crates/relay" }
```

```rust
use mars_relay::{WebRequest, policy};

fn main() {
    let request = WebRequest::get("https://example.com/");
    request.validate().expect("bounded request fields");
    let url = policy::web_url(&request.url).expect("HTTP(S), no credentials");
    assert_eq!(url.scheme(), "https");
    assert!(!policy::public_address("127.0.0.1".parse().unwrap()));
}
```

This example validates request syntax and a destination-policy rule without DNS or network
I/O. Actual connections additionally resolve and validate every destination and pin an accepted
address. Relay owns bounded HTTP/1.1/TLS operations; importing it grants no page/agent network
permission and starts no browser services. The network subset and restrictions appear below.

## Performance and footprint targets

The long-term target is for V9 to use no more than one tenth of V8's memory, run comparable
JavaScript work in no more than one tenth of its time, and occupy no more than one tenth of its
runtime size. For the complete native browser, the corresponding target is one tenth of the
current Electron browser's memory, elapsed time for equivalent work, and installed size. These
are unmeasured goals, not claims about this build. Browser comparisons count only when both
products complete the same workload with equivalent results; current compatibility is still in
development. The architecture document defines the comparison method and workload boundaries.

## Run Mars

From the repository root, with Rust 1.94 or newer:

```sh
cargo run --offline --manifest-path mars/Cargo.toml --bin mars -- desktop --url demo
cargo run --offline --manifest-path mars/Cargo.toml --bin mars -- desktop --url forms
cargo run --offline --manifest-path mars/Cargo.toml --bin mars -- desktop --agent --url demo
cargo run --offline --manifest-path mars/Cargo.toml --bin mars -- desktop --mcp --url demo
cargo run --offline --manifest-path mars/Cargo.toml --bin mars -- demo
cargo run --offline --manifest-path mars/Cargo.toml --bin mars -- agent
```

The window requires macOS. Click **Open demo**, enter a name and click **Say hello**. V9 reads
the input and changes the greeting. Follow the second-page link, then try Back, Forward,
Reload, scrolling and resizing. **Try choosing an option →** opens the separate synthetic
`/compat/choose-option` case: enter `2`, `en`, `ja` or `02`, then choose **Find option**.
Its ordinary semicolonless V9 script composes the model with array-literal spread, trims the input,
compares numeric IDs with `==`, uses `some` for the match branch and `find` for the selected entry. Empty input and no match have separate
literal messages. Both output paragraphs publish together after a successful action.
This owned case does not rewrite example.com or establish its full compatibility.
The address field accepts supported public HTTP/HTTPS pages.
Unsupported syntax produces an error and retains the last successfully committed page.

Offline builds need the locked crates cached. On a fresh machine, omit `--offline` once to
download ordinary Rust dependencies; no browser engine is downloaded. AppKit supplies native
window/widgets/font shaping and drawing. Rustls supplies verified TLS, webpki-roots its trust
roots, url URL syntax, serde serialization, and libc cancellable OS pipe polling. Oya owns
HTTP framing, parsing, document state, layout, language execution and native agent authority.
The macOS desktop stores supported session cookies in the system Keychain; clear them from the
Mars menu with **Clear Saved Cookies…**. This profile does not import Electron cookies.
On an ordinary launch, the desktop restores a bounded ordered list of HTTP(S) tabs after secure
profile recovery succeeds. Restored tabs use GET and start under human control. An explicit `--url`
launch takes precedence and leaves the saved session unchanged for that run. Session URLs are
stored in a separate Keychain item; query and fragment data are hidden from agent page views.
If the final tab-session write fails on close, the desktop offers retry, keep-open, or explicit
close without saving.

## Native agent connection

### MCP stdio

`desktop --mcp` opens one new visible native window and speaks MCP **2025-11-25**
over stdin/stdout. This is an explicitly pinned initialization-based revision,
not the latest MCP revision. Use either `--mcp` or `--agent`; diagnostics go to
stderr. Both connections start under human control. Choose **Allow agent** in
the native toolbar, or add `--agent-control` for an explicit operator startup grant.

The terminal command above demonstrates launching the process. An MCP client must
launch it and own its stdin/stdout. For clients using the common `mcpServers`
configuration format (other clients use different formats):

```json
{
  "mcpServers": {
    "mars": {
      "command": "mars",
      "args": ["desktop", "--mcp", "--url", "demo"]
    }
  }
}
```

The command must resolve in your MCP client's environment. If it does not, use the
absolute path to your built `mars/target/debug/mars` executable. This
starts a new window; it does not attach to an existing preview or install client configuration.
If Cargo uses a custom target directory, use that directory instead of `mars/target`.
The MCP `serverInfo.name` remains `oya-native`, and existing protocol operation/tool names,
saved-profile namespace and Keychain service/account identities remain unchanged. Existing local
saved data needs no migration; the Mars name does not select a new secure store.

Initialize, discover tools, call `runtime_capabilities` and `tab_list`, then inspect
the chosen tab with `control_status` and `page_inspect`. Inspection works before
a grant. After the person chooses **Allow agent**, read ownership and the page
again. Every mutation uses those exact native tab/page/node/revision/epoch values;
the adapter neither refreshes stale preconditions nor retries mutations. Finish
with `control_release`.

The fixed catalog contains `runtime_capabilities`, `tab_list`, `control_status`,
`page_inspect`, `page_layout`, `page_navigate`, `page_type`, `page_click`,
`page_set_checked`, `page_select`, `page_submit`, `page_back`, `page_forward`,
`page_reload`, `page_scroll` and `control_release`. Discovery supplies strict
argument schemas. Runtime capabilities also describe operations outside this MCP
subset; only `tools/list` defines the exposed MCP tools. No control-grant,
arbitrary-script, screenshot, window-management or shutdown tool is exposed.

Tool results contain native JSON as both `structuredContent` and serialized text.
Native failures retain the complete native error with `isError: true`; protocol
errors remain JSON-RPC errors. Password and hidden-value projections remain
protected. Cancellation suppresses a late reply and revokes only a matching
mutation's existing agent epoch. It cannot undo an already committed operation or
transmitted submission. A fresh grant is always a human action.

MCP disconnect, output failure or terminal session failure immediately revokes
ownership, cancels pending publication and closes only its launched window and
services through bounded noninteractive cleanup. Committed data remains preserved.
The MCP session ends and its launched window closes. Start a new connection to open
a new window. Native NDJSON retains its keep-window-open disconnect behavior below.
**Clear Saved Cookies…** and **Clear Saved Site Storage…** are disabled in MCP
windows so a confirmation cannot block disconnect cleanup. Use an ordinary native
window for those confirmed profile actions. **Retry Saved Cookies** remains available.

### Native NDJSON

`desktop --agent` connects stdin/stdout to the **visible window**, using versioned NDJSON.
Diagnostics use stderr. One request receives one correlated response containing exactly one
of `result` or `error`. Frames and parameter schemas are bounded and strict. The headless
`agent` command retains its local-only single-owner contract.

```json
{"version":2,"id":"discover","op":"runtime.capabilities","params":{}}
{"version":2,"id":"control","op":"control.status","params":{}}
```

The native toolbar's **Allow agent** grants control; **Take control** immediately revokes queued
and in-flight agent mutations. Agents can inspect without control and release control, but
cannot grant it to themselves. `--agent-control` is an explicit operator startup grant for
unattended sessions. Every desktop agent mutation also requires `control_epoch` from
`control.status`. A null-id service_unavailable reply reports terminal worker failure; treat outstanding requests
as failed and restart instead of replaying. On EOF or output failure, ownership returns to the person and the window stays
open. Closing the window cancels pending work, kills/reaps all engines and joins its tasks.

| Operation | Parameters, excluding desktop mutation `control_epoch` |
| --- | --- |
| `runtime.capabilities`, `tab.list`, `control.status` | `{}` |
| `tab.open` (human only) | `{}` |
| `tab.select`, `tab.close` (human only) | `{"tab_id":"tab-1"}` |
| `page.inspect`, `page.layout`, `page.back`, `page.forward`, `page.reload` | `{"tab_id":"tab-1"}` |
| `page.navigate` | `{"tab_id":"tab-1","url":"https://public.example/page"}` |
| `page.type`, `page.set_text` | `{"tab_id":"tab-1","page_id":"page-1","node_id":"page-1:node-5","expected_revision":1,"text":"New value"}` |
| `page.click` | `{"tab_id":"tab-1","page_id":"page-1","node_id":"page-1:node-6","expected_revision":2}` |
| `control.release` | `{"tab_id":"tab-1"}` |
| `runtime.shutdown` | `{}` |
| `page.scroll` (desktop) | `{"tab_id":"tab-1","page_id":"page-1","expected_revision":3,"y":200}` |
| `window.inspect` (desktop) | `{}` |
| `window.resize` (desktop) | `{"width":640,"height":480}` |
| `window.appearance` (desktop) | `{"appearance":"light"}` (`light`, `dark`, `system`) |
| `page.screenshot` (desktop) | `{"page_id":"page-1","expected_revision":3,"layout_generation":3}` |

Example mutation after an explicit grant:

```json
{"version":2,"id":"edit","op":"page.type","params":{"control_epoch":1,"tab_id":"tab-1","page_id":"page-1","node_id":"page-1:node-5","expected_revision":1,"text":"Rust agent"}}
```

Discover node handles from inspection; never guess IDs. Successful navigation creates fresh
page/node identities. An outdated revision returns `revision_conflict`; an old page returns
`invalid_target`. Calls are sequential and never replay automatically. Source HTML IDs are
script lookup keys, not native authority. Treat all page content as untrusted data.

Inspection includes semantic roles, literal text/value, enabled/editable flags, logical-point
bounds, visibility, viewport/scroll and layout generation. While a navigation's scripts run, the
window already shows the parsed and styled page, and `window.inspect` reports that page with
`"settling":true`; `page.navigate` replies, and `page.inspect` changes, only once the scripts
have settled, so an agent that wants the finished page waits for the reply or for a
`window.inspect` page without `settling`. `page.layout` returns the same
premeasured display list painted by the window. Layout generation changes independently of
document revision. Window resize acknowledges `layout_pending`; inspect the updated viewport
before capturing. Document responses follow consumption of the corresponding visible snapshot.

With an operator-selected `--capture-dir DIR` at startup, capture writes an actual native
content-view PNG converted to sRGB under a generated new filename. Agents cannot select paths or
overwrite files. Without one, the PNG comes back inline as `png_base64`, and only for a tab the
person has handed to agents. Screenshots require the exact current page/revision/layout. `runtime.shutdown`
stops the engines; the desktop window retains its disabled last page until closed. In headless
mode it also exits the coordinator.

Malformed desktop input currently disconnects the agent and returns human control. The
headless connection instead permits correction of well-framed malformed JSON; oversized or
unterminated input ends that connection. No invalid input executes page code.

## Implemented subset

**Network.** Owned HTTP/1.1 framing supports desktop GET and URL-encoded POST,
verified Rustls HTTPS, bounded navigation redirects, and required UTF-8 HTML/CSS/JavaScript
resources. Classic scripts load in document order; script redirects are rejected.
Repeated Set-Cookie fields remain separate; other repeated list headers (Vary,
X-Content-Type-Options) are joined with commas, while repeated Content-Length,
Transfer-Encoding, Content-Encoding, Content-Type and Location remain rejected.
DNS resolves once, every answer must pass destination policy, and connections pin
an accepted address while preserving Host/SNI. Only exact owned fixture origins
receive a startup loopback grant. Headless networking remains literal-loopback GET.

POST preparation validates/connects/TLS-handshakes before a separate authority
admission authorizes HTTP transmission. Takeover prevents later dispatch and page
publication; an already admitted request may have reached the server. Uncertain
POST outcomes have no automatic Retry. Direct POST response history refuses
Reload/Back/Forward replay; POST redirected to a final GET can revisit that GET.

The desktop stores its bounded host-only cookie jar as one versioned macOS Keychain
item. Session-only cookies persist across app restarts; expiry and server deletion
still apply. **Clear Saved Cookies…** in the Mars menu removes saved and live
cookies after confirmation. If secure storage cannot be read, cookie-bearing
navigation is blocked until retry or clear succeeds. If secure storage cannot persist
the recovery lock after a failed clear, the browser warns that cookies may remain and
must stay open until a clear succeeds. Cookie values never enter agent inspection or
diagnostics. Headless runs remain in memory only. Cookies check
paths, Secure/prefix requirements, SameSite, expiry/deletion and insecure replacement.
Domain cookies are refused; same-site matching conservatively requires the same
scheme and exact hostname. This does not provide Electron profile import or general
production session compatibility. Cookie commits require current authority,
independently of subsequent document parsing. Password-bearing GET chains omit
query/fragment from surfaced URLs.

**Document/forms.** Owned bounded tokenization supports comments, case-insensitive
names, single/double/unquoted attributes, common character references and omitted
wrappers/selected optional closings. Inert attributes remain data. This is not a
complete HTML5 tree builder. Static tables support captions, groups, rows, cells,
and basic header associations; spans, nested tables and advanced table CSS remain
unsupported. Other unsupported embeds, modules,
`integrity`/`crossorigin` scripts and http-equiv values other than content-type and
X-UA-Compatible fail explicitly. Up to eight inline or external classic scripts run blocking,
then `defer`, then `async`, each in document order, and are supported within the existing V9 language and source budgets.
Static PNG and JPEG resources load through the governed network service, with
bounded encoded/decoded sizes, intrinsic geometry, native AppKit painting,
agent image roles/bounds and alt-text fallback on ordinary resource failure.
SVG, animation, background images, responsive selection and lazy loading remain
unsupported.

Forms support labels, text/password/hidden values, checkbox/radio state, one-option
select, textarea, disabled/read-only controls and submitters. Submission preserves
field order/repeated names, excludes unsuccessful controls, normalizes newlines,
and performs GET or URL-encoded POST. Only implemented required constraints are
validated. Password/hidden values are omitted from inspection; protected values
are unavailable to the current V9 DOM-read seam.

**CSS/layout.** Required inline/linked styles use an owned basic cascade for
simple type/class/ID/descendant selectors, source order/specificity/importance,
text styles, colors, spacing, widths, borders and visibility. Owned block/inline
flow uses native font metrics; the same display list supplies paint and agent
bounds. Unsafe unsupported selectors/layout/conditions refuse publication rather
than exposing hidden controls. Some decoration produces bounded diagnostics.
HTML and BODY retain separate attributes and computed formatting records without
creating native node IDs or actionable wrapper targets. Inheritance includes the
font, text alignment, line height and offered color schemes. The supported `font`
shorthand requires a size and native `system-ui`/`sans-serif` family list, accepts
an optional supported weight and slash line height, and resets omitted supported
longhands. Unitless line-height factors remain relative when descendants change
font size; percentages and supported lengths inherit as computed point lengths.
Nonnegative `em` and `vh` lengths resolve from the computed font and actual page
viewport. BODY and ordinary block containers support horizontal automatic margins,
bounded content-box width/min/max-width, padding and per-line text alignment.

`color-scheme` supports normal, light, dark, light dark and dark light. Literal
`light-dark()` pairs select the element's used scheme. A root solid background
covers the whole canvas; a transparent root permits BODY background propagation
without double painting. Distinct body backgrounds/borders use non-target
presentation geometry. Matching default text and link colors preserve readability;
an explicit color declaration on a link still wins. Unauthored documents retain
native semantic colors and the existing 32-point inset/960-point reading width.
Winning BODY box declarations replace that legacy inset once. Paragraph defaults
and margin collapsing are not expanded into general browser UA stylesheet support.
This remains a partial CSS implementation with compatibility limitations.

The page retains its resolved stylesheet sources. Resize, native appearance changes
and tab activation restyle and reflow the current document with one checked page
viewport/appearance context, without fetching resources or rerunning scripts.
Presentation changes preserve page/node identity, revision, edited fields and VM/
Storage state, while advancing layout generation. A failing candidate keeps the
previous context and display. Native painting, controls and inspection consume the
same resolved colors, line advances and boxes.

Owned row/column flex layout supports row wrapping, pixel gaps, bounded growth/
shrinkage and start/center/end alignment, space-between justification and stretch.
Explicit content-box and border-box sizing includes padding and border correctly;
width, max-width and automatic/zero/pixel min-width constrain flex items. Flex,
gap, margin and padding shorthands participate in declaration-order cascade.
The forms demo uses two 289-point field columns at a 1100-point viewport and
wraps to one 550-point column at 640 points, retaining the same control identities.

Height properties are not supported. Auto-height columns derive their main extent
from content; a genuine assigned cross-axis stretch can supply a definite height.
Unresolved percentage bases use content sizing, and automatic column minimums
prevent collapsed content. Grid, inline-flex, reverse directions, column wrapping,
order, align-self/align-content/baseline, flex-item/table auto margins, percentage
gaps, positioning, writing modes and media/container queries remain unsupported. Flex containers with
direct non-whitespace text and table/flex combinations are rejected until their
structural semantics are implemented. Wrapped text elements remain supported.
One request-local measurement/wrapping cache and shared work budget bound intrinsic
sizing, dry runs and final placement. Every actual native measurement passes through
a checked seam that charges the submitted Unicode scalars before shaping. Wrapping
retains measured widths, uses forward UTF-8 boundaries and bounded prefix probes,
and avoids remeasuring growing merged lines. Nonfinite, over-budget or unsupported
layout rejects the candidate page.

Static readable text has separate UTF-8 byte limits: 16 KiB per stored node and
512 KiB across the document. The parser keeps complete source text in its original
node; wrapping creates paint lines, never replacement DOM nodes. Editable values,
control captions/labels, titles, attributes and script-generated document writes
retain the 2,048-byte limit. V9 string/source/work and wire budgets are unchanged.
A maximum-size static input can still fail its measurement, display or serialized
response budget before publication.

The native validation/error banner measures its complete message below both the
address toolbar and tab row, reserves a separate fixed action row and reduces the
page viewport by the resulting height. Messages taller than the available banner
scroll internally while keeping Retry/Dismiss and at least 120 points of page
viewport available. Existing retry/replay policy and inline field errors remain.

Frozen [reading fixtures](fixtures/reading/README.md) separate compatibility stages.
The unchanged example.com page and its frozen `/s.js` commit as a real navigation
through the supervised services (`tests/example_reading.rs`). The script's
`insertAdjacentHTML`, `querySelectorAll`, `prepend`, reflected `lang` and
`navigator.languages` reads run in V9 against Eye's fragment parser, which the app
passes in at the service edge. V9 returns an ordered structural change log (text,
insert, move, set attribute) that Eye replays atomically. Navigation fails if the
structure digest V9 computed differs from Eye's applied document. Script-inserted
inline styles join the page's stylesheets. The icon paints as native vector paths,
the Arabic paragraph shapes right to left, and the SVG stays out of agent targets.
`navigator.languages` comes from the OS preferred-language list (override with
`MARS_LANGUAGES=fr-FR,en`). The unchanged httpbin HTML exercises its complete
3,588-byte paragraph through the real services. Broader page compatibility,
script modules and full web-platform behavior remain unfinished.

**V9.** The [source map](UPSTREAM.md) pins the V8 parser and bytecode-generator
routines used to guide this Rust slice. The bounded language includes typed
numbers, strings, booleans, null and undefined; object and array literals;
own-property reads and writes; `let`/`const`/`var`; first-class functions, anonymous/named
function expressions, directly nested declarations, callbacks and lexical closures
with shared captured bindings; parameters, returns and recursive calls; `if`,
`while`, `break` and `continue`;
arithmetic, comparisons and short-circuit logic; explicit input reads and
`textContent` proposals, with automatic semicolon insertion in the supported classic-script grammar.

Completed supported statements may end at EOF, before `}`, or at a permitted
inter-token line break. LF, CR, CRLF, U+2028 and U+2029 in whitespace or comments
count as boundaries; line changes inside a string literal do not. Calls, indexing,
members, binary operators and assignments still continue across newlines when the
expression grammar permits them. A newline immediately after `return` makes it a
bare return; unlabeled `break`/`continue`, postfix updates and arrow heads respect
their restricted line-break rules. Both standard-for header separators remain
explicit, and insertion cannot repair incomplete expressions, delimiters or missing
control-flow bodies. Inline named-call handlers keep their existing punctuation
and token-accounting policy. No synthetic Program token or empty statement is
allocated for an implicit terminator; existing limits and unsupported syntax remain.

`let` cells are mutable; `const` cells require an initializer and refuse reassignment.
Bindings exist from entry to their lexical block and fail before initialization,
including in their own initializer. Each invocation sees its own blocks and the
defining lexical environments and page globals rather than its caller's local variables.
Sibling closures share captured cells, while separate factory calls are independent.
Named function expressions keep their recursive self-name private. Assigning that
self-name remains explicitly unsupported. Functions can be assigned, passed,
returned, stored in objects/arrays and invoked through variables, parentheses,
returned functions and own-property member expressions. Member calls preserve the object receiver. Ordinary functions use the call-site receiver;
arrows retain their defining `this`, including after return and serialized native events. Function identity and truthiness
are supported; function-to-primitive conversion is not. Inline handlers retain
the named-call front door, including positional arguments; they cannot introduce
function literals or general expression callees. Boolean, null and undefined
values retain their types. Supported comparison/logical operators are `==`, `!=`, `===`, `!==`,
`<`, `<=`, `>`, `>=`, `!`, `&&` and `||`; the latter two short-circuit and return an
operand value. Blocks, `if`/`else`, `while`, `break` and `continue` share the bounded
execution budget. Even empty infinite loops stop explicitly. Standard
`for (initializer; condition; update)` loops admit omitted clauses, one lexical
`let`/`const` declaration, a `var` declaration list or a supported expression/assignment in the initializer,
and supported expression/assignment updates. `let` headers get fresh shallow
per-iteration cells while object identities remain shared. `continue` runs the
update; `break` and return skip it. Lexical header names disappear when the loop finishes;
`var` headers keep the containing function/script cell, shared by every iteration.
`for…in`, `for…of`, `for await`, multiple lexical declarators,
logical/nullish/bitwise/exponentiation assignments and optional chaining remain unsupported.
Comma (sequence) expressions evaluate their operands left to right and return the last one;
each operand charges the shared operation budget.

Abstract equality (`==`, `!=`) supports Number, raw UTF-16 String, Boolean, Null and
Undefined values, plus existing object/function/host identity paths. Null and undefined
compare equal to each other; Number/String comparisons use the existing numeric-string
conversion and Boolean operands normalize to 0 or 1. Same-type values keep strict equality
rules, including NaN unequal and signed zeros equal. Two object-like values compare by
stable identity without calling their methods; different object/function/host identities
are unequal. Nullish values compare unequal to other types without object conversion.
A String/Number against an object-like value, including after Boolean normalization,
explicitly refuses the unavailable object-to-primitive conversion. It does not call own
`valueOf`/`toString` methods or silently substitute NaN. Operands evaluate once, left to
right; `!=` negates only a successful comparison. Strict `===`/`!==` behavior is unchanged.
These are bounded supported semantics; source-access limits and the terminal-false
inference are recorded in [UPSTREAM.md](UPSTREAM.md).

Simple `var` declarations belong to the containing function or classic script,
including declarations inside blocks, loops and unreachable branches. All their
cells exist with an undefined value before execution. Optional initializers run
left to right; a declaration without one preserves the current value. Repeated
vars share their cell with parameters and supported direct function declarations.
Functions initialize before statements; the last direct duplicate supplies the
function value until an executed initializer or assignment replaces it. Lexical
name conflicts remain early errors, and nested functions own their own vars.
Each classic script instantiates when reached; later scripts are not hoisted into
earlier execution. Every declarator consumes the existing declaration budget,
including repeated names. The page permits at most 64 distinct global object
bindings separately from the existing 64 lexical-global limit.

Simple assignment expressions; arithmetic compound assignments (`+=`, `-=`, `*=`,
`/=`, `%=`); prefix and postfix numeric updates (`++`, `--`); conditional expressions;
`typeof` and `void` are supported. Assignment targets remain supported identifiers
and own-property member references. Plain assignment returns its assigned value
and associates right to left. Compounds retain the original receiver/key and old
value before running the right-hand expression. Updates convert supported
primitives to Number; prefix returns the new Number and postfix the converted old
Number, including negative zero. Object/function numeric conversion stays
explicitly unsupported. Existing const, TDZ, strict, protected-global, array and
readonly function-metadata store checks still apply.

Conditional expressions evaluate one arm. `typeof` reports supported value types
and returns `"undefined"` for an unresolved ordinary identifier, while TDZ,
implicit-arguments and unavailable host values still fail explicitly. `void`
evaluates its operand once and returns undefined. Postfix updates cannot cross a
line break, including one inside a comment. Inline handlers retain the named-call
boundary. Standalone source-only APIs keep their literal DOM read/write grammar;
projected page execution uses the document bridge described below. All expression
children, including call arguments and unselected branches, participate in source
depth validation; only executed operands consume the runtime work budget.

String primitives support UTF-16 `length` and indexed reads, `trim()` and literal-
separator `split()`. Indices return one code unit, preserving lone surrogates;
out-of-range indices return undefined. Trimming uses ECMAScript whitespace and
leaves interior text unchanged. String stores are ignored in non-strict code and
fail in strict code. Splitting preserves empty fields, supports an omitted or
undefined separator, and converts primitive limits with ToUint32. Empty separators
split into UTF-16 units. Requested limits are not silently clamped to engine caps:
actual oversized results fail, as can the independent aggregate property budget.

Array literals admit spread of actual owned arrays and primitive UTF-16 strings,
for example `[first, ...options, ...'A😀B']`. Each operand evaluates once in source order;
one spread finishes copying before the next operand evaluates. The destination array
is fresh and copies references shallowly: object/function/host identity and aliases
survive. Sparse source positions become present undefined entries, rather than holes.
The source array is unchanged. Each owned-array copy captures its length after its
operand completes; this is an equivalent implementation inference for the supported
hook-free copy, not a general iterator contract.

String spread emits code points: a valid UTF-16 surrogate pair forms one two-unit string
element; lone surrogates remain one-unit strings in VM state. Combining marks stay separate;
there is no grapheme segmentation or normalization. String `.length`, indexed reads and
`split('')` keep their existing code-unit semantics. A later DOM write of an unmatched
surrogate retains its existing explicit UTF-8 boundary refusal.

Every spread-produced element consumes work before copying/allocation, and length,
shared properties and retained-string capacity are checked throughout construction.
Source and destination entries share the existing independent limits; fitting the source
array limit does not guarantee its copy can fit. No intermediate list of all string code
points is built and no collection occurs while construction temporaries are live.
The existing policy allocates one empty destination before the first operand; at full
heap capacity this can fail before that operand runs. This differs from V8's reviewed
leading-spread fast path and is not a claim about matching allocation-failure order.
Any later failure discards the complete candidate, including prior script/Storage/DOM edits.

Only owned arrays and primitive strings supply this spread path. Null, undefined, Number
and Boolean operands fail as noniterable values; ordinary objects, functions and host
references fail with explicit unsupported iteration. The relevant ellipsis retains its
execution error location; errors inside operand evaluation keep their original location.
Custom/generic Symbol.iterator, iterator objects/cleanup, NodeList iteration, literal
elisions, call/new/object spread, rest and destructuring remain unsupported. Recognizing
ellipsis does not introduce any of those capabilities or a general iterator protocol.

Owned arrays support `map`, `filter`, `join`, `some` and `find`. Callback methods
snapshot the initial length and read each current value immediately before invocation,
so future changes and filled holes are observed and later appends beyond that length are
ignored. Map, filter and some skip absent entries; find visits them as undefined, even
after the array shrinks. Present undefined values are visited by every callback method.
Map preserves holes; filter packs the original visited values whose predicates are truthy.
Some stops at its first truthy predicate and returns true, or false after exhaustion.
Find stops at its first truthy predicate and returns the value captured before the
callback, or undefined after exhaustion. A matching hole therefore returns undefined
and still stops. Some/find allocate no result array.

Callbacks are validated even for empty arrays and receive value, numeric index and the
original array, with an optional receiver. Ordinary strict/sloppy, arrow and bound callback
receiver rules reuse the same callable dispatcher and primitive-boxing limits. Every
original index costs work, including holes; callbacks and nested traversals share the
existing operation and call-depth allowances. Join emits empty fields for holes, null and
undefined and converts supported primitive separators and elements. These methods are
first-class functions with stable identity, name/length metadata and existing `call`,
`apply`, `bind` and event serialization. Own array properties shadow supplied methods,
including properties whose value is undefined.

No regex separators, boxed primitives, general String/Array constructors or
prototype APIs, generic borrowed array-like receivers, object coercion or
asynchronous callbacks are supported. Nested arrays, objects, functions and host
values cannot be coerced by join. Unsupported cases fail explicitly; operation,
string, heap, property and wire limits still apply to the complete candidate.
The greeting trims only its processed name, preserving the person's entered text.

The native runtime supplies a real document projection to V9. `getElementById`
returns document-scoped element references or null; references can be retained in
closures and compared by identity. `.value` reads current permitted field text,
and `.textContent` writes use the existing document validation and transaction.
Secret input values remain unavailable. Unknown host properties, primitive
coercion of host references and unsupported writes fail explicitly.

`addEventListener("click", callback)` and `removeEventListener` support callable
listeners on enabled, visible, interactive buttons with explicit `type="button"`
and no command/popover behavior. Missing/default, submit and reset button types
are not eligible. Null/undefined callbacks are harmless; other listener objects
are unsupported. Options must be omitted or undefined; explicit boolean or
object options are unsupported. Duplicate callback registrations are suppressed.
Inline handlers run first, followed by the listener snapshot in registration
order. Removal suppresses a pending callback; additions and remove/re-add wait
until the next click. Human and agent clicks use this same transaction.

Listeners receive a bounded event with readonly `type`, `target`, `currentTarget`
and `defaultPrevented`, plus `preventDefault()`. Ordinary callback receivers are
the button; arrows and bound functions keep their established receiver semantics.
Retained events have null `currentTarget` after delivery. There is no bubbling,
capture, synthetic dispatch, document/window listener, other event type or
form-submit listener integration. A callback error rejects the complete candidate,
including earlier listener, Storage and document changes; this differs from a
complete browser's exception-reporting behavior.

Text replacement updates subsequent queries immediately. Retaining a reference,
event target or registration for a removed child is explicitly unsupported and
rolls the candidate back; detached-node semantics are deferred. Up to 64 listeners
and 64 retained event records share existing graph, work and state limits.
The public `load_document_scripts`/`click_document_scripts` APIs require a real
`ScriptDocument`. Existing `execute`/`load`/`event` APIs retain their source-only
behavior and do not synthesize a document from source.

Arrow functions support `name => expression`, `() => expression` and simple
parenthesized parameter lists, including a trailing comma. Concise expression bodies
return their value implicitly; block bodies use explicit `return` and otherwise
return undefined. Nested arrows capture defining lexical cells and ordinary functions
can coexist in the same script. No line break is allowed before `=>`; line breaks
after it are allowed. Arrow recognition uses assignment precedence, so a binary
operand needs grouping, for example `false || (x => x)`.
Function kind comes from parsed source, never a supplied graph field. Arrows create
no own `this` or implicit `arguments` binding; lexical `this` is captured from the defining context. Explicitly declared lexical/parameter
`arguments` values work; implicit arguments objects remain unsupported. An ordinary
function's `var arguments` is explicitly unsupported unless an explicit parameter
or direct function declaration supplies that binding; arrows may own such a var.
An ordinary activation that would supply an implicit arguments object blocks
fallback to outer cells; a same-activation explicit binding is resolved normally.

An exact unescaped `"use strict"` or `'use strict'` expression statement in a
script/function directive prologue activates strict mode, with an explicit or
permitted implicit terminator. Escaped, grouped or later strings do not; a following
call/member continuation prevents the string from being a completed directive. Functions
inherit their defining source mode, including arrow block directives; each classic
script starts its own mode. Strict ordinary detached calls receive undefined;
non-strict detached calls receive the page-global object. Top-level `this` remains
that object even in strict classic scripts. Restricted strict identifiers are early
errors, including in unreachable bodies.

One page-owned ordinary heap object represents the supported script-global receiver.
`globalThis` resolves to it unless lexically shadowed. Root var/function names read/write
the same cells through identifiers and receiver properties; lexical `let`/`const`
cells are absent from the object. Other page-authored own properties resolve through
bare identifiers, and unresolved non-strict assignments create properties. Strict
unresolved assignments fail. The global object is always a collector root. Arrow
captures and invocation receivers are source-shape-validated references and share
existing string/state/allocation budgets.

This object is not a complete Window/WindowProxy. `window` retains only the fixed
Storage facade. Global native-host properties and writes to protected names
(`globalThis`, `undefined`, `NaN`, `Infinity`, `window`, `document`, `fetch`, `eval`,
`require`, `process`, `localStorage`, `sessionStorage`) fail explicitly as unsupported,
as do root var/function declarations replacing them. Lexical shadowing remains separate.
No native shell, agent handle or network authority is reachable through receivers.
Primitive receiver boxing, arbitrary prototype replacement and broader reflection remain unsupported.

The demo's **Add one** button retains a numeric counter across events and switches
to **Target reached.** at three. A standard `for` loop computes the displayed sum
from one to the current count; **Reset counter** restores zero. These use the
page's own Rust-executed script and ordinary native document updates.
The script uses `count++`, `i++`, `sum += i` and a conditional branch value.

The **Methods and saved callbacks** greeting form calls an ordinary method on an
object controller. The returned arrow reads and writes the controller through `this`,
while accepting an input-processing callback. Its count survives separate **Say hello**
events after the factory and method return. The count is independent of **Reset counter**;
reload creates a fresh controller. Empty names default to “world”. The controller
saves a bound ordinary method with a greeting prefix. Its registered click callback
reads cached element references with current input values and uses `apply` for
inputs and `call` for name processing; a local arrow captures the controller receiver.
The saved callback uses a script-global `var`; an earlier read observes its hoisted
undefined value. Local var redeclarations retain the processed name without changing
the visible greeting journey.

Function values expose first-class `call`, `apply` and `bind` through a shared,
callable Function prototype. Extracted methods retain their identity and require a
callable method receiver. Own data properties can shadow inherited methods; changing
the supported prototype method properties affects subsequent lookups. `Function`
resolves to the canonical constructor unless shadowed or replaced; dynamic function
construction remains explicitly unsupported. Calling `Function.prototype` returns
undefined. No native handles are reachable through this function layer.

`call` forwards an explicit receiver and positional arguments. `apply` accepts null
or undefined for an empty list, supported arrays (including holes), and ordinary
object/function array-likes. It reads length once, applies primitive ToLength
conversion and checks the argument cap before allocation or indexed reads. Object
conversion for length remains unsupported. Missing indexed values become undefined.

`bind` creates a distinct callable retaining its target, receiver and prefixed
arguments across events. Rebinding keeps the original inner receiver and puts inner
prefixes before outer prefixes. Arrow targets retain lexical this. Bound chains are
limited to 16 hops, with at most 16 cumulative arguments. Every invocation, including
intrinsic adaptation, shares operation and call-depth budgets.

Supported function `name` and `length` are readonly own metadata. Non-strict writes
are ignored; strict writes fail. Names come from declarations, explicit expression
names, direct anonymous declarators/identifier assignments and fixed object-literal
properties. Other anonymous expressions keep empty names. Binding prefixes the name
with “bound ” and subtracts prefixed arguments from length, down to zero. Descriptor
redefinition, deletion and general reflection APIs remain unsupported; this does
not claim those APIs or complete prototype behavior.

Each page snapshot carries lexical cells and flat, bounded object, function and
environment arenas, so aliases and cycles survive service serialization. Parsed
source owns executable code and exact scope schemas; private graphs carry only
IDs and data. Environment chains are checked iteratively before execution.
Event execution mutates a candidate snapshot; neither captured-cell/heap changes nor DOM proposals commit if any later
operation fails. Heap, property, array length, source, token, call-depth and
wire-byte limits are reported through the existing located error surface.
There are at most 512 allocated function records, including sixteen always-rooted
canonical intrinsics, 512 environments and 2,048 environment cells per candidate, with
16 levels of syntactic function nesting. Some/find append intrinsic IDs 14/15 while
preserving IDs 0–13; the total function allowance remains 512.
The 128 KiB private-state ceiling also applies. Successful request boundaries trace
and reclaim unreachable records across all three arenas, including cycles, while
preserving monotonic IDs. Long-running requests can exhaust an allocation cap
before the operation budget; collection does not run during evaluation.
V9 supports `window.localStorage` and `window.sessionStorage` with only
`getItem`, `setItem`, `removeItem` and `clear`. Both areas hold string keys and
values, up to 256 entries and 64 KiB per origin-area (2,048 UTF-16 units per
key/value). Local storage is origin-scoped and kept in a separate Keychain item;
session storage is scoped to the tab and origin and disappears when the tab closes.
Cookie clearing leaves local storage intact. Storage writes remain candidates
until script execution, document validation, layout and authority publication succeed.
The aggregate localStorage record is limited to 512 KiB. Secure-store failures block
storage use rather than falling back to plaintext. An uncertain write or rollback also stores a
separate recovery marker. Startup checks it before loading site values; explicit recovery clears
the origin map before removing the marker. Cookie clearing does not touch either Web Storage item.
Object coercion, general object prototypes, primitive
wrappers, async arrows, constructors,
implicit arguments objects, rest/default/destructured parameters,
block-function declarations, generators, general DOM
access, exceptions, modules, asynchronous execution, BigInt, templates, regex,
legacy octal and non-ASCII identifiers remain unsupported. This remains a
current bounded JavaScript subset, not V8 or Test262 conformance. The pinned `ryu-js`
primitive supplies Number formatting only. Lone surrogates remain in VM state;
the current UTF-8 document boundary refuses them explicitly.
No complete V8, Test262, Blink or current-Oya compatibility is claimed. All script
changes remain candidate data until document validation and authority publication.

[Upstream notices](THIRD_PARTY_NOTICES.md) accompany adapted material.

## Bounds and lifecycle

| Resource | Bound |
| --- | --- |
| JSON frame including escaping/newline | 16 MiB |
| MCP frame / pending calls / queued output / whole-session request IDs | 64 MiB / 16 / 16 / 4,096 |
| MCP initialize / native request / complete output frame deadline | 30 s / 30 s / 2 s |
| HTML / HTTP headers / chunk framing | 2 MiB / 32 KiB / 128 KiB |
| Nodes / nesting / table rows / CSS rules | 8,192 / 32 / 256 / 512 |
| Static readable UTF-8 text per node / per document | 16 KiB / 512 KiB |
| Editable values, control labels/captions, titles, attributes and document writes | 2 KiB UTF-8 each |
| V9 per-script / page aggregate source / script count / tokens / statements / operations / call depth / parameters | 4 KiB / 32 KiB / 8 / 1,024 / 64 / 2,048 / 16 / 16 |
| V9 objects / own properties / array length / state bytes | 256 / 1,024 / 1,024 / 128 KiB |
| Expression recursion/AST depth / statement nesting / JS string / environment strings | 32 / 32 / 2,048 UTF-16 units / 16,384 UTF-16 units |
| Stylesheets / each sheet / aggregate CSS | 4 linked / 16 KiB / 64 KiB |
| Session cookies / per host / aggregate jar / cookie Keychain record | 64 / 16 / 32 KiB / 48 KiB |
| Web Storage entries / area / per key or value / aggregate Keychain record | 256 / 64 KiB / 2,048 UTF-16 units / 512 KiB |
| URL / correlation ID | 2 KiB / 128 bytes |
| Shared layout work / display lines / maximum layout extent | 5,242,880 / 32,768 / 1,048,576 logical points |
| Page viewport width / height | 280–4,096 / 120–4,096 logical points |
| History entries / desktop command queue | 64 / 16 |
| Headless network / ordinary engine RPC | 2 s / 3 s |
| Desktop network budget / outer network RPC deadline | 8 s / 10 s |

DNS and TLS internal I/O remain inside the supervised network process; the outer deadline
kills the runtime if they cannot return in time. Cancellation polls at most every 25 ms while
waiting for child replies. A service/transport failure retires all three engines without replay.
Ordinary navigation errors retain the previous page and report previous_page_retained:true.
History records only successful navigations; back/forward move only after successful load.

Services use inherited private pipes, not public network listeners. They have cleared
environments and no agent credentials, persistence or grandchildren. Process separation is
**not an OS sandbox**. The current build has no imported Electron profiles,
downloads, media, frames, extensions, cloud deployment or full JavaScript. Killing only the coordinator is detected independently by each engine
through an expected-parent watchdog, including while OS DNS or another dispatch is blocked.

## Verify

```sh
cargo fmt --manifest-path mars/Cargo.toml --all --check
cargo clippy --offline --workspace --all-targets --all-features --manifest-path mars/Cargo.toml -- -D warnings
cargo test --offline --workspace --all-features --manifest-path mars/Cargo.toml
cargo test --offline --all-features --manifest-path mars/Cargo.toml --test window -- --ignored --nocapture --test-threads=1
cargo test --offline --all-features --manifest-path mars/Cargo.toml --test mcp_sdk -- --ignored --nocapture --test-threads=1
```

The window command deliberately opens the actual macOS window and saves native screenshots.
The MCP SDK command opens an isolated native window through the installed official Node SDK
in `server/node_modules`. It exercises the actual toolbar grant/takeover, greeting and form
journeys and requires clean EOF shutdown without the SDK's signal fallback. It does not test
Codex, Cursor or other product client configuration and installs no dependency.
GUI checks require `test-support` and use fresh in-memory cookie, storage and session profiles;
they do not read or write the user's Keychain profile. Their startup-only `--test-profile`
and `--verify-ui`/`--verify-mcp-ui` flags are absent from ordinary builds. The verification deadline covers tabs
as well as field/button actions. Normal desktop launches keep the system profile.
Other tests are hermetic; loopback fixtures use synthetic in-memory data and no external sites
or .env. TLS test key/certificate are disposable, deliberately untrusted fixtures. The optional
test-support executable exists only for child-fault tests.

See [ARCHITECTURE.md](ARCHITECTURE.md) for ownership, boundaries and implementation adaptations.
