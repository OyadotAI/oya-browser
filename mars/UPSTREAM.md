# Mars source-guided Rust engines

Eye, V9 and Relay are owned Rust engines; the existing prototype is not a complete
Rust port of V8, Blink or Chromium networking.
The replacement target remains the behavior of the current Oya browser. An
implemented subset is not evidence of full browser compatibility.

## V8 conformance repairs (2026-10-25)

The upstream `test/mjsunit` checkout at the pinned V8 revision is executed without
editing test bodies or assertions. V9's optional conformance host preserves
separate Script parsing, declared fixtures and timer/microtask delivery. Missing
capabilities and resource-limit failures remain failures, and exclusions retain
their own counts. These are compatibility measurements, not a claim of V8 parity.

`crates/v9/src/v9/value/number/radix.rs` adapts the complete
`DoubleToRadixStringView` routine from pinned `src/numbers/conversions.cc`, using
Rust vectors instead of a shared character buffer. It retains the next-double
rounding interval, fractional carry and zero filling for unrepresented integer
digits. Rust supplies `next_up` and native binary64 arithmetic; no V8 runtime or
dependency is embedded. The corresponding source notice is retained in
`THIRD_PARTY_NOTICES.md`.

The fixed-decimal formatter uses Rust's exact decimal conversion and detects true
binary midpoints from the significand and exponent before correcting ties to
round upward in magnitude. Multiplying by a decimal scale first could manufacture
an apparent midpoint, so that double rounding is avoided. The reviewed string
protocol source is pinned `src/builtins/string-match-search.tq`,
`string-startswith.tq` and `string-endswith.tq`: the Rust helpers preserve
RequireObjectCoercible, IsRegExp, GetMethod and original receiver order.
The fallback also follows `RegExpCreate` in pinned `src/builtins/regexp.tq`,
initializing directly rather than entering the constructor and repeating an
observable IsRegExp lookup. The pinned `MaybeCallFunctionAtSymbol` helper in
`src/builtins/builtins-string-gen.cc` only dispatches pattern hooks on objects;
primitive arguments do not consult inherited hooks. The unchanged mjsunit
`es6/pattern-brand-check.js` and `regress/regress-1092896.js` pin those distinctions.

## Pinned references

### Native MCP adapter

