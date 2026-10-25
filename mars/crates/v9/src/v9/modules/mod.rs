/**
 * JavaScript modules: the page's module map. V9 has no network, so a module graph loads in rounds:
 * V9 names the module URLs it needs (`Execution::module_requests`), the app fetches them in CORS
 * mode under its own policy, and the next `load_modules` request carries them. Every request
 * recompiles the delivered sources at fixed offsets (code sites stay put), and the committed
 * records name each module's suspended body, environment and namespace. A graph that fails to
 * load, link or evaluate fails alone; the page and its other graphs keep running.
 */
mod import_map;
mod link;

pub(in crate::v9) use link::{Outcome, advance_dynamic, discover, refuse_dynamic, run_entry};

use super::compile::{Code, ModuleInfo};
use super::syntax;
use super::value::{JsString, JsValue, ObjectId};
use super::vm::heap::{ENUMERABLE, EnvId, Key, Prop, Slot as HeapSlot, WRITABLE};
use super::vm::realm::{Builder, symbol};
use super::vm::{Call, Res, Vm};
use crate::constants::{MODULE_BYTES, MODULES, SCRIPT_BYTES, SCRIPT_COUNT, SCRIPT_TOTAL_BYTES};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::syntax::Name;
use import_map::ImportMap;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, OnceLock};
use url::Url;

/** Fetched modules compile from this page offset on, past every classic source. */
pub(in crate::v9) const MODULE_OFFSET: usize = 128 * 1024 * 1024;
/* Classic sources end before the module region, and modules end before dynamic code. */
const _: () = assert!(
    SCRIPT_TOTAL_BYTES + SCRIPT_COUNT <= MODULE_OFFSET
        && MODULE_OFFSET + MODULE_BYTES + MODULES <= super::DYNAMIC_OFFSET
);

/// The module scripts, import maps and fetched modules of a page, supplied with every request.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleInput {
    /// Page sources that are module scripts or import maps rather than classic scripts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<ModuleEntry>,
    /// Fetched modules in delivery order. The app only appends, so code offsets never move.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<ModuleSource>,
}

impl ModuleInput {
    /// No module scripts, import maps or fetched modules: a classic-only page.
    pub const NONE: &'static ModuleInput = &ModuleInput {
        entries: Vec::new(),
        sources: Vec::new(),
    };
}

/// One page source that is not a classic script.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleEntry {
    /// Its position in `PageInput::sources`.
    pub index: usize,
    /// What it is.
    pub kind: EntryKind,
    /// An external module script's URL; an inline one's source is the page source itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// What a non-classic page source is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// `<script type=module>`: deferred, it runs in order with deferred classic scripts.
    Module,
    /// `<script type=module async>`: it runs as soon as its graph is ready.
    AsyncModule,
    /// `<script type=importmap>`: JSON that maps bare specifiers to URLs.
    ImportMap,
}

/// One fetched module.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleSource {
    /// The URL V9 asked for, which is the module's identity in the map.
    pub url: String,
    /// The JavaScript source, or None when the fetch, the policy or a bound refused it.
    #[serde(default)]
    pub source: Option<String>,
    /// A classic script a page element asked for (`Execution::script_requests`) rather than a
    /// module: it compiles as a script and runs once when delivered.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub classic: bool,
}

/// A module the page needs fetched before its graph can run.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleRequest {
    /// The absolute module URL.
    pub url: String,
    /// Subresource Integrity metadata an import map gave it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrity: Option<String>,
}

/** Committed module records and the load progress that survives between requests. */
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::v9) struct ModuleState {
    /// Records by module key: the URL, or `inline:N` for an inline module script.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub records: BTreeMap<String, Record>,
    /// The page-load position a deferred module script paused at, waiting for fetches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<usize>,
    /// Async module scripts and dynamic imports waiting for their graphs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub waiting: Vec<Waiting>,
}

impl ModuleState {
    /** Whether a page has used no module at all. */
    pub fn is_empty(&self) -> bool {
        self.records.is_empty() && self.cursor.is_none() && self.waiting.is_empty()
    }
    /** Structural bounds of untrusted committed state. */
    fn validate(&self, sources: usize) -> EngineResult<()> {
        if self.records.len() > MODULES || self.waiting.len() > MODULES {
            return Err(EngineError::limit("V9 module records", MODULES));
        }
        if self.cursor.is_some_and(|cursor| cursor >= sources) {
            return Err(EngineError::invalid("Invalid V9 module cursor."));
        }
        Ok(())
    }
}

