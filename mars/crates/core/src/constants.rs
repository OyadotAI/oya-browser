/** Explicit resource budgets for the native Rust browser runtime. */
use std::time::Duration;

/// Wire compatibility is independent of the package version.
pub const VERSION: u32 = 2;
/// Includes JSON escaping and the terminating newline. The largest frames are the styled
/// Document beside its stylesheets and the script projection beside its sources: NODES *
/// NODE_WIRE_BYTES (8 MiB) plus worst-case six-byte escaping of SCRIPT_TOTAL_BYTES or
/// STYLE_TOTAL_BYTES plus the other source-derived strings stays under this 128 MiB frame
/// (asserted below). It is a ceiling checked as bytes arrive, never a preallocated buffer.
pub const FRAME_BYTES: usize = 512 * 1024 * 1024;
/// The measured upper bound on one styled node's JSON, used only by the frame assert.
pub const NODE_WIRE_BYTES: usize = 1024;
/** The first native MCP adapter explicitly supports this initialization-based protocol revision. */
pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";
/** MCP returns native inspection JSON as both structured content and escaped text; 16 MiB fits any inspection reply without allocating multi-gigabyte stdio buffers. */
pub const MCP_FRAME_BYTES: usize = 16 * 1024 * 1024;
/** Whole-session request identities remain unique without an unbounded replay-tracking set. */
pub const MCP_SESSION_REQUESTS: usize = 4096;
/** Human and agent commands share one bounded coordinator admission queue. */
pub const DESKTOP_QUEUE: usize = 16;
/** Cancelled calls retain a slot until their native reply has been drained. */
pub const MCP_PENDING_REQUESTS: usize = DESKTOP_QUEUE;
/** Protocol output cannot grow indefinitely behind an unresponsive client. */
pub const MCP_OUTPUT_QUEUE: usize = DESKTOP_QUEUE;
/** Initialization must complete even if a client keeps pinging an otherwise idle connection. */
pub const MCP_INITIALIZE_DEADLINE: Duration = Duration::from_secs(30);
/** Native request deadlines include coordinator admission and queue delay. */
pub const MCP_REQUEST_DEADLINE: Duration = Duration::from_secs(30);
/** Partial output progress cannot keep a blocked stdio connection alive indefinitely. */
pub const MCP_WRITE_DEADLINE: Duration = Duration::from_secs(2);
/// Committed V9 state (heap, globals, platform) as JSON. Wikipedia's startup needs 488 KB and
/// five large libraries together 459 KB; YouTube's app holds 19 MB once its page renders, so
/// 64 MiB leaves room for what it builds while a person browses.
pub const VM_STATE_BYTES: usize = 64 * 1024 * 1024;
/// Origin storage has an independent bounded serialized profile record.
pub const WEB_STORAGE_PROFILE_BYTES: usize = 512 * 1024;
/// A bounded encrypted record stores at most the active native tab session.
pub const SESSION_PROFILE_BYTES: usize = 64 * 1024;
/// Versioned tab-session records reject incompatible restore semantics.
pub const SESSION_PROFILE_VERSION: u32 = 1;
/// Versioned secure Web Storage records reject unknown future semantics.
pub const WEB_STORAGE_PROFILE_VERSION: u32 = 1;
/// A single Web Storage value remains small enough for a bounded page candidate.
pub const STORAGE_VALUE_BYTES: usize = 16 * 1024;
/// Each origin area has a small explicit entry ceiling.
pub const STORAGE_ENTRIES: usize = 256;
/// Each local or session area is bounded independently.
pub const STORAGE_AREA_BYTES: usize = 64 * 1024;
/// Limits a fetched page before parsing or interpretation; 16 MiB fits cnn.com's 6.9 MB homepage.
pub const HTML_BYTES: usize = 16 * 1024 * 1024;
/// One image transfer: 2 MiB fits real hero photos and stays small inside one private JSON frame.
pub const IMAGE_BYTES: usize = 2 * 1024 * 1024;
/// A committed page owns a bounded number of image resources; news front pages show dozens.
pub const IMAGE_COUNT: usize = 64;
/// Encoded bytes all of a page's images may hold together, separate from the per-image bound.
pub const IMAGE_TOTAL_BYTES: usize = 16 * 1024 * 1024;
/// Decoded bitmap pixels (4 bytes each) a page's images may hold together.
pub const IMAGE_DECODED_BYTES: usize = 128 * 1024 * 1024;
/// Image dimensions prevent pathological decoder allocations; 8192 admits 2x hero images, and
/// IMAGE_DECODED_BYTES stays the real bound on what one image may decode to.
pub const IMAGE_DIMENSION: u32 = 8192;
/// Table grids stay below document and display-list caps; Hacker News lays out about 90 rows.
pub const TABLE_ROWS: usize = 256;
/// A wide grid cannot consume the full node budget with empty cells.
pub const TABLE_COLUMNS: usize = 16;
/// Cell allocations remain independently bounded per table.
pub const TABLE_CELLS: usize = TABLE_ROWS * TABLE_COLUMNS;
/// Headers have a separate budget from the body; github sends 10.9 KB and youtube 12.3 KB.
pub const HEADER_BYTES: usize = 32 * 1024;
/// Chunk-size lines and CRLFs share their own budget, allowing chunks averaging about 150 B at the
/// maximum body (HTML_BYTES / 16 = 128 KiB of framing).
pub const CHUNK_FRAMING_BYTES: usize = HTML_BYTES / 16;
/// Bounds snapshots and layout work; wikipedia has about 4051 tags. FRAME_BYTES, PAINT_LINES
/// and LAYOUT_WORK derive from it.
pub const NODES: usize = 32768;
/// Editable values, attributes and script/native text writes share one per-node bound; 64 KiB
/// lets a script fill a text area or a status line the way real pages do.
pub const TEXT_BYTES: usize = 64 * 1024;
/// Ordinary reading text can exceed an editable field without raising any VM or mutation quota.
/// It stays above TEXT_BYTES so a script or a person can fill any node a page can hold.
pub const STATIC_TEXT_NODE_BYTES: usize = 2 * TEXT_BYTES;
/// Stored static text shares one document allowance before serialization or measurement.
/// Readable text is a fraction of source; a quarter of HTML_BYTES bounds worst-case six-byte
/// JSON escaping to 3 MiB of frame.
pub const STATIC_TEXT_TOTAL_BYTES: usize = HTML_BYTES / 4;
/// Painted lines scale with the node budget so a flat maximum-size page fits.
pub const PAINT_LINES: usize = NODES * 4;
/// Style rules per page. Real sites ship thousands (wikipedia, python.org, amazon), and the
/// cascade buckets rules by id, class and tag, so matching stays cheap even at this bound.
pub const CSS_RULES: usize = 65536;
/// Compatibility facades use the same initial page viewport as the native coordinator.
pub const DEFAULT_VIEWPORT_WIDTH: f64 = 1100.0;
/// The Mars shell (toolbar, tabs, status) is excluded when the native window later supplies its actual viewport.
pub const DEFAULT_VIEWPORT_HEIGHT: f64 = 680.0;
/// The error banner must preserve a usable page viewport rather than collapse it to zero.
pub const MIN_VIEWPORT_HEIGHT: f64 = 120.0;
/// The existing native normal line advance remains stable for unstyled documents.
pub const CSS_NORMAL_LINE_HEIGHT: f64 = 1.5;
/// Limits one script's source before tokenization; 12 MiB fits the largest real bundles seen
/// (CNN inlines 2.3 MB, YouTube's app bundle is several MB). A larger script is skipped, never page-fatal.
pub const SCRIPT_BYTES: usize = 12 * 1024 * 1024;
/// Bounds the number of executable scripts accepted in one candidate document; later ones are
/// skipped with a diagnostic. Chromium has no count cap, and real apps ship many chunks (Twitch
/// loads 78), so this sits well past them; SCRIPT_TOTAL_BYTES still bounds the work.
pub const SCRIPT_COUNT: usize = 512;
/// All of a page's sources cross the V9 pipe in one frame, so they share this allowance; a
/// script that would cross it is skipped and the page keeps the ones before it.
pub const SCRIPT_TOTAL_BYTES: usize = 2 * SCRIPT_BYTES;
/// JavaScript modules one page may load, statically imported and dynamically imported together;
/// github.com's home page loads 77. A module past it fails its graph, never the page.
pub const MODULES: usize = 256;
/// Every fetched module's source crosses the V9 pipe beside the classic sources on each request,
/// so they share this allowance (github.com's graph is 6.7 MB); one module stays under
/// SCRIPT_BYTES. A module past either bound fails its graph.
pub const MODULE_BYTES: usize = 16 * 1024 * 1024;
/// Compiled code the V9 process keeps between requests, in bytes of bytecode, strings and
/// positions: a page's lifecycle is many requests over one set of sources, and recompiling them
/// each time cost more than running them. YouTube's 10.8 MB app bundle compiles to 74 MB, so
/// this holds two such pages' worth (the one open and the one just left); the least recently
/// used unit goes first.
pub const CODE_CACHE_BYTES: usize = 256 * 1024 * 1024;
/// Specifier entries across a page's import maps (imports and every scope together); entries past
/// it are ignored, as an invalid entry is.
pub const IMPORT_MAP_ENTRIES: usize = 4096;
/// Bounds parsed statements per script, interpreter variables and proposed mutations. Minified
/// bundles average about one statement per 30 source bytes, so a thirty-second of SCRIPT_BYTES
/// admits any script the source bound admits (YouTube's 10.8 MB app bundle needs over 65536).
pub const STATEMENTS: usize = SCRIPT_BYTES / 32;
/// Bounds one script's tokens and AST nodes. Every token is at least one byte and minified code
/// averages three to four, so half of SCRIPT_BYTES admits any realistic bundle.
pub const TOKENS: usize = SCRIPT_BYTES / 2;
/// Bounds one script's (or one event's handlers') interpreter work; SCRIPT_DEADLINE stops it
/// sooner on slow hardware. YouTube's app runs past 50 million operations initializing its
/// components after load, so this admits a few seconds of optimized work.
pub const OPERATIONS: usize = 400_000_000;
/// Wall-clock guard for one V9 request (all of a page's scripts, or one click). It sits under
/// RPC_TIMEOUT so decoding and the reply still fit before the coordinator gives up on the service.
/// Loading YouTube's 10.8 MB app bundle and the scripts around it takes over 1.5 s in an
/// optimized build, so the guard allows what a real page load needs on this hardware.
pub const SCRIPT_DEADLINE: Duration = Duration::from_millis(4000);
/// The interpreter reads the clock once per this many operations, keeping the guard cheap.
pub const SCRIPT_CLOCK_STRIDE: usize = 4096;
/// Text and attribute bytes one V9 request may propose in DOM changes, so STATEMENTS changes of
/// TEXT_BYTES each cannot outgrow the reply frame.
pub const SCRIPT_CHANGE_BYTES: usize = 4 * 1024 * 1024;
/// Parsing and evaluation recurse along the syntax tree; this bounds the combined depth across
/// statements, expressions and nested functions so the V9 thread stack always suffices.
pub const SYNTAX_DEPTH: usize = 1024;
/// V9 runs each request on its own thread with this stack, sized for SYNTAX_DEPTH levels of
/// parsing, evaluation, cloning and dropping in unoptimized builds.
pub const V9_STACK_BYTES: usize = 512 * 1024 * 1024;
/// Numeric update operators add or subtract one after supported primitive conversion.
pub const JS_UPDATE_STEP: f64 = 1.0;
/// Registered click callbacks retain functions and must not grow the document graph without bound.
pub const JS_EVENT_LISTENERS: usize = 64;
/// Retained event facades count toward a separate per-document allocation allowance.
pub const JS_EVENT_RECORDS: usize = 64;
/// Limits control input before URL parsing. Google's sign-in reports errors to a URL that
/// embeds the whole page address, past 2 KiB; 8 KiB is what common servers accept.
pub const URL_BYTES: usize = 8 * 1024;
/// Limits response correlation identifiers.
pub const ID_BYTES: usize = 128;
/// Absolute deadline for a complete HTTP exchange.
pub const NETWORK_TIMEOUT: Duration = Duration::from_secs(2);
/// Covers child writes and reads, not just response reads; a second past SCRIPT_DEADLINE for
/// decoding the request and encoding the reply.
pub const RPC_TIMEOUT: Duration = Duration::from_secs(5);
/// Successful status supported by the minimal HTTP implementation.
pub const HTTP_OK: u16 = 200;
/// Protocol separator for HTTP headers and body.
pub const HEADER_END: &[u8] = b"\r\n\r\n";
/// Small socket reads keep transient allocation below the complete response budget.
pub const NETWORK_CHUNK_BYTES: usize = 1024;
/// Native layout coordinates count Unicode scalars, not pixels.
pub const LAYOUT_UNIT: &str = "unicode_scalar_cells";
/// Desktop web requests include TLS and redirects within one absolute budget.
pub const WEB_TIMEOUT: Duration = Duration::from_secs(8);
/// The external supervisor also bounds uncancellable operating-system DNS calls.
pub const WEB_RPC_TIMEOUT: Duration = Duration::from_secs(10);
/// Redirects must revalidate destinations and cannot create an unbounded chain.
pub const REDIRECTS: usize = 5;
/// Excessive or mixed DNS answer sets are rejected rather than partially trusted.
pub const DNS_ANSWERS: usize = 16;
/// Explicit supported markup depth prevents recursive parser/layout exhaustion.
pub const DOCUMENT_DEPTH: usize = 32;
/// Committed history is a bounded local session resource.
pub const HISTORY_ENTRIES: usize = 64;
/// Native windows cap retained page documents to bound aggregate tab memory.
pub const TABS: usize = 16;
/// Cancellation stays responsive even while a worker waits for a child reply.
pub const RPC_POLL: Duration = Duration::from_millis(25);
/// A child can detect coordinator death independently of a blocked engine dispatch.
pub const OWNER_PID_ENV: &str = "OYA_NATIVE_OWNER_PID";
/// One stylesheet's bytes Eye applies in full; CNN inlines a 2.94 MB sheet. A larger sheet still
/// reaches Eye, which keeps only its hiding rules (as agent masks) or fails closed for agents.
pub const STYLE_BYTES: usize = 4 * 1024 * 1024;
/// Linked sheets fetched for one candidate; later ones are skipped with a diagnostic. Chromium
/// has no count limit, and The Verge links 66 sheets, so this is a sanity bound well above real
/// pages; STYLE_TOTAL_BYTES still caps what they carry together.
pub const STYLE_SHEETS: usize = 256;
/// All of a page's sheets cross the document pipe together, so they share this allowance (the
/// largest the frame assert below admits beside a maximum document). Sheets past it cannot
/// reach Eye, so the runtime fails that page closed for agents instead of dropping hiding rules.
pub const STYLE_TOTAL_BYTES: usize = STYLE_BYTES + STYLE_BYTES / 2;
/// Submission data is bounded independently of document source: what a form or a script may
/// send in one request body (Google's sign-in posts 20 KiB error reports), the same bound V9
/// applies before a request leaves the page.
pub const FORM_BYTES: usize = VM_REQUEST_BODY_BYTES;
/// Deprecated: Relay's jar no longer uses this; it keeps up to 3000 cookies.
pub const COOKIE_COUNT: usize = 64;
/// Deprecated: Relay's jar no longer uses this per-host bound.
pub const HOST_COOKIES: usize = 16;
/// A cookie cannot grow a request beyond the framing budget.
pub const COOKIE_BYTES: usize = 4096;
/// Deprecated: Relay bounds its jar with its own COOKIE_JAR_BYTES.
pub const COOKIE_JAR_BYTES: usize = 32 * 1024;
/// One saved jar record: at least Relay's own bound (1 MiB of cookies plus framing for 3000).
pub const COOKIE_PROFILE_BYTES: usize = 1024 * 1024 + 3000 * 192;
/// A stuck OS Keychain request cannot hold the browser worker or shutdown indefinitely.
pub const PROFILE_STORE_TIMEOUT: Duration = Duration::from_secs(3);
/// Resource and compatibility reports never grow with arbitrary source features.
pub const DIAGNOSTICS: usize = 32;
/// Recursive parsing and AST evaluation share an independently checked depth bound; minified
/// string concatenations and call chains need a few hundred levels.
pub const EXPRESSION_DEPTH: usize = 256;
/// Blocks and single-statement control bodies cannot exhaust the parser or evaluator stack.
pub const STATEMENT_DEPTH: usize = 128;
/// UTF-16 code units of one JavaScript string or literal. Google's inline scripts carry
/// literals past 2048 units; a whole script response body (VM_RESPONSE_BYTES) must fit too, and
/// YouTube serializes its initial data (over 256K units) into one string while loading.
pub const JS_STRING_UNITS: usize = JS_ENVIRONMENT_UNITS / 2;
/** A leading UTF-16 surrogate combines only with an immediately following trailing surrogate. */
pub const UTF16_HIGH_SURROGATE_START: u16 = 0xd800;
/** The inclusive upper bound prevents low surrogates from being treated as leading units. */
pub const UTF16_HIGH_SURROGATE_END: u16 = 0xdbff;
/** Trailing surrogates remain independent when no leading surrogate immediately precedes them. */
pub const UTF16_LOW_SURROGATE_START: u16 = 0xdc00;
/** This inclusive endpoint retains all valid UTF-16 surrogate pairs without decoding them. */
pub const UTF16_LOW_SURROGATE_END: u16 = 0xdfff;
/// Live string units across the heap. Wikipedia's startup holds 48714 and five large libraries
/// together 36868; YouTube's loaded app holds 723072, and this leaves room for two whole
/// response bodies beside that.
pub const JS_ENVIRONMENT_UNITS: usize = 2 * 1024 * 1024;
/// Live objects. Google's main bundle and YouTube's app need far more than Wikipedia's 5204:
/// YouTube holds 47869 once loaded and allocates as it renders, and LinkedIn's React app keeps
/// more than 262144 alive while it hydrates (Chromium has no count bound at all, only memory).
/// VM_STATE_BYTES stays the memory cap on what a page commits; this only stops a runaway run.
pub const VM_OBJECTS: usize = 1_048_576;
/** The first allocation is the page's permanent global receiver, never a replaceable supplied object. */
pub const VM_GLOBAL_OBJECT_ID: u32 = 0;
/// Live properties. Google's main bundle needs more than 131072 and YouTube's loaded app holds
/// 225108 before it renders anything; four per object at the old 262144-object bound keeps the
/// ratio real pages show (many objects a large app keeps are small: arrays, fibers, closures' data).
pub const VM_PROPERTIES: usize = 1_048_576;
/// UTF-16 units of one property key; CNN's inline scripts use keys past 64 units, and YouTube's
/// app keys its caches by whole request URLs and serialized bodies past 1024.
pub const VM_PROPERTY_KEY_UNITS: usize = 64 * 1024;
/// Elements of one array. Wikipedia's startup builds a module registry past 1024 entries and
/// YouTube's app bundle builds tables past 16384 while loading.
pub const VM_ARRAY_LENGTH: usize = 262_144;
/// Parameters per function and arguments per call; minified helpers occasionally pass dozens.
pub const FUNCTION_PARAMETERS: usize = 64;
/// Retained lexical contexts and within-request allocations have explicit independent ceilings.
/// Five large libraries together hold 680 live environments. Every live closure may hold its
/// own captured environment, so this is at least VM_FUNCTIONS.
pub const VM_ENVIRONMENTS: usize = VM_FUNCTIONS;
/// Closures are records rather than serialized executable callbacks. Five large libraries
/// together hold 2458 live functions, but YouTube's app keeps over 65536 alive once it renders
/// (one per module-level function, then one per bound handler of every stamped component); kept
/// at the old object bound, since environments and bindings scale with it.
pub const VM_FUNCTIONS: usize = 262_144;
/// Function literals one page's sources may contain, fetched scripts included; YouTube's app
/// bundle and the chunks it loads declare tens of thousands.
pub const VM_CODE_SITES: usize = 131_072;
/// Captured cells cannot multiply the per-variable string budget without bound. Five large
/// libraries together hold 3682 live bindings; YouTube's app bundle captures more than 16384
/// while loading (every module closure keeps its module's locals).
pub const VM_BINDINGS: usize = 2 * VM_FUNCTIONS;
/// Function syntax nesting is bounded independently of calls and ordinary expression grouping;
/// bundlers wrap modules in several function layers.
pub const FUNCTION_NESTING: usize = 64;
/// Callable helpers cannot expand one invocation beyond the parser's argument ceiling.
pub const VM_CALL_ARGUMENTS: usize = FUNCTION_PARAMETERS;
/// Binding chains are checked iteratively before evaluation or private-state publication.
pub const VM_BOUND_HOPS: usize = 16;
/// Canonical intrinsic identities are page-owned and cannot be selected by serialized code.
pub const VM_FUNCTION_CONSTRUCTOR_ID: u32 = 0;
pub const VM_FUNCTION_PROTOTYPE_ID: u32 = 1;
pub const VM_FUNCTION_CALL_ID: u32 = 2;
pub const VM_FUNCTION_APPLY_ID: u32 = 3;
pub const VM_FUNCTION_BIND_ID: u32 = 4;
/// The document lookup method has one stable owned function identity across native events.
pub const VM_GET_ELEMENT_BY_ID_ID: u32 = 5;
/// Listener registration remains a first-class function without exposing native authority.
pub const VM_ADD_EVENT_LISTENER_ID: u32 = 6;
/// Removal shares the callable layer and compares the original callback identity.
pub const VM_REMOVE_EVENT_LISTENER_ID: u32 = 7;
/// Event cancellation changes only a bounded candidate event record.
pub const VM_PREVENT_DEFAULT_ID: u32 = 8;
/// Source closures allocate beyond every canonical callable reserved by this build.
pub const VM_FUNCTION_INTRINSICS: u32 = 19;
/** String trimming shares one callable identity throughout the page realm. */
pub const VM_STRING_TRIM_ID: u32 = 9;
/** Literal splitting is a bounded owned builtin, not a parser-special call. */
pub const VM_STRING_SPLIT_ID: u32 = 10;
/** Array mapping preserves callback and result identities through native events. */
pub const VM_ARRAY_MAP_ID: u32 = 11;
/** Array filtering shares normal callback invocation and candidate rollback. */
pub const VM_ARRAY_FILTER_ID: u32 = 12;
/** Array joining uses the same UTF-16 output bounds as other string operations. */
pub const VM_ARRAY_JOIN_ID: u32 = 13;
/** Array existence predicates keep one first-class identity across requests. */
pub const VM_ARRAY_SOME_ID: u32 = 14;
/** Array selection returns an existing value without allocating another array. */
pub const VM_ARRAY_FIND_ID: u32 = 15;
/** Type-selector queries share one callable identity across Document, Body and Element hosts. */
pub const VM_QUERY_SELECTOR_ALL_ID: u32 = 16;
/** Fragment insertion goes through one owned callable so its bounds apply on every receiver. */
pub const VM_INSERT_ADJACENT_HTML_ID: u32 = 17;
/** Subtree moves to first child share one callable identity across hosts. */
pub const VM_PREPEND_ID: u32 = 18;
/** ECMAScript ToUint32 wraps finite truncated numbers at this exact power of two. */
pub const JS_UINT32_MODULUS: f64 = 4_294_967_296.0;
/** Linear string scans consume request work in bounded groups of UTF-16 units. */
pub const JS_STRING_SCAN_UNITS: usize = 64;
/** Repeated measurement and final placement share one finite per-document work allowance. */
pub const LAYOUT_WORK_PER_NODE: usize = 128;
/** Measuring, wrapping and placing one stored text byte charges a few units across the passes. */
pub const LAYOUT_WORK_PER_TEXT_BYTE: usize = 8;
/**
 * Nested flex measurement cannot multiply work beyond this shared request budget. It covers a
 * page at both NODES and STATIC_TEXT_TOTAL_BYTES: 1,048,576 for nodes plus 4,194,304 for text.
 */
