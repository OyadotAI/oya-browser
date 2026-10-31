/**
 * The browser platform V9 gives page scripts: window, document, navigator, location, history,
 * timers, events, storage, console, fetch and XMLHttpRequest. Nothing here has native authority:
 * the document is the app's inert projection, network calls and navigations are recorded for the
 * app to perform. Timer scheduling is virtual; native hosts may enable script-visible live time.
 */
pub(in crate::v9) mod animations;
pub(in crate::v9) mod layout_host;
pub use animations::{AnimationTarget, OpacityEffect};
pub use layout_host::{LAYOUT_CALLS, LayoutAnswer, LayoutHook, LayoutHost};
pub(in crate::v9) mod blob;
pub(in crate::v9) mod blob_urls;
pub(in crate::v9) mod broadcast;
pub(in crate::v9) mod canvas;
pub(in crate::v9) mod channel;
mod chrome_env;
mod chrome_shapes;
pub(in crate::v9) mod clock;
pub(in crate::v9) mod collections;
pub(in crate::v9) mod components;
mod console;
/** DevTools evaluation (Runtime.evaluate and friends) over the page's realm; new entry points only. */
pub mod devtools;
mod dom;
mod dom_parser;
mod encoding;
pub(in crate::v9) mod events;
mod form_data;
mod form_submit;
mod geometry;
mod handler_policy;
pub(in crate::v9) mod host_bridge;
pub use handler_policy::{HANDLER_POLICY_LIMIT, HandlerHash, HandlerPolicy, HandlerRestriction};
pub(in crate::v9) mod frames;
pub(in crate::v9) mod html;
pub(in crate::v9) mod images;
mod implementation;
pub(in crate::v9) mod indexed_db;
mod install;
mod interfaces;
pub(in crate::v9) mod intersection_observers;
mod keyboard;
pub(in crate::v9) mod messaging;
pub(in crate::v9) mod resize_observers;
mod xml_frames;
mod xml_serializer;
pub(in crate::v9) use keyboard::press_key;
mod attr;
mod attr_dom;
mod listener_store;
mod namespaces;
mod net;
pub use blob_urls::BlobUrlSource;
pub(in crate::v9) use net::validate_state as validate_network_state;
pub(in crate::v9) use net::validate_summary as validate_network_summary;
mod nodes;
pub(in crate::v9) mod observable;
mod offsets;
mod performance;
mod plugins;
mod pseudo_attributes;
#[cfg(feature = "conformance")]
pub(in crate::v9) mod random;
mod range;
mod range_contents;
mod range_index;
mod range_live;
pub(in crate::v9) mod scripts;
mod select;
mod trusted_types;
pub(in crate::v9) use trusted_types::trusted_script_text;
pub(in crate::v9) mod frame_directory;
mod scrolling;
mod selector;
mod sheets;
pub(in crate::v9) use sheets::publish_local as publish_local_sheets;
pub(in crate::v9) mod media;
mod timers;
mod token_list;
mod token_set;
mod url_bindings;
mod url_parser;
mod window;
pub(in crate::v9) mod worker_global;
pub(in crate::v9) mod workers;

pub(in crate::v9) use dom::{focus, named_element, native_name};
pub use html::SimpleFragmentParser;
pub use images::{ImageAdoption, ImageCancellation, ImageCors, ImageRequest, ImageResponse};
pub use media::{MediaCommand, MediaSettings, MediaSupport};

pub(in crate::v9) use install::install;
pub(in crate::v9) use net::XhrState;
pub(in crate::v9) use nodes::{BODY, HEAD, HTML};
pub(in crate::v9) use timers::schedule as schedule_task;

impl Page<'_> {
    /** Give every page script a head `<script>` element (once per page), after the page's
    metadata elements. */
    pub fn script_elements(&mut self, count: usize) -> EngineResult<Vec<usize>> {
        if let Some(elements) = self.state.parsed_scripts.clone() {
            let unique: std::collections::BTreeSet<_> = elements.iter().copied().collect();
            if elements.len() != count
                || unique.len() != count
                || elements.iter().any(|id| self.tag(*id) != "script")
            {
                return Err(crate::protocol::EngineError::invalid(
                    "Invalid parsed script identities.",
                ));
            }
            return Ok(elements);
        }
        self.metadata_elements()?;
        let mut elements = Vec::with_capacity(count);
        for index in 0..count {
            let node = self.create("script", None)?;
            let attributes = self.environment.script_attributes.get(index).cloned();
            let src = self
                .environment
                .scripts
                .get(index)
                .filter(|src| !src.is_empty())
                .cloned();
            if let Some(record) = self.state.detached.get_mut(&node) {
                record.attributes.extend(attributes.unwrap_or_default());
                if let Some(src) = src {
                    record.attributes.insert("src".into(), src);
                }
            }
            self.insert(HEAD, node, None)?;
            elements.push(node);
        }
        self.state.parsed_scripts = Some(elements.clone());
        Ok(elements)
    }
}

impl Page<'_> {
    /// The app's metadata elements (`meta`, `link`, `style`, `title`, `base`, data-block
    /// `script`) become V9-local records at their source places, in source order: head ones in
    /// the head, body ones under their parent before the first child parsed after them, so
    /// `querySelector('meta[name=locale]')` and a `<react-partial>`'s lookup of its own embedded
    /// data both find them. They never enter the projection, as Eye keeps them out of layout.
    fn metadata_elements(&mut self) -> EngineResult<()> {
        let metadata = std::mem::take(&mut self.environment.metadata);
        for element in &metadata {
            let text = (!element.text.is_empty()).then(|| element.text.clone());
            let node = self.create(&element.tag, text)?;
            if element.tag == "style" {
                self.state
                    .parsed_style_hashes
                    .insert(node, sheets::text_hash(&element.text));
            }
            if let Some(record) = self.state.detached.get_mut(&node) {
                record.attributes.clone_from(&element.attributes);
            }
            self.place_metadata(element, node)?;
        }
        self.environment.metadata = metadata;
        Ok(())
    }
    /** Put one metadata record at its source place; a body one whose parent the projection no
    longer holds falls back to the body's end. */
    fn place_metadata(
        &mut self,
        element: &crate::document::MetadataElement,
        node: usize,
    ) -> EngineResult<()> {
        if !element.in_body {
            // These records were just allocated and cannot need adoption/removal. Avoid the
            // generic insertion path's scan of all prior head nodes for each metadata element.
            self.state.head.push(node);
            if let Some(record) = self.state.detached.get_mut(&node) {
                record.parent = Some(HEAD);
            }
            self.touch();
            return Ok(());
        }
        let (parent, before) = match element.parent {
            Some(parent) if !self.is_projected(parent) => (BODY, None),
            parent => {
                let parent = parent.unwrap_or(BODY);
                let before = self
                    .all_children(parent)
                    .into_iter()
                    .find(|child| *child >= element.next && *child < nodes::VIRTUAL_BASE);
                (parent, before)
            }
        };
        self.state
            .local_children
            .entry(parent)
            .or_default()
            .push((node, before));
        if let Some(record) = self.state.detached.get_mut(&node) {
            record.parent = Some(parent);
        }
        Ok(())
    }
}

/** First identity of nodes outside the projection. */
pub(in crate::v9) fn nodes_virtual_base() -> usize {
    nodes::VIRTUAL_BASE
}

/** First identity of script-created nodes, numbered by `PageState::next_detached`. */
pub(in crate::v9) fn nodes_detached_base() -> usize {
    nodes::DETACHED_BASE
}

/** Fire a trusted lifecycle or click event; clicks are cancelable. */
pub(in crate::v9) fn fire(vm: &mut Vm, target: Target, kind: &str, bubbles: bool) -> Res<bool> {
    events::fire(vm, target, kind, bubbles, kind == "click")
}

/** A trusted, bubbling, cancelable `submit` at a form; true when no handler prevented it. */
pub(in crate::v9) fn fire_submit(vm: &mut Vm, form: usize) -> Res<bool> {
    events::fire(vm, Target::Node(form), "submit", true, true)
}

/** A trusted `popstate` at the window whose `state` is the entry's restored state. */
pub(in crate::v9) fn fire_pop_state(vm: &mut Vm, state: JsValue) -> Res<bool> {
    events::fire_with_state(vm, Target::Window, "popstate", state)
}

/** A saved history state parsed back into a value, or None when it no longer parses. */
pub(in crate::v9) fn json_value(vm: &mut Vm, json: &str) -> Option<JsValue> {
    crate::v9::builtins::json_parse_text(vm, json).ok()
}

/** An unprevented click on (or inside) a link asks the app to navigate. */
pub(in crate::v9) fn follow_link(vm: &mut Vm, target: usize) {
    let mut candidates = vec![target];
    candidates.extend(vm.page.ancestors(target));
    for node in candidates {
        let Some(view) = vm.page.view(node) else {
            continue;
        };
        if view.tag == "a" || view.tag == "area" {
            if let Some(href) = view.attributes.get("href")
                && !href
                    .trim_start()
                    .to_ascii_lowercase()
                    .starts_with("javascript:")
                && let Some(url) = net::resolve(vm, href)
            {
                vm.page.effects.navigation = Some(NavigationRequest {
                    kind: "assign".into(),
                    url,
                    delta: 0,
                });
            }
            return;
        }
    }
}

use super::StorageState;
use super::value::{JsValue, ObjectId};
use super::vm::data::{Class, HostData};
use super::vm::heap::{Key, Prop};
use super::vm::{GLOBAL, Res, Vm};
use crate::constants::{VM_CONSOLE_BYTES, VM_CONSOLE_ENTRIES};
use crate::document::{DomChange, FragmentParser, ScriptChange, ScriptDocument};
use crate::protocol::{EngineError, EngineResult, SourceLocation};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Which change log the app's document engine replays.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomLevel {
    /// Only `DomChange` (text by id, fragment insert, move, lang/dir). Scripts that need more
    /// fail alone with an unsupported error, exactly as before.
    #[default]
    Legacy,
    /// The full `ScriptChange` log in `Execution::script_changes`.
    Extended,
}