/** One module's committed record. */
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::v9) struct Record {
    /// The async holder of its body (suspended after hoisting) and its completion promise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub holder: Option<ObjectId>,
    /// Its namespace object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<ObjectId>,
    /// Its `import.meta` object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<ObjectId>,
    /// The environment its bindings live in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<EnvId>,
    /// Evaluation visited it (it runs, or waits for its dependencies).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub entered: bool,
    /// Its body began running.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub started: bool,
    /// Dependencies on a cycle back to it, which it never waits for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cycle: Vec<String>,
    /// Why its graph failed and was rolled back; importing it again fails the same way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed: Option<String>,
}

/** A graph waiting for its modules: an async module script, or a dynamic import's promise. */
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::v9) struct Waiting {
    /// The root module's key.
    pub key: String,
    /// The async module script's source index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<usize>,
    /// The dynamic import's promise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promise: Option<ObjectId>,
}

/** One compiled module: the base its specifiers resolve against, and its body or why not. */
struct Compiled {
    base: Url,
    body: Result<(Arc<Code>, Arc<ModuleInfo>), EngineError>,
}

/** Where an export's value lives once its module is linked. */
#[derive(Clone, Debug, PartialEq, Eq)]
enum Target {
    /** A binding: the module's key and its environment slot. */
    Binding(Name, u16),
    /** Another module's namespace (`export * as name`). */
    Namespace(Name),
}

/** The module map of one request: the committed records plus what this request compiled. */
#[derive(Default)]
pub(in crate::v9) struct Modules {
    /// Committed records and load progress.
    pub state: ModuleState,
    /// Module URLs this request found missing, for the app to fetch.
    pub requests: Vec<ModuleRequest>,
    compiled: HashMap<String, Compiled>,
    /// Delivered classic scripts by URL, compiled at their fixed offsets, or why not.
    loaded: HashMap<String, EngineResult<Arc<Code>>>,
    entries: BTreeMap<usize, ModuleEntry>,
    /// Offset spans of fetched modules, for import() referrers.
    ranges: Vec<(usize, usize, String)>,
    /// Offset spans of page sources.
    scripts: Vec<(usize, usize)>,
    /// The page sources' URLs ("" when inline).
    urls: Vec<String>,
    map: ImportMap,
    document: Option<Url>,
    /// Resolved exports by namespace module and name.
    exports: HashMap<String, HashMap<Name, Option<Target>>>,
}

/** Native functions modules create with captured values. */
struct Natives {
    /// Run a module once the dependency it waited for settles.
    resume: u32,
    /// Evaluate dynamic imports whose graphs are ready.
    advance: u32,
    /// Fulfill a dynamic import with its namespace.
    imported: u32,
    /// Reject a dynamic import.
    refused: u32,
    /// Report an async module script's failure.
    report: u32,
    /// `import.meta.resolve`.
    resolve: u32,
}

/** Native indexes, set once at install. */
static NATIVES: OnceLock<Natives> = OnceLock::new();

/** The module natives. */
fn natives() -> &'static Natives {
    NATIVES.get().expect("module natives installed")
}

/** Register the natives module records capture. */
pub(in crate::v9) fn install(b: &mut Builder) {
    let _ = NATIVES.set(Natives {
        resume: b.native("", 1, link::resume_native),
        advance: b.native("", 0, link::advance_native),
        imported: b.native("", 1, link::imported_native),
        refused: b.native("", 1, link::refused_native),
        report: b.native("", 1, link::report_native),
        resolve: b.native("resolve", 1, meta_resolve),
    });
}

