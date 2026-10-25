/**
 * The browser platform V9 gives page scripts: window, document, navigator, location, history,
 * timers, events, storage, console, fetch and XMLHttpRequest. Nothing here has native authority:
 * the document is the app's inert projection, network calls and navigations are recorded for the
 * app to perform, and time is a virtual clock the app advances.
 */
mod channel;
pub(in crate::v9) mod components;
mod console;
/** DevTools evaluation (Runtime.evaluate and friends) over the page's realm; new entry points only. */
pub mod devtools;
mod dom;
mod dom_parser;
pub(in crate::v9) mod events;
pub(in crate::v9) mod html;
mod install;
mod interfaces;
mod net;
mod nodes;
mod range;
pub(in crate::v9) mod scripts;
mod selector;
mod sheets;
mod timers;
mod window;

pub(in crate::v9) use dom::{focus, native_name};
pub use html::SimpleFragmentParser;

pub(in crate::v9) use install::install;
pub(in crate::v9) use net::XhrState;
pub(in crate::v9) use nodes::{BODY, HEAD, HTML};

impl Page<'_> {
    /** Give every page script a head `<script>` element (once per page), after the page's
    metadata elements. */
    pub fn script_elements(&mut self, count: usize) -> EngineResult<Vec<usize>> {
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
        Ok(elements)
    }
}

impl Page<'_> {
    /// The app's metadata elements (`meta`, `link`, `style`, `title`, `base`, data-block
    /// `script`) become head records in source order, so lookups such as
    /// `querySelector('meta[name=locale]')` find them. Like other head content they stay V9-local.
    /// ponytail: one placed in the body is a head child here too; give it its body position if
    /// a page walks siblings around one.
    fn metadata_elements(&mut self) -> EngineResult<()> {
        let metadata = std::mem::take(&mut self.environment.metadata);
        for element in &metadata {
            let text = (!element.text.is_empty()).then(|| element.text.clone());
            let node = self.create(&element.tag, text)?;
            if let Some(record) = self.state.detached.get_mut(&node) {
                record.attributes.clone_from(&element.attributes);
            }
            self.insert(HEAD, node, None)?;
        }
        self.environment.metadata = metadata;
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
use std::collections::BTreeMap;

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
    /// Local time minus UTC in minutes, for Date's local-time methods.
    pub timezone_offset_minutes: f64,
    /// Seed for Math.random and crypto.randomUUID on a new page.
    pub random_seed: u64,
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
    /// Metadata elements Eye keeps out of layout, in source order; V9 makes each a head
    /// element at load so scripts find and read them.
    pub metadata: Vec<crate::document::MetadataElement>,
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
    /// The document's scroll offset, which client coordinates subtract.
    pub scroll_y: f64,
    /// The first node identity the last layout never saw: a node at or above it was inserted
    /// since, so `geometry` cannot hold its box. A browser would force a layout when script
    /// reads such a node's geometry; V9 cannot, so it reports the read in
    /// `HostEffects::stale_geometry` and the app may lay out and run the step again. None
    /// when the app gives no such promise, and no read is reported.
    pub laid_out_before: Option<usize>,
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
}