/// Facts the app supplies for one execution of a page. Everything is optional; defaults describe
/// a blank desktop page at `about:blank`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PageEnvironment {
    /// A dedicated worker has its own global profile, with no Window or document capabilities.
    pub worker_name: Option<String>,
    /// Native worker nesting level; zero retains the legacy first-level worker profile.
    pub worker_depth: u8,
    /// Module workers use module entry evaluation and cannot call classic importScripts.
    #[serde(default)]
    pub worker_module: bool,
    /// This worker is a service worker: the bootstrap gives it ServiceWorkerGlobalScope.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub service_worker: bool,
    /// Service worker facts as JSON for the bootstrap: a page's `{controller}`, or a service
    /// worker's own `{registration, worker}`. None when no service worker is involved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_worker_state: Option<String>,
    /// Host locale used when an Intl constructor receives no explicit locale.
    pub locale: String,
    /// Opt into the real RegExp buffer-boundary syntax proposal; ordinary pages preserve legacy grammar.
    pub regexp_buffer_boundaries: bool,
    /// The document URL; `location` reports it.
    pub url: String,
    /// `document.referrer`.
    pub referrer: String,
    /// `navigator.userAgent`.
    pub user_agent: String,
    /// `navigator.platform`.
    pub platform: String,
    /// `document.cookie` as the jar reports it for this URL (never HttpOnly cookies).
    pub cookie: String,
    /// `innerWidth`.
    pub viewport_width: f64,
    /// `innerHeight`.
    pub viewport_height: f64,
    /// `devicePixelRatio`.
    pub device_pixel_ratio: f64,
    /// `screen.width`.
    pub screen_width: f64,
    /// `screen.height`.
    pub screen_height: f64,
    /// Wall-clock milliseconds since the Unix epoch at the page's time origin.
    pub time_origin_ms: f64,
    /// Native hosts opt into within-task time; standalone and replay inputs stay deterministic.
    #[serde(default)]
    pub live_clock: bool,
    /// Local time minus UTC in minutes, for Date's local-time methods.
    pub timezone_offset_minutes: f64,
    /// Offset before the first supplied timezone transition, in seconds including historical LMT precision.
    pub timezone_initial_offset_seconds: Option<i32>,
    /// Strictly increasing UTC epoch milliseconds and the offset in seconds effective from each instant.
    pub timezone_transitions: Vec<(f64, i32)>,
    /// A TZif POSIX footer supplies recurring transitions after the final explicit timestamp.
    pub timezone_posix_rule: Option<String>,
    /// Standard and daylight long display names supplied by the host locale's timezone data.
    pub timezone_display_names: Option<(String, String)>,
    /// Daylight flags aligned with explicit timezone transitions; empty when the host supplies no names.
    pub timezone_daylight: Vec<bool>,
    /// Daylight status before the first explicit timezone transition.
    pub timezone_initial_daylight: bool,
    /// Seed for Math.random and crypto.randomUUID on a new page.
    pub random_seed: u64,
    /// Refuse eval and dynamic Function compilation while retaining normal host Script loading.
    pub disallow_code_generation_from_strings: bool,
    /// Host-compiled attribute policy, separate from eval and refreshed on each request.
    #[serde(default)]
    pub handler_policy: HandlerPolicy,
    /// `navigator.onLine`.
    pub online: bool,
    /// `document.title` at load.
    pub title: String,
    /// Which change log to emit.
    pub dom_level: DomLevel,
    /// Text of projected text nodes (and of elements whose text a script set), by local id.
    pub node_text: BTreeMap<usize, String>,
    /// The `src` of each page script, by source index ("" for inline scripts). V9 gives each a
    /// `<script>` element in the head so `document.currentScript` and script lookups work.
    pub scripts: Vec<String>,
    /// Each page script element's own attributes, by source index (empty when the app has
    /// none), so its head record carries the `id` and `data-*` values scripts look it up by.
    pub script_attributes: Vec<BTreeMap<String, String>>,
    /// Source indices whose acquisition was refused; empty source alone is a successful load.
    #[serde(default)]
    pub refused_scripts: Vec<usize>,
    /// Metadata elements Eye keeps out of layout, in source order; V9 makes each a head
    /// element at load so scripts find and read them.
    pub metadata: Vec<crate::document::MetadataElement>,
    /// Frame documents the app fetched, keyed by the URL the frame asked for. A frame whose
    /// `src` resolves to one shows that document to scripts; an entry without a source records a
    /// refusal, so the frame's load event still fires with no document.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub frame_documents: BTreeMap<String, FrameDocument>,
    /// Native owners routed to independent scripted pages; URL-sharing inert siblings stay local.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub scripted_frame_owners: BTreeSet<usize>,
    /// The coordinator's ordered immediate-parent directory, never a grant of document access.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parent_frames: Vec<frame_directory::FrameRoute>,
    /// This frame's coordinator-issued context identity, independent of its document generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub own_window: Option<i64>,
    /// This document's native context, including the root; independent of its local node identities.
    pub context_window: Option<i64>,
    /// Native parent/child relationships for the current bounded executable frame tree.
    pub frame_contexts: Vec<frame_directory::FrameContext>,
    /// The immediate embedder origin when this document runs in a frame; the native directory
    /// separately identifies its top-level ancestor. None for a top-level page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedder_origin: Option<String>,
    /// `window.name`: a frame's `name` attribute, which frame protocols pass settings in.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub window_name: String,
    /// The `<html>` element's own attributes, which the document engine keeps apart from its
    /// nodes; `document.documentElement` shows them under what scripts set since.
    pub html_attributes: BTreeMap<String, String>,
    /// The `<body>` element's own attributes (Google's sign-in hangs its root controller on
    /// `<body jscontroller>`), shown the same way.
    pub body_attributes: BTreeMap<String, String>,
    /// Painted boxes by node, in paint order, from the app's last layout: what
    /// `getBoundingClientRect`, the `offset*`/`client*` sizes and `elementFromPoint` answer.
    /// The app refreshes it before each step, so a step that changed the document sees the
    /// layout of the step before (a one-step lag; a browser would force layout). Empty when the
    /// page has not been laid out, where V9 falls back to nominal boxes.
    pub geometry: Vec<NodeBox>,
    /// Root and body retain measured raw layout sizes without borrowing ordinary node identities.
    pub resize_wrappers: mars_core::geometry::WrapperMetrics,
    /// The document's scroll offset, which client coordinates subtract.
    pub scroll_y: f64,
    /// The document's horizontal scroll offset: negative in a document overflowing to the left.
    pub scroll_x: f64,
    /// The `[min, max]` horizontal offsets the document can scroll to, from the last layout;
    /// None before one.
    pub scroll_x_range: Option<[f64; 2]>,
    /// Each scrolled scroll container's offset `[x, y]` by node, as the app last painted it:
    /// what `scrollLeft`/`scrollTop` answer until a script moves it.
    pub element_scroll: BTreeMap<usize, [f64; 2]>,
    /// The furthest offsets `[x, y]` of each scroll container the last layout found overflowing,
    /// by node. With `laid_out_before`, a script's scroll clamps to them (and a node that laid
    /// out without one cannot scroll), so a loop that scrolls until `scrollTop` stops growing ends.
    pub scroll_max: BTreeMap<usize, [f64; 2]>,
    /// The first node identity the last layout never saw: a node at or above it was inserted
    /// since, so `geometry` cannot hold its box. A browser would force a layout when script
    /// reads such a node's geometry; V9 cannot, so it reports the read in
    /// `HostEffects::stale_geometry` and the app may lay out and run the step again. None
    /// when the app gives no such promise, and no read is reported.
    pub laid_out_before: Option<usize>,
    /// Synchronous layout an in-process app supplies; never serialized, so a framed step has
    /// none and keeps reporting stale reads for the app to retry.
    #[serde(skip)]
    pub layout: LayoutHook,
    /// The cascaded `position` of each element the last style pass left non-static, by node:
    /// what `getComputedStyle(el).position` answers when no inline declaration overrides it.
    /// Pages branch on it (GitHub sizes its fixed header only when the wrapper computes to
    /// `fixed`), so the inline-only fallback of `static` hides layout from them.
    pub positions: BTreeMap<usize, String>,
    /// Eye's resolved values (`getComputedStyle`) for each node scripts have asked about, by
    /// node then CSS property. A node present with no entry for a property falls back to V9's
    /// own approximation; an absent rendered node is reported in `HostEffects::style_requests`.
    pub computed_styles: BTreeMap<usize, BTreeMap<String, String>>,
    /// Eye's resolved values for the root wrappers, under "html" and "body": V9 holds those two
    /// elements itself, so they have no node identity in `computed_styles`.
    pub root_styles: BTreeMap<String, BTreeMap<String, String>>,
    /// Upstream test fixtures, supplied as inert text only in the opt-in conformance build.
    /// The engine cannot read files; d8.file only sees exactly this host-owned registry.
    #[cfg(feature = "conformance")]
    pub conformance_sources: BTreeMap<String, String>,
    /** Allow the explicitly supplied async helper to use the implemented shell intrinsics. */
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_native_async_helper: bool,
    /** Only this host-selected classic script receives the approved native intrinsic grammar. */
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_native_main_source: Option<usize>,
    /// Permit the exact bundled Wasm exception helper to parse native introspection syntax.
    #[cfg(feature = "conformance")]
    pub conformance_native_wasm_exceptions_helper: bool,
    /// Enable experimental Wasm stringref types only when the shell flag requests them.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_wasm_stringref: bool,
    /// Enable custom descriptors in shell tests only when the actual proposal flag is present.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_wasm_custom_descriptors: bool,
    /// Enable non-sequentially-consistent Wasm atomic encodings in shell tests.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_wasm_acquire_release: bool,
    /** Enable the optional UTF-8 Wasm compile-time imports in the conformance shell. */
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_wasm_imported_strings_utf8: bool,
    /** Enable binary16 Wasm instructions in the conformance shell. */
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_wasm_fp16: bool,
    /** Enable custom descriptor JavaScript interop in the conformance shell. */
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_wasm_js_interop: bool,
    /** Enable 128-bit wide arithmetic instructions in the conformance shell. */
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_wasm_wide_arithmetic: bool,
    /** Permit mixed legacy and modern Wasm exception instructions in test shells. */
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_wasm_allow_mixed_eh: bool,
    /** Enable experimental Wasm stack-switching binary forms in the shell. */
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_wasm_wasmfx: bool,
    /// Enable the experimental shared-everything Wasm feature in the shell.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_wasm_shared: bool,
    /** Enables decoding grouped import encodings only when explicitly requested by d8. */
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_wasm_compact_imports: bool,
    /** Permits the dedicated Wasm testing NOP in the shell only when requested. */
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_wasm_testing_opcode: bool,
    /** Enables V8's explicit test-only unchecked reference cast instruction. */
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_wasm_ref_cast_nop: bool,
    /** Selects V8's Math import optimization; absent metadata retains its enabled default. */
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_wasm_math_intrinsics: Option<bool>,
    /// Shell test metadata can select whether its Wasm streaming callback is installed.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_wasm_test_streaming: Option<bool>,
    /// Explicit shell-test work budget; production requests retain their ordinary operation limit.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_operation_limit: Option<usize>,
    /// Share interned literal backing only when the d8 shared string table flag is enabled.
    #[cfg(feature = "conformance")]
    pub conformance_shared_string_table: bool,
    /// Opt into the shell's shared heap used by shared values and workers.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_shared_heap: bool,
    /// Permit explicit string sharing without requiring every literal to be interned.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_shared_strings: bool,
    /// Install the external-string extension only when the d8 host requests its flag.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_expose_externalize_string: bool,
    /// Exact fixture bytes are inert host input; binary reads do not grant engine filesystem access.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_files: BTreeMap<String, Vec<u8>>,
    /// Host-verified file and directory existence; absent keys grant no filesystem access.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_exists: BTreeMap<String, bool>,
    /// Run with an ordinary d8 shell global instead of browser Window accessors.
    /// This opt-in host choice keeps browser pages unchanged in conformance builds.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_d8: bool,
    /// Script arguments supplied after the shell's `--` delimiter.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_script_arguments: Vec<String>,
    /// The actual unsafe-constructor shell flag bypasses dynamic Function security-token checks.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_allow_unsafe_function_constructor: bool,
    /// The actual shell flag supplies a native extension name; absence never exposes a CPU marker global.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_cpu_trace_name: Option<String>,
    /// A shell seed selects its native per-context Math.random stream; zero requests system entropy.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_random_seed: Option<i32>,
    /// Real cooperative CPU samples use the requested native interval, in microseconds.
    #[cfg(feature = "conformance")]
    #[serde(default = "default_cpu_profile_interval")]
    pub conformance_cpu_profiler_interval_us: u64,
    /// Enable d8 Promise lifecycle hooks only when actual test metadata requested that extension.
    #[cfg(feature = "conformance")]
    pub conformance_async_hooks: bool,
    /// The actual --enable-inspector shell flag: exposes the `send` inspector session.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_inspector: bool,
    /// The actual --harmony-struct shell flag: exposes SharedArray and SharedStructType.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_harmony_struct: bool,
    /** Actual V8 metadata may disable async stack enrichment; physical captured frames are unaffected. */
    #[cfg(feature = "conformance")]
    pub conformance_async_stack_traces: bool,
    /// The actual experimental shell flag exposes real embedder API activations; ordinary pages keep them hidden.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_experimental_stack_trace_frames: bool,
    /// Preserve V8's default-true builtin-subclassing switch as actual opt-in test metadata.
    #[cfg(feature = "conformance")]
    pub conformance_builtin_subclassing: bool,
    /// Original shell flags select the staged Iterator.includes extension.
    #[cfg(feature = "conformance")]
    pub conformance_iterator_includes: bool,
    /// Join exposure is independent from includes, matching the native staging switches.
    #[cfg(feature = "conformance")]
    pub conformance_iterator_join: bool,
    /// Expose native test callbacks only when original d8 flags requested the FastCAPI helper.
    #[cfg(feature = "conformance")]
    pub conformance_expose_fast_api: bool,
    /// Native test helpers honor explicitly requested fuzzing behavior without pretending optimized calls occurred.
    #[cfg(feature = "conformance")]
    pub conformance_fuzzing: bool,
    /// D8's explicit host flag suppresses ordinary unhandled-rejection process failures.
    #[cfg(feature = "conformance")]
    pub conformance_ignore_unhandled_promises: bool,
    /// An explicit shell flag may lower the existing Wasm input byte ceiling, never raise it.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_wasm_max_module_bytes: Option<usize>,
    /// Original shell metadata may lower native table capacity without raising the array budget.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_wasm_max_table_entries: Option<usize>,
    /// Cap logical Wasm memory pages for a conformance shell session.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_wasm_max_memory_pages: Option<usize>,
    /// Native worker authority is scoped by the embedding test and is never serialized into page facts.
    #[cfg(feature = "conformance")]
    #[serde(skip)]
    pub conformance_workers: Option<std::sync::Arc<crate::v9::ConformanceWorkerHost>>,
    /// Only worker environments name an endpoint; parent script cancellation stays independent.
    #[cfg(feature = "conformance")]
    #[serde(skip)]
    pub conformance_worker_id: Option<u64>,
    /// A child observes its own native cancellation flag without taking a broker lock per instruction.
    #[cfg(feature = "conformance")]
    #[serde(skip)]
    pub conformance_worker_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// The original `--expose-statistics` shell flag exposes `getV8Statistics`.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_expose_statistics: bool,
    /// The original `--expose-trigger-failure` shell flag exposes the failure triggers.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_expose_trigger_failure: bool,
    /// The original `--enable-tracing` shell flag enables the V8 trace category.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_enable_tracing: bool,
    /// The original `--log` shell flag starts the d8 log capture when the page loads.
    #[cfg(feature = "conformance")]
    #[serde(default)]
    pub conformance_log: bool,
    /// The host's playable media types, for canPlayType and Media Source; empty plays nothing.
    #[serde(default, skip_serializing_if = "media_support_empty")]
    pub media: MediaSupport,
}