impl Modules {
    /** Open the module map for a request: validate the input and state, merge import maps and
    compile every module source at its fixed offset (a module that fails to compile fails only
    the graphs that import it). */
    pub fn open(
        vm: &mut Vm,
        input: &ModuleInput,
        sources: &[String],
        starts: &[usize],
        state: ModuleState,
    ) -> EngineResult<()> {
        validate(input, sources.len())?;
        state.validate(sources.len())?;
        let mut modules = Modules {
            state,
            document: Url::parse(&vm.page.environment.url).ok(),
            scripts: starts
                .iter()
                .zip(sources)
                .map(|(start, source)| (*start, source.len()))
                .collect(),
            urls: vm.page.environment.scripts.clone(),
            ..Modules::default()
        };
        for entry in &input.entries {
            modules.entries.insert(entry.index, entry.clone());
        }
        let document = modules.document.clone();
        for entry in modules.entries.values() {
            if entry.kind == EntryKind::ImportMap
                && let Some(base) = &document
            {
                let _ = modules.map.merge(&sources[entry.index], base);
            }
        }
        let inline: Vec<usize> = modules
            .entries
            .values()
            .filter(|entry| entry.kind != EntryKind::ImportMap && entry.url.is_none())
            .map(|entry| entry.index)
            .collect();
        for index in inline {
            let body = compile(vm, &sources[index], starts[index])?;
            if let Some(base) = document.clone() {
                modules
                    .compiled
                    .insert(inline_key(index), Compiled { base, body });
            }
        }
        let mut offset = MODULE_OFFSET;
        for module in &input.sources {
            let length = module.source.as_ref().map_or(0, String::len);
            modules
                .ranges
                .push((offset, offset + length, module.url.clone()));
            if module.classic {
                let code = match &module.source {
                    Some(source) => compile_classic(vm, source, offset),
                    None => Err(unavailable(&module.url)),
                };
                modules.loaded.insert(module.url.clone(), code);
                offset += length + 1;
                continue;
            }
            let body = match &module.source {
                Some(source) => compile(vm, source, offset)?,
                None => Err(unavailable(&module.url)),
            };
            offset += length + 1;
            if let Ok(base) = Url::parse(&module.url) {
                modules
                    .compiled
                    .insert(module.url.clone(), Compiled { base, body });
            }
        }
        vm.modules = modules;
        Ok(())
    }
    /** A delivered classic script's code, or why it has none; None while it is still on its way. */
    pub fn loaded(&self, url: &str) -> Option<EngineResult<Arc<Code>>> {
        self.loaded.get(url).cloned()
    }
    /** The code of every delivered classic script, whose lexical globals are page globals too. */
    pub fn loaded_codes(&self) -> Vec<Arc<Code>> {
        self.loaded
            .values()
            .filter_map(|code| code.clone().ok())
            .collect()
    }
    /** Whether the app delivered this URL already (as a source or as a failure). */
    pub fn delivered(&self, url: &str) -> bool {
        self.ranges.iter().any(|(_, _, delivered)| delivered == url)
    }
    /** The source a page byte offset falls in, for diagnostics: a page script's URL (or its
    one-based index when inline), a fetched module's or script's URL, or dynamic code. */
    pub fn source_name(&self, offset: usize) -> String {
        if let Some(index) = self
            .scripts
            .iter()
            .position(|(start, length)| offset >= *start && offset <= start + length)
        {
            return match self.urls.get(index).filter(|url| !url.is_empty()) {
                Some(url) => url.clone(),
                None => format!("script {}", index + 1),
            };
        }
        self.ranges
            .iter()
            .find(|(start, end, _)| offset >= *start && offset <= *end)
            .map_or_else(|| "dynamic code".into(), |(_, _, url)| url.clone())
    }
    /** What a page source is, if it is not a classic script. */
    pub fn entry(&self, index: usize) -> Option<EntryKind> {
        self.entries.get(&index).map(|entry| entry.kind)
    }
    /** The module key of a module script. */
    pub fn entry_key(&self, index: usize) -> String {
        match self.entries.get(&index).and_then(|entry| entry.url.clone()) {
            Some(url) => url,
            None => inline_key(index),
        }
    }
    /** The key of request `request` of module `key`, through the import maps. */
    fn dependency(&self, key: &str, request: usize) -> Result<String, String> {
        let compiled = self.compiled.get(key).ok_or("Module is not loaded.")?;
        let (_, info) = compiled
            .body
            .as_ref()
            .map_err(|error| error.message.clone())?;
        let specifier = info
            .requests
            .get(request)
            .ok_or("Invalid module request.")?;
        self.map
            .resolve(specifier, &compiled.base)
            .map(String::from)
    }
    /** Ask the app for a module; false once MODULES would be passed. */
    fn request(&mut self, url: &str) -> bool {
        if self.requests.iter().any(|request| request.url == url) {
            return true;
        }
        if url.starts_with("inline:") || self.compiled.len() + self.requests.len() >= MODULES {
            return false;
        }
        self.requests.push(ModuleRequest {
            url: url.into(),
            integrity: self.map.integrity(url),
        });
        true
    }
    /** The base URL code at `site` resolves `import()` against: its module, its external
    script, or the document. */
    fn referrer(&self, site: u32) -> (Option<String>, Option<Url>) {
        let offset = (site / syntax::SITE_TAGS) as usize;
        if let Some((_, _, key)) = self
            .ranges
            .iter()
            .find(|(start, end, _)| (*start..=*end).contains(&offset))
        {
            return (
                Some(key.clone()),
                self.compiled.get(key).map(|module| module.base.clone()),
            );
        }
        let script = self
            .scripts
            .iter()
            .position(|(start, length)| (*start..=start + length).contains(&offset));
        match script {
            Some(index) if self.entry(index).is_some() => {
                let key = self.entry_key(index);
                let base = self.compiled.get(&key).map(|module| module.base.clone());
                (Some(key), base.or_else(|| self.document.clone()))
            }
            Some(index) => {
                let base = self.urls.get(index).and_then(|url| Url::parse(url).ok());
                (None, base.or_else(|| self.document.clone()))
            }
            None => (None, self.document.clone()),
        }
    }
}