The independently authored Rust stdio adapter targets the archived MCP
**2025-11-25** [lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle),
[stdio transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports#stdio),
[tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools) and
[cancellation](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/cancellation)
contracts. It deliberately excludes the current 2026-07-28 discovery/per-request-version
protocol. No schema or SDK source is copied into the Rust adapter. New tests are
authored here; the opt-in client exchange uses the already installed official Node
SDK without adding a Rust dependency or modifying the server lockfile. Tool and
resource limits are local constraints, not requirements imposed by MCP.

The project's engine documentation pins Chromium
`2c592105bbcd9490a9894df48d0fe59b2c512651`. Its
[DEPS](https://chromium.googlesource.com/chromium/src/+/2c592105bbcd9490a9894df48d0fe59b2c512651/DEPS)
pins V8 `fedd021b538607860f9c56e5c193ad8dcd4f2bb1`.
These references identify the code being studied, rather than following a
moving branch. They are not runtime dependencies.

Local reference checkouts of exactly these commits live outside the repository, as
shallow sparse clones: `~/Dev/upstream/chromium` (Blink `renderer/core` and `public`,
`content/browser/devtools`, `third_party/inspector_protocol`, `net`, `services/network`,
`base`, selected `components` and `chrome/browser` areas, Skia headers) and
`~/Dev/upstream/v8` (`src`, `include`, `test/mjsunit`, `test/inspector`). Mars is a Rust
port guided by that C++: read it for exact semantics (`browser_protocol.pdl`,
`js_protocol.pdl`, `css_properties.json5`, `html.css`, the inspector agents) and never copy
code or add it as a dependency.

| Source inspected | Observed responsibilities | Rust destination |
| --- | --- | --- |
| [V8 scanner.cc](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/parsing/scanner.cc): constructor, token lookahead, comment skipping, ScanHexNumber, ScanEscape, ScanOctalEscape and ScanString | UTF-16 source/literals; exact escape decoding; comment and line-terminator distinctions; explicit illegal tokens | `crates/v9/src/v9/lexer/`, with a bounded source cursor and typed literals |
| [V8 parser-base.h](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/parsing/parser-base.h): `ParsePrimaryExpression`, `ParseAssignmentExpression`, `ParseArrayLiteral`, `ParseObjectLiteral`, `ParseProperty`, `ParseArguments`, `ParseFormalParameterList` | Primary/assignment grammar, literal property order, call arguments and formal parameters | `crates/v9/src/v9/parser/expression.rs` and `parser/statement.rs`; no bytecode or full grammar port |
| [V8 bytecode-generator.cc](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/interpreter/bytecode-generator.cc): `VisitIfStatement`, `VisitFunctionLiteral`, `VisitObjectLiteral`, `VisitArrayLiteral`, `BuildAssignment`, `VisitProperty`, `VisitCall` | Branch, function, object/array, assignment, property and call evaluation paths | `crates/v9/src/v9/interpreter/` and `value/heap.rs`; Rust evaluates the supported subset directly and does not generate bytecode |
| [V8 arithmetic builtin dispatch](https://github.com/v8/v8/blob/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/builtins-number-gen.cc) | Arithmetic delegates to binary/unary operation assemblers; this dispatch alone does not establish conversion semantics | `crates/v9/src/v9/value/`; conversion contract is taken from the normative operations below |
| [Blink html_document_parser.cc](https://chromium.googlesource.com/chromium/src/+/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/html/parser/html_document_parser.cc): context tokenizer modes and initial scheduling collaborators | Separate data/RCDATA/raw-text/script modes; tokenizer, tree builder, script runner, resource loader and parser scheduling cooperate | `crates/eye/src/document/` and the document service; present parser is still a prototype |
| [Chromium network_service.h](https://chromium.googlesource.com/chromium/src/+/2c592105bbcd9490a9894df48d0fe59b2c512651/services/network/network_service.h): service/context interfaces | Network service/context lifetimes; resolver, TLS, authentication and decoding interfaces; explicit test seams | `crates/relay/src/network/` and per-context coordinator ownership; present networking is still partial |

The parser and interpreter references above are from the pinned V8 revision.
They guide the grammar and evaluation order for the supported slice; they do not
establish full ECMAScript semantics or a line-by-line source port.

The lexical/control-flow layer additionally inspects these routines at that same
revision, with the downloaded source checked locally where the browser's source
view was truncated:

| Reviewed routines | Rust adaptation |
| --- | --- |
| `parser-base.h`: `ParseVariableDeclarations` (4521–4704), `ParseBlock` (5988–6034), `ParseIfStatement` (6183–6220), `ParseContinueStatement`/`ParseBreakStatement` (6223–6298), `ParseWhileStatement` (6409–6437) | let/const initialization grammar, block ownership, nearest-if else binding and enclosing-loop validation |
| `parser-base.h`: `ParseLogicalExpression`, `ParseBinaryContinuation`, `ParseBinaryExpression` (3536–3554, 3703–3779) | Typed operator precedence and left association for the supported operators |
| `bytecode-generator.cc`: `VisitBlockDeclarationsAndStatements`/`VisitVariableDeclaration` (2119–2179), `BuildThrowIfHole`/`BuildHoleCheckForVariableAssignment`/`BuildVariableAssignment` (4904–5100), `VisitAssignment` (5955 onward) | Instantiate lexical cells before statements; distinct initialization and assignment; resolve a store before its RHS, then enforce TDZ/const rules |
| `bytecode-generator.cc`: `VisitIfStatement` (2502–2541), Continue/Break visitors (2549–2559), `VisitWhileStatement` (3189–3208), logical OR/AND visitors (8460–8555) | Selected-branch evaluation, propagated loop completion and operand-preserving short circuit |

Rust executes the AST directly; this does not port Ignition's bytecode generation,
register allocation or optimizations. Function activations follow their defining
lexical environments rather than caller-local environments.
Event-state validation checks the exact source-declared global names/kinds without
replaying initializers. The coordinator still owns the trust and publication boundary.
The primitive and lexical cases in `tests/unit/v9/control.rs` are newly authored,
not an imported Test262 or V8 suite. Normative ToBoolean/ToNumber/ToString rules
were read alongside the pinned implementation references.

Standard `for` additionally follows pinned `parser-base.h` `ParseForStatement`
(6668–6828), `ParseStandardForLoopWithLexicalDeclarations` (6939–6997) and
`ParseStandardForLoop` (6997–7041), plus `bytecode-generator.cc`
`VisitForStatement` (3210–3258). The Rust AST retains explicit initializer, test,
update and body clauses. Execution owns a bounded header scope, copies `let`
cells per iteration, routes continue through update and skips update on break,
return or error. This adapts observed scope and execution ordering; it does not
port V8's closure desugaring or bytecode implementation. Closures now retain the
distinct iteration records. `tests/unit/v9/for_loop.rs` contains
newly authored cases, not an imported upstream conformance suite.

The first-class-function layer additionally inspects these pinned sources:

| Reviewed routines | Rust adaptation |
| --- | --- |
| `parser-base.h`: `ParseFunctionExpression` (4183–4220) | Anonymous/named expressions, simple parameter/body parsing and a private expression name; generators and full grammar remain unimplemented; arrows are described below |
| `bytecode-generator.cc`: `VisitFunctionLiteral` (3496–3524), `BuildVariableLoad` (4708–4806), `BuildNewLocalActivationContext`/context initialization/block context creation (8648–8742), `VisitCall` callee/receiver setup (6855–6998) | Source-owned code sites paired with captured contexts, lexical lookup with TDZ cells, activation/block records and callee-before-arguments evaluation; no bytecode or optimization machinery; receiver handling is described below |
| [V8 contexts.h](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/objects/contexts.h) (453–493, 546–602, 590–726, 954–1013) | Closures pair code with a context; contexts link to lexical parents and distinguish uninitialized bindings. Rust keeps ordinary typed cells and does not port optimization-specific ContextCell states |
| [V8 js-function.tq](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/objects/js-function.tq): `JSFunction.shared_function_info` and `context` | Function values identify parsed code and the defining environment; no native pointers or executable AST crosses the private wire |

`crates/v9/src/v9/registry.rs` rebuilds code/scope schemas from the same ordered source.
`graph/` validates flat function/environment references, lexical parents, exact
cell schemas and named self bindings. Activations and per-iteration environments
retain shared cells for closures after their active IDs are popped. At successful
request finish, an owned iterative tracer sweeps unreachable objects, functions
and environments together, including cycles. This simple collector is newly
authored Rust code, not a port of V8's garbage collector. No collection runs while
evaluator temporaries remain live. Per-request allocations have explicit caps.
`tests/unit/v9/closures.rs`, `graph.rs` and `tests/closures.rs` are newly authored
behavior and malformed-state cases, not imported upstream conformance suites.

The arrow-function layer additionally reads pinned `parser-base.h`
`NewFunctionScope` (890–907), `UseThis` (1305–1320),
`ParseAssignmentExpressionCoverGrammar`/continuation (3290–3383),
`ParsePrimaryExpression` arrow-head handling (2170–2310),
`ValidateFormalParameters` (1295–1301), `ParseFunctionBody` (4917–4967)
and `ParseArrowFunctionLiteral` (5108–5278). These routines were inspected in
the complete local copy acquired at the pinned revision. Oya browser navigation
to GitHub and googlesource timed out during this increment; no substitute browser
was used or successful web read claimed.

Rust recognizes arrows only at assignment-expression entry. A bounded token
prepass pairs parentheses so potential heads can be distinguished from ordinary
grouping without speculative reparsing. Simple names and parameter lists are
validated before bodies; token line coordinates enforce no line terminator before
`=>`, including in comments. Concise bodies synthesize a located return charged
to the existing statement and expression budgets. Shared function-context cleanup
isolates loop targets and declaration nesting on every completion.

`FunctionKind` is source-owned AST/registry metadata. Wire function records still
contain only code/environment IDs, and duplicate registry sites are rejected.
Arrows reuse the same closure cells, activation, invocation, validation and tracing
paths. V8's skipped default function bindings and enclosing receiver lookup inform
lexical receiver capture described below; no implicit arguments object is fabricated.
Explicit lexical/parameter `arguments` cells resolve normally; a lookup requiring
an ordinary activation's implicit arguments object is unsupported. Arguments
lookup stops at that ordinary activation before considering outer `arguments`
cells, while explicitly declared cells within it retain TDZ and assignment rules.
Arguments objects, async arrows and richer parameters remain unimplemented.
The new `tests/unit/v9/arrows.rs` and `arrows/grammar.rs` cases are authored here,
not imported upstream or full arrow conformance tests.

The receiver layer additionally inspects pinned `parser-base.h` `UseThis`
(1305–1320), `ParseStatementList` (5761–5829), and `bytecode-generator.cc`
`VisitCall` (6855–6998). Oya successfully opened and read the pinned source view
for this increment. The large arm64 file was truncated by that text view, so the
same pinned file was acquired as source through GitHub's contents API. The complete
`src/builtins/arm64/builtins-arm64.cc` `Generate_CallFunction` (2946–3030) was then
read locally: strict calls preserve receivers; sloppy null/undefined uses the
callee realm global proxy, and other primitives require ToObject.

Rust retains member base/key/callee references ahead of arguments, captures lexical
arrow receivers and converts detached ordinary calls according to source-owned
strictness. Exact raw string-token provenance and direct prologue statements set
program/function mode; inherited strictness and restricted-name early errors are
checked before execution. Caller mode never chooses callee mode. The AST evaluator
uses a page-owned heap global object with coherent existing root-function cells;
it does not port V8's global proxy, native context, machine-code call stub or
ToObject wrapper implementation. Window/WindowProxy, primitive wrappers, protected
global mutation and general reflection remain explicit unsupported limits.

`graph/` validates and traces arrow captures and invocation receivers, including
receiver-only object reachability. The canonical global object is always rooted.
The existing candidate boundary publishes receiver/global properties, captured
cells, heap, Storage and DOM together. `tests/unit/v9/receivers.rs` and the added
real-service rollback case are newly authored behavior tests, not imported V8 or
Test262 conformance suites.

The callable-object layer additionally reads pinned
[`function.tq::FastFunctionPrototypeBind`](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/function.tq),
[`builtins-function.cc::DoFunctionBind`](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/builtins-function.cc),
`builtins-arm64.cc::Generate_FunctionPrototypeCall`, `Generate_FunctionPrototypeApply`
and `Generate_CallBoundFunctionImpl`, plus
[`js-function.cc::CopyNameAndLength`, `JSBoundFunction::GetName` and `GetLength`](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/objects/js-function.cc).
The complete relevant routines were read through Oya; the larger js-function file
was truncated after those routines, so local excerpt line numbers are not upstream
line references. [`conversion.tq::ToLength`](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/conversion.tq)
was read for NaN/nonpositive handling, positive flooring and maximum-safe-integer
clamping. The already pinned parser's `SetFunctionNameFromIdentifierRef` and
`SetFunctionNameFromPropertyName` call sites guide supported name inference.

Rust retains stable intrinsic identities and a shared callable Function prototype,
while source closures and bound callbacks use the owned graph. Binding stores the
target/receiver/prefix values and derives readonly name/length metadata in the
reviewed order. Calls preserve callee-before-arguments evaluation; apply constructs
a bounded array-like list; bound-chain invocation preserves inner binding order.
Canonical identities, function properties and bound edges are validated, counted
and traced alongside existing closures. This does not port V8 machine-code stubs,
maps, accessors, optimization or general object/prototype machinery. Dynamic
Function construction, bound constructors, primitive wrappers and descriptor
reflection remain explicit limits. The new metadata, call and graph cases and
native bound-callback integrations are authored here, not imported conformance suites.

Var declaration instantiation additionally follows the pinned parser's
`ParseVariableDeclarations` (declaration-scope ownership), the already reviewed
`BytecodeGenerator::VisitVariableDeclaration` (lexical initialization versus var),
and [`runtime-scopes.cc::DeclareGlobal` and `Runtime_DeclareGlobals`](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/runtime/runtime-scopes.cc).
The complete global-declaration routines were read through Oya before the larger
file was truncated; local excerpt lines are not upstream line numbers. Existing
global vars retain values, new vars begin undefined, direct functions initialize
before execution, and lexical collisions refuse instantiation. Rust reuses a
source walker and its owned cell graph rather than V8 declaration arrays, contexts,
global dictionaries or property descriptors. Protected host globals, full global
descriptor semantics, destructuring, implicit arguments objects and Annex-B block
functions remain outside this slice. The var unit and real-service cases are newly
authored behavior tests, not imported conformance suites.

The ordinary-expression slice uses additional routines in those same pinned
parser and bytecode-generator files: `parser-base.h` assignment continuation
(3292–3445), conditional continuation (3675–3700), unary/prefix (3783–3843) and
unary/postfix (3876–3934); `bytecode-generator.cc` conditional (4110–4144),
assignment/compound (5955–6088), void/typeof (7349–7372) and count operation
(7514–7705). These routines guide right-associative assignment, lazy conditional
branches, retained references and old values for compound stores, numeric postfix
results, and special unresolved-name handling for `typeof`.

Rust uses one synchronous prepared-reference path over existing cells and arena
IDs. It does not reproduce V8 bytecode, machine-code operations or optimization.
Primitive conversion and protected-host restrictions remain the existing bounded
ones; general object coercion, accessors, logical/bitwise/exponentiation assignments
and arbitrary DOM property stores remain outside this slice. Automatic semicolon
insertion was added separately below. New expression tests are authored here rather than imported V8 or
Test262 cases.

## Implementation evidence

### Static reading, inherited CSS and canvas paint

The static-reading slice studies the same pinned Chromium revision
`2c592105bbcd9490a9894df48d0fe59b2c512651`. These are general value and block-layout
rules; there is no hostname-specific style path or embedded Blink engine.

| Inspected source/routines | Observed behavior | Owned Rust implementation |
| --- | --- | --- |
| [`shorthands_custom.cc` ConsumeFont and Font::ParseShorthand, 3094–3297](https://github.com/chromium/chromium/blob/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/css/properties/shorthands/shorthands_custom.cc#L3094) | Required size/family, optional slash line height, reset omitted font fields | Bounded native-family shorthand expands to supported longhands before cascade selection |
| [`style_builder_converter.cc` ConvertLineHeight, 2199–2241](https://github.com/chromium/chromium/blob/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/css/resolver/style_builder_converter.cc#L2199) | Unitless factor stays relative; percentage resolves against the declaring computed font; normal and lengths remain distinct | Typed `LineHeight` factor/length/normal consumed by measurement and painting |
| [`font_size_style.h` FontSizeStyle](https://github.com/chromium/chromium/blob/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/style/font_size_style.h) and [`css_to_length_conversion_data.h` FontSizes::Em and ViewportSize::Height](https://github.com/chromium/chromium/blob/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/css/css_to_length_conversion_data.h#L92) | Font-relative values retain font facts; viewport height is distinct from scrolling content height | Explicit checked page `RenderContext`; native system font strategy |
| [`css_length_resolver.cc` ViewportHeightPercent, 15–17; ZoomedComputedPixels vh/em branches, 182–183 and 271–273](https://github.com/chromium/chromium/blob/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/css/css_length_resolver.cc#L15) | `vh` uses viewport height/100; `em` uses the computed font | Nonnegative bounded relative lengths recompute for the current page viewport |
| [`length_utils.cc` ComputeInlineSizeForFragmentInternal, 527–598, and ResolveInlineAutoMargins, 1551–1570](https://github.com/chromium/chromium/blob/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/layout/length_utils.cc#L527) | Box conversion/min-max constraints precede horizontal auto-margin distribution; inline-start does not become negative on overflow | Owned BODY content-box width and normal-block auto-margin placement |
| [`style_builder_converter.cc` ResolveLightDarkPair, 3015–3021; ResolveColorValue; ExtractColorSchemes, 3846–3881](https://github.com/chromium/chromium/blob/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/css/resolver/style_builder_converter.cc#L3015) | `light-dark()` selects by the used scheme; parsing offered flags is distinct from choosing the preference | Closed supported scheme values, literal pair validation and matching text/link defaults |
| [`box_background_paint_context.cc` LayoutView constructor, 92–115](https://github.com/chromium/chromium/blob/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/paint/box_background_paint_context.cc#L92) and inspected portions of [`view_painter.cc` PaintBoxDecorationBackground/PaintRootElementGroup](https://github.com/chromium/chromium/blob/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/paint/view_painter.cc) | Root background paints the canvas, including at least the visible area | Canvas colors plus non-target BODY decoration in the native display list |

The pinned used-scheme preference-selection routine and BODY-background fallback
routine were not located. Scheme negotiation follows the reviewed primary
[CSS Color Adjustment sections 2.2–2.4](https://drafts.csswg.org/css-color-adjust-1/#color-scheme-prop);
root/BODY propagation follows the reviewed [CSS 2.2 background rules](https://www.w3.org/TR/CSS22/colors.html#background).
The complete computed-style inheritance implementation was not inspected: inherited
line-height behavior combines the inspected converter representation with standard
inheritance semantics. Native font selection, legacy unauthored reading geometry,
work caps and immutable runtime presentation are Oya policies. `vh` support does
not imply mobile dynamic/small viewport, arbitrary unit or general layout support.

Source access is recorded precisely. Cached pinned shorthand source was reused.
New files were first opened through Oya; after truncated Oya views, the complete
converter and length-utils files were acquired through an approved GitHub API
command before the coordinator's no-API clarification. This is not described as
complete-file Oya reading. Subsequent new source access used Oya exclusively.
Complete retained artifacts include those two files, css_length_resolver.cc,
css_to_length_conversion_data.h, box_background_paint_context.cc and
font_size_style.h, with their headers. The view-painter browser capture ends near
the end of the inspected group routine; it is partial evidence. Browser excerpt
line numbers are not upstream source line numbers. The inspected
LGPLv2-or-later style_resolver_state.cc is not an implementation template and no
code from it is copied or translated here. Source notices are retained in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

The authored tests are `tests/unit/document/static_css.rs`, static-text tests and
`tests/static_reading.rs`; they are not imported Blink/web-platform conformance
cases. Frozen source bytes and resource references live in
[fixtures/reading](fixtures/reading/README.md). Original example.com document-stage
rendering retains `/s.js`; unsupported required JavaScript/DOM/SVG still prevents a
full-compatibility claim. Full navigation never strips or skips that resource.
The separate unchanged httpbin fixture exercises its complete paragraph through
owned services. Larger static text remains subject to independent node/aggregate,
measurement, display and wire budgets. The banner geometry and scrolling text
region are owned AppKit shell implementation, with no upstream browser-layout
code adapted for the banner.

### Flex layout and box sizing

Pinned Blink at Chromium revision `2c592105bbcd9490a9894df48d0fe59b2c512651`
guides the owned flex implementation. Oya source inspection and retained-header
copies establish these adaptation references:

| Source | Observed behavior | Rust adaptation |
| --- | --- | --- |
| [`flex_item.h`](https://chromium.googlesource.com/chromium/src/+/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/layout/flex/flex_item.h) and [`flex_line_breaker.cc`](https://chromium.googlesource.com/chromium/src/+/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/layout/flex/flex_line_breaker.cc), BreakIntoLines/GreedyBreakFlexItemsIntoLines | Content base and constrained hypothetical size; greedy outer-size wrapping with gaps only between items | Bounded owned item records and row lines |
| [`line_flexer.cc`](https://chromium.googlesource.com/chromium/src/+/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/layout/flex/line_flexer.cc), LineFlexer/FreezeItems/ResolveFlexibleLengths | Grow or weighted shrink, partial factors, min/max violation freezing and recomputation | Checked finite Rust allocation with an explicit item-count iteration bound |
| [`flex_layout_algorithm.cc`](https://chromium.googlesource.com/chromium/src/+/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/layout/flex/flex_layout_algorithm.cc), ShouldApplyAutoMinSize/ConstructAndAppendFlexItems/DoesItemStretch/PlaceFlexItems/InitialContentPositionOffset/ContentDistributionSpace/ComputeMinMaxSizeOfRowContainer | Automatic minimums, basis resolution, cross sizing/stretch and justification | Existing native metrics and shared render-local measurement/allocation context |
| [`length_utils.cc`](https://chromium.googlesource.com/chromium/src/+/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/layout/length_utils.cc), ResolveInlineLengthInternal/ResolveBlockLengthInternal | Content-box adds decoration; border-box includes it with a nonnegative content floor | Central owned box conversions |
| [`shorthands_custom.cc`](https://chromium.googlesource.com/chromium/src/+/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/css/properties/shorthands/shorthands_custom.cc), Flex::ParseShorthand | Omitted basis is 0%; explicit third unitless zero is 0px; none is 0 0 auto | Typed basis and ordered longhand expansion before cascade selection |

Large layout/parser source copies were acquired at the same pin after Oya views
truncated. The length-utils artifact contains complete resolver routines within a
partial file. Runtime work/extent/iteration bounds are owned safety adaptations,
not claims that Blink uses the same caps. Anonymous-block source order in the
LGPL-covered LayoutBlock::AddChild was inspected only; its implementation is not
copied or translated. The current Rust DOM cannot reconstruct direct mixed-text
order for flex, so that structure explicitly fails instead.

The supported subset excludes height properties, grid/inline-flex/reverse flow,
column wrapping, visual order, advanced alignment, flex-item/table auto margins,
percentage gaps, positioning and table/flex interaction. Automatic column content minimums still
apply: a 0px basis does not imply a zero-height item. New geometry, real-service
form and actual native-frame tests are authored here, not imported web-platform
or Blink conformance cases. Full Blink compatibility remains unfinished.


### Bounded abstract equality

At the pinned V8 revision above, the complete
[`runtime-operators.cc`](https://github.com/v8/v8/blob/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/runtime/runtime-operators.cc)
was read through Oya. `Runtime_Equal` (lines 19–27) delegates to `Object::Equals`,
propagates exceptions and returns the successful Boolean result. `Runtime_NotEqual`
(lines 29–37) delegates to the same operation and negates only success. Strict
comparisons use a separate `Object::StrictEquals` path. These wrappers establish
error/negation dispatch, not the complete core comparison algorithm.

The complete retained-header pinned
[`conversion.tq`](https://github.com/v8/v8/blob/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/conversion.tq)
was also read locally, including `TryGetExoticToPrimitive`,
`CallExoticToPrimitive`, `NonPrimitiveToPrimitive_Default`, `TryToPrimitiveMethod`
and `OrdinaryToPrimitive_Number`/`OrdinaryToPrimitive_String` (lines 162–290).
It confirms genuine method lookup/calls, primitive-result checks, ordinary fallback
order and conversion failure. Those callable conversion semantics remain
unimplemented here: Rust refuses comparisons at the required ToPrimitive step
rather than fabricating a string or NaN. That refusal is a project capability
boundary, not the standard ECMAScript comparison result.

Oya's source readers exposed only file prefixes before the pinned
`Object::Equals` body and before the current normative `IsLooselyEqual` algorithm.
The pinned source viewer identified `Object::Equals` at line 918, but its body
remained unread. No complete pinned equality-core or current normative algorithm
inspection is claimed.

The alternate primary source actually inspected is the TC39
[Record/Tuple Stage 2 draft, April 15, 2025](https://tc39.es/proposal-record-tuple/#sec-islooselyequal),
which is not the current normative ECMAScript specification. Its fully visible
`IsLooselyEqual` steps 1–12 establish same-language-type strict delegation,
symmetric null/undefined pairing and Number/String conversion, Boolean
normalization before object conversion, and symmetric primitive/Object
ToPrimitive branches. The preceding `SameValueNonGeneric` body establishes
exact UTF-16 code-unit strings, Boolean and object identity. Its distinct numeric
SameValue rules are not substituted for existing Number strict equality.
The existing tested Number equality and numeric-string conversion are reused.

For the closed Number/String/Boolean/Null/Undefined/object/function/host variants,
terminal false for remaining different-type pairs, especially nullish versus
non-nullish/object-like, is an explicit inference from the inspected branch
exclusions and the owned type model: the draft's final false tail was not visible.
This increment is therefore a bounded source-guided implementation using reviewed
primary branches, reused existing leaves and a labeled inference. It is not a
complete V8 equality-core port or complete current-spec verification. No
BigInt/Symbol/Record/Tuple, HTMLDDA/undetectable host behavior, primitive wrappers
or general object conversion is supplied.

`crates/v9/src/v9/value/equality.rs` owns the pure closed-value comparison;
`interpreter/arithmetic.rs` preserves located failures and successful negation.
The scanner keeps strict spellings longest-first; the existing equality precedence
and evaluator retain once-only left-to-right operands. The symmetric public
`tests/unit/v9/abstract_equality` cases are authored here, not imported V8/Test262
conformance tests. The separate synthetic `/compat/choose-option` page exercises
numeric-string equality with ordinary some/find callbacks; it neither rewrites
example.com nor demonstrates its remaining DOM/script compatibility.

### String and Array processing

The same pinned V8 revision guides canonical String and Array methods:

| Source routine | Observed behavior | Rust adaptation |
| --- | --- | --- |
| [`string-trim.tq`](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/string-trim.tq), StringTrim | Exact ECMAScript whitespace and edge scanning | UTF-16 edge scans using the existing whitespace predicate |
| [`builtins-string-gen.cc`](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/builtins-string-gen.cc), StringPrototypeSplit; [`runtime-regexp.cc`](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/runtime/runtime-regexp.cc), Runtime_StringSplit | Receiver, limit and separator conversion order; zero/undefined shortcuts; UTF-16 empty-separator behavior; nonoverlapping literal matches and empty fields | Owned linear UTF-16 matcher, primitive ToUint32 limits and actual-output caps |
| [`array-map.tq`](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/array-map.tq), ArrayMap/ArrayMapLoopContinuation; [`array-filter.tq`](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/array-filter.tq), ArrayFilter/ArrayFilterLoopContinuation | Snapshot length, validate callback, fresh presence/value at each index, callback arguments and filter selection of the captured value | Bounded sparse heap arrays and the ordinary callable dispatcher |
| [`array-join.tq`](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/array-join.tq), ArrayPrototypeJoin/ArrayJoinImpl | Length snapshot, separator conversion before empty shortcut, fresh indexed reads, nullish/hole empty fields and output bounds | Primitive-only conversion and checked UTF-16 output |
| [`array-some.tq`](https://github.com/v8/v8/blob/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/array-some.tq), ArraySome (line 110), ArraySomeLoopContinuation (line 51); [`array-find.tq`](https://github.com/v8/v8/blob/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/array-find.tq), ArrayPrototypeFind (line 117), ArrayFindLoopContinuation (line 63), ArrayFindLoopAfterCallbackLazyDeoptContinuation | Callback validation before empty traversal, initial length snapshot, optional thisArg, some HasProperty skips versus find unconditional Get, truthy early exit and captured find result | Owned-array traversal, existing call_value and shared work budget; no result-array allocation |

The routines were inspected through Oya. Complete pinned source was acquired after
Oya truncated the large split/runtime/join files; retained-header copies supplied
the previously hidden routines. These are semantic adaptations to the owned Rust
interpreter, not translations of V8 code generation or optimized element storage.
The complete pinned 142-line some and 148-line find Torque files, including their
source headers, entry routines and continuations, were read through Oya and locally.
Their separate normative array-method webpages were not fully inspected. Some
checks fresh presence before Get; find performs Get for each snapshotted index,
including missing/deleted positions as undefined in this engine's prototype-free
arrays. Both observe later changes within the original range and ignore appends
beyond it. Find's after-callback continuation explicitly retains the captured value,
so callback overwrites do not change the selected result. Both methods stop at the
first truthy result and share ordinary callback receiver/depth/work semantics.

Some/find append canonical IDs 14/15, preserving earlier IDs and keeping the fixed
512-function limit. Existing closed-selector validation and finish-only tracing root
all sixteen canonical functions and their own-property edges. The Rust implementation
imports no optimized storage or deoptimization machinery. Tests under
`tests/unit/v9/array_selection` are newly authored public-surface cases, not upstream
V8 or Test262 tests. No new dependency or persisted-profile migration is introduced.

No regex/custom splitters, general primitive boxing, constructors/prototype APIs,
generic borrowed array-like receivers, object coercion, species or asynchronous
callbacks are supplied. Independent aggregate quotas can reject a result even
when its individual array length fits. New tests are authored here rather than
imported V8 or Test262 cases.


### Bounded array-literal spread

At pinned V8 `fedd021b538607860f9c56e5c193ad8dcd4f2bb1`, the retained-header
complete cached [`bytecode-generator.cc`](https://github.com/v8/v8/blob/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/interpreter/bytecode-generator.cc#L4481)
supplied inspected `BuildFillArrayWithIterator` (4481–4512),
`BuildCreateArrayLiteral` (4514–4673), `VisitArrayLiteral` (4675–4678) and
`BuildGetIterator`/`BuildGetIteratorRecord`/`BuildIteratorNext` (8093–8196).
They show operand evaluation when reached, complete iterator consumption before
later elements, fresh array containers and direct yielded-value stores. Ordinary
literal elisions advance without a store, separately from spread-produced values.
Rust adapts this order into bounded AST evaluation, without Ignition bytecode or
a generic iterator implementation. Its existing destination-before-first-operand
allocation policy is retained; V8's leading-spread specialization instead evaluates
the operand and uses CreateArrayFromIterable. Exact allocation-failure ordering
between these implementations is deliberately not claimed.

Oya exposed the complete early routines of pinned
[`builtins-iterator-gen.cc`](https://github.com/v8/v8/blob/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/builtins-iterator-gen.cc):
`GetIteratorMethod`/`GetIterator`, `IteratorStep`/`IteratorComplete`/`IteratorValue`,
`IterableToList`/`FillFixedArrayFromIterable`, `IterableToListMayPreserveHoles` and
`IterableToListConvertHoles`. The view/excerpt is a prefix, not the complete file;
the guarded array/string portion of `FastIterableToList` was visible before later
code truncated. The actual-array branch explicitly converts holes to undefined
when prototype/iterator guards permit it. Its separate MayPreserveHoles helper
is documented as inconsistent with actual iteration and is not the spread model.
The owned Rust shortcut therefore creates dense Undefined stores for absent source
positions, retaining shallow object/function/host references. It does not silently
supply iteration for ordinary objects with length/index properties.

The complete Oya-read 47-line
[`string-iterator.tq`](https://github.com/v8/v8/blob/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/string-iterator.tq#L29)
includes `StringPrototypeIterator` and `StringIteratorPrototypeNext` (29–46).
The latter loads a UTF-16 surrogate pair, forms the emitted string and advances
by its UTF-16 length. Complete cached
[`builtins-string-gen.cc`](https://github.com/v8/v8/blob/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/builtins-string-gen.cc#L1757)
supplied inspected `StringFromSingleUTF16EncodedCodePoint` (406–434) and
`LoadSurrogatePairAt` (1757–1820): only a high surrogate followed immediately by
a low surrogate combines; otherwise the original single unit survives. Rust uses
a borrowed UTF-16 cursor and bounded emitted-string allocation, without replacement,
normalization or grapheme segmentation. Indexed reads/split remain code-unit paths.

The complete Oya-read 199-line
[`array-from.tq`](https://github.com/v8/v8/blob/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/array-from.tq)
was inspected only as supporting evidence: its iterable branch repeatedly steps
an iterator; its separate array-like branch snapshots length. This does not mean
spread is specified as Array.from or that Array.from is implemented here. The exact
pinned `ArrayIteratorPrototypeNext` leaf was not inspected (the guessed
array-iterator.tq path returned 404). Its live-length behavior is not claimed as
verified by this read. Capturing length after each owned-array operand is an
observational-equivalence inference restricted to the hook-free synchronous copy:
no author code runs between its reads. It is not a global snapshot before operands
or a contract for custom iterators/getters.

`ArrayElement` stays in the parsed source schema, and validation/registry traversal
walk each operand without adding executable private-wire data. Rust appends through
one checked path, charging each spread output before clone/allocation/store under
the existing length/property/environment/work/heap/state limits. No all-code-point
list, quota reset, new dependency or collection during temporary lifetime is added.
The Choose an option model uses real spread composition with the existing visible
journey. Current tests are authored here; no V8/Test262 test import or complete
iterator/browser conformance is claimed.

Access provenance: the bytecode-generator and builtins-string-gen copies were
already acquired/attributed by prior work and only named routines were newly
inspected locally. String-iterator and array-from were complete Oya source reads
with headers retained. The iterator-gen excerpt retains its original header but
is not the full 664-line file; iterator.tq protocol declarations/step/value/close
observations also came from a partial reader view. Inspected generic IteratorClose
behavior is not implemented or promised by this owned-array/string shortcut.
Custom/generic Symbol.iterator, iterator objects and cleanup, NodeList iteration,
prototypes/getters, literal elisions, call/new/object spread, rest/destructuring and
Array.from remain outside this increment. No performance or full core-port claim
follows from this source study. The separate equality source-access limitations
and proposal/inference boundary above remain unchanged.

### Automatic semicolon insertion in supported classic scripts

At pinned V8 `fedd021b538607860f9c56e5c193ad8dcd4f2bb1`, the complete cached
[`parser-base.h`](https://github.com/v8/v8/blob/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/parsing/parser-base.h#L987)
supplied these inspected complete routines: `ExpectSemicolon` (987–1012),
`ParsePostfixExpression` (3900–3911), `ParsePostfixContinuation` (3916–3933),
`ParseArrowFunctionLiteral` (5108–5288; newline guard 5120–5128),
`ParseVariableStatement` (6052–6074), `ParseExpressionOrLabelledStatement`
(6093–6180), `ParseContinueStatement` (6223–6253), `ParseBreakStatement`
(6256–6290), `ParseReturnStatement` (6294–6338) and `ParseStandardForLoop`
(6997–7024). The first-header semicolon branches of `ParseForStatement` near
6724 and 6805 were also inspected; this is not a port of for-in/of or desugaring.
The complete cached parser retains its 2012 V8 header and earlier acquisition
provenance described above. V8 first parses the expression, then consumes an
explicit semicolon or accepts a trivia line terminator/automatic terminal. Rust
adapts that order into its existing bounded AST parser, preserving newline call,
index, member, binary and assignment continuations.

The Oya-read prefix of pinned
[`token.h`](https://github.com/v8/v8/blob/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/parsing/token.h)
includes the complete enum group and `Token::IsAutoSemicolon`: semicolon,
right brace and End are exactly its contiguous terminals. Right parenthesis and
`else` are not automatic boundaries. Supported completed statement tails accept
EOF/right brace or permitted inter-token line breaks without consuming a virtual
token or manufacturing an empty statement. Incomplete expressions/delimiters and
missing control-flow bodies still fail. Standard-for separators use strict
punctuation; inline named-call handlers retain their existing punctuation and
synthetic-token accounting policy.

The Oya-read scanner prefixes fully exposed the named routines used here:
[`scanner.h`](https://github.com/v8/v8/blob/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/parsing/scanner.h)
`HasLineTerminatorBeforeNext`/`AfterNext`/`AfterNextNext`;
[`scanner.cc`](https://github.com/v8/v8/blob/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/parsing/scanner.cc)
`Initialize`, `Next`, `PeekAhead`, `PeekAheadAhead`, `SkipSingleLineComment` and
`SkipMultiLineComment`; and
[`scanner-inl.h`](https://github.com/v8/v8/blob/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/parsing/scanner-inl.h)
`SkipWhiteSpace`. The first two files carry 2011 V8 notices, scanner-inl 2018,
and token.h 2012. These are header-bearing partial-file views, not full scanner
or token-file reads; local excerpt line numbers do not establish upstream lines.
The `ScanEscape` prefix separately shows escaped-line handling, including CRLF,
but its complete function was not visible in this read.

The complete 320-line Oya-read
[`unicode.h`](https://github.com/v8/v8/blob/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/strings/unicode.h#L278)
retains its 2011 V8 notice. `IsLineTerminator` recognizes LF, CR, U+2028 and U+2029;
`IsStringLiteralLineTerminator` separately recognizes only LF/CR. A CRLF trivia
sequence sets one Boolean boundary fact. Newlines inside string contents or
escaped continuations are not trivia before the following token. Rust records
the trivia fact on each source token, including End, using its existing source
cursor; this does not port V8's diagnostic line-counter normalization, which was
not inspected in full. Original UTF-8 byte/UTF-16 column/source-index reporting
and separate-source End accounting remain existing owned policies.

Restricted return, unlabeled break/continue, postfix and arrow boundaries reuse
that token fact. No newline is permitted between a supported arrow head and `=>`.
The existing directive walker still classifies complete expression statements
and exact raw spelling; a continued call/member expression cannot activate a
directive. This is an integration implication of inspected parsing order, not a
new full upstream directive-parser read. The inspected `ParseDoWhileStatement`
special semicolon branch near 6397–6402 and `ParseThrowStatement` (6438–6454)
establish boundaries only: do-while and throw remain unsupported here, as do labels,
modules and the other existing unsupported grammar.

All original programs, including unreachable bodies, still parse/validate before
candidate effects publish. Implicit punctuation adds no Program token or statement
allocation, and source/token/AST/statement/depth/execution/retained-state caps stay
unchanged. The option fixture uses ordinary semicolonless source with its existing
named inline handler and visible journey; acquired external scripts are not rewritten.
Public behavior tests and the independent Node reference corpus are authored here,
not imported V8/Test262 conformance suites. Installed Node v26.9.0 uses
V8 14.6.202.34-node.32, a different build from the inspected source pin; it is a
behavioral oracle, not a matching-pin engine or production dependency. No complete
ASI, scanner, ECMAScript, V8 or browser conformance follows from this adaptation.

### Document references and click listeners

The document bridge studies Blink at Chromium revision
`2c592105bbcd9490a9894df48d0fe59b2c512651`, read through Oya. Its BSD-style
[`event_target.cc`](https://chromium.googlesource.com/chromium/src/+/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/dom/events/event_target.cc)
listener snapshot/removed-record loop and
[`event_listener_map.cc`](https://chromium.googlesource.com/chromium/src/+/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/dom/events/event_listener_map.cc)
callback identity/order routines are the adaptation references. The snapshot
excludes new registrations and checks whether an original registration was
removed before invoking it. Rust uses bounded function identities and registration
IDs instead of Blink callback wrappers and listener vectors.

The full event-target source was acquired at the same pinned revision after the
Oya source view truncated it. Complete browser-read listener-map source retains
its original notices; its browser representation omits blank lines, so local
excerpt line numbers are not upstream line references.

[`tree_scope.cc`](https://chromium.googlesource.com/chromium/src/+/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/dom/tree_scope.cc)
and `tree_ordered_map.cc` show empty/missing lookup and first tree-order matching
element identity. Mapping that identity to a document-scoped Rust data reference
is our implementation design. The runtime supplies actual node metadata; the VM
does not infer a document from script source or receive agent handles.

The LGPL-covered Event, EventDispatcher, EventTarget IDL, RegisteredEventListener
and HTMLButtonElement files were inspected only for observable behavior. No
implementation from those files is copied or translated into the Rust bridge.
The inspected behavior includes nullable callbacks, sole event arguments,
current-target receivers and cleanup, readonly event fields, cancellation and
button default actions. The Chromium-authored `js_event_listener.cc` receiver
call was also inspected. [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) retains
the applicable BSD adaptation notices.

Deliberate differences: only explicit action-free `type="button"` controls,
click-only callable listeners, no options except omitted/undefined, no propagation
or synthetic dispatch, and no implicit DOM wrappers. Listener errors reject one
atomic candidate instead of continuing through full browser exception reporting.
Retaining detached-node references is unsupported and rolls back the candidate.
The public-boundary and real-service tests are authored here, not imported Blink
or web-platform conformance tests.

For each adapted algorithm, retain required upstream notices, name the exact
source routine and revision, record deliberate differences, and test through
the public Rust service. Test provenance must distinguish adapted upstream
cases from newly authored conformance cases. Passing a selected case set is
not a V8, Test262, Blink or web-platform conformance claim.

The Rust implementation carries typed scalar values, flat heap object IDs,
lexical bindings and DOM proposals through page load and native events. It is
source-guided, not a line-by-line translation of V8. The normative
[ToNumber/StringToNumber/ToString operations](https://tc39.es/ecma262/multipage/abstract-operations.html#sec-tonumber)
were reviewed for conversion requirements. The selected decimal cases in
`tests/unit/v9/expressions.rs` adapt the inspected decimal section of pinned
[number-tostring.js](https://github.com/v8/v8/blame/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/test/mjsunit/number-tostring.js);
the remaining expression/lexical cases there are newly authored.

`ryu-js` is pinned to 1.0.3. Its package and Buffer implementation were inspected
before integration; it supplies Number-to-string formatting only. Other numeric
syntax/conversions, UTF-16 storage, scanning, parsing and interpretation are
owned Rust code. [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) retains notices.

Deliberate limits: ASCII identifiers; bounded own data properties and sparse
arrays; no general object prototypes, getters/setters or object coercion; function declarations
only at program root or directly inside function bodies, plus anonymous/named
expressions with lexical closures;
no async arrows, rest/default/destructured parameters, primitive receiver wrappers, implicit
arguments objects, constructors, block functions, BigInt, templates,
regular expressions or Annex-B legacy octal. Automatic semicolon insertion is
limited to the supported classic-script grammar above. Unpaired
surrogates survive in VM state, but the current UTF-8 document boundary refuses
them explicitly. Full V8 semantics, broader DOM/CSS, media, profiles, Oya
workflow integration and production platform protections remain open.


## V9 unchanged mjsunit execution (October 2026)

The opt-in runner executes sources from the pinned V8 checkout at
`fedd021b538607860f9c56e5c193ad8dcd4f2bb1` without editing upstream tests.
Its existing directory and native-syntax exclusions remain separately reported;
`CONF_REQUIRE_PASS=1` rejects any executed failure. Sources retain their own
strict directives, declared fixture order, and real asynchronous assertions.
The shell host uses ordinary globals and inert, checkout-confined fixture text.
Realm and ShadowRealm use separate intrinsic banks and module instances;
termination stops the actual source/task pipeline. These host operations are
absent from production builds.

The current engine extends the earlier historical subsets above with mapped
arguments, instance private brands, descriptor and proxy invariants, sparse
resident array elements, species constructors, exact binary summation, Set
algebra, source-preserving functions, UTF-16-safe Unicode normalization,
binary16 storage, resizable/transferable buffers, precise weak edges and
deferred finalization, iterator helpers and joint iteration, Uint8Array codecs,
streaming grouping and typed-array construction. Resource regions use the
existing finally completion machinery and await individual disposal results;
they preserve abrupt completions and construct genuine SuppressedError chains.
Regex property escapes use complete ICU4X 2.3.0 Unicode data rather than
approximate character predicates. New regression tests live in `crates/v9/tests`; they do
not replace the unchanged upstream assertions. Object and resident-data limits,
operation deadlines and state validation remain enforced.

The owned WebAssembly implementation validates and executes its own binary
format and heap records. Scalar, SIMD, recursive GC references, exceptions and
atomic instructions on unshared memory have public-boundary regression tests.
Compile-time string imports operate on real UTF-16/UTF-8 data. Unsupported
instructions and host capabilities still produce failures. No external JS or
Wasm VM is used to obtain a pass. Selected-domain results are not full V8,
Test262, browser or WebAssembly conformance claims.


The expanded implementation includes signed limb BigInts with bounded allocation,
parameter/body environment separation, eval extension deletion, captured class
field keys, auto-accessors, and collection of unreachable dynamic source units.
Saved code retains immutable source offsets and validates nonoverlapping ranges
before recompilation. Date parsing follows the pinned V8 legacy token/composer
rules; time-zone transitions and POSIX recurrence rules are validated inert host
input, while the native VM itself has no zoneinfo filesystem access.

RegExp matching uses backward evaluation for lookbehind, whole-code-point Unicode
backreferences and leading-anchor search bounds. Simple fold data comes from
regex-syntax 0.8.11's generated Unicode 16.0 table, supplemented by the mutually
inverse single-scalar case pairs from Rust 1.94.0's Unicode 17.0 data. The Unicode
[CaseFolding 17.0 source](https://www.unicode.org/Public/17.0.0/ucd/CaseFolding.txt)
distinguishes common/simple mappings from expanding and Turkic mappings; the
matcher uses single-scalar equivalence. Notices are retained separately.
Buffer-boundary assertions require the explicit `regexp_buffer_boundaries`
environment setting; the runner forwards the source's actual proposal flag.

The shell serializer uses V8's actual value-serialization tags and varints rather
than a JSON envelope, with bounds and graph validation before materialization.
Promise hooks observe actual lifecycle and job execution. WebAssembly suspension
retains owned, typed interpreter frames and resumes through genuine Promise
reactions without replaying imports or draining jobs synchronously. Unsupported
capabilities remain explicit.

This inventory deliberately keeps the original exclusions unchanged. It also
contains upstream tests with conflicting configuration assumptions: the pinned
`mjsunit.status` marks `bugs/*` as expected FAIL, marks the no-i18n RegExp case FAIL
when i18n is enabled, and skips `regress/wasm/regress-1254675` when SIMD is
available. These failures are reported in the fixed inventory; V9 must not adopt
obsolete semantics or reject valid SIMD to manufacture a passing assertion.