/** An empty media support list is omitted from serialized environments. */
fn media_support_empty(support: &MediaSupport) -> bool {
    *support == MediaSupport::default()
}

/// One painted box the app's layout gave a node, in document coordinates.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeBox {
    /// The node's local identity.
    pub node: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Untransformed document position keeps layout offsets independent from visual transforms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_position: Option<[f64; 2]>,
    /// Raw border-box dimensions keep layout measurements independent from visual transforms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_size: Option<[f64; 2]>,
    /// Distinct native content and border measurements; absent hosts cannot claim resize box support.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resize_box: Option<mars_core::geometry::BoxMetrics>,
    /// Ordered transformed border-box corners reject empty corners of the visual bounding rectangle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quad: Option<mars_core::geometry::Quad>,
    /// Existing rectangular overflow clip in document coordinates, before paint-time pin translation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<[f64; 4]>,
    /// Sticky/fixed top, start and room mirror the native pin without changing raw layout geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<[f64; 3]>,
}

impl Default for PageEnvironment {
    /** A blank desktop page. */
    fn default() -> Self {
        Self {
            worker_name: None,
            worker_depth: 0,
            worker_module: false,
            media: MediaSupport::default(),
            service_worker: false,
            service_worker_state: None,
            locale: "en-US".into(),
            regexp_buffer_boundaries: false,
            url: "about:blank".into(),
            referrer: String::new(),
            user_agent: DEFAULT_USER_AGENT.into(),
            platform: "MacIntel".into(),
            cookie: String::new(),
            viewport_width: crate::constants::DEFAULT_VIEWPORT_WIDTH,
            viewport_height: crate::constants::DEFAULT_VIEWPORT_HEIGHT,
            device_pixel_ratio: DEFAULT_PIXEL_RATIO,
            screen_width: DEFAULT_SCREEN_WIDTH,
            screen_height: DEFAULT_SCREEN_HEIGHT,
            time_origin_ms: 0.0,
            live_clock: false,
            timezone_offset_minutes: 0.0,
            timezone_initial_offset_seconds: None,
            timezone_transitions: Vec::new(),
            timezone_posix_rule: None,
            timezone_display_names: None,
            timezone_daylight: Vec::new(),
            timezone_initial_daylight: false,
            random_seed: 0,
            disallow_code_generation_from_strings: false,
            handler_policy: HandlerPolicy::default(),
            online: true,
            title: String::new(),
            dom_level: DomLevel::Legacy,
            node_text: BTreeMap::new(),
            scripts: Vec::new(),
            script_attributes: Vec::new(),
            refused_scripts: Vec::new(),
            metadata: Vec::new(),
            frame_documents: BTreeMap::new(),
            scripted_frame_owners: BTreeSet::new(),
            parent_frames: Vec::new(),
            own_window: None,
            context_window: None,
            frame_contexts: Vec::new(),
            embedder_origin: None,
            window_name: String::new(),
            html_attributes: BTreeMap::new(),
            body_attributes: BTreeMap::new(),
            geometry: Vec::new(),
            resize_wrappers: mars_core::geometry::WrapperMetrics::default(),
            scroll_y: 0.0,
            scroll_x: 0.0,
            scroll_x_range: None,
            element_scroll: BTreeMap::new(),
            scroll_max: BTreeMap::new(),
            laid_out_before: None,
            layout: LayoutHook::default(),
            positions: BTreeMap::new(),
            computed_styles: BTreeMap::new(),
            root_styles: BTreeMap::new(),
            #[cfg(feature = "conformance")]
            conformance_sources: BTreeMap::new(),
            #[cfg(feature = "conformance")]
            conformance_native_async_helper: false,
            #[cfg(feature = "conformance")]
            conformance_native_main_source: None,
            #[cfg(feature = "conformance")]
            conformance_native_wasm_exceptions_helper: false,
            #[cfg(feature = "conformance")]
            conformance_wasm_stringref: false,
            #[cfg(feature = "conformance")]
            conformance_wasm_custom_descriptors: false,
            #[cfg(feature = "conformance")]
            conformance_wasm_acquire_release: false,
            #[cfg(feature = "conformance")]
            conformance_wasm_imported_strings_utf8: false,
            #[cfg(feature = "conformance")]
            conformance_wasm_fp16: false,
            #[cfg(feature = "conformance")]
            conformance_wasm_js_interop: false,
            #[cfg(feature = "conformance")]
            conformance_wasm_wide_arithmetic: false,
            #[cfg(feature = "conformance")]
            conformance_wasm_allow_mixed_eh: false,
            #[cfg(feature = "conformance")]
            conformance_wasm_wasmfx: false,
            #[cfg(feature = "conformance")]
            conformance_wasm_shared: false,
            #[cfg(feature = "conformance")]
            conformance_wasm_compact_imports: false,
            #[cfg(feature = "conformance")]
            conformance_wasm_testing_opcode: false,
            #[cfg(feature = "conformance")]
            conformance_wasm_ref_cast_nop: false,
            #[cfg(feature = "conformance")]
            conformance_wasm_math_intrinsics: None,
            #[cfg(feature = "conformance")]
            conformance_wasm_test_streaming: None,
            #[cfg(feature = "conformance")]
            conformance_operation_limit: None,
            #[cfg(feature = "conformance")]
            conformance_shared_string_table: false,
            #[cfg(feature = "conformance")]
            conformance_shared_heap: false,
            #[cfg(feature = "conformance")]
            conformance_shared_strings: false,
            #[cfg(feature = "conformance")]
            conformance_expose_externalize_string: false,
            #[cfg(feature = "conformance")]
            conformance_files: BTreeMap::new(),
            #[cfg(feature = "conformance")]
            conformance_exists: BTreeMap::new(),
            #[cfg(feature = "conformance")]
            conformance_d8: false,
            #[cfg(feature = "conformance")]
            conformance_script_arguments: Vec::new(),
            #[cfg(feature = "conformance")]
            conformance_allow_unsafe_function_constructor: false,
            #[cfg(feature = "conformance")]
            conformance_cpu_trace_name: None,
            #[cfg(feature = "conformance")]
            conformance_random_seed: None,
            #[cfg(feature = "conformance")]
            conformance_cpu_profiler_interval_us: default_cpu_profile_interval(),
            #[cfg(feature = "conformance")]
            conformance_async_hooks: false,
            #[cfg(feature = "conformance")]
            conformance_inspector: false,
            #[cfg(feature = "conformance")]
            conformance_harmony_struct: false,
            #[cfg(feature = "conformance")]
            conformance_async_stack_traces: true,
            #[cfg(feature = "conformance")]
            conformance_experimental_stack_trace_frames: false,
            #[cfg(feature = "conformance")]
            conformance_builtin_subclassing: true,
            #[cfg(feature = "conformance")]
            conformance_iterator_includes: false,
            #[cfg(feature = "conformance")]
            conformance_iterator_join: false,
            #[cfg(feature = "conformance")]
            conformance_expose_fast_api: false,
            #[cfg(feature = "conformance")]
            conformance_fuzzing: false,
            #[cfg(feature = "conformance")]
            conformance_ignore_unhandled_promises: false,
            #[cfg(feature = "conformance")]
            conformance_wasm_max_module_bytes: None,
            #[cfg(feature = "conformance")]
            conformance_wasm_max_table_entries: None,
            #[cfg(feature = "conformance")]
            conformance_wasm_max_memory_pages: None,
            #[cfg(feature = "conformance")]
            conformance_workers: None,
            #[cfg(feature = "conformance")]
            conformance_worker_id: None,
            #[cfg(feature = "conformance")]
            conformance_worker_cancel: None,
            #[cfg(feature = "conformance")]
            conformance_expose_statistics: false,
            #[cfg(feature = "conformance")]
            conformance_expose_trigger_failure: false,
            #[cfg(feature = "conformance")]
            conformance_enable_tracing: false,
            #[cfg(feature = "conformance")]
            conformance_log: false,
        }
    }
}