/** The key of the inline module script at a source index. */
fn inline_key(index: usize) -> String {
    format!("inline:{index}")
}

/** A module the app could not deliver. */
fn unavailable(url: &str) -> EngineError {
    EngineError::new(
        "module_unavailable",
        format!("Failed to load module script {url}."),
        "Check that the module is reachable and served as JavaScript.",
    )
}

/** Input bounds: MODULES sources within MODULE_BYTES, none over SCRIPT_BYTES, entries that
name real page sources once each. */
fn validate(input: &ModuleInput, sources: usize) -> EngineResult<()> {
    if input.sources.len() > MODULES {
        return Err(EngineError::limit("modules", MODULES));
    }
    let mut total = 0usize;
    for module in &input.sources {
        let length = module.source.as_ref().map_or(0, String::len);
        total = total
            .saturating_add(length)
            .saturating_add(module.url.len());
        if length > SCRIPT_BYTES {
            return Err(EngineError::limit("module bytes", SCRIPT_BYTES));
        }
    }
    if total > MODULE_BYTES {
        return Err(EngineError::limit("page module bytes", MODULE_BYTES));
    }
    let mut seen = std::collections::BTreeSet::new();
    if input.entries.len() > SCRIPT_COUNT
        || input
            .entries
            .iter()
            .any(|entry| entry.index >= sources || !seen.insert(entry.index))
    {
        return Err(EngineError::invalid("Invalid V9 module entries."));
    }
    Ok(())
}

/** A compiled module's code and record, or the syntax or registry error that stopped it. */
type ModuleCompilation = Result<(Arc<Code>, Arc<ModuleInfo>), EngineError>;

/** Compile a delivered classic script at its fixed offset, through the process code cache. */
fn compile_classic(vm: &mut Vm, source: &str, offset: usize) -> EngineResult<Arc<Code>> {
    vm.registry
        .script(source, offset, || super::parse_at(source, offset, None))
}

/** Compile one module at its page offset; the outer error is a service failure. */
fn compile(vm: &mut Vm, source: &str, offset: usize) -> EngineResult<ModuleCompilation> {
    Ok(vm.registry.module(source, offset, || {
        super::lexer::tokenize_module(source)
            .map_err(|mut error| {
                if let Some(location) = error.location.as_deref_mut() {
                    location.byte_offset += offset;
                }
                error
            })
            .and_then(|mut tokens| {
                for token in &mut tokens {
                    token.location.byte_offset += offset;
                }
                syntax::parse_module(tokens)
            })
    }))
}

