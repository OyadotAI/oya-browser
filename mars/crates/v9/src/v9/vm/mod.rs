/**
 * V9's virtual machine: an explicit-frame bytecode interpreter over a bounded, serializable heap.
 * Every root is explicit, so collection can run between instructions and suspended generator and
 * async frames are plain data.
 */
pub(in crate::v9) mod arguments;
pub(in crate::v9) mod array_elements;
pub(in crate::v9) mod asyncgen;
pub(in crate::v9) mod bigint;
pub(in crate::v9) mod call;
mod codegen;
pub(in crate::v9) mod convert;
pub(in crate::v9) mod data;
mod define_refusal;
mod dynamic;
mod eval_origin;
pub(in crate::v9) use eval_origin::EvalOrigin;
pub(in crate::v9) mod error;
mod function_keys;
pub(in crate::v9) mod function_name;
pub(in crate::v9) mod gc;
mod has_instance;
pub(in crate::v9) mod heap;
pub(in crate::v9) mod iter;
mod native_activations;
pub(in crate::v9) use native_activations::{NativeActivation, NativeKind};
#[cfg(feature = "conformance")]
pub(in crate::v9) mod memory;
pub(in crate::v9) mod private_elements;
pub(in crate::v9) mod property;
pub(in crate::v9) mod proxy;
pub(in crate::v9) mod realm;
pub(in crate::v9) mod realms;
pub(in crate::v9) mod run;
pub(in crate::v9) mod shape;
pub(in crate::v9) mod string_budget;
pub(in crate::v9) mod templates;
pub(in crate::v9) mod trie;
pub(in crate::v9) mod validate;
pub(in crate::v9) use dynamic::{
    source_base as dynamic_source_base, validate_sources as validate_dynamic_sources,
};
pub(in crate::v9) mod weak;

/** How far the object and function counts may overshoot their bounds between a full heap
noticed outside native code and the collection at the next instruction: an eighth. */
const GC_SLACK_DIVISOR: usize = 8;
use super::binding::Binding;
use super::compile::{Code, Registry};
use super::platform::Page;
use super::value::{JsString, JsValue, ObjectId};
use crate::protocol::EngineError;
use data::{Class, Handler, Job};
use heap::{EnvId, FastMap, Heap, Object};
/** Periodically purge dead weak compiled-string entries without scanning on every lookup. */
#[cfg(feature = "conformance")]
const CODE_CONSTANT_INDEX_SWEEP_INTERVAL: u16 = 256;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

/** A thrown script value, or an engine failure scripts cannot catch (limits, deadlines). */
#[derive(Debug)]
pub(in crate::v9) enum Thrown {
    Value(JsValue),
    Fatal(EngineError),
}

impl From<EngineError> for Thrown {
    /** Engine errors are uncatchable. */
    fn from(error: EngineError) -> Self {
        Self::Fatal(error)
    }
}

/** Results of script-visible operations. */
pub(in crate::v9) type Res<T> = Result<T, Thrown>;

/** One native invocation. */
pub(in crate::v9) struct Call<'a> {
    pub this: JsValue,
    pub args: &'a [JsValue],
    /** Set when called with `new`. */
    pub new_target: Option<JsValue>,
    /** The function object being called. */
    pub callee: ObjectId,
}

impl Call<'_> {
    /** Argument `index`, or undefined. */
    pub fn arg(&self, index: usize) -> JsValue {
        self.args.get(index).cloned().unwrap_or(JsValue::Undefined)
    }
}

/** An executing activation. */
pub(in crate::v9) struct Frame {
    /** The function's creation context selects intrinsics and global bindings during execution. */
    pub realm: u32,
    pub code: Arc<Code>,
    pub pc: usize,
    pub stack: Vec<JsValue>,
    pub regs: Vec<JsValue>,
    pub env: Option<EnvId>,
    pub this: JsValue,
    pub callee: JsValue,
    pub args: Vec<JsValue>,
    pub new_target: JsValue,
    pub handlers: Vec<Handler>,
    pub envs: u16,
    /** Returning from this frame returns from the current `run`. */
    pub boundary: bool,
    /** A native callback entry conceals the native caller without changing return boundaries. */
    pub caller_hidden: bool,
    /** A `new` call: an object return replaces `this`, anything else returns `this`. */
    pub construct: bool,
    /** How a resumed generator re-entered, for `yield*`. */
    pub resume: Resume,
}

/** How a suspended frame resumes. */
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(in crate::v9) enum Resume {
    #[default]
    Next,
    Throw,
    Return,
}