/** The user agent a page sees when the app supplies none (desktop Chrome, as Electron reports). */
const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
/** Device pixel ratio default. */
const DEFAULT_PIXEL_RATIO: f64 = 2.0;
/** Screen width default. */
const DEFAULT_SCREEN_WIDTH: f64 = 1440.0;
/** Screen height default. */
const DEFAULT_SCREEN_HEIGHT: f64 = 900.0;

/// A network request a script made; the app performs it (through Relay) and answers with
/// `HostResponse` of the same id. V9 itself never touches the network.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostRequest {
    /// Correlates the response.
    pub id: u32,
    /// "fetch", "xhr" or "beacon" (beacons expect no response).
    pub kind: String,
    /// Upper-case method.
    pub method: String,
    /// Absolute URL, resolved against the document URL.
    pub url: String,
    /// Request headers in order.
    pub headers: Vec<(String, String)>,
    /// Text body, if any.
    pub body: Option<String>,
    /// A binary body (an ArrayBuffer or typed array whose bytes are not UTF-8 text, such as
    /// YouTube's gzipped Innertube requests). `body` is then None.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body_bytes: Vec<u8>,
    /// Actual fetch-time bytes from this context's Blob registry; absent for every network request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob_source: Option<BlobUrlSource>,
    /// "omit", "same-origin" or "include".
    pub credentials: String,
    /// "cors", "no-cors" or "same-origin" (fetch's `mode`; XMLHttpRequest is always "cors").
    /// The app decides what a cross-origin answer may show from it.
    #[serde(default)]
    pub mode: String,
}

/// The app's answer to one `HostRequest`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HostResponse {
    /// The request id.
    pub id: u32,
    /// HTTP status; 0 with `error` set means a network error.
    pub status: u16,
    /// Reason phrase.
    pub status_text: String,
    /// Final URL after redirects.
    pub url: String,
    /// Response headers in order (lowercase names).
    pub headers: Vec<(String, String)>,
    /// Body text, at most VM_RESPONSE_BYTES.
    pub body: String,
    /// Exact non-text body bytes; the combined text and byte representation has the same limit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body_bytes: Vec<u8>,
    /// A network failure message; the script sees a TypeError (fetch) or an error event (XHR).
    pub error: Option<String>,
}

/// One console call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsoleEntry {
    /// log, info, warn, error or debug.
    pub level: String,
    /// The formatted message, at most VM_CONSOLE_BYTES.
    pub text: String,
}

/// An actual native CPU marker invocation, paired with the owning host's current VM clock.
#[cfg(feature = "conformance")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CpuTraceMark {
    /// The exact accepted unsigned marker, independently of the architecture-specific instruction encoding.
    pub marker: u32,
    /// Milliseconds elapsed on the real host-supplied VM clock.
    pub time_ms: f64,
}

/// A navigation a script asked for; V9 never performs it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavigationRequest {
    /// "assign", "replace", "reload", "open", "back", "forward" or "go".
    pub kind: String,
    /// Absolute target URL (empty for reload and history traversal).
    pub url: String,
    /// History delta for "go", "back" (-1) and "forward" (1).
    pub delta: i32,
}

/// A same-document history change from pushState, replaceState or a hash assignment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryRecord {
    /// True for replaceState.
    pub replace: bool,
    /// The new absolute URL.
    pub url: String,
    /// The title argument.
    pub title: String,
    /// The state argument as JSON, handed back by `popstate` when the person returns to this
    /// entry. None for a hash change or a state of null or undefined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

