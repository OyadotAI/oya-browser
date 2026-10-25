/**
 * V9, Mars's own JavaScript engine: a bytecode VM with a bounded, serializable heap, the
 * ECMAScript library, and a web platform over the app's inert document projection. Every entry
 * point takes explicit data and returns proposals; nothing here has native authority.
 */
mod binding;
mod builtins;
mod compile;
mod errors;
mod lexer;
mod literals;
mod modules;
mod operator;
mod platform;
mod regexp;
mod source;
mod syntax;
mod value;
mod vm;

pub use binding::{Binding, BindingKind, Globals};
pub use modules::{EntryKind, ModuleEntry, ModuleInput, ModuleRequest, ModuleSource};
pub use platform::NodeBox;
pub use platform::devtools;
pub use platform::{
    ConsoleEntry, DomLevel, HistoryRecord, HostEffects, HostRequest, HostResponse,
    NavigationRequest, PageEnvironment, SimpleFragmentParser,
};
pub use value::{CodePointCursor, JsString, JsValue};

use crate::constants::{
    LANGUAGE_TAG_BYTES, NAVIGATOR_LANGUAGES, SCRIPT_BYTES, SCRIPT_COUNT, SCRIPT_DEADLINE,
    SCRIPT_TOTAL_BYTES, STORAGE_AREA_BYTES, STORAGE_ENTRIES, STORAGE_VALUE_BYTES, TEXT_BYTES,
};
use crate::document::{
    DomChange, FragmentParser, ScriptChange, ScriptDocument, ScriptNode, TextChange,
};
use crate::protocol::{EngineError, EngineResult, SourceLocation};
use compile::{Code, Registry};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use vm::data::{Class, HostData, Job};
use vm::heap::Heap;
use vm::{Limits, Symbols, Thrown, Vm};

/** Dynamic code (new Function, indirect eval) gets offsets past every page source. */
pub(in crate::v9) const DYNAMIC_OFFSET: usize = 256 * 1024 * 1024;
/** Legacy inline handlers compile at offsets no page or dynamic source reaches. */
const HANDLER_OFFSET: usize = 512 * 1024 * 1024;

/// A page's committed engine state: heap, lexical globals and platform state. The app keeps it
/// between requests and passes it back; it is validated on the way in.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VmState {
    #[serde(default)]
    globals: Globals,
    #[serde(default)]
    heap: Heap,
    #[serde(default)]
    symbols: Symbols,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    jobs: Vec<Job>,
    #[serde(default)]
    page: platform::PageState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dynamic: Vec<vm::DynamicSource>,
    #[serde(default, skip_serializing_if = "modules::ModuleState::is_empty")]
    modules: modules::ModuleState,
}

impl VmState {
    /// Committed top-level let, const and class bindings, for inspection.
    pub fn globals(&self) -> &Globals {
        &self.globals
    }
    /// A state holding only lexical globals (used by tests and legacy callers).
    pub fn from_globals(globals: Globals) -> Self {
        Self {
            globals,
            ..Self::default()
        }
    }
    /// Whether a registered click listener sits on this node (directly).
    pub fn has_click_listener(&self, identity: &str, local_id: usize) -> bool {
        self.page.identity == identity
            && self.page.listeners.iter().any(|listener| {
                listener.kind == "click" && listener.target == platform::Target::Node(local_id)
            })
    }
    /// Whether a click on this node reaches any listener: the node, an ancestor, the document
    /// or the window (`document` is the current projection).
    pub fn click_reaches_listener(&self, document: &ScriptDocument, local_id: usize) -> bool {
        if self.page.identity != document.identity {
            return false;
        }
        let mut path = vec![local_id];
        let mut cursor = Some(local_id);
        while let Some(id) = cursor {
            cursor = document
                .nodes
                .iter()
                .find(|node| node.local_id == id)
                .and_then(|node| node.parent);
            if let Some(parent) = cursor {
                path.push(parent);
            }
            if path.len() > crate::constants::DOCUMENT_DEPTH * 2 {
                break;
            }
        }
        self.page.listeners.iter().any(|listener| {
            listener.kind == "click"
                && match listener.target {
                    platform::Target::Node(node) => {
                        path.contains(&node) || node >= platform::BODY - 2
                    }
                    platform::Target::Document | platform::Target::Window => true,
                    platform::Target::Object(_) => false,
                }
        })
    }
    /// Check that retained node references fit the document the app just committed.
    pub fn validate_document(&self, document: &ScriptDocument) -> EngineResult<()> {
        document.validate()?;
        self.validate(&Limits::default())?;
        if self.page.initialized && self.page.identity != document.identity {
            return Err(EngineError::invalid(
                "V9 state belongs to another document.",
            ));
        }
        let projected: std::collections::BTreeSet<usize> =
            document.nodes.iter().map(|node| node.local_id).collect();
        let known = |node: &usize| {
            projected.contains(node)
                || self.page.detached.contains_key(node)
                || *node >= platform::nodes_virtual_base()
        };
        // A listener on a node is registered through a wrapper of a node the page had; one on a
        // node the document never held is forged.
        if self.page.listeners.iter().any(
            |listener| matches!(listener.target, platform::Target::Node(node) if !known(&node)),
        ) {
            return Err(EngineError::invalid("Invalid V9 listener target."));
        }
        for node in self.page.wrappers.keys() {
            let known = projected.contains(node)
                || self.page.detached.contains_key(node)
                || *node >= platform::nodes_virtual_base();
            if !known {
                return Err(EngineError::unsupported(
                    "Retaining references to nodes the document no longer has is not supported.",
                ));
            }
        }
        Ok(())
    }
    /// Structural and size validation of untrusted saved state.
    fn validate(&self, limits: &Limits) -> EngineResult<()> {
        let counts = self.heap.counts;
        // Global lexical strings live outside the heap yet are retained environment text too.
        let global_units: usize = self
            .globals
            .values()
            .map(|binding| binding.value.as_ref().map_or(0, JsValue::string_units))
            .sum();
        for (count, limit, name) in [
            (counts.objects, limits.objects + 2, "V9 heap objects"),
            (counts.functions, limits.functions, "V9 functions"),
            (
                counts.properties,
                limits.properties,
                "V9 aggregate own properties",
            ),
            (
                counts.units + global_units,
                limits.units,
                "V9 environment UTF-16 units",
            ),
            (counts.environments, limits.environments, "V9 environments"),
            (
                counts.bindings + self.globals.len(),
                limits.bindings,
                "V9 bindings",
            ),
        ] {
            if count > limit {
                return Err(EngineError::limit(name, limit));
            }
        }
        if self
            .heap
            .objects
            .keys()
            .any(|id| *id >= self.heap.next_object && *id < vm::heap::INTRINSIC_BASE)
            || self.heap.envs.keys().any(|id| *id >= self.heap.next_env)
        {
            return Err(EngineError::invalid("Invalid V9 heap identities."));
        }
        if self.page.timers.len() > crate::constants::VM_TIMERS
            || self.page.listeners.len() > platform::events::LISTENERS
            || self.page.pending.len() > crate::constants::VM_HOST_REQUESTS
            || self.page.detached.len() > crate::constants::VM_DETACHED_NODES
            || self.jobs.len() > limits.jobs
            || self.dynamic.len() > crate::constants::VM_DYNAMIC_SOURCES
            || self.symbols.descriptions.len() > crate::constants::VM_SYMBOLS
        {
            return Err(EngineError::limit(
                "V9 page state",
                crate::constants::VM_TIMERS,
            ));
        }
        let bytes = serde_json::to_vec(self)
            .map_err(|_| EngineError::invalid("Cannot encode V9 state."))?
            .len();
        if bytes > limits.state_bytes {
            return Err(EngineError::limit("V9 state bytes", limits.state_bytes));
        }
        Ok(())
    }
}