/** How `run` left its boundary frame. */
pub(in crate::v9) enum Exit {
    Return(JsValue),
    /** The frame yielded and is now in `Vm::suspended`. */
    Yield(JsValue),
    /** Synchronous delegation yielded an authored iterator-result object unchanged. */
    YieldRaw(JsValue),
    /** The frame awaits and is now in `Vm::suspended`. */
    Await(JsValue),
}

/**
 * One dynamic source (`new Function`, indirect or direct eval) kept so restored state can
 * recompile it. A direct eval's caller scopes ride beside the text, never inside it, so no page
 * string can claim them.
 */
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(from = "DynamicWire")]
pub(in crate::v9) struct DynamicSource {
    pub source: String,
    /// The compiler-made caller context of direct eval code; None for ordinary dynamic code.
    pub context: Option<String>,
    /// Explicit immutable offset after code collection leaves gaps in the source ledger.
    pub base: Option<usize>,
    /// Real eval compilation provenance is diagnostic data and carries no object or realm authority.
    pub origin: Option<EvalOrigin>,
}

/** The stored form: plain text for ordinary code (the older form too), a pair for direct eval. */
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
enum DynamicWire {
    Origin(String, Option<String>, Option<usize>, EvalOrigin),
    At(String, Option<String>, usize),
    Plain(String),
    Eval(String, String),
}

impl From<DynamicWire> for DynamicSource {
    /** Either stored form. */
    fn from(wire: DynamicWire) -> Self {
        match wire {
            DynamicWire::Origin(source, context, base, origin) => Self {
                source,
                context,
                base,
                origin: Some(origin),
            },
            DynamicWire::At(source, context, base) => Self {
                source,
                context,
                base: Some(base),
                origin: None,
            },
            DynamicWire::Plain(source) => Self {
                source,
                context: None,
                base: None,
                origin: None,
            },
            DynamicWire::Eval(source, context) => Self {
                source,
                context: Some(context),
                base: None,
                origin: None,
            },
        }
    }
}

impl serde::Serialize for DynamicSource {
    /** Plain text unless it carries a context. */
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if let Some(origin) = &self.origin {
            return DynamicWire::Origin(
                self.source.clone(),
                self.context.clone(),
                self.base,
                origin.clone(),
            )
            .serialize(serializer);
        }
        if let Some(base) = self.base {
            return DynamicWire::At(self.source.clone(), self.context.clone(), base)
                .serialize(serializer);
        }
        match &self.context {
            None => DynamicWire::Plain(self.source.clone()),
            Some(context) => DynamicWire::Eval(self.source.clone(), context.clone()),
        }
        .serialize(serializer)
    }
}

/** Symbols beyond the well-known ones. */
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(in crate::v9) struct Symbols {
    /** Descriptions of user symbols, by identity minus the well-known count. */
    pub descriptions: Vec<Option<JsString>>,
    /** Symbol.for registry. */
    pub registry: BTreeMap<String, u32>,
}

/** The bounds one VM enforces; the public entry points always use the core constants. */
#[derive(Clone, Copy, Debug)]
pub(in crate::v9) struct Limits {
    pub objects: usize,
    pub functions: usize,
    pub properties: usize,
    pub units: usize,
    pub environments: usize,
    pub bindings: usize,
    pub array_length: usize,
    pub state_bytes: usize,
    pub operations: usize,
    pub frames: usize,
    pub jobs: usize,
}

impl Default for Limits {
    /** The shared core bounds. */
    fn default() -> Self {
        use crate::constants::*;
        Self {
            objects: VM_OBJECTS,
            functions: VM_FUNCTIONS,
            properties: VM_PROPERTIES,
            units: JS_ENVIRONMENT_UNITS,
            environments: VM_ENVIRONMENTS,
            bindings: VM_BINDINGS,
            array_length: VM_ARRAY_LENGTH,
            state_bytes: VM_STATE_BYTES,
            operations: OPERATIONS,
            frames: VM_CALL_DEPTH,
            jobs: VM_PENDING_JOBS,
        }
    }
}

/** Numeric source fingerprints are weak and bounded independently of script count. */
#[cfg(feature = "conformance")]
const NUMBER_SOURCE_CACHE_CAPACITY: usize = 4096;