/// What the app must do after an execution besides committing state and changes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HostEffects {
    /// Worker messages wait until module entry evaluation, including top-level await, completes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_ready: Option<bool>,
    /// Dedicated worker tasks remain inert until the owning candidate admits them.
    pub worker_commands: Vec<workers::WorkerCommand>,
    /// Uncancelled worker exceptions become asynchronous errors on the owning Worker handle.
    pub worker_errors: Vec<String>,
    /// Existing image identity associations move with real DOM adoption, without changing source generation.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub image_adoptions: Vec<ImageAdoption>,
    /// Complete native opacity animation layer; omission restores the authored cascade.
    pub opacity_effects: Vec<OpacityEffect>,
    /// Source-generation requests; only actual host decoding may answer them with `resume_images`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub image_requests: Vec<ImageRequest>,
    /// Media element commands for the coordinator's players, in the order the page made them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media: Vec<MediaCommand>,
    /// Superseded generations release queued native captures before admitting replacement sources.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image_cancellations: Vec<ImageCancellation>,
    /// Requests to perform; answer them with `resume_requests`.
    pub requests: Vec<HostRequest>,
    /// IndexedDB transactions committed during this execution, in commit order; the coordinator persists them for the context's storage partition.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub indexed_db: Vec<indexed_db::IndexedDbCommit>,
    /** Previously published requests cancelled by native signals release host queues and snapshots. */
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cancelled_requests: Vec<u32>,
    /// The last navigation request, if any.
    pub navigation: Option<NavigationRequest>,
    /// A person's or agent's form submission whose `submit` event nobody cancelled: the app
    /// submits this form natively. None when a handler called preventDefault.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submit: Option<usize>,
    /// A native Enter that no keydown or keypress handler cancelled: the app performs its
    /// default (submit the field's form, or a newline in a textarea).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub key_default: bool,
    /// pushState, replaceState and hash changes, in order.
    pub history: Vec<HistoryRecord>,
    /// Raw `document.cookie = ...` fields, in order, for `CookieJar::set_document_cookie`.
    pub cookies: Vec<String>,
    /// Console output, at most VM_CONSOLE_ENTRIES.
    pub console: Vec<ConsoleEntry>,
    /// Console entries dropped past the bound.
    pub console_dropped: usize,
    /// Actual CPU markers share the console diagnostic entry allowance and never represent sampled profiler data.
    #[cfg(feature = "conformance")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conformance_trace_marks: Vec<CpuTraceMark>,
    /// Calls still execute their native marker instruction when bounded host records must be dropped.
    #[cfg(feature = "conformance")]
    pub conformance_trace_marks_dropped: usize,
    /// Completed actual CPU profile JSON; only the owning test host chooses a confined artifact path.
    #[cfg(feature = "conformance")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conformance_cpu_profiles: Vec<String>,
    /// The first actual uncaught callback or rejection, distinct from script console output.
    /// Only the opt-in upstream host uses this to score asynchronous tests honestly.
    #[cfg(feature = "conformance")]
    pub conformance_error: Option<String>,
    /// Original native failure code and message, preserved independently of lazy diagnostic getters.
    #[cfg(feature = "conformance")]
    pub conformance_native_error: Option<(String, String)>,
    /// D8 counts terminal unhandled rejections across child isolates when deciding its process result.
    #[cfg(feature = "conformance")]
    pub conformance_rejection: bool,
    /// Captured d8 log text while logging is active; `None` once stopped or never started.
    #[cfg(feature = "conformance")]
    #[serde(skip)]
    pub conformance_log: Option<String>,
    /// Virtual time of the next pending timer or animation frame, if any.
    pub next_timer_ms: Option<f64>,
    /// Unsupported APIs scripts reached (and may have caught), first occurrence each.
    pub unsupported: Vec<String>,
    /// `document.title` when a script changed it.
    pub title: Option<String>,
    /// Script read the geometry of a connected node inserted after the app's last layout, so
    /// it saw an empty box where a browser would have laid the page out first.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stale_geometry: bool,
    /// Rendered nodes whose computed style a script read without the app having supplied it;
    /// the app may style them and run the step again, as it does for stale geometry.
    #[serde(skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub style_requests: std::collections::BTreeSet<usize>,
    /// Native geometry changes need an inert observation turn only while registrations or records exist.
    pub intersection_observers: bool,
    /// Queued observer entries need a dedicated native observation turn even outside author wake windows.
    pub intersection_observers_pending: bool,
    /// Persistent resize registrations need measured layout invalidation without author timers.
    pub resize_observers: bool,
    /// At least one measured resize entry is waiting for an observer-only delivery turn.
    pub resize_observers_pending: bool,
    /// An observer-only render cycle either needs fresh layout or has finalized this rendering opportunity.
    pub resize_turn: Option<resize_observers::ResizeTurn>,
    /// Absolute URLs of `<link rel=stylesheet>` elements scripts connected (in the head too,
    /// which stays V9's), for the app to fetch and apply, as a browser loads an inserted sheet.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stylesheets: Vec<String>,
    /// The text of every `<style>` that lives only in V9 (the head, script-local children) and
    /// that the app does not already apply, in document order, sent when the list changed. CSS-in-JS
    /// libraries (Emotion, styled-components) insert their rules into head styles this way.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_sheets: Option<Vec<String>>,
    /// Offsets `[x, y]` scripts scrolled scroll containers to, by node: the app paints them
    /// there (Eye clamps each to its container's range), as a browser scrolls on assignment.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub element_scroll: BTreeMap<usize, [f64; 2]>,
    /// The document scroll offset a script scrolled the viewport to, for the app to adopt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll_y: Option<f64>,
    /// The horizontal document scroll offset a script scrolled the viewport to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll_x: Option<f64>,
    /// Messages scripts posted to windows in other documents (the embedding page or a frame),
    /// in order, for the app to deliver.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<messaging::FrameMessage>,
    /// Connected detachments retire contexts even when the element is reinserted before publication.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub frame_detaches: Vec<frame_directory::FrameDetach>,
    /// Messages posted on MessagePorts whose peers live in other documents, in order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub port_messages: Vec<channel::PortMessage>,
    /// Closed external links retire native routes before later queued deliveries.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub port_closes: Vec<u64>,
    /// Ordered candidate-owned registrations, posts and explicit closures for native publication.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub broadcast_commands: Vec<broadcast::BroadcastCommand>,
    /// Full final open-endpoint snapshot, including empty, reconciles GC after ordered commands.
    #[serde(default)]
    pub broadcast_channels: Vec<broadcast::BroadcastEndpoint>,
}

/** A scheduled callback. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct Timer {
    pub id: u32,
    /** Scheduling order, renewed when an interval reschedules, so equal due times fire as browsers do. */
    #[serde(default)]
    pub order: u32,
    pub due: f64,
    pub interval: Option<f64>,
    pub callback: JsValue,
    pub args: Vec<JsValue>,
    pub frame: bool,
    /** HTML's timer nesting level: how many timeouts deep this one was scheduled. */
    #[serde(default)]
    pub nesting: u8,
}

/** Where a listener is registered. */
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::v9) enum Target {
    Node(usize),
    Document,
    Window,
    Object(ObjectId),
}

/** One event listener registration. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct Listener {
    pub id: u32,
    pub target: Target,
    pub kind: String,
    pub callback: JsValue,
    pub capture: bool,
    pub once: bool,
    /** Passive listeners cannot cancel: preventDefault and returnValue writes are ignored. */
    #[serde(default)]
    pub passive: bool,
    /** The AbortSignal whose abort removes this listener; absent for listeners without one. */
    #[serde(default)]
    pub signal: Option<JsValue>,
}

/** A node created by script and not (yet) in the document. */
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(in crate::v9) struct Detached {
    pub tag: String,
    pub attributes: BTreeMap<String, String>,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub text: Option<String>,
}

/** A request waiting on the app. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct Pending {
    pub id: u32,
    /** The fetch promise or the XMLHttpRequest object. */
    pub owner: ObjectId,
    pub xhr: bool,
    /** A platform bridge request (`host_bridge`): its owner is a promise and it has no signal. */
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub platform: bool,
    /** Native dependent signal remains reachable until completion or cancellation. */
    #[serde(default)]
    pub signal: Option<ObjectId>,
    /** Bytes and metadata retained by the native Blob snapshot until response delivery or abort. */
    #[serde(default)]
    pub blob_bytes: usize,
    /** An IndexedDB coordinator request: `owner` is the callback that receives the answer text. */
    #[serde(default)]
    pub indexed: bool,
}

/** document.readyState. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::v9) enum ReadyState {
    #[default]
    Loading,
    Interactive,
    Complete,
}

/** A frame's document as the app fetched it. `source` is None when the app refused the frame or
could not fetch it: the frame then shows no document, and its load event still fires. */
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameDocument {
    /// The absolute URL the frame asked for, which is its key in `PageEnvironment`.
    pub address: String,
    /// The media type the response declared, after any fallback the app applied.
    pub media_type: String,
    /// The document's text, or None when the frame has no document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/** A frame document V9 asks the app to fetch: the absolute URL an iframe's `src` names. */
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameRequest {
    /// The absolute URL to fetch.
    pub url: String,
}

/** One script element a script connected to the document. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::v9) struct ScriptLoad {
    /// The resolved `src`, or "" for an inline script that already ran.
    pub url: String,
    /// The script element, by node identity.
    pub element: usize,
    /// The element's `nonce`, for the app's Content-Security-Policy check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    /// The element's `integrity` metadata, which the app verifies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrity: Option<String>,
    /// The element's `crossorigin` attribute, which picks the app's fetch mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cross_origin: Option<String>,
    /// Whether it ran (or failed) already.
    pub done: bool,
}

/** Pinned d8's CPU profiler default is one real-time sample interval per millisecond. */
#[cfg(feature = "conformance")]
const fn default_cpu_profile_interval() -> u64 {
    1_000
}

/** Heap-number constants belong to one compiled code site and source. */
#[cfg(feature = "conformance")]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::v9) struct NumberConstantPool {
    pub source: [u8; 32],
    pub values: BTreeMap<u64, JsValue>,
}