/// Only input IDs and literal values cross into V9, never native agent handles.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InputValue {
    /// Source lookup key within the exact candidate document.
    pub html_id: String,
    /// Bounded current field text.
    pub value: String,
}

/// The outcome of one request: proposals, the new state and what the app must do next.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    /// Ordered DomChange proposals (the legacy log); the document engine replays them.
    pub changes: Vec<DomChange>,
    /// The extended log, used instead of `changes` when `PageEnvironment::dom_level` is Extended.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub script_changes: Vec<ScriptChange>,
    /// Digest of the final script projection; the applying engine must reproduce it exactly.
    #[serde(default)]
    pub structure: u64,
    /// Globals, heap and platform state commit atomically with document proposals.
    pub state: VmState,
    /// Storage proposals remain candidates until the coordinator commits the page.
    pub storage: StorageState,
    /// Scripts an isolated page load stopped and rolled back, in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed: Vec<ScriptFailure>,
    /// Requests, navigations, history, cookies, console output and the next timer.
    #[serde(default)]
    pub effects: HostEffects,
    /// Modules the page's graphs need; deliver them with `load_modules`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub module_requests: Vec<ModuleRequest>,
    /// Classic scripts the page's elements asked for; deliver each as a classic `ModuleSource`
    /// (or as failed) with `load_modules`, which runs it and fires the element's event.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub script_requests: Vec<ScriptLoadRequest>,
}

/** A `<script src>` a script connected to the document, for the app to fetch the way a browser
fetches a classic script: under the page's Content-Security-Policy (the element's nonce counts),
with a JavaScript media type, and against the element's integrity metadata. */
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptLoadRequest {
    /// The absolute script URL, which is its identity in the delivered sources.
    pub url: String,
    /// The element's `nonce` attribute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    /// The element's `integrity` attribute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrity: Option<String>,
    /// The element's `crossorigin` attribute ("anonymous" or "use-credentials").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cross_origin: Option<String>,
}

/** One script that stopped during an isolated page load; none of its effects were kept. */
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptFailure {
    /// Zero-based position in the sources the caller passed.
    pub index: usize,
    /// Why it stopped: a syntax error, an exception, an unsupported feature or a limit.
    pub error: EngineError,
}

/** Only string keys and values cross the V9 storage boundary. */
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StorageState {
    /// Origin-scoped localStorage candidate.
    pub local: BTreeMap<String, String>,
    /// Tab and origin-scoped sessionStorage candidate.
    pub session: BTreeMap<String, String>,
}

impl StorageState {
    /// Enforce independent entry and byte budgets before a candidate enters or leaves V9.
    pub fn validate(&self) -> EngineResult<()> {
        validate_storage_area(&self.local)?;
        validate_storage_area(&self.session)
    }
}

/** Storage limits apply to UTF-8 key/value bytes and never count profile encoding overhead. */
fn validate_storage_area(area: &BTreeMap<String, String>) -> EngineResult<()> {
    let bytes = area
        .iter()
        .map(|(key, value)| key.len().saturating_add(value.len()))
        .sum::<usize>();
    if area.len() > STORAGE_ENTRIES
        || area.iter().any(|(key, value)| {
            key.len() > STORAGE_VALUE_BYTES || value.len() > STORAGE_VALUE_BYTES
        })
        || bytes > STORAGE_AREA_BYTES
    {
        return Err(EngineError::limit(
            "Web Storage area bytes",
            STORAGE_AREA_BYTES,
        ));
    }
    Ok(())
}

/// Per-execution browser facts and the fragment parser, supplied by the app at the service edge.
pub struct DocumentHost<'a> {
    /// Preferred language tags, most preferred first, as `navigator.languages` reports them.
    pub languages: &'a [String],
    /// Parses inserted HTML so the same run sees new nodes; V9 never imports Eye for this.
    pub fragments: &'a (dyn FragmentParser + Sync),
}

impl Default for DocumentHost<'static> {
    /** No languages and a parser that refuses every fragment. */
    fn default() -> Self {
        Self {
            languages: &[],
            fragments: &RefuseFragments,
        }
    }
}

/** Callers that supply no parser get an explicit unsupported error, never a silent no-op. */
struct RefuseFragments;

impl FragmentParser for RefuseFragments {
    /** Every fragment is refused before any node identity is consumed. */
    fn parse(&self, _: &str, _: &str, _: usize) -> EngineResult<Vec<ScriptNode>> {
        Err(EngineError::unsupported(
            "insertAdjacentHTML requires a host fragment parser.",
        ))
    }
}

/// Everything one page request needs besides committed state.
pub struct PageInput<'a> {
    /// The page's classic scripts in execution order (failed ones blanked to spaces).
    pub sources: &'a [String],
    /// The current projection.
    pub document: ScriptDocument,
    /// Origin storage candidates.
    pub storage: StorageState,
    /// Languages and the fragment parser.
    pub host: &'a DocumentHost<'a>,
    /// URL, viewport, clock, cookie and the change-log level.
    pub environment: &'a PageEnvironment,
    /// Which sources are module scripts or import maps, and the modules fetched so far.
    pub modules: &'a ModuleInput,
}

/** The module input of callers that have no modules. */
const NO_MODULES: &ModuleInput = ModuleInput::NONE;

/// Page lifecycle events the app fires after scripts load.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageEvent {
    /// readyState becomes "interactive"; DOMContentLoaded fires at the document.
    DomContentLoaded,
    /// readyState becomes "complete"; load fires at the window.
    Load,
    /// The viewport scrolled: `scroll` fires at the document and bubbles to the window, as
    /// Blink's `ScrollableArea` queues it once per frame that moved. Lazy loaders listen for it.
    Scroll,
}