pub const LAYOUT_WORK: usize =
    NODES * LAYOUT_WORK_PER_NODE + STATIC_TEXT_TOTAL_BYTES * LAYOUT_WORK_PER_TEXT_BYTE;
/**
 * Distinct native text measurements per layout. Each costs a fixed AppKit call (about 20 us)
 * that per-scalar work undercounts, so cache-defeating text is refused inside the render budget.
 */
pub const LAYOUT_MEASURES: usize = 16_384;
/** Native positions and extents must remain finite and inside this logical-point bound. */
pub const LAYOUT_EXTENT: f64 = 1_048_576.0;
/** Flexible-size freezing ignores only numerical noise beneath this tolerance. */
pub const LAYOUT_EPSILON: f64 = 1e-7;
/** CSS numeric inputs remain within the finite geometry range; larger lengths clamp to it. Long
 * pages and reftests set `height: 10000px`, which a 4096 bound dropped. */
pub const CSS_MAX_SCALAR: f64 = 65_536.0;
/** Native text remains readable under the supported bounded CSS font range. */
pub const CSS_MIN_FONT_SIZE: f64 = 8.0;
/** Upper font sizing limits bound glyph measurement and line allocation. Reftests draw Ahem
 * squares at 100px and 200px, so a 72px cap shrank every one of them. */