/** Page state that survives between requests, inside VmState. */
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(in crate::v9) struct PageState {
    /// Object URL bytes survive Blob wrapper collection until revocation or context disposal.
    pub blob_urls: BTreeMap<String, blob_urls::BlobUrlSource>,
    /// Handles root their event targets until the owning document is destroyed.
    pub workers: BTreeMap<u32, workers::Handle>,
    /// close() prevents future owner posts while allowing the current synchronous task to finish.
    pub worker_closed: bool,
    /// Imported Script offsets retain their governed source URL for later stack traces.
    pub worker_imports: BTreeMap<usize, String>,
    /// Legacy handlers are retained Script units whose declarations survive serialized events.
    #[serde(default, skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub legacy_handlers: std::collections::BTreeSet<usize>,
    /// A saved context cannot be reopened with a different global authority profile.
    pub worker_name: Option<String>,
    /// Saved nesting authority cannot be changed when a worker resumes.
    #[serde(default)]
    pub worker_depth: u8,
    /// Worker kind is immutable across saved-state restoration and module graph continuations.
    pub worker_module: bool,
    /** Each context owns real bounded pixels; detached state lives only while reachable. */
    pub canvases: BTreeMap<usize, canvas::State>,
    /** Retained paths follow wrapper reachability rather than becoming permanent page roots. */
    pub path2d: BTreeMap<ObjectId, canvas::path2d::Retained>,
    /** Permanent per-isolate read-only roots for one-byte iterator characters. */
    #[cfg(feature = "conformance")]
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub character_roots: BTreeMap<u8, crate::v9::value::JsString>,
    /** Current isolate continuation data, retained across state transport. */
    #[cfg(feature = "conformance")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continuation_data: Option<JsValue>,
    /** Per-isolate code constant cells preserve literal identity without sharing mutable metadata between isolates. */
    #[cfg(feature = "conformance")]
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub code_constants: BTreeMap<u64, crate::v9::value::JsString>,
    /** Actual old heap-number constants, deduplicated within one compiled source site. */
    #[cfg(feature = "conformance")]
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub number_constants: BTreeMap<u32, NumberConstantPool>,
    /** Weak intern entries are temporarily carried between VM lifetimes, then restored as weak handles. */
    #[cfg(feature = "conformance")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interned_strings: Vec<crate::v9::value::JsString>,

    /** Installed shell debug delegate and bounded event counters for this isolate. */
    #[cfg(feature = "conformance")]
    #[serde(
        default,
        skip_serializing_if = "crate::v9::builtins::DebugState::is_empty"
    )]
    pub debug_delegate: crate::v9::builtins::DebugState,

    /** A d8 test can forbid blocking waits for this agent across state transport. */
    #[cfg(feature = "conformance")]
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub atomics_wait_disallowed: bool,
    /** The isolate's real unnamed session and callback survive requests without native thread handles. */
    #[cfg(feature = "conformance")]
    #[serde(
        default,
        skip_serializing_if = "crate::v9::builtins::ProfileState::is_empty"
    )]
    pub cpu_profile: crate::v9::builtins::ProfileState,
    /** Inert source generations and actual image decode results; no additional heap roots. */
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub images: BTreeMap<usize, images::State>,
    /** Media element bookkeeping: the bootstrap's dispatcher, changed elements and players. */
    pub media: media::State,
    pub identity: String,
    /** Virtual milliseconds since the time origin, used only for timer scheduling. */
    pub clock: f64,
    /** Last successful live execution's script-visible high-water mark survives state transport. */
    pub observed_clock: f64,
    /** Date.now() at clock zero. */
    pub epoch: f64,
    pub random: u64,
    /** Only explicit shell seed requests create bounded per-realm xorshift128+ state. */
    #[cfg(feature = "conformance")]
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub math_random: BTreeMap<u32, random::State>,
    pub timers: Vec<Timer>,
    pub next_timer: u32,
    /// Active and filling animations retain their actual effect and Promise graphs.
    pub active_animations: Vec<ObjectId>,
    /// Branded viewport observers retain measured records and thresholds across native layout updates.
    pub intersection_observers: BTreeMap<ObjectId, intersection_observers::State>,
    /// Branded resize observations retain last delivered raw dimensions independently of transforms.
    pub resize_observers: BTreeMap<ObjectId, resize_observers::State>,
    /// Depth gating survives resident and serialized continuations between fresh host layouts.
    pub resize_cycle: Option<resize_observers::Cycle>,
    /** Next scheduling sequence number. */
    pub next_order: u32,
    pub listeners: listener_store::ListenerStore,
    pub next_listener: u32,
    /** Node wrappers by node id, so identity survives requests. */
    pub wrappers: BTreeMap<usize, ObjectId>,
    pub detached: BTreeMap<usize, Detached>,
    pub next_detached: usize,
    /** Namespace, prefix and local name of each script-created element, by node. Projected
    elements have no record and take their namespace from their tag. */
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub element_ns: BTreeMap<usize, namespaces::ElementNs>,
    /** Namespace of each attribute set through a namespace-aware method, by node and then by
    stored name. Null-namespace attributes have no record. */
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attr_ns: BTreeMap<usize, BTreeMap<String, namespaces::AttrNs>>,
    /** Stored name of a namespaced attribute, by node, then namespace, then local name, so a
    namespace-aware read finds it without scanning the node's attributes. */
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attr_index: BTreeMap<usize, BTreeMap<String, BTreeMap<String, String>>>,
    pub pending: Vec<Pending>,
    pub next_request: u32,
    /** When the last trusted input happened (`script_now`), for transient user activation;
    consumed by APIs that use it up. */
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation: Option<f64>,
    /** Whether any trusted input ever reached this document (sticky user activation). */
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sticky_activation: bool,
    /** The bootstrap's installer for the coordinator-backed APIs, until something first uses
    one of them (`host_bridge::ensure_installed`). */
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_installer: Option<ObjectId>,
    pub ready: ReadyState,
    /** The current URL (pushState changes it). */
    pub href: String,
    /** The main document's encoding name, from the parser; empty reads as UTF-8. */
    #[serde(default)]
    pub charset: String,
    pub history_state: Option<JsValue>,
    pub history_length: u32,
    pub cookie: String,
    pub title: Option<String>,
    /** Script-written text of nodes, until the app reports it back in `node_text`. */
    pub texts: BTreeMap<usize, String>,
    /** XMLHttpRequest objects and their state. */
    pub xhrs: BTreeMap<ObjectId, XhrState>,
    /** Field values scripts set. */
    pub values: BTreeMap<usize, String>,
    /** Current option state; presence marks script-dirty selectedness. */
    #[serde(default)]
    pub selected_options: BTreeMap<usize, bool>,
    /** Attribute changes affect selectedness only until a script or user dirties an option. */
    #[serde(default)]
    pub dirty_options: std::collections::BTreeSet<usize>,
    /** Children of the virtual head, which the projection does not carry. */
    pub head: Vec<usize>,
    /** Extra children of the virtual html element after head and body. */
    pub html: Vec<usize>,
    /** V9-local children of a projected element (or the body), each with the projected sibling
    it sits before (None: last): the page's metadata elements at their source places. */
    pub local_children: BTreeMap<usize, Vec<(usize, Option<usize>)>>,
    /** Attributes of the virtual html/head/body nodes. */
    pub virtual_attributes: BTreeMap<usize, BTreeMap<String, String>>,
    pub initialized: bool,
    /** The page's Selection object, once a script asked for it; its ranges stay reachable. */
    pub selection: Option<ObjectId>,
    /** Live Range objects, whose boundary points move with the tree. Held weakly: the sweep drops
    any the heap collected, so a script's unreferenced ranges do not stay alive here. */
    pub live_ranges: range_index::LiveRanges,
    /** Script elements scripts connected, in start order: each `<script src>` asks the app for
    its source once and runs once when it arrives; an inline one is recorded as done. */
    pub script_loads: Vec<ScriptLoad>,
    /** Stable parser-created script identities survive module loading pauses. */
    #[serde(default)]
    pub parsed_scripts: Option<Vec<usize>>,
    /** A hash of each parsed head `<style>`'s text when the page loaded, by node: the app
    already applies that text, so only a changed one is reported in `local_sheets`. */
    #[serde(default)]
    pub parsed_style_hashes: BTreeMap<usize, u64>,
    /** A hash of the last `local_sheets` list reported, so an unchanged list is not resent. */
    #[serde(default)]
    pub local_sheets_hash: u64,
    /** Iframes scripts connected whose documents the app has not answered yet, by element: the
    URL each asked for. The answer fires the element's `load` event and leaves the list. */
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub frame_loads: BTreeMap<usize, String>,
    /** Frame documents the step asked the app to fetch, delivered with the step's result. */
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frame_requests: Vec<FrameRequest>,
    /** The focused element (`document.activeElement`), set by `focus()`, typing and clicks. */
    pub focused: Option<usize>,
    /** Custom element definitions, shadow and template bookkeeping, mutation observers. */
    pub components: components::State,
    /** Each same-origin iframe's document node, made on first `contentDocument`. */
    pub frames: BTreeMap<usize, usize>,
    /** Each iframe's contentWindow object. */
    pub frame_windows: BTreeMap<usize, ObjectId>,
    /** Proxies for windows in other documents, by target (`messaging::PARENT` or an iframe). */
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub remote_windows: BTreeMap<i64, ObjectId>,
    /** Per-owner context incarnations prevent a retained proxy from addressing a reattached frame. */
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub frame_incarnations: BTreeMap<usize, u64>,
    /** MessagePorts linked to ports in other documents, by link; kept alive while linked, as
    an entangled port is, so a message across the link still finds its handler. */
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub linked_ports: BTreeMap<u64, ObjectId>,
    /** Branded endpoints are weak unless open message listeners retain them. */
    #[serde(default)]
    pub broadcast: broadcast::State,
    /** The reverse of `frames`: the iframe a frame document belongs to, so defaultView finds
    the frame's window without scanning every frame. */
    pub document_frames: BTreeMap<usize, usize>,
    /** Markup written to an open document, parsed when it closes. */
    pub writes: BTreeMap<usize, String>,
    /** The document a script-made root node belongs to (a doctype or processing instruction
    from DOMImplementation or createProcessingInstruction), for ownerDocument. */
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub owners: BTreeMap<usize, usize>,
    /** The media type of each script-made document that is not text/html (XML documents). */
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub content_types: BTreeMap<usize, String>,
    /** The documents DOMParser made. They share the page's URL, where a script-made document
    is about:blank. */
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub parsed_documents: BTreeSet<usize>,
    /** Each script-made document's DOMImplementation object, by the implementation. */
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub implementations: BTreeMap<ObjectId, usize>,
    /** The reverse of `implementations`: a document's implementation object, kept so its
    identity holds across reads. */
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub implementation_of: BTreeMap<usize, ObjectId>,
    /** Attr records by id: each Attr object's owner, name, namespace and detached value. */
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attr_nodes: BTreeMap<u64, attr::AttrNode>,
    /** The attached Attr of each element attribute, by node and then by stored name, so one
    attribute has one Attr object. */
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attr_of: BTreeMap<usize, BTreeMap<String, u64>>,
    /** The next Attr id; ids only grow, so none is reused. */
    #[serde(default)]
    pub next_attr: u64,
    /** Attr wrapper objects by id, so an Attr keeps its identity across reads. */
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attr_wrappers: BTreeMap<u64, ObjectId>,
    /** DOMTokenList objects by node and then attribute, so classList reads return one object. */
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub token_lists: BTreeMap<usize, BTreeMap<String, ObjectId>>,
}