/**
 * Parsing and evaluation recurse along the syntax tree, so every entry point runs on its own
 * thread with V9_STACK_BYTES of stack. A panic stays on that thread and becomes an error.
 */
fn on_v9_stack<T: Send>(work: impl FnOnce() -> EngineResult<T> + Send) -> EngineResult<T> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("v9".into())
            .stack_size(crate::constants::V9_STACK_BYTES)
            .spawn_scoped(scope, work)
            .map_err(|_| EngineError::unsupported("V9 could not start its execution thread."))?
            .join()
            .unwrap_or_else(|_| Err(EngineError::invalid("V9 stopped on an internal error.")))
    })
}

/** Tokenize and parse one source whose bytes start at `offset` of the page. */
pub(in crate::v9) fn parse_at(
    source: &str,
    offset: usize,
    index: Option<usize>,
) -> EngineResult<syntax::Program> {
    let mut tokens = lexer::tokenize(source).map_err(|mut error| {
        if let Some(location) = error.location.as_deref_mut() {
            location.byte_offset += offset;
            location.script_index = index.map(|index| index + 1);
        }
        error
    })?;
    for token in &mut tokens {
        token.location.byte_offset += offset;
    }
    syntax::parse(tokens).map_err(|mut error| {
        if let Some(location) = error.location.as_deref_mut() {
            location.script_index = index.map(|index| index + 1);
        }
        error
    })
}

/** Compile dynamic source at `base` into the registry; returns its top-level code. `context` is
the compiler-made caller context of direct eval code, never read from the source. */
pub(in crate::v9) fn compile_dynamic(
    registry: &mut Registry,
    source: &str,
    base: usize,
    context: Option<&str>,
) -> EngineResult<Arc<Code>> {
    let site = syntax::site_of(syntax::Pos {
        offset: base as u32,
        line: 1,
        column: 1,
    });
    if let Some(code) = registry.get(site) {
        return Ok(code.clone());
    }
    match context {
        Some(context) => registry.eval(source, base, context, || on_parse(source, base)),
        None => registry.script(source, base, || on_parse(source, base)),
    }
}

/** Parse dynamic or handler source. */
fn on_parse(source: &str, base: usize) -> EngineResult<syntax::Program> {
    if source.len() > SCRIPT_BYTES {
        return Err(EngineError::limit("script bytes", SCRIPT_BYTES));
    }
    parse_at(source, base, None)
}

/** Parse one classic script without running it, the way a page load would read it. */
pub fn check(source: &str) -> EngineResult<()> {
    on_v9_stack(|| {
        let program = parse_at(source, 0, None)?;
        Registry::default().add(&program).map(|_| ())
    })
}

/** Byte offsets of each source in the page. */
fn offsets(sources: &[String]) -> Vec<usize> {
    let mut offset = 0;
    sources
        .iter()
        .map(|source| {
            let start = offset;
            offset += source.len() + 1;
            start
        })
        .collect()
}

/** Validate host language tags: bounded count and length, ASCII tag characters only. */
fn validate_languages(languages: &[String]) -> EngineResult<Vec<String>> {
    if languages.len() > NAVIGATOR_LANGUAGES {
        return Err(EngineError::limit(
            "navigator language tags",
            NAVIGATOR_LANGUAGES,
        ));
    }
    if languages.iter().any(|tag| {
        tag.is_empty()
            || tag.len() > LANGUAGE_TAG_BYTES
            || !tag
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    }) {
        return Err(EngineError::invalid("Invalid navigator language tag."));
    }
    Ok(languages.to_vec())
}

/** What a session compiles from the sources. */
enum Compiled {
    /** Every script must compile; the first failure fails the request. */
    All(Vec<Arc<Code>>),
    /** Each script compiles alone; failures are reported per index. */
    Each(Vec<EngineResult<Arc<Code>>>),
}

/** Open a VM for a request: restore state, compile sources, set the clock and deadline. The
deadline is armed once compilation is done: scanning and parsing are already bounded by
SCRIPT_BYTES, TOKENS and STATEMENTS, and a max-token script takes several times SCRIPT_DEADLINE
to compile in an unoptimized build, so charging it would refuse admitted sources. */
#[allow(clippy::too_many_arguments)]
fn session<'h>(
    limits: Limits,
    sources: &[String],
    isolated: bool,
    document: Option<ScriptDocument>,
    storage: StorageState,
    host: &DocumentHost<'h>,
    environment: &PageEnvironment,
    state: Option<VmState>,
    modules: &ModuleInput,
) -> EngineResult<(Vm<'h>, Compiled)> {
    if let Some(document) = &document {
        document.validate()?;
    }
    storage.validate()?;
    let languages = validate_languages(host.languages)?;
    let state = state.unwrap_or_default();
    state.validate(&limits)?;
    if let Some(document) = &document
        && state.page.initialized
        && state.page.identity != document.identity
    {
        return Err(EngineError::invalid(
            "V9 state belongs to another document.",
        ));
    }
    if sources.len() > SCRIPT_COUNT && !isolated {
        return Err(EngineError::limit("classic scripts", SCRIPT_COUNT));
    }
    let page = platform::Page::new(
        environment.clone(),
        languages,
        host.fragments,
        document,
        state.page,
        storage,
    );
    let mut vm = Vm::new(state.heap, page, limits);
    vm.globals = state.globals;
    vm.symbols = state.symbols;
    vm.jobs = state.jobs.into();
    initialize_page(&mut vm);
    modules::Modules::open(&mut vm, modules, sources, &offsets(sources), state.modules)?;
    let compiled = compile_sources(&mut vm, sources, isolated)?;
    for (index, source) in state.dynamic.iter().enumerate() {
        let base = builtins::dynamic_base(&state.dynamic[..index]);
        compile_dynamic(
            &mut vm.registry,
            &source.source,
            base,
            source.context.as_deref(),
        )?;
    }
    vm.dynamic = state.dynamic;
    vm.validate_graph()?;
    let loaded = vm.modules.loaded_codes();
    let mut scripts: Vec<&Code> = match &compiled {
        Compiled::All(codes) => codes.iter().map(|code| &**code).collect(),
        Compiled::Each(codes) => codes.iter().flatten().map(|code| &**code).collect(),
    };
    scripts.extend(loaded.iter().map(|code| &**code));
    vm.validate_globals(&scripts)?;
    vm.deadline = Some(std::time::Instant::now() + SCRIPT_DEADLINE);
    Ok((vm, compiled))
}

/** First-request page facts: identity, clock, URL, cookie, random seed. */
fn initialize_page(vm: &mut Vm) {
    let identity = vm
        .page
        .document
        .as_ref()
        .map(|document| document.identity.clone())
        .unwrap_or_else(|| "legacy".into());
    let state = &mut vm.page.state;
    if state.initialized {
        return;
    }
    let environment = &vm.page.environment;
    state.identity = identity;
    state.epoch = environment.time_origin_ms;
    state.random = environment.random_seed;
    state.href = environment.url.clone();
    state.cookie = environment.cookie.clone();
    state.history_length = 1;
    state.initialized = true;
}