pub const CSS_MAX_FONT_SIZE: f64 = 1024.0;
/** Unitless line-height factors stay bounded through inheritance (the old 4096 / 72 range). */
pub const CSS_MAX_LINE_HEIGHT_FACTOR: f64 = 56.0;
/** Border widths share the finite geometry bound; Blink caps none below it, and paint work
 * already scales with the bounded box, so a 96px reftest border paints whole. */
pub const CSS_MAX_BORDER_WIDTH: f64 = CSS_MAX_SCALAR;
/** Font-relative CSS lengths share the owned default root font size. */
pub const CSS_ROOT_FONT_SIZE: f64 = 16.0;
/// ToLength clamps to ECMAScript's maximum safe integer before the smaller invocation cap applies.
pub const VM_MAX_SAFE_LENGTH: f64 = 9_007_199_254_740_991.0;
/** One script-inserted fragment keeps its own bound, independent of script sources: a node's
text bound, since YouTube's component templates are tens of kilobytes of markup each. */
pub const FRAGMENT_BYTES: usize = TEXT_BYTES;
/** All fragments one execution inserts share a budget that still fits the reply frame; a
component framework registers hundreds of templates while its bundle loads. */
pub const FRAGMENT_TOTAL_BYTES: usize = SCRIPT_CHANGE_BYTES;
const _: () = assert!(VM_STATE_BYTES + STORAGE_AREA_BYTES + FRAGMENT_TOTAL_BYTES < FRAME_BYTES);
/* A maximum-size styled Document plus source-derived strings crosses the private pipe. */
const _: () =
    assert!(NODES * NODE_WIRE_BYTES + 2 * HTML_BYTES + 6 * STATIC_TEXT_TOTAL_BYTES <= FRAME_BYTES);