/** Runtime platform state for one request. */
pub(in crate::v9) struct Page<'h> {
    /// Borrowed only during this execution; callbacks cannot survive in serialized state.
    pub imports: Option<&'h dyn crate::v9::ImportScriptsHost>,
    /// Nested imports retain their actual VM activations and share the same turn budgets.
    pub import_depth: usize,
    /// Callback count bounds synchronous host work within one worker turn.
    pub import_count: usize,
    /// Aggregate delivered bytes share the ordinary page source budget.
    pub import_bytes: usize,
    /// Error-handler exceptions propagate without recursively firing the worker error handler.
    pub reporting_worker_error: bool,
    /// Observer continuations already sampled the supplied layout; finish must not resample stale boxes.
    pub sampled_observer_turn: bool,
    /// Synchronous host layouts this step has asked for, bounded by `LAYOUT_CALLS`.
    pub layout_calls: u32,
    /// The nesting level of the timeout callback running now; zero outside one.
    pub timer_nesting: u8,
    /// A final intersection callback may register resize work for the next rendering opportunity.
    pub resize_registration_changed: bool,
    /** Native graphics is injected for each request and is never serialized. */
    pub canvas_host: &'h dyn mars_core::canvas::CanvasHost,
    /** Synchronous raster/readback work left in this execution. */
    pub canvas_work: usize,
    /** Each page script's inline source text by source index (empty for external scripts),
    supplied with every step and never serialized, so `script.textContent` reads the source the
    way pages read inline scripts without the text weighing on retained state. */
    pub inline_scripts: Vec<String>,
    pub environment: PageEnvironment,
    pub languages: Vec<String>,
    pub fragments: &'h dyn FragmentParser,
    pub document: Option<ScriptDocument>,
    pub state: PageState,
    /** Request-local live-time anchor never enters serialized state or script rollback snapshots. */
    pub execution_clock: clock::ExecutionClock,
    pub storage: StorageState,
    pub changes: Vec<DomChange>,
    pub script_changes: Vec<ScriptChange>,
    pub effects: HostEffects,
    pub throw_location: Option<SourceLocation>,
    /** The activations at the first throw, innermost first, reported beside the location. */
    pub throw_trace: Vec<String>,
    pub joining: Vec<JsValue>,
    pub unhandled: Vec<ObjectId>,
    pub change_bytes: usize,
    /**
     * Where each (node, attribute) last written sits in `script_changes`. A hint only: a stale
     * slot is checked against the log before use, so a restored log cannot be corrupted by it.
     */
    pub attribute_slots: std::collections::BTreeMap<(usize, String), usize>,
    /** Events this request created, which give each one its own timestamp tick. */
    pub event_ticks: u64,
    /** The event being dispatched, which `window.event` reads; restored when dispatch nests. */
    pub current_event: Option<ObjectId>,
    pub fragment_bytes: usize,
    pub next_id: usize,
    /** Values seen by legacy getElementById(...).value reads. */
    pub legacy: bool,
    pub tree: Option<nodes::Tree>,
    /** The tree's change generation: bumped by every mutation, so a live collection rescans only after one. */
    pub generation: u64,
    /** The script element of the script running now. */
    pub current_script: Option<usize>,
    /** Legacy-log InsertHtml entries this execution made: change index, root, subtree. */
    pub inserted: Vec<(usize, usize, Vec<usize>)>,
    /** Changes the current DOM call made, for custom element reactions and observers. */
    pub mutations: Vec<components::Mutation>,
    /** Token sets of class-like attributes, by element and attribute name, kept for this request so
    repeated classList calls do not re-split the attribute. Each set is reused only while the
    attribute still holds the text it was read from. */
    pub token_sets: std::collections::BTreeMap<(usize, String), token_set::TokenSet>,
}

impl<'h> Page<'h> {
    /** A page over an optional projection. */
    pub fn new(
        environment: PageEnvironment,
        languages: Vec<String>,
        fragments: &'h dyn FragmentParser,
        document: Option<ScriptDocument>,
        state: PageState,
        storage: StorageState,
    ) -> Self {
        // Projected nodes a script removed keep their ids in page state (as detached nodes or
        // wrappers) after they leave the projection, so new ids start past those too; reusing one
        // would give a new node an old node's wrapper.
        let retained = state
            .detached
            .keys()
            .chain(state.wrappers.keys())
            .copied()
            .filter(|id| *id < nodes::VIRTUAL_BASE);
        let next_id = document
            .as_ref()
            .map(|document| document.nodes.iter().map(|node| node.local_id))
            .into_iter()
            .flatten()
            .chain(retained)
            .map(|id| id + 1)
            .max()
            .unwrap_or(0);
        let effects = HostEffects {
            #[cfg(feature = "conformance")]
            conformance_log: environment.conformance_log.then(String::new),
            ..HostEffects::default()
        };
        Self {
            environment,
            imports: None,
            import_depth: 0,
            import_count: 0,
            import_bytes: 0,
            reporting_worker_error: false,
            sampled_observer_turn: false,
            layout_calls: 0,
            timer_nesting: 0,
            resize_registration_changed: false,
            languages,
            fragments,
            canvas_host: &mars_core::canvas::Unavailable,
            canvas_work: mars_core::canvas::WORK,
            inline_scripts: Vec::new(),
            document,
            state,
            execution_clock: clock::ExecutionClock::default(),
            storage,
            changes: Vec::new(),
            script_changes: Vec::new(),
            effects,
            throw_location: None,
            throw_trace: Vec::new(),
            joining: Vec::new(),
            unhandled: Vec::new(),
            change_bytes: 0,
            attribute_slots: std::collections::BTreeMap::new(),
            event_ticks: 0,
            current_event: None,
            fragment_bytes: 0,
            next_id,
            legacy: false,
            tree: None,
            generation: nodes::next_generation(),
            inserted: Vec::new(),
            current_script: None,
            mutations: Vec::new(),
            token_sets: std::collections::BTreeMap::new(),
        }
    }
    /** Record a console line within the bounds. */
    pub fn console(&mut self, level: &str, text: String) {
        let used = self.effects.console.len();
        #[cfg(feature = "conformance")]
        let used = used + self.effects.conformance_trace_marks.len();
        if used >= VM_CONSOLE_ENTRIES {
            self.effects.console_dropped += 1;
            return;
        }
        let mut text = text;
        if text.len() > VM_CONSOLE_BYTES {
            let mut end = VM_CONSOLE_BYTES;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
        }
        self.effects.console.push(ConsoleEntry {
            level: level.into(),
            text,
        });
    }
    /** Note an unsupported API once. */
    pub fn note_unsupported(&mut self, feature: &str) {
        if self.effects.unsupported.len() < VM_CONSOLE_ENTRIES
            && !self
                .effects
                .unsupported
                .iter()
                .any(|known| known == feature)
        {
            self.effects.unsupported.push(feature.into());
        }
    }
    /** Values the page keeps alive: timers, listeners, wrappers with data, pending requests. */
    pub fn roots(&self, out: &mut Vec<JsValue>) {
        media::roots(&self.state.media, &self.state.wrappers, out);
        intersection_observers::roots(&self.state, out);
        resize_observers::roots(&self.state, out);
        #[cfg(feature = "conformance")]
        out.extend(
            self.state
                .number_constants
                .values()
                .flat_map(|pool| pool.values.values().cloned()),
        );
        #[cfg(feature = "conformance")]
        if let Some(listener) = self.state.cpu_profile.listener {
            out.push(JsValue::Object(listener));
        }
        #[cfg(feature = "conformance")]
        if let Some(context) = self.state.cpu_profile.context {
            out.push(JsValue::Object(context));
        }
        out.extend(
            self.state
                .active_animations
                .iter()
                .copied()
                .map(JsValue::Object),
        );
        for timer in &self.state.timers {
            out.push(timer.callback.clone());
            out.extend(timer.args.iter().cloned());
        }
        for (node, image) in &self.state.images {
            if image.requested && !image.complete {
                out.extend(self.state.wrappers.get(node).copied().map(JsValue::Object));
            }
        }
        for listener in self.state.listeners.iter() {
            if matches!(listener.target, Target::Window | Target::Document) {
                out.push(listener.callback.clone());
                out.extend(listener.signal.clone());
            }
        }
        out.extend(
            self.state
                .pending
                .iter()
                .map(|pending| JsValue::Object(pending.owner)),
        );
        out.extend(
            self.state
                .pending
                .iter()
                .filter_map(|pending| pending.signal)
                .map(JsValue::Object),
        );
        if let Some(value) = &self.state.history_state {
            out.push(value.clone());
        }
        out.extend(self.state.platform_installer.map(JsValue::Object));
        #[cfg(feature = "conformance")]
        if let Some(value) = &self.state.continuation_data {
            out.push(value.clone());
        }
        #[cfg(feature = "conformance")]
        out.extend(
            self.state
                .character_roots
                .values()
                .cloned()
                .map(JsValue::String),
        );
        out.extend(self.state.selection.map(JsValue::Object));
        out.extend(
            self.state
                .workers
                .values()
                .map(|handle| JsValue::Object(handle.object)),
        );
        out.extend(
            self.state
                .linked_ports
                .values()
                .map(|id| JsValue::Object(*id)),
        );
        out.extend(self.joining.iter().cloned());
        out.extend(self.unhandled.iter().map(|id| JsValue::Object(*id)));
        self.state.components.roots(out, &self.state.wrappers);
    }
    /** Forget wrappers and requests whose objects were collected. */
    pub fn sweep(&mut self, live: &super::vm::heap::FastSet<ObjectId>) {
        broadcast::sweep(&mut self.state.broadcast, live);
        self.state.path2d.retain(|id, _| live.contains(id));
        self.state
            .resize_observers
            .retain(|id, _| live.contains(id));
        self.state
            .intersection_observers
            .retain(|id, _| live.contains(id));
        self.state
            .canvases
            .retain(|_, state| live.contains(&state.context));
        self.state.active_animations.retain(|id| live.contains(id));
        self.state.live_ranges.retain(|id| live.contains(id));
        self.state.wrappers.retain(|_, id| live.contains(id));
        self.state
            .listeners
            .retain(|listener| match listener.target {
                Target::Node(node) => self.state.wrappers.contains_key(&node),
                Target::Object(id) => live.contains(&id),
                _ => true,
            });
        self.state.xhrs.retain(|id, _| live.contains(id));
        self.state.attr_wrappers.retain(|_, id| live.contains(id));
        messaging::sweep(&mut self.state.remote_windows, live);
        for by_attribute in self.state.token_lists.values_mut() {
            by_attribute.retain(|_, id| live.contains(id));
        }
    }
}