/** Compile page sources at their page offsets. */
fn compile_sources(vm: &mut Vm, sources: &[String], isolated: bool) -> EngineResult<Compiled> {
    let starts = offsets(sources);
    let mut total = 0usize;
    let mut each = Vec::with_capacity(sources.len());
    for (index, (source, start)) in sources.iter().zip(starts).enumerate() {
        total = total.saturating_add(source.len());
        let result = if index >= SCRIPT_COUNT {
            Err(EngineError::limit("classic scripts", SCRIPT_COUNT))
        } else if source.len() > SCRIPT_BYTES {
            Err(EngineError::limit("script bytes", SCRIPT_BYTES))
        } else if total > SCRIPT_TOTAL_BYTES {
            Err(EngineError::limit("page script bytes", SCRIPT_TOTAL_BYTES))
        } else if vm.modules.entry(index).is_some() {
            // Module scripts and import maps compiled with the module map; never as classic.
            Err(EngineError::invalid("V9 module entry."))
        } else {
            vm.registry
                .script(source, start, || parse_at(source, start, Some(index)))
        };
        if !isolated {
            each.push(Ok(result?));
        } else {
            each.push(result);
        }
    }
    Ok(if isolated {
        Compiled::Each(each)
    } else {
        Compiled::All(
            each.into_iter()
                .map(|code| code.expect("checked"))
                .collect(),
        )
    })
}

/** Run one script's top-level code, then its microtasks. */
fn run_one(vm: &mut Vm, code: Arc<Code>, index: usize) -> EngineResult<()> {
    vm.remaining = vm.limits.operations;
    vm.page.throw_location = None;
    let result = vm.run_script(code).and_then(|_| vm.drain_jobs());
    result.map(|_| ()).map_err(|thrown| {
        let location = vm.page.throw_location.unwrap_or_default();
        let mut error = vm.uncaught(thrown, location);
        if let Some(location) = error.location.as_deref_mut() {
            location.script_index = Some(index + 1);
        }
        error
    })
}

/** Everything a failed script could change, restored on rollback. */
struct Snapshot {
    heap: Heap,
    globals: Globals,
    symbols: Symbols,
    jobs: std::collections::VecDeque<Job>,
    state: platform::PageState,
    storage: StorageState,
    document: Option<ScriptDocument>,
    changes: Vec<DomChange>,
    script_changes: Vec<ScriptChange>,
    effects: HostEffects,
    dynamic: Vec<vm::DynamicSource>,
    modules: modules::ModuleState,
    next_id: usize,
    change_bytes: usize,
    fragment_bytes: usize,
}

/** Take a snapshot between scripts. */
fn snapshot(vm: &Vm) -> Snapshot {
    Snapshot {
        heap: vm.heap.clone(),
        globals: vm.globals.clone(),
        symbols: vm.symbols.clone(),
        jobs: vm.jobs.clone(),
        state: vm.page.state.clone(),
        storage: vm.page.storage.clone(),
        document: vm.page.document.clone(),
        changes: vm.page.changes.clone(),
        script_changes: vm.page.script_changes.clone(),
        effects: vm.page.effects.clone(),
        dynamic: vm.dynamic.clone(),
        modules: vm.modules.state.clone(),
        next_id: vm.page.next_id,
        change_bytes: vm.page.change_bytes,
        fragment_bytes: vm.page.fragment_bytes,
    }
}

/** Restore a snapshot after a failed script. */
fn restore(vm: &mut Vm, saved: Snapshot) {
    vm.heap = saved.heap;
    vm.globals = saved.globals;
    vm.symbols = saved.symbols;
    vm.jobs = saved.jobs;
    vm.page.state = saved.state;
    vm.page.storage = saved.storage;
    vm.page.document = saved.document;
    vm.page.changes = saved.changes;
    vm.page.script_changes = saved.script_changes;
    vm.page.effects.requests = saved.effects.requests;
    vm.page.effects.navigation = saved.effects.navigation;
    vm.page.effects.history = saved.effects.history;
    vm.page.effects.cookies = saved.effects.cookies;
    vm.page.effects.title = saved.effects.title;
    vm.dynamic = saved.dynamic;
    vm.modules.state = saved.modules;
    vm.page.next_id = saved.next_id;
    vm.page.change_bytes = saved.change_bytes;
    vm.page.fragment_bytes = saved.fragment_bytes;
    vm.page.invalidate();
    vm.frames.clear();
    vm.pins.clear();
    vm.native_depth = 0;
    vm.pinning = false;
}

/** Collect, validate and package the request's outcome. */
fn finish(mut vm: Vm) -> EngineResult<Execution> {
    vm.frames.clear();
    vm.pins.clear();
    vm.collect_now().map_err(fatal)?;
    let effects_timer = vm.next_timer();
    let script_requests: Vec<ScriptLoadRequest> = vm
        .page
        .state
        .script_loads
        .iter()
        .filter(|load| !load.done && !load.url.is_empty() && !vm.modules.delivered(&load.url))
        .map(|load| ScriptLoadRequest {
            url: load.url.clone(),
            nonce: load.nonce.clone(),
            integrity: load.integrity.clone(),
            cross_origin: load.cross_origin.clone(),
        })
        .collect();
    let platform::Page {
        state: page,
        storage,
        document,
        changes,
        script_changes,
        mut effects,
        ..
    } = vm.page;
    effects.next_timer_ms = effects_timer;
    let module_requests = std::mem::take(&mut vm.modules.requests);
    let state = VmState {
        globals: vm.globals,
        heap: vm.heap,
        symbols: vm.symbols,
        jobs: vm.jobs.into(),
        page,
        dynamic: vm.dynamic,
        modules: std::mem::take(&mut vm.modules.state),
    };
    state.validate(&vm.limits)?;
    storage.validate()?;
    #[cfg(feature = "measure")]
    measure::record(&state);
    Ok(Execution {
        structure: document.as_ref().map_or(0, ScriptDocument::digest),
        changes,
        script_changes,
        state,
        storage,
        failed: Vec::new(),
        effects,
        module_requests,
        script_requests,
    })
}

/** An engine error out of a script-level result. */
fn fatal(thrown: Thrown) -> EngineError {
    match thrown {
        Thrown::Fatal(error) => error,
        Thrown::Value(_) => EngineError::invalid("V9 stopped on an uncaught internal exception."),
    }
}