impl Default for PageEnvironment {
    /** A blank desktop page. */
    fn default() -> Self {
        Self {
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
            timezone_offset_minutes: 0.0,
            random_seed: 0,
            online: true,
            title: String::new(),
            dom_level: DomLevel::Legacy,
            node_text: BTreeMap::new(),
            scripts: Vec::new(),
            script_attributes: Vec::new(),
            metadata: Vec::new(),
            html_attributes: BTreeMap::new(),
            body_attributes: BTreeMap::new(),
            geometry: Vec::new(),
            scroll_y: 0.0,
            laid_out_before: None,
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
}

/// What the app must do after an execution besides committing state and changes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HostEffects {
    /// Requests to perform; answer them with `resume_requests`.
    pub requests: Vec<HostRequest>,
    /// The last navigation request, if any.
    pub navigation: Option<NavigationRequest>,
    /// pushState, replaceState and hash changes, in order.
    pub history: Vec<HistoryRecord>,
    /// Raw `document.cookie = ...` fields, in order, for `CookieJar::set_document_cookie`.
    pub cookies: Vec<String>,
    /// Console output, at most VM_CONSOLE_ENTRIES.
    pub console: Vec<ConsoleEntry>,
    /// Console entries dropped past the bound.
    pub console_dropped: usize,
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
    /// Absolute URLs of `<link rel=stylesheet>` elements scripts connected (in the head too,
    /// which stays V9's), for the app to fetch and apply, as a browser loads an inserted sheet.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stylesheets: Vec<String>,
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
}

/** Where a listener is registered. */
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

/** Page state that survives between requests, inside VmState. */
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(in crate::v9) struct PageState {
    pub identity: String,
    /** Virtual milliseconds since the time origin; performance.now(). */
    pub clock: f64,
    /** Date.now() at clock zero. */
    pub epoch: f64,
    pub random: u64,
    pub timers: Vec<Timer>,
    pub next_timer: u32,
    /** Next scheduling sequence number. */
    pub next_order: u32,
    pub listeners: Vec<Listener>,
    pub next_listener: u32,
    /** Node wrappers by node id, so identity survives requests. */
    pub wrappers: BTreeMap<usize, ObjectId>,
    pub detached: BTreeMap<usize, Detached>,
    pub next_detached: usize,
    pub pending: Vec<Pending>,
    pub next_request: u32,
    pub ready: ReadyState,
    /** The current URL (pushState changes it). */
    pub href: String,
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
    /** Children of the virtual head, which the projection does not carry. */
    pub head: Vec<usize>,
    /** Extra children of the virtual html element after head and body. */
    pub html: Vec<usize>,
    /** Attributes of the virtual html/head/body nodes. */
    pub virtual_attributes: BTreeMap<usize, BTreeMap<String, String>>,
    pub initialized: bool,
    /** The page's Selection object, once a script asked for it; its ranges stay reachable. */
    pub selection: Option<ObjectId>,
    /** Script elements scripts connected, in start order: each `<script src>` asks the app for
    its source once and runs once when it arrives; an inline one is recorded as done. */
    pub script_loads: Vec<ScriptLoad>,
    /** The focused element (`document.activeElement`), set by `focus()`, typing and clicks. */
    pub focused: Option<usize>,
    /** Custom element definitions, shadow and template bookkeeping, mutation observers. */
    pub components: components::State,
}

/** Runtime platform state for one request. */
pub(in crate::v9) struct Page<'h> {
    pub environment: PageEnvironment,
    pub languages: Vec<String>,
    pub fragments: &'h dyn FragmentParser,
    pub document: Option<ScriptDocument>,
    pub state: PageState,
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
    pub fragment_bytes: usize,
    pub next_id: usize,
    /** Values seen by legacy getElementById(...).value reads. */
    pub legacy: bool,
    pub tree: Option<nodes::Tree>,
    /** The script element of the script running now. */
    pub current_script: Option<usize>,
    /** Legacy-log InsertHtml entries this execution made: change index, root, subtree. */
    pub inserted: Vec<(usize, usize, Vec<usize>)>,
    /** Changes the current DOM call made, for custom element reactions and observers. */
    pub mutations: Vec<components::Mutation>,
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
        Self {
            environment,
            languages,
            fragments,
            document,
            state,
            storage,
            changes: Vec::new(),
            script_changes: Vec::new(),
            effects: HostEffects::default(),
            throw_location: None,
            throw_trace: Vec::new(),
            joining: Vec::new(),
            unhandled: Vec::new(),
            change_bytes: 0,
            fragment_bytes: 0,
            next_id,
            legacy: false,
            tree: None,
            inserted: Vec::new(),
            current_script: None,
            mutations: Vec::new(),
        }
    }
    /** Record a console line within the bounds. */
    pub fn console(&mut self, level: &str, text: String) {
        if self.effects.console.len() >= VM_CONSOLE_ENTRIES {
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
        for timer in &self.state.timers {
            out.push(timer.callback.clone());
            out.extend(timer.args.iter().cloned());
        }
        for listener in &self.state.listeners {
            out.push(listener.callback.clone());
            if let Target::Object(id) = listener.target {
                out.push(JsValue::Object(id));
            }
        }
        out.extend(
            self.state
                .pending
                .iter()
                .map(|pending| JsValue::Object(pending.owner)),
        );
        if let Some(value) = &self.state.history_state {
            out.push(value.clone());
        }
        out.extend(self.state.selection.map(JsValue::Object));
        out.extend(self.joining.iter().cloned());
        out.extend(self.unhandled.iter().map(|id| JsValue::Object(*id)));
        self.state.components.roots(out, &self.state.wrappers);
    }
    /** Forget wrappers and requests whose objects were collected. */
    pub fn sweep(&mut self, live: &super::vm::heap::FastSet<ObjectId>) {
        self.state.wrappers.retain(|_, id| live.contains(id));
        self.state.xhrs.retain(|id, _| live.contains(id));
    }
}

/** The error a request gets when it runs out of wall-clock time. */
pub(in crate::v9) fn timeout() -> EngineError {
    EngineError::new(
        "limit_exceeded",
        "Script execution stopped: time limit exceeded.",
        "Reduce the script work; that script's changes were not applied.",
    )
}

impl Vm<'_> {
    /** Report an uncaught error from a callback (timer, listener, job) to the console. */
    pub fn report_error(&mut self, error: &JsValue) {
        let text = self.describe(error);
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
                    .get(&Key::from(crate::v9::vm::error::TRACE_KEY))
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
        HostData::NodeList(_) | HostData::Children(..) => "NodeList".into(),
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
        HostData::StyleSheet(_) => "CSSStyleSheet".into(),
        HostData::TokenList(..) => "DOMTokenList".into(),
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
