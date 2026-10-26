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
pub(in crate::v9) mod convert;
pub(in crate::v9) mod data;
mod dynamic;
pub(in crate::v9) mod error;
pub(in crate::v9) mod function_name;
pub(in crate::v9) mod gc;
pub(in crate::v9) mod heap;
pub(in crate::v9) mod iter;
#[cfg(feature = "conformance")]
pub(in crate::v9) mod memory;
pub(in crate::v9) mod private_elements;
pub(in crate::v9) mod property;
pub(in crate::v9) mod proxy;
pub(in crate::v9) mod realm;
pub(in crate::v9) mod realms;
pub(in crate::v9) mod run;
pub(in crate::v9) mod shape;
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
}

/** The stored form: plain text for ordinary code (the older form too), a pair for direct eval. */
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
enum DynamicWire {
    At(String, Option<String>, usize),
    Plain(String),
    Eval(String, String),
}

impl From<DynamicWire> for DynamicSource {
    /** Either stored form. */
    fn from(wire: DynamicWire) -> Self {
        match wire {
            DynamicWire::At(source, context, base) => Self {
                source,
                context,
                base: Some(base),
            },
            DynamicWire::Plain(source) => Self {
                source,
                context: None,
                base: None,
            },
            DynamicWire::Eval(source, context) => Self {
                source,
                context: Some(context),
                base: None,
            },
        }
    }
}

impl serde::Serialize for DynamicSource {
    /** Plain text unless it carries a context. */
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
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
    pub frames: Vec<Frame>,
    /** A prepareStackTrace hook cannot recursively invoke another formatting hook in this VM. */
    pub formatting_stack: bool,
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
    /** Whether any Proxy exists, so ordinary property access skips the proxy check entirely. */
    pub proxies: bool,
    /** The page's module map: compiled module sources and their committed records. */
    pub modules: super::modules::Modules,
}

/** The global object's fixed identity. */
pub(in crate::v9) const GLOBAL: ObjectId = crate::constants::VM_GLOBAL_OBJECT_ID;

impl<'h> Vm<'h> {
    /** A VM over a heap, with the global object created if missing. */
    pub fn new(heap: Heap, page: Page<'h>, limits: Limits) -> Self {
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
            frames: Vec::new(),
            formatting_stack: false,
            pins: Vec::new(),
            kept_weak: FastMap::default(),
            pinning: false,
            collect_due: false,
            native_depth: 0,
            remaining: limits.operations,
            deadline: None,
            jobs: VecDeque::new(),
            suspended: None,
            regexps: FastMap::default(),
            unsupported: None,
            limits,
            page,
            dynamic: Vec::new(),
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
                realm::realm().ids.object_prototype
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
                "Reduce the script work; that script's changes were not applied.",
            ))
        })?;
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
        let counts = self.heap.counts;
        let limits = self.limits;
        // Global let, const and class bindings live outside the heap but are bindings too.
        if counts.properties > limits.properties
            || counts.units > limits.units
            || counts.environments >= limits.environments
            || counts.bindings + self.global_binding_count() > limits.bindings
        {
            self.collect_now()?;
            let counts = self.heap.counts;
            let bindings = counts.bindings + self.global_binding_count();
            for (count, limit, name) in [
                (
                    counts.properties,
                    limits.properties,
                    "V9 aggregate own properties",
                ),
                (counts.units, limits.units, "V9 environment UTF-16 units"),
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