/** Load ordered scripts one at a time; each failure rolls back only that script. With `resume`
the load continues where a deferred module script paused it, once the app delivered modules. */
fn load_isolated(
    input: PageInput<'_>,
    state: Option<VmState>,
    limits: Limits,
    resume: bool,
) -> EngineResult<Execution> {
    let initial = (input.document.clone(), input.storage.clone());
    let (mut vm, compiled) = session(
        limits,
        input.sources,
        true,
        Some(input.document),
        input.storage,
        input.host,
        input.environment,
        state,
        input.modules,
    )?;
    let Compiled::Each(codes) = compiled else {
        return Err(EngineError::invalid(
            "V9 compiled scripts in the wrong mode.",
        ));
    };
    let mut failed = Vec::new();
    let mut ran = Vec::new();
    let from = if resume {
        vm.modules.state.cursor.take()
    } else {
        queue_async_modules(&mut vm, codes.len());
        Some(0)
    };
    if let Some(from) = from {
        run_sources(&mut vm, &codes, from, !resume, &mut failed, &mut ran)?;
    }
    if resume {
        platform::scripts::run_delivered(&mut vm)?;
    }
    run_async_modules(&mut vm, &mut failed, &mut ran);
    if resume {
        run_dynamic_imports(&mut vm);
    }
    let execution = match finish(vm) {
        Ok(execution) => execution,
        Err(error) => {
            failed.extend(ran.into_iter().map(|index| ScriptFailure {
                index,
                error: error.clone(),
            }));
            let empty = PageInput {
                sources: &[],
                document: initial.0,
                storage: initial.1,
                host: input.host,
                environment: input.environment,
                modules: NO_MODULES,
            };
            let (vm, _) = session(
                limits,
                empty.sources,
                true,
                Some(empty.document),
                empty.storage,
                empty.host,
                empty.environment,
                None,
                empty.modules,
            )?;
            finish(vm)?
        }
    };
    failed.sort_by_key(|failure| failure.index);
    Ok(Execution {
        failed,
        ..execution
    })
}

/** Run page sources in order from `from`, each classic script rolled back alone on failure. A
deferred module script runs its whole graph the same way; while that graph is still loading the
load pauses there (later scripts wait, as they would in a browser) until `load_modules`. A
`fresh` load also queues the parsed sheets' `load` events once their elements exist. */
fn run_sources(
    vm: &mut Vm,
    codes: &[EngineResult<Arc<Code>>],
    from: usize,
    fresh: bool,
    failed: &mut Vec<ScriptFailure>,
    ran: &mut Vec<usize>,
) -> EngineResult<()> {
    let elements = vm.page.script_elements(codes.len())?;
    if fresh {
        platform::scripts::parsed_sheets(vm).map_err(fatal)?;
    }
    for (index, code) in codes.iter().enumerate().skip(from) {
        if let Some(kind) = vm.modules.entry(index) {
            if kind == EntryKind::Module {
                match modules::run_entry(vm, index) {
                    modules::Outcome::Ran => ran.push(index),
                    modules::Outcome::Failed(error) => failed.push(failure(index, error)),
                    modules::Outcome::Waiting => {
                        vm.modules.state.cursor = Some(index);
                        // Later module scripts' graphs load meanwhile, as browsers fetch them.
                        modules::discover(vm, index + 1..codes.len());
                        return Ok(());
                    }
                }
            }
            continue;
        }
        vm.page.current_script = elements.get(index).copied();
        let code = match code {
            Ok(code) => code.clone(),
            Err(error) => {
                failed.push(failure(index, error.clone()));
                continue;
            }
        };
        if vm.expired() {
            failed.push(ScriptFailure {
                index,
                error: platform::timeout(),
            });
            continue;
        }
        let saved = snapshot(vm);
        match run_one(vm, code, index) {
            Ok(()) => ran.push(index),
            Err(error) => {
                restore(vm, saved);
                failed.push(ScriptFailure { index, error });
            }
        }
        vm.page.current_script = None;
    }
    Ok(())
}

/** A failure at a source index, located there. */
fn failure(index: usize, mut error: EngineError) -> ScriptFailure {
    if let Some(location) = error.location.as_deref_mut() {
        location.script_index = Some(index + 1);
    }
    ScriptFailure { index, error }
}

/** Async module scripts do not wait for the parse or each other: they wait for their graphs. */
fn queue_async_modules(vm: &mut Vm, count: usize) {
    for index in 0..count {
        if vm.modules.entry(index) == Some(EntryKind::AsyncModule) {
            let key = vm.modules.entry_key(index);
            vm.modules.state.waiting.push(modules::Waiting {
                key,
                entry: Some(index),
                promise: None,
            });
        }
    }
}

/** Run the async module scripts whose graphs are complete, each isolated like any script. */
fn run_async_modules(vm: &mut Vm, failed: &mut Vec<ScriptFailure>, ran: &mut Vec<usize>) {
    let waiting = std::mem::take(&mut vm.modules.state.waiting);
    let mut kept = Vec::new();
    for item in waiting {
        let Some(index) = item.entry else {
            kept.push(item);
            continue;
        };
        if vm.expired() {
            failed.push(ScriptFailure {
                index,
                error: platform::timeout(),
            });
            continue;
        }
        match modules::run_entry(vm, index) {
            modules::Outcome::Ran => ran.push(index),
            modules::Outcome::Failed(error) => failed.push(failure(index, error)),
            modules::Outcome::Waiting => kept.push(item),
        }
    }
    kept.append(&mut vm.modules.state.waiting);
    vm.modules.state.waiting = kept;
}

/** Evaluate the dynamic imports the delivered modules completed. An engine failure (a limit,
the deadline) rolls the attempt back and rejects those imports, so the page keeps running. */
fn run_dynamic_imports(vm: &mut Vm) {
    let saved = snapshot(vm);
    vm.remaining = vm.limits.operations;
    let result = modules::advance_dynamic(vm).and_then(|()| vm.drain_jobs());
    if let Err(thrown) = result {
        let error = vm.uncaught(thrown, Default::default());
        restore(vm, saved);
        modules::refuse_dynamic(vm, &error.message);
    }
}

/// Load a page's scripts like a browser: each failing script rolls back alone. Microtasks drain
/// after every script; timers wait for `run_timers`; lifecycle events wait for `page_event`.
pub fn load_page(input: PageInput<'_>) -> EngineResult<Execution> {
    on_v9_stack(|| load_isolated(input, None, Limits::default(), false))
}

/// Deliver fetched modules (`PageInput::modules` now holds them) against committed state: a page
/// load paused at a module script continues, and async module scripts and dynamic imports whose
/// graphs are complete evaluate. `Execution::module_requests` may ask for more.
pub fn load_modules(input: PageInput<'_>, state: VmState) -> EngineResult<Execution> {
    on_v9_stack(|| load_isolated(input, Some(state), Limits::default(), true))
}