/* Decoded source with worst-case \u00XX escaping crosses the network pipe. */
const _: () = assert!(6 * HTML_BYTES < FRAME_BYTES);
/* One image fits the page total, and the page's images, base64-encoded, fit one engine frame. */
const _: () = assert!(IMAGE_BYTES <= IMAGE_TOTAL_BYTES && IMAGE_TOTAL_BYTES / 3 * 4 < FRAME_BYTES);
/* STATEMENTS changes of the smallest wire size (about 64 bytes each) fit one reply frame. */
const _: () = assert!(STATEMENTS * 64 < FRAME_BYTES);
/* A script fetched over the network pipe is bounded by the body limit before SCRIPT_BYTES. */
const _: () = assert!(SCRIPT_BYTES <= HTML_BYTES);
/* The script projection, every escaped source, the escaped node text, the VM state, storage
and one step's escaped responses cross in one V9 frame. */
const _: () = assert!(
    NODES * NODE_WIRE_BYTES
        + 6 * SCRIPT_TOTAL_BYTES
        + 6 * MODULE_BYTES
        + 6 * STATIC_TEXT_TOTAL_BYTES
        + 6 * METADATA_BYTES
        + 2 * VM_STATE_BYTES
        + 2 * STORAGE_AREA_BYTES
        + SCRIPT_CHANGE_BYTES
        + 6 * SCRIPT_REQUESTS_PER_STEP * VM_RESPONSE_BYTES
        <= FRAME_BYTES
);
/* One module fits the per-script source bound, and the graph's sources fit their own total. */
const _: () = assert!(SCRIPT_BYTES <= MODULE_BYTES && MODULES <= MODULE_BYTES);
/* Each live closure can keep its own environment alive. */
const _: () = assert!(VM_ENVIRONMENTS >= VM_FUNCTIONS);
/* No single string can exceed what the live heap may hold. */
const _: () = assert!(JS_STRING_UNITS <= JS_ENVIRONMENT_UNITS);
/* The styled Document crosses beside every escaped stylesheet in one document frame. */
const _: () = assert!(
    NODES * NODE_WIRE_BYTES + 2 * HTML_BYTES + 6 * STATIC_TEXT_TOTAL_BYTES + 6 * STYLE_TOTAL_BYTES
        <= FRAME_BYTES
);
/* The V9 guard must fire before the coordinator abandons the service call. */
const _: () = assert!(SCRIPT_DEADLINE.as_millis() < RPC_TIMEOUT.as_millis());
/** navigator.languages exposes at most this many preferred language tags. */
pub const NAVIGATOR_LANGUAGES: usize = 16;
/** BCP 47 recommends implementations accept language tags of at least this length. */
pub const LANGUAGE_TAG_BYTES: usize = 35;
/** One SVG path cannot request unbounded native drawing work. */
pub const SVG_PATH_SEGMENTS: usize = 1024;
/** An SVG `use` may reference an element but never another `use`. */
pub const SVG_USE_DEPTH: usize = 1;
/// Bytecode frames one V9 request may nest; bundles recurse well past a few dozen levels.
pub const VM_CALL_DEPTH: usize = 512;
/// Promise jobs and queued microtasks one page may hold at once.
pub const VM_PENDING_JOBS: usize = 4096;
/// Pending setTimeout, setInterval and requestAnimationFrame callbacks one page may hold;
/// YouTube's app keeps hundreds scheduled while it renders.
pub const VM_TIMERS: usize = 4096;
/// fetch and XMLHttpRequest calls waiting on the app at once; later calls fail like a network error.
pub const VM_HOST_REQUESTS: usize = 32;
/// Bytes of one request body a script may send through the app.
pub const VM_REQUEST_BODY_BYTES: usize = 64 * 1024;
/// Bytes of one response body the app may hand back to a script.
pub const VM_RESPONSE_BYTES: usize = 256 * 1024;
/// Script requests the app performs per V9 round trip; later ones fail like a network error.
/// YouTube fetches its icon set in one burst of over thirty requests.
pub const SCRIPT_REQUESTS_PER_STEP: usize = 48;
/// Console entries one execution returns; later ones are counted, not kept.
pub const VM_CONSOLE_ENTRIES: usize = DIAGNOSTICS;
/// Bytes of one console entry's text.
pub const VM_CONSOLE_BYTES: usize = 1024;
/// Symbols one page may create beyond the well-known ones.
pub const VM_SYMBOLS: usize = 1024;
/// Backtracking steps one regular expression match may take before it fails as a limit.
pub const REGEXP_STEPS: usize = 1_000_000;
/// Nodes in one compiled regular expression.
pub const REGEXP_NODES: usize = 16_384;
/// Capture groups in one regular expression.
pub const REGEXP_GROUPS: usize = 256;
/// Virtual milliseconds between animation frames, as on a 60 Hz display.
pub const ANIMATION_FRAME_MS: f64 = 16.0;
/// Browsers clamp repeating and deeply nested timers to this delay in milliseconds.
pub const TIMER_MIN_DELAY_MS: f64 = 4.0;
/// Navigation, history and cookie writes one execution records for the app.
pub const VM_HOST_RECORDS: usize = 64;
/// Dynamic sources (new Function, indirect eval, and inline scripts inserted by script) one
/// page may compile; YouTube's app inserts dozens while loading.
pub const VM_DYNAMIC_SOURCES: usize = 512;
/// Bytes of all dynamic sources one page keeps so later requests can recompile them; a page
/// that inserts inline scripts needs far more than the 64 KiB `new Function` alone did.
pub const VM_DYNAMIC_BYTES: usize = 2 * 1024 * 1024;
/// Nodes a script may create outside the document before inserting them: as many as the
/// document itself may hold, since a component framework keeps every element's template
/// content detached (YouTube's app registers hundreds of templates while loading).
pub const VM_DETACHED_NODES: usize = NODES;
/// Metadata elements (`meta`, `link`, `style`, `title`, `base`, inert `script`) one page gives
/// V9 as head records, leaving most of VM_DETACHED_NODES for the nodes scripts create.
pub const METADATA_ELEMENTS: usize = 256;
/// Bytes of attribute values and text all of one page's metadata elements carry to V9 together;
/// past it an element keeps no text, or is left out when its attributes alone do not fit.
pub const METADATA_BYTES: usize = 1024 * 1024;
/// Event listeners one page may register on nodes, the document and the window together.
pub const VM_LISTENERS: usize = 512;
/// Bytes of the `document.cookie` string the app supplies, and of one cookie a script writes.
pub const VM_DOCUMENT_COOKIE_BYTES: usize = 4096;