/** The virtual machine for one request. */
pub(in crate::v9) struct Vm<'h> {
    /** Current activation context; production builds keep the root context. */
    pub active_realm: u32,
    /** The conformance host's bounded saved contexts, without filesystem or browser authority. */
    #[cfg(feature = "conformance")]
    pub realms: realms::State,
    /** d8's entered contexts are separate from the currently executing function's realm. */
    #[cfg(feature = "conformance")]
    pub realm_entries: Vec<u32>,
    /** Actual host evaluation boundaries enrich captured JS stacks without creating fake code frames. */
    #[cfg(feature = "conformance")]
    pub trace_boundaries: Vec<(usize, ObjectId, JsValue, u32, &'static str)>,
    pub heap: Heap,
    /** Top-level let, const and class bindings. */
    pub globals: BTreeMap<String, Binding>,
    pub symbols: Symbols,
    pub registry: Registry,
    /** Isolate-local canonical index for persisted compiled string cells. */
    #[cfg(feature = "conformance")]
    code_constant_index: FastMap<u64, Vec<(crate::v9::value::WeakString, bool)>>,
    #[cfg(feature = "conformance")]
    code_constant_index_ops: u16,
    /** Weak source references cache fingerprints without retaining compiled scripts. */
    #[cfg(feature = "conformance")]
    number_source_hashes: FastMap<usize, (std::sync::Weak<str>, [u8; 32])>,
    pub frames: Vec<Frame>,
    /** Selected native bodies retain their real position only during the active invocation. */
    pub native_activations: Vec<NativeActivation>,
    /** Native async drivers identify current output promises; committed continuations remain in the ordinary heap graph. */
    pub async_drivers: Vec<ObjectId>,
    /** A prepareStackTrace hook cannot recursively invoke another formatting hook in this VM. */
    pub formatting_stack: bool,
    /** Nested microtask checkpoints leave the active drain in charge of its queue. */
    pub draining_jobs: bool,
    /** Values native code holds across calls back into scripts. */
    pub pins: Vec<JsValue>,
    /** WeakRef targets observed in this task survive through its complete microtask checkpoint. */
    pub kept_weak: FastMap<data::TableKey, JsValue>,
    /** True while native Rust code runs, so its allocations are pinned. */
    pub pinning: bool,
    /** A full heap noticed outside native code: the collector runs at the next instruction,
    where every live value sits in a frame, instead of under a Rust local it cannot see. */
    pub collect_due: bool,
    /** Nested native-to-script calls. */
    pub native_depth: usize,
    pub remaining: usize,
    pub deadline: Option<std::time::Instant>,
    /** A fresh native clock anchors monotonic profiler timestamps across each execution request. */
    #[cfg(feature = "conformance")]
    pub profile_clock: crate::v9::builtins::ProfileClock,
    pub jobs: VecDeque<Job>,
    /** The frame a Yield or Await just suspended. */
    pub suspended: Option<data::SavedFrame>,
    /** Compiled regular expressions, by source and flags. */
    pub regexps: FastMap<(JsString, String), Rc<super::regexp::Program>>,
    /** The error object an unsupported feature threw last, so an uncaught one reports as such. */
    pub unsupported: Option<(ObjectId, EngineError)>,
    pub limits: Limits,
    pub page: Page<'h>,
    /** Dynamic sources compiled by `new Function` and indirect eval, kept so state can recompile. */
    pub dynamic: Vec<DynamicSource>,
    /** Frozen template identities belong to their parsed source and creation realm. */
    pub templates: templates::Cache,
    /** Whether any Proxy exists, so ordinary property access skips the proxy check entirely. */
    pub proxies: bool,
    /** The page's module map: compiled module sources and their committed records. */
    pub modules: super::modules::Modules,
}

/** The global object's fixed identity. */
pub(in crate::v9) const GLOBAL: ObjectId = crate::constants::VM_GLOBAL_OBJECT_ID;