/** The error a request gets when it runs out of wall-clock time. */
pub(in crate::v9) fn timeout() -> EngineError {
    EngineError::new(
        "limit_exceeded",
        "Script execution stopped: time limit exceeded.",
        "Reduce the script work; what it did before it stopped stays, as in browsers.",
    )
}

impl Vm<'_> {
    /** Report an uncaught error from a callback (timer, listener, job) to the console. */
    pub fn report_error(&mut self, error: &JsValue) {
        #[cfg(feature = "conformance")]
        self.note_conformance_native_error(error);
        let text = self.describe(error);
        #[cfg(feature = "conformance")]
        let text = self.conformance_description(error).unwrap_or(text);
        #[cfg(feature = "conformance")]
        if self.page.effects.conformance_error.is_none() {
            self.page.effects.conformance_error = Some(format!("Uncaught {text}"));
        }
        let place = self
            .page
            .throw_location
            .take()
            .map(|location| {
                format!(
                    " ({} line {}, column {})",
                    self.modules.source_name(location.byte_offset),
                    location.line,
                    location.column
                )
            })
            .unwrap_or_default();
        // An Error object remembers where it was made, which beats where it was rethrown
        // (a deferred `setTimeout(() => { throw e })` says nothing about the fault).
        let recorded = error
            .object()
            .and_then(|id| self.heap.get(id))
            .and_then(|object| {
                object
                    .props
                    .get(&Key::private(crate::v9::vm::error::TRACE_KEY))
            })
            .and_then(|prop| match &prop.slot {
                crate::v9::vm::heap::Slot::Data(JsValue::String(text)) => Some(text.lossy()),
                _ => None,
            })
            .filter(|text| !text.is_empty())
            .map(|text| text.lines().map(String::from).collect::<Vec<_>>());
        let frames = recorded.unwrap_or_else(|| std::mem::take(&mut self.page.throw_trace));
        self.page.throw_trace.clear();
        let trace: String = frames
            .iter()
            .map(|frame| format!("\n    at {frame}"))
            .collect();
        self.page
            .console("error", format!("Uncaught {text}{place}{trace}"));
    }
}

/** The interface name `Object.prototype.toString` reports for a host object, as browsers do. */
pub(in crate::v9) fn interface_tag(vm: &mut Vm, id: ObjectId) -> Option<String> {
    if id == GLOBAL {
        #[cfg(feature = "conformance")]
        if vm.page.environment.conformance_d8 {
            return Some("global".into());
        }
        return Some("Window".into());
    }
    if dom::node_of(vm, &JsValue::Object(id)) == Some(dom::DOCUMENT) {
        return Some("HTMLDocument".into());
    }
    Some(match host_data(vm, &JsValue::Object(id))? {
        HostData::Node(dom::DOCUMENT) | HostData::Document(_) => "HTMLDocument".into(),
        HostData::Node(node) => element_interface(&vm.page.view(node)?.tag),
        HostData::Event(_) => "Event".into(),
        HostData::Style(_) | HostData::Computed(_) => "CSSStyleDeclaration".into(),
        HostData::ClassList(_) => "DOMTokenList".into(),
        HostData::Dataset(_) => "DOMStringMap".into(),
        HostData::NodeList(_) | HostData::Children(_, false) => "NodeList".into(),
        HostData::Children(_, true) => "HTMLCollection".into(),
        HostData::Live(_) => "HTMLCollection".into(),
        HostData::Xhr(_) => "XMLHttpRequest".into(),
        HostData::Response(_) => "Response".into(),
        HostData::Headers(_) => "Headers".into(),
        HostData::Url(_) => "URL".into(),
        HostData::SearchParams(_) => "URLSearchParams".into(),
        HostData::Storage(_) => "Storage".into(),
        HostData::Observer => "Object".into(),
        HostData::Range(_) => "Range".into(),
        HostData::Selection(_) => "Selection".into(),
        HostData::Intl(data) => format!("Intl.{}", data.kind),
        #[cfg(feature = "conformance")]
        HostData::BreakIterator(_) => "Intl.v8BreakIterator".into(),
        HostData::StyleSheet(_) => "CSSStyleSheet".into(),
        HostData::TokenList(..) => "DOMTokenList".into(),
        HostData::Attr(_) => "Attr".into(),
        HostData::Request(_) => "Request".into(),
    })
}

/** The element interface a tag name maps to; unknown tags are plain HTMLElement. */
fn element_interface(tag: &str) -> String {
    let name = match tag {
        "#text" => return "Text".into(),
        "#comment" => return "Comment".into(),
        "svg" => return "SVGSVGElement".into(),
        "path" | "g" | "circle" | "rect" | "line" | "polygon" | "polyline" | "ellipse" | "use"
        | "text" | "defs" | "symbol" | "clipPath" | "mask" | "linearGradient"
        | "radialGradient" | "stop" | "image" | "foreignObject" => return "SVGElement".into(),
        "a" => "Anchor",
        "p" => "Paragraph",
        "div" => "Div",
        "span" => "Span",
        "img" => "Image",
        "input" => "Input",
        "button" => "Button",
        "form" => "Form",
        "select" => "Select",
        "option" => "Option",
        "textarea" => "TextArea",
        "label" => "Label",
        "ul" => "UList",
        "ol" => "OList",
        "li" => "LI",
        "table" => "Table",
        "tr" => "TableRow",
        "td" | "th" => "TableCell",
        "body" => "Body",
        "html" => "Html",
        "head" => "Head",
        "script" => "Script",
        "style" => "Style",
        "iframe" => "IFrame",
        "canvas" => "Canvas",
        "video" => "Video",
        "audio" => "Audio",
        "br" => "BR",
        "hr" => "HR",
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => "Heading",
        _ => "",
    };
    format!("HTML{name}Element")
}

/** Host objects' own virtual properties (indexes of node lists, storage keys, style). */
pub(in crate::v9) fn host_own(vm: &mut Vm, id: ObjectId, key: &Key) -> Res<Option<Prop>> {
    dom::host_own(vm, id, key)
}

/** Give a fresh page's window every global interface as its own property, the way WebIDL puts a
[Global] interface's members on the object itself: `window.hasOwnProperty("Promise")` and
`Object.getOwnPropertyDescriptor(window, "Date")` are how polyfills and feature tests look, and
with the constructors only on the prototype both said no. Values stay shared intrinsics; only the
property slots are copied, non-enumerable like a browser's. Deleting one still leaves the
prototype's copy reachable by name (ponytail: a deletion mark on the chain walk would close that). */
pub(in crate::v9) fn expose_window_globals(vm: &mut Vm) {
    use super::vm::heap::Slot;
    use super::vm::realm::{Static, realm};
    use crate::v9::value::JsValue;
    let interfaces = if vm.page.environment.worker_name.is_some() {
        realm().ids.worker_interfaces
    } else {
        realm().ids.window_interfaces
    };
    let Some(def) = realm().object(interfaces) else {
        return;
    };
    let globals: Vec<(Key, JsValue, u8)> = def
        .props
        .iter()
        .filter_map(|(key, value, attrs)| match value {
            Static::Value(value) => Some((key.clone(), value.clone(), *attrs)),
            Static::Accessor(..) => None,
        })
        .collect();
    // ECMAScript's own value properties: NaN, Infinity and undefined are frozen on the global
    // object, and globalThis is the object itself as a writable, configurable data property.
    let frozen = ["NaN", "Infinity", "undefined"].map(Key::from);
    let global_this = Key::from("globalThis");
    for (key, value, attrs) in globals {
        let attrs = if frozen.contains(&key) { 0 } else { attrs };
        let value = if key == global_this {
            JsValue::Object(GLOBAL)
        } else {
            value
        };
        let prop = Prop {
            slot: Slot::Data(value),
            attrs,
        };
        vm.heap.put(GLOBAL, key, prop);
    }
}

/** Host objects' own writes; None means ordinary handling. */
pub(in crate::v9) fn host_write(
    vm: &mut Vm,
    id: ObjectId,
    key: &Key,
    value: &JsValue,
) -> Res<Option<bool>> {
    dom::host_write(vm, id, key, value)
}

/** Host objects' own deletes; true when handled. */
pub(in crate::v9) fn host_delete(vm: &mut Vm, id: ObjectId, key: &Key) -> Res<bool> {
    dom::host_delete(vm, id, key)
}

/** Host objects' own virtual keys. */
pub(in crate::v9) fn host_keys(vm: &mut Vm, id: ObjectId) -> Res<Vec<Key>> {
    dom::host_keys(vm, id)
}

/** The host data of an object, cloned. */
pub(in crate::v9) fn host_data(vm: &Vm, value: &JsValue) -> Option<super::vm::data::HostData> {
    let JsValue::Object(id) = value else {
        return None;
    };
    match vm.heap.get(*id).map(|object| &object.class) {
        Some(Class::Host(data)) => Some((**data).clone()),
        _ => None,
    }
}