/** `import(specifier)`: a promise for the namespace, once the graph loads and evaluates. The
graph evaluates in a microtask when it is all here, and otherwise after the app delivers it. */
pub(in crate::v9) fn dynamic_import(vm: &mut Vm, specifier: JsValue, site: u32) -> Res<JsValue> {
    let promise = vm.new_promise()?;
    vm.pin(JsValue::Object(promise));
    let text = match vm.to_string(&specifier) {
        Ok(text) => text.lossy(),
        Err(super::vm::Thrown::Value(error)) => {
            vm.settle(promise, super::vm::data::PromiseState::Rejected(error))?;
            return Ok(JsValue::Object(promise));
        }
        Err(fatal) => return Err(fatal),
    };
    let (_, base) = vm.modules.referrer(site);
    let resolved = match base {
        Some(base) => vm.modules.map.resolve(&text, &base),
        None => Err("Failed to resolve module specifier: the page has no base URL.".into()),
    };
    let key = match resolved {
        Ok(url) => String::from(url),
        Err(message) => return link::reject_type_error(vm, promise, &message),
    };
    if vm.modules.state.waiting.len() >= MODULES {
        return link::reject_type_error(vm, promise, "Too many module imports are waiting.");
    }
    vm.modules.state.waiting.push(Waiting {
        key,
        entry: None,
        promise: Some(promise),
    });
    let advance = vm.native_closure(natives().advance, Vec::new())?;
    vm.enqueue(super::vm::data::Job::Callback(JsValue::Object(advance)))?;
    Ok(JsValue::Object(promise))
}

/** `import.meta.resolve(specifier)`: the URL an import of it would load. */
fn meta_resolve(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let slots = vm.slots(call.callee);
    let Some(JsValue::String(key)) = slots.first() else {
        return Err(vm.type_error("Illegal invocation"));
    };
    let key = key.lossy();
    let specifier = super::builtins::arg_string(vm, &call, 0)?.lossy();
    let base = vm
        .modules
        .compiled
        .get(&key)
        .map(|module| module.base.clone());
    let resolved = match base {
        Some(base) => vm.modules.map.resolve(&specifier, &base),
        None => Err("Module is not loaded.".into()),
    };
    match resolved {
        Ok(url) => Ok(JsValue::String(JsString::from_utf8(url.as_str())?)),
        Err(message) => Err(vm.type_error(&message)),
    }
}

/** A namespace's own export: its binding's current value, or a ReferenceError while the
binding is uninitialized. Other keys are the namespace's ordinary properties. */
pub(in crate::v9) fn namespace_own(vm: &mut Vm, module: &str, key: &Key) -> Res<Option<Prop>> {
    let Key::Str(name) = key else {
        return Ok(None);
    };
    let name = Name::from(name.lossy().as_str());
    let Some(target) = export_target(vm, module, &name) else {
        return Ok(None);
    };
    let value = match target {
        Target::Namespace(other) => JsValue::Object(link::namespace(vm, &other)?),
        Target::Binding(other, slot) => {
            let value = vm
                .modules
                .state
                .records
                .get(&*other)
                .and_then(|record| record.env)
                .and_then(|env| vm.heap.env(env))
                .and_then(|env| env.slots.get(slot as usize).cloned());
            match value {
                Some(value) if value != JsValue::Hole => value,
                _ => {
                    return Err(vm.reference_error(&format!(
                        "Cannot access '{name}' before initialization"
                    )));
                }
            }
        }
    };
    Ok(Some(Prop {
        slot: HeapSlot::Data(value),
        attrs: WRITABLE | ENUMERABLE,
    }))
}

/** A namespace's export names, sorted, without ambiguous star exports. */
pub(in crate::v9) fn namespace_keys(vm: &mut Vm, module: &str) -> Res<Vec<Key>> {
    let mut names = std::collections::BTreeSet::new();
    link::export_names(&vm.modules, module, &mut Vec::new(), &mut names);
    let mut keys = Vec::new();
    for name in names {
        if export_target(vm, module, &name).is_some() {
            keys.push(Key::Str(JsString::from_utf8(&name)?));
        }
    }
    Ok(keys)
}

/** Resolve an export once per request and namespace. */
fn export_target(vm: &mut Vm, module: &str, name: &Name) -> Option<Target> {
    if let Some(found) = vm
        .modules
        .exports
        .get(module)
        .and_then(|names| names.get(name))
    {
        return found.clone();
    }
    let found = link::resolve_export(&vm.modules, module, name, &mut Vec::new());
    vm.modules
        .exports
        .entry(module.to_string())
        .or_default()
        .insert(name.clone(), found.clone());
    found
}

/** The `Symbol.toStringTag` a namespace carries. */
fn to_string_tag() -> Key {
    Key::Sym(symbol::TO_STRING_TAG)
}