/// Fire DOMContentLoaded or load against committed state.
pub fn page_event(
    input: PageInput<'_>,
    state: VmState,
    event: PageEvent,
) -> EngineResult<Execution> {
    on_v9_stack(|| page_event_with(input, state, event, Limits::default()))
}

/** page_event under explicit bounds. */
fn page_event_with(
    input: PageInput<'_>,
    state: VmState,
    event: PageEvent,
    limits: Limits,
) -> EngineResult<Execution> {
    {
        let (mut vm, _) = open(&input, state, limits)?;
        match event {
            PageEvent::DomContentLoaded => {
                vm.page.state.ready = platform::ReadyState::Interactive;
                fire_lifecycle(
                    &mut vm,
                    platform::Target::Document,
                    "readystatechange",
                    false,
                )?;
                fire_lifecycle(
                    &mut vm,
                    platform::Target::Document,
                    "DOMContentLoaded",
                    true,
                )?;
            }
            PageEvent::Load => {
                vm.page.state.ready = platform::ReadyState::Complete;
                fire_lifecycle(
                    &mut vm,
                    platform::Target::Document,
                    "readystatechange",
                    false,
                )?;
                fire_lifecycle(&mut vm, platform::Target::Window, "load", false)?;
                fire_lifecycle(&mut vm, platform::Target::Window, "pageshow", false)?;
            }
            PageEvent::Scroll => {
                fire_lifecycle(&mut vm, platform::Target::Document, "scroll", true)?;
            }
        }
        finish(vm)
    }
}

/** Open a session over committed state for an event, timer or response. */
fn open<'h>(
    input: &PageInput<'h>,
    state: VmState,
    limits: Limits,
) -> EngineResult<(Vm<'h>, Compiled)> {
    session(
        limits,
        input.sources,
        true,
        Some(input.document.clone()),
        input.storage.clone(),
        input.host,
        input.environment,
        Some(state),
        input.modules,
    )
}

/** Fire one lifecycle event and drain microtasks; script errors go to the console. */
fn fire_lifecycle(
    vm: &mut Vm,
    target: platform::Target,
    kind: &str,
    bubbles: bool,
) -> EngineResult<()> {
    vm.remaining = vm.limits.operations;
    let result = platform::fire(vm, target, kind, bubbles).and_then(|_| vm.drain_jobs());
    match result {
        Ok(()) => Ok(()),
        Err(Thrown::Value(error)) => {
            vm.report_error(&error);
            Ok(())
        }
        Err(Thrown::Fatal(error)) => Err(error),
    }
}

/// A person typed into a field the app already updated: V9 records the value (None for a
/// secret field, whose value never enters the engine) and fires trusted `input` and `change`
/// events at the node, so a form's script learns what was typed.
pub fn input_page(
    input: PageInput<'_>,
    state: VmState,
    target: usize,
    value: Option<String>,
) -> EngineResult<Execution> {
    on_v9_stack(|| {
        let (mut vm, _) = open(&input, state, Limits::default())?;
        if !vm.page.exists(target) {
            return Err(EngineError::invalid("Invalid projected input target."));
        }
        if let Some(value) = value {
            vm.page.state.values.insert(target, value);
        }
        vm.remaining = vm.limits.operations;
        if let Err(Thrown::Fatal(error)) = platform::focus(&mut vm, target) {
            return Err(error);
        }
        for kind in ["input", "change"] {
            fire_lifecycle(&mut vm, platform::Target::Node(target), kind, true)?;
        }
        finish(vm)
    })
}

/// Dispatch a trusted click at a projected node (bubbling to the document and window), then
/// drain microtasks. An unprevented click on a link records a navigation in the effects.
pub fn click_page(input: PageInput<'_>, state: VmState, target: usize) -> EngineResult<Execution> {
    on_v9_stack(|| click_page_with(input, state, target, Limits::default()))
}

/** click_page under explicit bounds. */
fn click_page_with(
    input: PageInput<'_>,
    state: VmState,
    target: usize,
    limits: Limits,
) -> EngineResult<Execution> {
    let (mut vm, _) = open(&input, state, limits)?;
    if !vm.page.exists(target) {
        return Err(EngineError::invalid("Invalid projected click target."));
    }
    vm.remaining = vm.limits.operations;
    // A press focuses what it lands on before the click, as a pointer's mousedown does.
    if let Err(Thrown::Fatal(error)) = platform::focus(&mut vm, target) {
        return Err(error);
    }
    let proceed = match platform::fire(&mut vm, platform::Target::Node(target), "click", true)
        .and_then(|proceed| vm.drain_jobs().map(|()| proceed))
    {
        Ok(proceed) => proceed,
        Err(thrown) => {
            let location = vm.page.throw_location.unwrap_or_default();
            let mut error = vm.uncaught(thrown, location);
            error.input_kind = Some("handler".into());
            return Err(error);
        }
    };
    if proceed {
        platform::follow_link(&mut vm, target);
    }
    finish(vm)
}

/// Advance the virtual clock to `now_ms`, running due timers and animation frames (earliest
/// first, at most `budget` callbacks, each followed by its microtasks). `effects.next_timer_ms`
/// says when to call again.
pub fn run_timers(
    input: PageInput<'_>,
    state: VmState,
    now_ms: f64,
    budget: usize,
) -> EngineResult<Execution> {
    on_v9_stack(|| run_timers_with(input, state, now_ms, budget, Limits::default()))
}

/** run_timers under explicit bounds. */
fn run_timers_with(
    input: PageInput<'_>,
    state: VmState,
    now_ms: f64,
    budget: usize,
    limits: Limits,
) -> EngineResult<Execution> {
    let (mut vm, _) = open(&input, state, limits)?;
    if !now_ms.is_finite() {
        return Err(EngineError::invalid("Invalid V9 timer clock."));
    }
    vm.remaining = vm.limits.operations;
    vm.run_due_timers(now_ms, budget).map_err(fatal)?;
    finish(vm)
}

/// Hand back responses to `effects.requests`; fetch promises settle and XMLHttpRequests fire.
pub fn resume_requests(
    input: PageInput<'_>,
    state: VmState,
    responses: Vec<HostResponse>,
) -> EngineResult<Execution> {
    on_v9_stack(|| {
        let (mut vm, _) = open(&input, state, Limits::default())?;
        vm.remaining = vm.limits.operations;
        vm.deliver(responses).map_err(fatal)?;
        finish(vm)
    })
}

/** The default environment for entry points that predate PageEnvironment. */
fn default_environment() -> PageEnvironment {
    PageEnvironment::default()
}