impl<'h> Vm<'h> {
    /** Track a native or interpreter string without extending its allocation lifetime. */
    pub(in crate::v9) fn track_string(&mut self, text: &JsString) {
        self.heap.track_string(text);
    }
    /** Give cached compiled text one independently mutable cell per isolate and stable code location. */
    #[cfg(feature = "conformance")]
    pub(super) fn isolate_code_constant(&mut self, site: u32, index: u32, source: &JsString) -> JsString {
        let key = (u64::from(site) << 32) | u64::from(index);
        if let Some(value) = self.page.state.code_constants.get(&key)
            && value == source
        {
            return value.clone();
        }
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        source.hash(&mut hasher);
        let hash = hasher.finish();
        let value = self.code_constant_index.get(&hash).and_then(|bucket| bucket.iter().filter_map(|(entry, one_byte)| entry.upgrade(*one_byte)).find(|value| value == source));
        let value = if let Some(value) = value { value } else {
            let value = source.isolate_code_constant();
            self.track_string(&value);
            let (_, weak) = value.weak_registration();
            self.code_constant_index.entry(hash).or_default().push((weak, value.is_latin1()));
            self.code_constant_index_ops = self.code_constant_index_ops.saturating_add(1);
            if self.code_constant_index_ops >= CODE_CONSTANT_INDEX_SWEEP_INTERVAL {
                self.code_constant_index.retain(|_, entries| { entries.retain(|(entry, _)| entry.is_alive()); !entries.is_empty() });
                self.code_constant_index_ops = 0;
            }
            value
        };
        self.page.state.code_constants.insert(key, value.clone());
        value
    }
    /** Return the isolate's permanent read-only canonical cell for one Latin-1 character. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn character_root(&mut self, unit: u8) -> Res<JsString> {
        if let Some(value) = self.page.state.character_roots.get(&unit) { return Ok(value.clone()); }
        let value = JsString::character_root(unit);
        self.track_string(&value);
        self.check_heap_bounds()?;
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hasher);
        let (_, weak) = value.weak_registration();
        self.code_constant_index.entry(hasher.finish()).or_default().push((weak, true));
        self.page.state.character_roots.insert(unit, value.clone());
        Ok(value)
    }
    /** Reuse one old heap allocation for each numeric literal in one bytecode constant pool. */
    #[cfg(feature = "conformance")]
    pub(super) fn number_literal(&mut self, site: u32, source: &std::sync::Arc<str>, number: f64) -> Res<JsValue> {
        let pointer = source.as_ptr() as usize;
        let hash = if let Some((weak, hash)) = self.number_source_hashes.get(&pointer) {
            if weak.upgrade().is_some_and(|previous| std::sync::Arc::ptr_eq(&previous, source)) { *hash } else { crate::v9::compile::source_key(source) }
        } else { crate::v9::compile::source_key(source) };
        if self.number_source_hashes.len() >= NUMBER_SOURCE_CACHE_CAPACITY && !self.number_source_hashes.contains_key(&pointer) {
            self.number_source_hashes.retain(|_, (weak, _)| weak.strong_count() != 0);
        }
        if self.number_source_hashes.len() < NUMBER_SOURCE_CACHE_CAPACITY || self.number_source_hashes.contains_key(&pointer) {
            self.number_source_hashes.insert(pointer, (std::sync::Arc::downgrade(source), hash));
        }
        let pools = &mut self.page.state.number_constants;
        if pools.get(&site).is_some_and(|pool| pool.source != hash) { pools.remove(&site); }
        if let Some(value) = pools.get(&site).and_then(|pool| pool.values.get(&number.to_bits())) { return Ok(value.clone()); }
        let id = self.alloc(heap::Object::new(data::Class::Number(number), None))?;
        if let Some(object) = self.heap.get_mut(id) { object.age = gc::PROMOTION_AGE; }
        let value = JsValue::LocalNumber(std::sync::Arc::new(crate::v9::value::SharedNumberData { value: number, root: id }));
        self.page.state.number_constants.entry(site).or_insert_with(|| crate::v9::platform::NumberConstantPool { source: hash, values: BTreeMap::new() }).values.insert(number.to_bits(), value.clone());
        Ok(value)
    }
    /** Drop roots for code replaced by dynamic evaluation before snapshotting the registry. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn prune_number_constants(&mut self) {
        let mut hashes: FastMap<usize, [u8; 32]> = FastMap::default();
        self.page.state.number_constants.retain(|site, pool| self.registry.codes.get(site).and_then(|code| code.source.as_ref()).is_some_and(|(source, _)| {
            *hashes.entry(source.as_ptr() as usize).or_insert_with(|| crate::v9::compile::source_key(source)) == pool.source
        }));
    }
    /** Intern against live isolate-owned cells, turning noncanonical ordinary strings into thin aliases. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn internalize_string(&mut self, source: &JsString) -> Res<JsString> {
        use std::hash::{Hash, Hasher};
        if let Some(target) = source.thin_target() { return Ok(target); }
        if source.is_interned() { return Ok(source.clone()); }
        let _ = self.materialize_string(source)?;
        let flat = source.flatten_for_intrinsic()?;
        self.track_string(&flat);
        if self.page.environment.conformance_shared_string_table {
            let target = if let Some(target) = crate::v9::value::find_shared_internalized(&flat) {
                self.track_string(&target);
                target
            } else {
                self.with_string_output(&flat, flat.len(), |vm| {
                    let target = crate::v9::value::intern_shared_internalized(&flat)?;
                    vm.track_string(&target);
                    Ok(target)
                })?
            };
            if !target.mark_interned() || (!source.is_shared() && !source.forward_to(&target)) { return Err(self.unsupported_error("shared thin string transition")); }
            if !flat.same_backing(source) && !flat.forward_to(&target) { return Err(self.unsupported_error("shared flat string transition")); }
            return Ok(target);
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        flat.hash(&mut hasher);
        let hash = hasher.finish();
        let found = self.code_constant_index.get(&hash).and_then(|entries| entries.iter().filter_map(|(entry, one_byte)| entry.upgrade(*one_byte)).find(|value| value.is_interned() && value == &flat));
        let target = if let Some(target) = found {
            target
        } else if flat.is_interned() {
            flat.clone()
        } else {
            let can_reuse = flat.is_old() && flat.can_intern_in_place() && !flat.is_shared() && !self.page.environment.conformance_shared_string_table && (flat.is_latin1() || flat.code_units().any(|unit| unit > 255));
            let candidate = if can_reuse && flat.mark_interned() {
                flat.clone()
            } else {
                let candidate = self.copy_string(&flat, false)?;
                if !candidate.mark_interned() { return Err(self.unsupported_error("internalized string representation")); }
                candidate
            };
            let (_, weak) = candidate.weak_registration();
            self.code_constant_index.entry(hash).or_default().push((weak, candidate.is_latin1()));
            self.code_constant_index_ops = self.code_constant_index_ops.saturating_add(1);
            if self.code_constant_index_ops >= CODE_CONSTANT_INDEX_SWEEP_INTERVAL {
                self.code_constant_index.retain(|_, entries| { entries.retain(|(entry, _)| entry.is_alive()); !entries.is_empty() });
                self.code_constant_index_ops = 0;
            }
            candidate
        };
        if !flat.is_shared() && !flat.same_backing(&target) && !flat.forward_to(&target) {
            return Err(self.unsupported_error("thin flat string transition"));
        }
        if !source.is_shared() && !source.same_backing(&target) && !source.forward_to(&target) {
            return Err(self.unsupported_error("thin string transition"));
        }
        Ok(target)
    }
    /** Save only weak intern entries whose backing is still retained by the isolate. */
    #[cfg(feature = "conformance")]
    pub(super) fn save_interned_strings(&mut self) {
        let mut unique = std::collections::BTreeMap::new();
        for (entry, one_byte) in self.code_constant_index.values().flatten() {
            if let Some(value) = entry.upgrade(*one_byte).filter(JsString::is_interned) { unique.insert(value.clone(), value); }
        }
        self.page.state.interned_strings = unique.into_values().collect();
    }
    /** Bound temporary and final contiguous buffers before a native consumer materializes a rope. */
    pub(in crate::v9) fn materialize_string<'a>(&mut self, text: &'a JsString) -> Res<&'a [u16]> {
        let result = self.heap.string_budget.preflight(text, self.limits.units);
        self.heap.last_registered_string_units = self.heap.string_budget.upper_units();
        let reserved = result?;
        let result = text.try_as_units();
        self.heap.string_budget.release(reserved);
        if let Some(units) = self.heap.string_budget.refresh_materialization() {
            self.heap.last_registered_string_units = units;
        }
        Ok(result?)
    }
    /** Reserve both temporary and final buffers before copying a native string. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn copy_string(&mut self, source: &JsString, two_byte: bool) -> Res<JsString> {
        let reserved = self.heap.string_budget.reserve_copy(source, self.limits.units)?;
        let units = source.code_units().collect();
        let result = if two_byte { JsString::from_units_two_byte(units) } else { JsString::from_units(units) };
        if let Ok(value) = &result { self.track_string(value); }
        self.heap.string_budget.release(reserved);
        Ok(result?)
    }
    /** Reserve and register a real shared backing before publishing it to another agent. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn share_string(&mut self, source: &JsString) -> Res<JsString> {
        if source.is_shared() { return Ok(source.clone()); }
        let _ = self.materialize_string(source)?;
        if let Some(shared) = crate::v9::value::find_shared_string(source) {
            self.track_string(&shared);
            return Ok(shared);
        }
        self.with_string_output(source, source.len(), |vm| {
            let shared = crate::v9::value::intern_shared_string(source)?;
            vm.track_string(&shared);
            Ok(shared)
        })
    }
    /** Charge a native string-derived buffer for the full duration of its use. */
    pub(in crate::v9) fn with_string_output<T>(&mut self, source: &JsString, units: usize, f: impl FnOnce(&mut Self) -> Res<T>) -> Res<T> {
        let reserved = match self.heap.string_budget.reserve_output(source, units, self.limits.units) {
            Ok(reserved) => reserved,
            // Nested script callbacks also have native_depth > 0; pinning, not depth, marks
            // whether execution is currently holding unrooted native temporaries.
            Err(_) if !self.pinning && !self.frames.is_empty() => {
                self.collect_now()?;
                self.heap.string_budget.reserve_output(source, units, self.limits.units)?
            }
            Err(error) => return Err(error.into()),
        };
        let result = f(self);
        self.heap.string_budget.release(reserved);
        result
    }
    /** Reserve a flat concatenation's peak backing before either input can be copied. */
    pub(in crate::v9) fn concat_strings(&mut self, left: &JsString, right: &JsString) -> Res<JsString> {
        let length = left.len().saturating_add(right.len());
        if length > crate::constants::JS_STRING_UNITS {
            return Err(self.range_error("Invalid string length"));
        }
        if !left.concat_will_flatten(right) {
            return Ok(left.concat(right)?);
        }
        self.track_string(right);
        self.with_string_output(left, length.saturating_mul(2), |vm| {
            let value = left.concat(right)?;
            vm.track_string(&value);
            Ok(value)
        })
    }
    /** Copy a UTF-16 range with normalized encoding, charging both transient and final backing. */
    pub(in crate::v9) fn copy_string_range(&mut self, source: &JsString, start: usize, end: usize) -> Res<JsString> {
        let count = end - start;
        self.with_string_output(source, count.saturating_mul(2), |vm| {
            let units = source.code_units().skip(start).take(count).collect();
            let value = JsString::from_units(units)?;
            vm.track_string(&value);
            Ok(value)
        })
    }
    /** Reserve any source cache and final output before a repetition falls back to copying. */
    pub(in crate::v9) fn repeat_string(&mut self, source: &JsString, count: usize) -> Res<JsString> {
        let length = source.len().saturating_mul(count);
        if length > crate::constants::JS_STRING_UNITS || !source.repeat_will_flatten(count) {
            return Ok(source.repeat(count)?);
        }
        let _ = self.materialize_string(source)?;
        self.with_string_output(source, length.saturating_mul(2), |vm| {
            let value = source.repeat(count)?;
            vm.track_string(&value);
            Ok(value)
        })
    }
    /** Restored top-level bindings live outside the heap but still own string allocations. */
    pub(super) fn register_global_strings(&mut self) {
        let strings: Vec<_> = self.globals.values().filter_map(|binding| match &binding.value {
            Some(JsValue::String(text)) => Some(text.clone()),
            _ => None,
        }).collect();
        for text in strings { self.track_string(&text); }
        #[cfg(feature = "conformance")]
        {
            let constants: Vec<_> = self.page.state.code_constants.values().chain(self.page.state.character_roots.values()).cloned().collect();
            for text in constants { self.track_string(&text); }
        }
    }
    /** A VM over a heap, with the global object created if missing. */
    pub fn new(heap: Heap, page: Page<'h>, limits: Limits) -> Self {
        #[cfg(feature = "conformance")]
        let mut page = page;
        #[cfg(feature = "conformance")]
        let code_constant_index = {
            use std::hash::{Hash, Hasher};
            let mut index: FastMap<u64, Vec<(crate::v9::value::WeakString, bool)>> = FastMap::default();
            for value in page.state.code_constants.values().chain(page.state.interned_strings.iter()).chain(page.state.character_roots.values()) {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                value.hash(&mut hasher);
                let (_, weak) = value.weak_registration();
                index.entry(hasher.finish()).or_default().push((weak, value.is_latin1()));
            }
            index
        };
        #[cfg(feature = "conformance")]
        page.state.interned_strings.clear();
        let mut heap = heap;
        heap.string_budget = string_budget::StringBudget::default();
        heap.last_registered_string_units = 0;
        heap.register_existing_strings();
        #[cfg(feature = "conformance")]
        let profile_clock =
            crate::v9::builtins::ProfileClock::new(page.state.cpu_profile.last_time());
        #[cfg(feature = "conformance")]
        realm::banks::select(0);
        let mut vm = Self {
            active_realm: 0,
            #[cfg(feature = "conformance")]
            realms: realms::State::default(),
            #[cfg(feature = "conformance")]
            realm_entries: vec![0],
            #[cfg(feature = "conformance")]
            trace_boundaries: Vec::new(),
            heap,
            globals: BTreeMap::new(),
            symbols: Symbols::default(),
            registry: Registry::default(),
            #[cfg(feature = "conformance")]
            code_constant_index,
            #[cfg(feature = "conformance")]
            code_constant_index_ops: 0,
            #[cfg(feature = "conformance")]
            number_source_hashes: FastMap::default(),
            frames: Vec::new(),
            native_activations: Vec::new(),
            async_drivers: Vec::new(),
            formatting_stack: false,
            draining_jobs: false,
            pins: Vec::new(),
            kept_weak: FastMap::default(),
            pinning: false,
            collect_due: false,
            native_depth: 0,
            remaining: limits.operations,
            deadline: None,
            #[cfg(feature = "conformance")]
            profile_clock,
            jobs: VecDeque::new(),
            suspended: None,
            regexps: FastMap::default(),
            unsupported: None,
            limits,
            page,
            dynamic: Vec::new(),
            templates: templates::Cache::default(),
            proxies: false,
            modules: super::modules::Modules::default(),
        };
        vm.proxies = vm
            .heap
            .objects
            .values()
            .any(|object| matches!(object.class, Class::Proxy(_)));
        if vm.heap.get(GLOBAL).is_none() {
            let proto = realm::realm().ids.window_prototype;
            #[cfg(feature = "conformance")]
            let proto = if vm.page.environment.conformance_d8 {
                realm::realm().ids.shell_global_prototype
            } else {
                proto
            };
            vm.heap
                .insert_at(GLOBAL, Object::new(Class::Plain, Some(proto)));
            vm.heap.next_object = vm.heap.next_object.max(GLOBAL + 1);
            #[cfg(feature = "conformance")]
            if vm.page.environment.conformance_d8 {
                vm.seed_realm_global(GLOBAL, 0);
            }
            #[cfg(feature = "conformance")]
            if !vm.page.environment.conformance_d8 {
                crate::v9::builtins::expose_conformance_globals(&mut vm, GLOBAL, 0);
            }
        }
        // %ThrowTypeError% is frozen from the start; the reflection builtins read
        // extensibility from the heap, so it lives there before any script can see it.
        let _ = vm.materialize(realm::realm().ids.throw_type_error);
        vm
    }
    /** Allocate an object, pinning it while native code runs, collecting first when full. */
    pub fn alloc(&mut self, object: Object) -> Res<ObjectId> {
        let function = object.function().is_some();
        let counts = self.heap.counts;
        let (count, limit, name) = if function {
            (counts.functions, self.limits.functions, "V9 functions")
        } else {
            (counts.objects, self.limits.objects, "V9 heap objects")
        };
        if count >= limit {
            if self.pinning {
                // Inside a native every object it made is pinned, so collecting now is safe.
                self.collect_now()?;
                self.check_heap_bounds()?;
            } else {
                // The interpreter or platform code holds ids in Rust locals the collector cannot
                // see (a promise whose resolving functions are being made), so the collection
                // waits for the next instruction and the heap overshoots by a bounded slack.
                self.collect_due = true;
                if count >= limit + limit / GC_SLACK_DIVISOR {
                    return Err(EngineError::limit(name, limit).into());
                }
            }
        }
        let id = self.heap.insert(object)?;
        if self.pinning {
            self.pins.push(JsValue::Object(id));
        }
        Ok(id)
    }
    /** A plain object with Object.prototype. */
    pub fn new_object(&mut self) -> Res<ObjectId> {
        let proto = realm::realm().ids.object_prototype;
        self.alloc(Object::new(Class::Plain, Some(proto)))
    }
    /** An array with the given elements. */
    pub fn new_array(&mut self, elements: Vec<JsValue>) -> Res<ObjectId> {
        let elements = array_elements::ArrayElements::from(elements);
        if elements.resident_len() > self.limits.array_length {
            return Err(
                EngineError::limit("V9 resident array elements", self.limits.array_length).into(),
            );
        }
        let proto = realm::realm().ids.array_prototype;
        self.alloc(Object::new(Class::Array(elements), Some(proto)))
    }
    /** Logical holes do not allocate elements or consume the resident element allowance. */
    pub fn new_array_length(&mut self, length: usize) -> Res<ObjectId> {
        if length > u32::MAX as usize {
            return Err(self.range_error("Invalid array length"));
        }
        let elements = array_elements::ArrayElements::holes(length);
        let proto = realm::realm().ids.array_prototype;
        self.alloc(Object::new(Class::Array(elements), Some(proto)))
    }
    /** Keep a value alive while native code calls back into scripts. */
    pub fn pin(&mut self, value: JsValue) {
        if matches!(value, JsValue::Object(_)) {
            self.pins.push(value);
        }
    }
    /** Native waits poll collection requests only after releasing their own synchronization locks. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn wait_safepoint(&mut self) -> Res<bool> {
        if let Some(host) = self.page.environment.conformance_workers.clone() {
            host.collection_safepoint(self.page.environment.conformance_worker_id, self)?;
        }
        Ok(self.expired())
    }
    /** Charge work against the operation budget and the request deadline. */
    pub fn charge(&mut self, amount: usize) -> Res<()> {
        #[cfg(feature = "conformance")]
        if self
            .page
            .environment
            .conformance_worker_cancel
            .as_ref()
            .is_some_and(|cancel| cancel.load(std::sync::atomic::Ordering::Acquire))
        {
            return Err(EngineError::new(
                "conformance_worker_cancelled",
                "Worker execution was cancelled.",
                "The owning test host terminated this child isolate.",
            )
            .into());
        }
        self.remaining = self.remaining.checked_sub(amount).ok_or_else(|| {
            Thrown::Fatal(EngineError::new(
                "limit_exceeded",
                "Script execution stopped: operation limit exceeded.",
                "Reduce the script work; what it did before it stopped stays, as in browsers.",
            ))
        })?;
        #[cfg(feature = "conformance")]
        crate::v9::builtins::profile_tick(self)?;
        Ok(())
    }
    /** True once the request deadline has passed. */
    pub fn expired(&self) -> bool {
        #[cfg(feature = "conformance")]
        if self
            .page
            .environment
            .conformance_worker_cancel
            .as_ref()
            .is_some_and(|cancel| cancel.load(std::sync::atomic::Ordering::Acquire))
        {
            return true;
        }
        self.deadline
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
    }
    /** Run the collection a full heap deferred to this instruction boundary, where every live
    value is in a frame or pinned; the bounds then hold or the script stops. */
    pub fn collect_if_due(&mut self) -> Res<()> {
        if !self.collect_due {
            return Ok(());
        }
        self.collect_due = false;
        self.collect_now()?;
        self.check_heap_bounds()
    }
    /** After a collection, objects and functions must be back under their bounds. */
    pub(in crate::v9) fn check_heap_bounds(&mut self) -> Res<()> {
        let counts = self.heap.counts;
        if counts.functions >= self.limits.functions {
            return Err(EngineError::limit("V9 functions", self.limits.functions).into());
        }
        if counts.objects >= self.limits.objects {
            return Err(EngineError::limit("V9 heap objects", self.limits.objects).into());
        }
        Ok(())
    }
    /** Collect when any live count reached its bound; fail if it is still over afterwards. */
    pub fn collect_if_needed(&mut self) -> Res<()> {
        if let Some(units) = self.heap.string_budget.refresh_materialization() {
            self.heap.last_registered_string_units = units;
        }
        let counts = self.heap.counts;
        let limits = self.limits;
        #[cfg(feature = "conformance")]
        let units = counts
            .units
            .saturating_add(self.page.state.cpu_profile.units());
        #[cfg(not(feature = "conformance"))]
        let units = counts.units;
        // Global let, const and class bindings live outside the heap but are bindings too.
        if counts.properties > limits.properties
            || units > limits.units
            || self.heap.last_registered_string_units > limits.units
            || counts.environments >= limits.environments
            || counts.bindings + self.global_binding_count() > limits.bindings
        {
            self.collect_now()?;
            let counts = self.heap.counts;
            #[cfg(feature = "conformance")]
            let units = counts
                .units
                .saturating_add(self.page.state.cpu_profile.units());
            #[cfg(not(feature = "conformance"))]
            let units = counts.units;
            let bindings = counts.bindings + self.global_binding_count();
            for (count, limit, name) in [
                (
                    counts.properties,
                    limits.properties,
                    "V9 aggregate own properties",
                ),
                (units, limits.units, "V9 environment UTF-16 units"),
                (self.heap.last_registered_string_units, limits.units, "V9 resident string units"),
                (counts.environments, limits.environments, "V9 environments"),
                (bindings, limits.bindings, "V9 bindings"),
            ] {
                if count > limit {
                    return Err(EngineError::limit(name, limit).into());
                }
            }
        }
        Ok(())
    }
}