/** Load projected scripts as one all-or-nothing transaction: any script's failure fails the call. */
pub fn load_document_scripts(
    sources: &[String],
    document: ScriptDocument,
    storage: StorageState,
) -> EngineResult<Execution> {
    let environment = default_environment();
    let host = DocumentHost::default();
    on_v9_stack(|| {
        let (mut vm, compiled) = session(
            Limits::default(),
            sources,
            false,
            Some(document),
            storage,
            &host,
            &environment,
            None,
            NO_MODULES,
        )?;
        let Compiled::All(codes) = compiled else {
            return Err(EngineError::invalid(
                "V9 compiled scripts in the wrong mode.",
            ));
        };
        for (index, code) in codes.into_iter().enumerate() {
            run_one(&mut vm, code, index)?;
        }
        finish(vm)
    })
}

/** The page-load entry point the V9 service uses: isolated, with the default environment. */
pub fn load_document_scripts_with(
    sources: &[String],
    document: ScriptDocument,
    storage: StorageState,
    host: &DocumentHost,
) -> EngineResult<Execution> {
    load_document_scripts_isolated(sources, document, storage, host)
}

/** Load projected scripts one at a time, rolling back each failing script alone. */
pub fn load_document_scripts_isolated(
    sources: &[String],
    document: ScriptDocument,
    storage: StorageState,
    host: &DocumentHost,
) -> EngineResult<Execution> {
    let environment = default_environment();
    load_page(PageInput {
        sources,
        document,
        storage,
        host,
        environment: &environment,
        modules: NO_MODULES,
    })
}

/** Dispatch one projected click with the default host. */
pub fn click_document_scripts(
    sources: &[String],
    target: usize,
    state: VmState,
    document: ScriptDocument,
    storage: StorageState,
) -> EngineResult<Execution> {
    click_document_scripts_with(
        sources,
        target,
        state,
        document,
        storage,
        &DocumentHost::default(),
    )
}

/** Dispatch one projected click with the same host facts and fragment seam as page load. */
pub fn click_document_scripts_with(
    sources: &[String],
    target: usize,
    state: VmState,
    document: ScriptDocument,
    storage: StorageState,
    host: &DocumentHost,
) -> EngineResult<Execution> {
    let environment = default_environment();
    click_page(
        PageInput {
            sources,
            document,
            storage,
            host,
            environment: &environment,
            modules: NO_MODULES,
        },
        state,
        target,
    )
}

/** A synthetic projection for legacy callers that address fields by id. */
fn legacy_document(inputs: &[InputValue]) -> EngineResult<ScriptDocument> {
    let mut nodes = Vec::new();
    for (index, input) in inputs.iter().enumerate() {
        if input.value.len() > TEXT_BYTES {
            return Err(EngineError::limit("V9 DOM input UTF-8 bytes", TEXT_BYTES));
        }
        if nodes
            .iter()
            .any(|node: &ScriptNode| node.html_id == input.html_id)
        {
            continue;
        }
        nodes.push(ScriptNode {
            local_id: index,
            html_id: input.html_id.clone(),
            parent: None,
            value: Some(input.value.clone()),
            text_writable: true,
            click_listener: false,
            inline_handler: None,
            tag: "input".into(),
            attributes: [("id".to_string(), input.html_id.clone())]
                .into_iter()
                .collect(),
        });
    }
    for (index, node) in nodes.iter_mut().enumerate() {
        node.local_id = index;
    }
    Ok(ScriptDocument {
        identity: "legacy".into(),
        nodes,
    })
}

/** Run legacy scripts (and optionally a handler) all-or-nothing. */
fn legacy(
    sources: &[String],
    handler: Option<&str>,
    state: Option<VmState>,
    inputs: &[InputValue],
    storage: StorageState,
) -> EngineResult<Execution> {
    let environment = default_environment();
    let host = DocumentHost::default();
    let document = legacy_document(inputs)?;
    on_v9_stack(|| {
        let (mut vm, compiled) = session(
            Limits::default(),
            sources,
            false,
            Some(document),
            storage,
            &host,
            &environment,
            state,
            NO_MODULES,
        )?;
        vm.page.legacy = true;
        let Compiled::All(codes) = compiled else {
            return Err(EngineError::invalid(
                "V9 compiled scripts in the wrong mode.",
            ));
        };
        match handler {
            None => {
                for (index, code) in codes.into_iter().enumerate() {
                    run_one(&mut vm, code, index)?;
                }
            }
            Some(handler) => {
                let parse = || {
                    parse_at(handler, HANDLER_OFFSET, None).map_err(|mut error| {
                        error.input_kind = Some("handler".into());
                        if let Some(location) = error.location.as_deref_mut()
                            && location.byte_offset >= HANDLER_OFFSET
                        {
                            location.byte_offset -= HANDLER_OFFSET;
                        }
                        error
                    })
                };
                let code = vm.registry.script(handler, HANDLER_OFFSET, parse)?;
                run_one(&mut vm, code, 0).map_err(|mut error| {
                    error.input_kind = Some("handler".into());
                    if let Some(location) = error.location.as_deref_mut() {
                        location.script_index = None;
                        if location.byte_offset >= HANDLER_OFFSET {
                            location.byte_offset -= HANDLER_OFFSET;
                        }
                    }
                    error
                })?;
            }
        }
        finish(vm)
    })
}

/// Compatibility entry point for source-only callers: the text changes a script proposes.
pub fn execute(source: &str) -> EngineResult<Vec<TextChange>> {
    Ok(load(source, &[])?
        .changes
        .into_iter()
        .filter_map(|change| match change {
            DomChange::Text { html_id, text } => Some(TextChange { html_id, text }),
            _ => None,
        })
        .collect())
}

/// Load source once against field values and return its state.
pub fn load(source: &str, inputs: &[InputValue]) -> EngineResult<Execution> {
    legacy(
        &[source.to_string()],
        None,
        None,
        inputs,
        StorageState::default(),
    )
}

/// Load ordered classic scripts with shared state.
pub fn load_scripts(sources: &[String], inputs: &[InputValue]) -> EngineResult<Execution> {
    load_scripts_with_storage(sources, inputs, StorageState::default())
}

/** Load scripts against coordinator-owned storage candidates. */
pub fn load_scripts_with_storage(
    sources: &[String],
    inputs: &[InputValue],
    storage: StorageState,
) -> EngineResult<Execution> {
    legacy(sources, None, None, inputs, storage)
}

/// Run a handler (such as `run()`) against state produced by the same source.
pub fn event(
    source: &str,
    handler: &str,
    state: VmState,
    inputs: &[InputValue],
) -> EngineResult<Execution> {
    event_scripts(&[source.to_string()], handler, state, inputs)
}

/// Run a handler against state produced by the same ordered scripts.
pub fn event_scripts(
    sources: &[String],
    handler: &str,
    state: VmState,
    inputs: &[InputValue],
) -> EngineResult<Execution> {
    event_scripts_with_storage(sources, handler, state, inputs, StorageState::default())
}

/** Run a handler against storage copies supplied by the coordinator. */
pub fn event_scripts_with_storage(
    sources: &[String],
    handler: &str,
    state: VmState,
    inputs: &[InputValue],
    storage: StorageState,
) -> EngineResult<Execution> {
    legacy(sources, Some(handler), Some(state), inputs, storage)
}

impl Vm<'_> {
    /** Run a compiled top-level script with the global object as `this`. */
    pub(in crate::v9) fn run_script(&mut self, code: Arc<Code>) -> Result<JsValue, Thrown> {
        let regs = vec![JsValue::Undefined; code.registers as usize];
        self.frames.push(vm::Frame {
            code,
            pc: 0,
            stack: Vec::new(),
            regs,
            env: None,
            this: JsValue::Object(vm::GLOBAL),
            callee: JsValue::Undefined,
            args: Vec::new(),
            new_target: JsValue::Undefined,
            handlers: Vec::new(),
            envs: 0,
            boundary: true,
            construct: false,
            resume: vm::Resume::Next,
        });
        let pinning = std::mem::replace(&mut self.pinning, false);
        self.native_depth += 1;
        let result = self.run();
        self.native_depth -= 1;
        self.pinning = pinning;
        match result? {
            vm::Exit::Return(value) => Ok(value),
            _ => Err(EngineError::invalid("V9 script suspended at the top level.").into()),
        }
    }
}

/** Measurement-only entry points (feature `measure`); the service build never contains them. */
#[cfg(feature = "measure")]
pub mod measure {
    use super::*;
    use std::sync::Mutex;

    /// Objects, functions, properties, string units, environments, bindings and state bytes.
    pub type Counts = (usize, usize, usize, usize, usize, usize, usize);

    /** The live counts of the last measured load's committed state. */
    static LAST: Mutex<Option<Counts>> = Mutex::new(None);

    /** Record the committed state's live counts and bytes. */
    pub(super) fn record(state: &VmState) {
        let counts = state.heap.counts;
        let bytes = serde_json::to_vec(state).map_or(0, |bytes| bytes.len());
        if let Ok(mut last) = LAST.lock() {
            *last = Some((
                counts.objects,
                counts.functions,
                counts.properties,
                counts.units,
                counts.environments,
                counts.bindings,
                bytes,
            ));
        }
    }

    /// Live objects, functions, properties, string units, environments, bindings and state
    /// bytes after the last measured load, as the bounds would count them.
    pub fn last_counts() -> Option<Counts> {
        LAST.lock().ok().and_then(|last| *last)
    }

    /** Every heap bound multiplied by `scale`. */
    fn scaled(scale: usize) -> Limits {
        let base = Limits::default();
        Limits {
            objects: base.objects * scale,
            functions: base.functions * scale,
            properties: base.properties * scale,
            units: base.units * scale,
            environments: base.environments * scale,
            bindings: base.bindings * scale,
            array_length: base.array_length * scale,
            state_bytes: base.state_bytes * scale,
            ..base
        }
    }

    /// Load with every heap bound multiplied by `scale`, to find what a script needs.
    pub fn load_page_scaled(input: PageInput<'_>, scale: usize) -> EngineResult<Execution> {
        on_v9_stack(|| load_isolated(input, None, scaled(scale), false))
    }

    /// `page_event` with scaled bounds.
    pub fn page_event_scaled(
        input: PageInput<'_>,
        state: VmState,
        event: PageEvent,
        scale: usize,
    ) -> EngineResult<Execution> {
        on_v9_stack(|| page_event_with(input, state, event, scaled(scale)))
    }

    /// `click_page` with scaled bounds.
    pub fn click_page_scaled(
        input: PageInput<'_>,
        state: VmState,
        target: usize,
        scale: usize,
    ) -> EngineResult<Execution> {
        on_v9_stack(|| click_page_with(input, state, target, scaled(scale)))
    }

    /// `run_timers` with scaled bounds.
    pub fn run_timers_scaled(
        input: PageInput<'_>,
        state: VmState,
        now_ms: f64,
        budget: usize,
        scale: usize,
    ) -> EngineResult<Execution> {
        on_v9_stack(|| run_timers_with(input, state, now_ms, budget, scaled(scale)))
    }
}

/// Replay an execution's changes onto a projection, as the app's document engine does, for
/// callers that run V9 without Eye (tests, tools). Text that V9's own fragment reader sees is
/// added to `node_text` so later requests can read it.
pub fn replay(
    document: &mut ScriptDocument,
    execution: &Execution,
    fragments: &dyn FragmentParser,
    node_text: &mut BTreeMap<usize, String>,
) -> EngineResult<()> {
    let changes = execution
        .changes
        .iter()
        .cloned()
        .map(|change| ScriptChange::Dom { change })
        .chain(execution.script_changes.iter().cloned());
    for change in changes {
        match change {
            // Definedness is a document-engine fact for `:defined`; the projection has no field.
            ScriptChange::Defined { .. } => {}
            ScriptChange::Dom { change } => match change {
                DomChange::Text { html_id, text } => {
                    let node = document
                        .get_by_id(&html_id)
                        .ok_or_else(|| EngineError::invalid("Unknown text target."))?;
                    document.set_text(node, &text)?;
                    node_text.insert(node, text);
                }
                DomChange::InsertHtml {
                    target,
                    position,
                    html,
                    first_id,
                } => {
                    let context = target
                        .and_then(|target| {
                            document.nodes.iter().find(|node| node.local_id == target)
                        })
                        .map_or_else(|| "body".to_string(), |node| node.tag.clone());
                    let nodes = fragments.parse(&context, &html, first_id)?;
                    for (id, text) in platform::html::fragment_texts(&html, first_id)? {
                        node_text.insert(id, text);
                    }
                    document.insert(target, position, nodes)?;
                }
                DomChange::Move {
                    node,
                    target,
                    position,
                } => {
                    document.relocate(node, target, position)?;
                }
                DomChange::SetAttribute { node, name, value } => {
                    document.set_attribute(node, &name, &value)?;
                }
            },
            ScriptChange::Remove { node } => {
                document.remove(node)?;
            }
            ScriptChange::Attribute { node, name, value } => {
                document.set_any_attribute(node, &name, value.as_deref())?;
            }
            ScriptChange::NodeText { node, text } => {
                document.set_node_text(node, &text)?;
                node_text.insert(node, text);
            }
            ScriptChange::Value { .. } => {}
        }
    }
    if document.digest() != execution.structure {
        return Err(EngineError::invalid(
            "Replayed projection diverged from V9's structure digest.",
        ));
    }
    Ok(())
}

/** Keep the unused host data import referenced for documentation of the wrapper model. */
#[allow(dead_code)]
fn _host(_: HostData, _: Class, _: SourceLocation) {}
