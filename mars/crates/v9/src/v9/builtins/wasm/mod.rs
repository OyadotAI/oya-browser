/** Owned, bounded WebAssembly binary validation and scalar execution over V9's serializable heap. */
use super::*;
use crate::v9::vm::Thrown;
use crate::v9::vm::data::{Bytes, Class, PromiseState};
use crate::v9::vm::heap::{HIDDEN_ATTRS, Object, Prop, Slot};
use crate::v9::vm::property::Descriptor;
use crate::v9::vm::realm::{NativeFn, realm, string};
mod address;
mod async_api;
mod atomic;
mod binary;
mod compile_limits;
mod constants;
mod diagnostics;
mod exceptions;
#[cfg(feature = "conformance")]
pub(in crate::v9) use exceptions::{exception_tag_id, exception_values};
mod execute;
mod gc;
mod gc_types;
mod global;
mod imports;
mod instance;
mod jspi;
mod memory;
#[cfg(feature = "conformance")]
pub(in crate::v9) use memory::force_detach as wasm_force_detach;
mod numeric;
mod prototypes;
mod roots;
mod segments;
#[cfg(feature = "conformance")]
mod serialization;
mod shared_memory;
mod simd;
mod streaming;
mod strings;
mod table;
mod trace;
pub(in crate::v9) use trace::trace_metadata;
mod type_context;
mod types;
mod validate;
#[cfg(feature = "conformance")]
mod worker;
use constants::*;
use types::{Error, Module, Type};
#[cfg(feature = "conformance")]
pub(in crate::v9) use worker::{
    WasmMemoryPacket, WasmModulePacket, wasm_worker_memory, wasm_worker_memory_buffer,
    wasm_worker_module, wasm_worker_nonshared_gc, wasm_worker_opaque, wasm_worker_restore, wasm_worker_restore_memory,
};

/** Static identities do not hold page objects or authority. */
#[derive(Clone, Copy)]
struct Ids {
    module: ObjectId,
    instance: ObjectId,
    memory: ObjectId,
    table: ObjectId,
    global: ObjectId,
    tag: ObjectId,
    exception: ObjectId,
    js_tag: ObjectId,
    suspending: ObjectId,
    suspend_error: ObjectId,
    promise_invoke: u32,
    async_resume: u32,
    instantiate_received: u32,
    stream_received: u32,
    prototype_method: u32,
    prototype_constructor: u32,
    compile_error: ObjectId,
    link_error: ObjectId,
    runtime_error: ObjectId,
    invoke: u32,
    builtin: u32,
}
/** Every page uses the same immutable intrinsic identities; values live only in that page's heap. */
static IDS: std::sync::OnceLock<Ids> = std::sync::OnceLock::new();
/** Installed identities, available after the realm finishes construction. */
fn ids(vm: &Vm) -> Ids {
    let ids = *IDS.get().expect("WebAssembly intrinsics installed");
    Ids {
        module: vm.intrinsic_id(ids.module),
        instance: vm.intrinsic_id(ids.instance),
        memory: vm.intrinsic_id(ids.memory),
        table: vm.intrinsic_id(ids.table),
        global: vm.intrinsic_id(ids.global),
        tag: vm.intrinsic_id(ids.tag),
        exception: vm.intrinsic_id(ids.exception),
        js_tag: vm.intrinsic_id(ids.js_tag),
        suspending: vm.intrinsic_id(ids.suspending),
        suspend_error: vm.intrinsic_id(ids.suspend_error),
        promise_invoke: ids.promise_invoke,
        async_resume: ids.async_resume,
        instantiate_received: ids.instantiate_received,
        stream_received: ids.stream_received,
        prototype_method: ids.prototype_method,
        prototype_constructor: ids.prototype_constructor,
        compile_error: vm.intrinsic_id(ids.compile_error),
        link_error: vm.intrinsic_id(ids.link_error),
        runtime_error: vm.intrinsic_id(ids.runtime_error),
        invoke: ids.invoke,
        builtin: ids.builtin,
    }
}

/** Install the supported capabilities; none delegates to a foreign VM or native filesystem. */
pub(super) fn install(b: &mut Builder) {
    let namespace = b.object(Some(b.ids.object_prototype));
    b.global("WebAssembly", namespace);
    tag(b, namespace, "WebAssembly");
    let module = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("Module", 1, module_new, module, None);
    b.alias(namespace, "Module", ctor);
    for (name, arity, body) in [
        ("imports", 1, module_imports as NativeFn),
        ("exports", 1, module_exports),
        ("customSections", 2, custom_sections),
    ] {
        let method = b.method(ctor, name, arity, body);
        b.value(
            ctor,
            name,
            JsValue::Object(method),
            HIDDEN_ATTRS | crate::v9::vm::heap::ENUMERABLE,
        );
    }
    let instance = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("Instance", 1, instance::construct, instance, None);
    b.alias(namespace, "Instance", ctor);
    b.builtin_accessor_with_attrs(
        instance,
        "exports",
        Some(instance::exports),
        None,
        crate::v9::vm::heap::CONFIGURABLE | crate::v9::vm::heap::ENUMERABLE,
    );
    let memory = memory::install(b, namespace);
    let table = table::install(b, namespace);
    let global = global::install(b, namespace);
    let (tag, exception, js_tag) = exceptions::install(b, namespace);
    let (suspending, promise_invoke) = jspi::install(b, namespace);
    for (proto, name) in [
        (module, "WebAssembly.Module"),
        (instance, "WebAssembly.Instance"),
        (memory, "WebAssembly.Memory"),
        (table, "WebAssembly.Table"),
        (global, "WebAssembly.Global"),
        (tag, "WebAssembly.Tag"),
        (exception, "WebAssembly.Exception"),
        (suspending, "WebAssembly.Suspending"),
    ] {
        self::tag(b, proto, name);
    }
    let stream_received = streaming::install(b, namespace);
    let (prototype_method, prototype_constructor) = prototypes::install(b);
    let suspend_error = error_class(b, namespace, "SuspendError");
    let compile_error = error_class(b, namespace, "CompileError");
    let link_error = error_class(b, namespace, "LinkError");
    let runtime_error = error_class(b, namespace, "RuntimeError");
    for (name, arity, body) in [
        ("validate", 1, validate_api as NativeFn),
        ("compile", 1, compile_api),
        ("instantiate", 1, instantiate_api),
    ] {
        let method = b.method(namespace, name, arity, body);
        b.value(
            namespace,
            name,
            JsValue::Object(method),
            HIDDEN_ATTRS | crate::v9::vm::heap::ENUMERABLE,
        );
    }
    let _ = IDS.set(Ids {
        module,
        instance,
        memory,
        table,
        global,
        tag,
        exception,
        js_tag,
        suspending,
        suspend_error,
        promise_invoke,
        async_resume: b.native("", 1, execute::resume_promising),
        instantiate_received: async_api::install(b),
        stream_received,
        prototype_method,
        prototype_constructor,
        compile_error,
        link_error,
        runtime_error,
        invoke: b.native("", 0, instance::invoke),
        builtin: b.native("", 0, imports::invoke),
    });
}

/** Standard tags are configurable data values and never depend on an instance's private branding. */
fn tag(b: &mut Builder, target: ObjectId, name: &'static str) {
    b.put(
        target,
        Key::Sym(crate::v9::vm::realm::symbol::TO_STRING_TAG),
        crate::v9::vm::realm::Static::Value(string(name)),
        crate::v9::vm::heap::CONFIGURABLE,
    );
}

/** Wasm's three error classes preserve Error.prototype and instanceof behavior. */
fn error_class(b: &mut Builder, namespace: ObjectId, name: &'static str) -> ObjectId {
    let proto = b.object(Some(b.ids.error_prototype));
    let ctor = b.constructor(name, 1, error_new, proto, None);
    b.value(proto, "name", string(name), HIDDEN_ATTRS);
    b.value(proto, "message", string(""), HIDDEN_ATTRS);
    b.alias(namespace, name, ctor);
    proto
}
/** Construct an Error subtype with authored subclass prototypes intact. */
fn error_new(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let ctor = call
        .new_target
        .as_ref()
        .cloned()
        .unwrap_or(JsValue::Object(call.callee));
    let proto = vm.prototype_for(&ctor, vm.intrinsic_id(realm().ids.error_prototype))?;
    let object = vm.alloc(Object::new(Class::Error, Some(proto)))?;
    if call.arg(0) != JsValue::Undefined {
        let message = arg_string(vm, &call, 0)?;
        vm.heap.put(
            object,
            key("message"),
            Prop {
                slot: Slot::Data(JsValue::String(message)),
                attrs: HIDDEN_ATTRS,
            },
        );
    }
    vm.record_trace(object)?;
    Ok(JsValue::Object(object))
}
/** Throw a genuine Wasm error object without executing any user-controlled constructor. */
fn failure(vm: &mut Vm, proto: ObjectId, message: &str) -> Thrown {
    match vm.alloc(Object::new(Class::Error, Some(proto))) {
        Ok(object) => match JsValue::string(message) {
            Ok(value) => {
                vm.heap.put(
                    object,
                    key("message"),
                    Prop {
                        slot: Slot::Data(value),
                        attrs: HIDDEN_ATTRS,
                    },
                );
                match vm.record_trace(object) {
                    Ok(()) => Thrown::Value(JsValue::Object(object)),
                    Err(error) => error.into(),
                }
            }
            Err(error) => error.into(),
        },
        Err(error) => error,
    }
}
/** Unsupported proposals stay explicit instead of being misreported as malformed binaries. */
fn compile_failure(vm: &mut Vm, error: Error) -> Thrown {
    if error.unsupported {
        vm.unsupported_error(&error.message)
    } else {
        let error = failure(vm, ids(vm).compile_error, &error.render());
        if let Thrown::Value(JsValue::Object(id)) = &error {
            set(vm, *id, "wasm.compile.native", JsValue::Boolean(true));
        }
        error
    }
}
/** A Wasm trap is catchable and distinguishable from a JS TypeError or a compile failure. */
fn trap(vm: &mut Vm, message: &str) -> Thrown {
    let error = failure(vm, ids(vm).runtime_error, message);
    if let Thrown::Value(JsValue::Object(id)) = &error {
        set(vm, *id, "wasm.trap", JsValue::Boolean(true));
    }
    error
}
/** The test intrinsic throws a real stack overflow that Wasm catch_all cannot intercept. */
#[cfg(feature = "conformance")]
pub(in crate::v9) fn forced_stack_overflow(vm: &mut Vm) -> Thrown {
    let error = vm.range_error("Maximum call stack size exceeded");
    if let Thrown::Value(JsValue::Object(id)) = &error {
        set(vm, *id, "wasm.trap", JsValue::Boolean(true));
    }
    error
}
/** Import failures are link errors; user getter exceptions are propagated before this helper. */
fn link(vm: &mut Vm, message: &str) -> Thrown {
    failure(vm, ids(vm).link_error, message)
}

/** A private inert field cannot be spelled or enumerated by page scripts. */
fn set(vm: &mut Vm, id: ObjectId, name: &str, value: JsValue) {
    vm.heap.put(
        id,
        Key::private(name),
        Prop {
            slot: Slot::Data(value),
            attrs: 0,
        },
    );
}
/** Read an inert private record without author getters or prototype lookup. */
fn get(vm: &Vm, id: ObjectId, name: &str) -> Option<JsValue> {
    vm.heap
        .get(id)?
        .props
        .get(&Key::private(name))
        .and_then(|p| match &p.slot {
            Slot::Data(v) => Some(v.clone()),
            _ => None,
        })
}
/** Every native receiver checks its private brand instead of trusting the prototype chain. */
fn brand(vm: &mut Vm, value: &JsValue, name: &str) -> Res<ObjectId> {
    value
        .object()
        .filter(|id| get(vm, *id, name).is_some())
        .ok_or_else(|| vm.type_error("incompatible WebAssembly receiver"))
}
/** BufferSource accepts actual buffers and actual views, preserving their byte offset and size. */
fn bytes(vm: &mut Vm, value: &JsValue) -> Res<Vec<u8>> {
    let Some(id) = value.object() else {
        return Err(buffer_source_error(vm));
    };
    let range = match vm.heap.get(id).map(|o| &o.class) {
        Some(Class::ArrayBuffer(bytes)) => return Ok(bytes.0.clone()),
        Some(Class::SparseWasmBuffer(bytes)) => {
            return bytes.read(0, bytes.length).ok_or_else(|| {
                crate::protocol::EngineError::limit("V9 buffer resident bytes", BUFFER_BYTES).into()
            });
        }
        Some(Class::View(_)) => {
            let view = super::view_of(vm, id).unwrap();
            if view.invalid {
                return Ok(vec![]);
            }
            Some((
                view.buffer,
                view.offset,
                view.length * super::typed::element_size(view.kind),
            ))
        }
        _ => None,
    };
    let Some((buffer, offset, length)) = range else {
        return Err(buffer_source_error(vm));
    };
    match vm.heap.get(buffer).map(|o| &o.class) {
        Some(Class::ArrayBuffer(bytes)) => bytes
            .0
            .get(offset..offset.saturating_add(length))
            .map(<[u8]>::to_vec)
            .ok_or_else(|| vm.type_error("detached WebAssembly input buffer")),
        Some(Class::SharedArrayBuffer(bytes)) => bytes
            .read(offset, length)
            .ok_or_else(|| vm.type_error("out-of-bounds WebAssembly input buffer")),
        Some(Class::SparseWasmBuffer(bytes)) => bytes.read(offset, length).ok_or_else(|| {
            crate::protocol::EngineError::limit("V9 buffer resident bytes", BUFFER_BYTES).into()
        }),
        _ => Err(vm.type_error("invalid WebAssembly input buffer")),
    }
}
/** Native argument failures get their own API prefix without rewriting authored exceptions. */
fn buffer_source_error(vm: &mut Vm) -> Thrown {
    let error = vm.type_error("Argument 0 must be a buffer source");
    if let Thrown::Value(JsValue::Object(id)) = &error {
        set(vm, *id, "wasm.compile.native", JsValue::Boolean(true));
    }
    error
}
/** Byte buffers use the same concrete type and quota as every JS typed array. */
fn buffer(vm: &mut Vm, bytes: Vec<u8>) -> Res<ObjectId> {
    if bytes.len() > BUFFER_BYTES {
        return Err(vm.range_error("WebAssembly buffer exceeds V9 buffer limit"));
    }
    vm.alloc(Object::new(
        Class::ArrayBuffer(Bytes(bytes)),
        Some(vm.intrinsic_id(realm().ids.array_buffer_prototype)),
    ))
}
/** Revalidate serialized bytes every time they enter native execution. */
fn module(vm: &mut Vm, value: &JsValue) -> Res<(ObjectId, Module)> {
    let object = brand(vm, value, MODULE)?;
    let record = get(vm, object, MODULE).unwrap();
    let bytes = bytes(vm, &record)?;
    let mut module = binary::parse(&bytes).map_err(|e| compile_failure(vm, e))?;
    module.wasmfx_enabled = get(vm, object, "wasm.feature.wasmfx") == Some(JsValue::Boolean(true));
    Ok((object, module))
}
/** Construct one immutable module from a byte copy, retaining no compiled code in page state. */
fn make_module(
    vm: &mut Vm,
    input: &JsValue,
    options: &JsValue,
    new_target: Option<&JsValue>,
) -> Res<ObjectId> {
    let bytes = bytes(vm, input)?;
    if bytes.is_empty() {
        return Err(compile_failure(
            vm,
            Error::invalid("BufferSource argument is empty"),
        ));
    }
    compile_limits::check(vm, bytes.len(), false)?;
    vm.charge(bytes.len())?;
    let options = imports::options(vm, options)?;
    finish_module(vm, bytes, options, new_target)
}
/** Shell conformance uses the test's explicit experimental feature flags. */
fn stringref_enabled(vm: &Vm) -> bool {
    #[cfg(feature = "conformance")]
    {
        return !vm.page.environment.conformance_d8
            || vm.page.environment.conformance_wasm_stringref;
    }
    #[cfg(not(feature = "conformance"))]
    {
        let _ = vm;
        true
    }
}
/** The d8 proposal is enabled only by its feature flag or an implying interop flag. */
fn custom_descriptors_enabled(vm: &Vm) -> bool {
    #[cfg(feature = "conformance")]
    {
        return !vm.page.environment.conformance_d8
            || vm.page.environment.conformance_wasm_custom_descriptors;
    }
    #[cfg(not(feature = "conformance"))]
    {
        let _ = vm;
        true
    }
}
/** Shell test metadata controls acquire/release binary encodings. */
fn acquire_release_enabled(vm: &Vm) -> bool {
    #[cfg(feature = "conformance")]
    {
        return !vm.page.environment.conformance_d8
            || vm.page.environment.conformance_wasm_acquire_release;
    }
    #[cfg(not(feature = "conformance"))]
    {
        let _ = vm;
        true
    }
}
/** Shell metadata controls scalar and SIMD binary16 instructions. */
fn fp16_enabled(vm: &Vm) -> bool {
    #[cfg(feature = "conformance")]
    {
        return !vm.page.environment.conformance_d8 || vm.page.environment.conformance_wasm_fp16;
    }
    #[cfg(not(feature = "conformance"))]
    {
        let _ = vm;
        true
    }
}
/** Interop is independently gated from custom descriptor binary syntax. */
fn js_interop_enabled(vm: &Vm) -> bool {
    #[cfg(feature = "conformance")]
    {
        return !vm.page.environment.conformance_d8
            || vm.page.environment.conformance_wasm_js_interop;
    }
    #[cfg(not(feature = "conformance"))]
    {
        let _ = vm;
        true
    }
}
/** Shell metadata controls the wide-integer Wasm proposal. */
fn wide_arithmetic_enabled(vm: &Vm) -> bool {
    #[cfg(feature = "conformance")]
    {
        return !vm.page.environment.conformance_d8
            || vm.page.environment.conformance_wasm_wide_arithmetic;
    }
    #[cfg(not(feature = "conformance"))]
    {
        let _ = vm;
        true
    }
}
/** Mixed exception encodings are available only under the testing flag. */
fn allow_mixed_eh(vm: &Vm) -> bool {
    #[cfg(feature = "conformance")]
    {
        return vm.page.environment.conformance_d8
            && vm.page.environment.conformance_wasm_allow_mixed_eh;
    }
    #[cfg(not(feature = "conformance"))]
    {
        let _ = vm;
        false
    }
}
/** Stack-switching forms are experimental and remain disabled outside the test host. */
fn wasmfx_enabled(vm: &Vm) -> bool {
    #[cfg(feature = "conformance")]
    {
        return vm.page.environment.conformance_d8 && vm.page.environment.conformance_wasm_wasmfx;
    }
    #[cfg(not(feature = "conformance"))]
    {
        let _ = vm;
        false
    }
}
/** Instance behavior follows the module's saved proposal state even after host restoration. */
fn wasmfx_instance_enabled(vm: &Vm, instance: ObjectId) -> bool {
    get(vm, instance, INSTANCE)
        .and_then(|value| value.object())
        .and_then(|module| get(vm, module, "wasm.feature.wasmfx"))
        == Some(JsValue::Boolean(true))
}
/** A reentrant JS call must also observe an active WasmFX stack owned by another instance. */
fn wasmfx_active(vm: &Vm) -> bool {
    vm.native_activations.iter().any(|entry| match entry.kind {
        crate::v9::vm::NativeKind::Wasm { instance, .. } => wasmfx_instance_enabled(vm, instance),
        _ => false,
    })
}
/** Shared-everything syntax is controlled by the shell feature, distinct from ordinary shared memory. */
fn shared_enabled(vm: &Vm) -> bool {
    #[cfg(feature = "conformance")]
    { return !vm.page.environment.conformance_d8 || vm.page.environment.conformance_wasm_shared; }
    #[cfg(not(feature = "conformance"))]
    { let _ = vm; true }
}
/** The shell enables compact grouped imports only under its proposal flag. */
fn compact_imports_enabled(vm: &Vm) -> bool {
    #[cfg(feature = "conformance")]
    { return !vm.page.environment.conformance_d8 || vm.page.environment.conformance_wasm_compact_imports; }
    #[cfg(not(feature = "conformance"))]
    { let _ = vm; true }
}
/** Host test limits can only lower the native table declaration bound. */
fn table_limit(vm: &Vm) -> u64 {
    #[cfg(feature = "conformance")]
    if let Some(limit) = vm.page.environment.conformance_wasm_max_table_entries {
        return (limit as u64).min(MAX_DECLARED_TABLE_MINIMUM);
    }
    let _ = vm;
    MAX_DECLARED_TABLE_MINIMUM
}
/** Copied inputs and inert options are the only accepted module construction state. */
fn finish_module(
    vm: &mut Vm,
    bytes: Vec<u8>,
    options: (Vec<JsValue>, Option<JsValue>),
    new_target: Option<&JsValue>,
) -> Res<ObjectId> {
    compile_limits::check(vm, bytes.len(), false)?;
    let parsed = binary::parse_with_features(
        &bytes,
        stringref_enabled(vm),
        custom_descriptors_enabled(vm),
        acquire_release_enabled(vm),
        fp16_enabled(vm),
        wide_arithmetic_enabled(vm),
        allow_mixed_eh(vm),
        wasmfx_enabled(vm),
        shared_enabled(vm),
        table_limit(vm),
        compact_imports_enabled(vm),
    )
    .map_err(|e| compile_failure(vm, e))?;
    let proto = match new_target {
        Some(t) => vm.prototype_for(t, ids(vm).module)?,
        None => ids(vm).module,
    };
    let object = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    let bytes = buffer(vm, bytes)?;
    set(vm, object, MODULE, JsValue::Object(bytes));
    set(
        vm,
        object,
        "wasm.feature.wasmfx",
        JsValue::Boolean(parsed.wasmfx_enabled),
    );
    imports::store(vm, object, options)?;
    imports::validate(vm, object, &parsed)?;
    Ok(object)
}
/** Fuzzing intrinsics create the same typed one-element i64 Wasm GC records as V8. */
#[cfg(feature = "conformance")]
pub(in crate::v9) fn wasm_intrinsic_object(vm: &mut Vm, array: bool) -> Res<JsValue> {
    const STRUCT: &[u8] = &[0,97,115,109,1,0,0,0,1,7,1,80,0,95,1,126,0];
    const ARRAY: &[u8] = &[0,97,115,109,1,0,0,0,1,6,1,80,0,94,126,0];
    const PAYLOAD: i128 = 0x7aad_f00d_baad_f00d;
    let mark = vm.pins.len();
    let result = (|| {
        let module = finish_module(vm, if array { ARRAY } else { STRUCT }.to_vec(), (vec![], None), None)?;
        vm.pin(JsValue::Object(module));
        gc::allocate_record(vm, JsValue::Object(module), 0, array, vec![JsValue::BigInt(PAYLOAD)])
    })();
    vm.pins.truncate(mark);
    result
}
/** new WebAssembly.Module consumes a BufferSource, never a JS iterable masquerading as bytes. */
fn module_new(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(vm.type_error("WebAssembly.Module must be invoked with 'new'"));
    }
    make_module(vm, &call.arg(0), &call.arg(1), call.new_target.as_ref())
        .map(JsValue::Object)
        .map_err(|e| api_prefix(vm, e, "WebAssembly.Module()"))
}
/** validate reports invalid binaries as false, while unsupported recognized proposals are explicit. */
fn validate_api(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let bytes = bytes(vm, &call.arg(0))?;
    if bytes.is_empty() || bytes.len() > compile_limits::maximum(vm) {
        return Ok(JsValue::Boolean(false));
    }
    vm.charge(bytes.len())?;
    let options = imports::options(vm, &call.arg(1))?;
    match binary::parse_with_features(
        &bytes,
        stringref_enabled(vm),
        custom_descriptors_enabled(vm),
        acquire_release_enabled(vm),
        fp16_enabled(vm),
        wide_arithmetic_enabled(vm),
        allow_mixed_eh(vm),
        wasmfx_enabled(vm),
        shared_enabled(vm),
        table_limit(vm),
        compact_imports_enabled(vm),
    ) {
        Ok(m) => {
            let object = vm.new_object()?;
            imports::store(vm, object, options)?;
            match imports::validate(vm, object, &m) {
                Ok(()) => Ok(JsValue::Boolean(true)),
                Err(Thrown::Value(_)) => Ok(JsValue::Boolean(false)),
                Err(e) => Err(e),
            }
        }
        Err(e) if !e.unsupported => Ok(JsValue::Boolean(false)),
        Err(e) => Err(compile_failure(vm, e)),
    }
}
/** Promise settlement uses V9's actual reaction queue; fatal quota errors remain fatal. */
fn promised(vm: &mut Vm, result: Res<JsValue>) -> Res<JsValue> {
    let promise = vm.new_promise()?;
    match result {
        Ok(value) => vm.resolve_promise(promise, value)?,
        Err(Thrown::Value(value)) => vm.settle(promise, PromiseState::Rejected(value))?,
        Err(fatal) => return Err(fatal),
    }
    Ok(JsValue::Object(promise))
}
/** compile resolves to a real validated Module and rejects on binary errors. */
fn compile_api(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = make_module(vm, &call.arg(0), &call.arg(1), None)
        .map(JsValue::Object)
        .map_err(|e| api_prefix(vm, e, "WebAssembly.compile()"));
    promised(vm, result)
}
/** instantiate supports either an existing Module or bytes, with the specified result shape. */
fn instantiate_api(vm: &mut Vm, call: Call) -> Res<JsValue> {
    async_api::instantiate(vm, call)
}
/** Exports retain the binary's declaration order. */
fn module_exports(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (_, m) = introspection_module(vm, &call.arg(0))?;
    let mut values = Vec::new();
    for export in m.exports {
        let object = vm.new_object()?;
        let name = JsValue::string(&export.name)?;
        vm.put(object, "name", name)?;
        vm.put(object, "kind", JsValue::string(kind_name(export.kind))?)?;
        values.push(JsValue::Object(object))
    }
    Ok(JsValue::Object(vm.new_array(values)?))
}
/** Imports describe the actual module/name/kind triples the linker will resolve. */
fn module_imports(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (module_id, m) = introspection_module(vm, &call.arg(0))?;
    let mut values = Vec::new();
    for import in m.imports {
        if imports::selected(vm, module_id, &import) {
            continue;
        }
        let object = vm.new_object()?;
        vm.put(object, "module", JsValue::string(&import.module)?)?;
        vm.put(object, "name", JsValue::string(&import.name)?)?;
        vm.put(
            object,
            "kind",
            JsValue::string(match import.kind {
                types::External::Function(_) => "function",
                types::External::Table(..) => "table",
                types::External::Memory(_) => "memory",
                types::External::Global(..) => "global",
                types::External::Tag(..) => "tag",
            })?,
        )?;
        values.push(JsValue::Object(object))
    }
    Ok(JsValue::Object(vm.new_array(values)?))
}
/** Custom section payloads are fresh ArrayBuffers, never aliases of the module's private bytes. */
fn custom_sections(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !call
        .arg(0)
        .object()
        .is_some_and(|id| get(vm, id, MODULE).is_some())
    {
        return Err(vm.type_error(
            "WebAssembly.Module.customSections(): Argument 0 must be a WebAssembly.Module",
        ));
    }
    let (_, m) = introspection_module(vm, &call.arg(0))?;
    let name = arg_string(vm, &call, 1)?.lossy();
    let mut values = Vec::new();
    for (_, bytes) in m.custom.into_iter().filter(|(n, _)| *n == name) {
        values.push(JsValue::Object(buffer(vm, bytes)?))
    }
    Ok(JsValue::Object(vm.new_array(values)?))
}
/** Static introspection checks its module argument without consulting authored properties. */
fn introspection_module(vm: &mut Vm, value: &JsValue) -> Res<(ObjectId, Module)> {
    if !value
        .object()
        .is_some_and(|id| get(vm, id, MODULE).is_some())
    {
        return Err(vm.type_error("Argument 0 must be a WebAssembly.Module"));
    }
    module(vm, value)
}
/** External kind names are shared by introspection and exports. */
fn kind_name(kind: u8) -> &'static str {
    match kind {
        0 => "function",
        1 => "table",
        2 => "memory",
        3 => "global",
        4 => "tag",
        _ => "unknown",
    }
}

/** Install the explicitly requested conformance host capability after the public Wasm intrinsics. */
#[cfg(feature = "conformance")]
pub(super) fn install_conformance(b: &mut Builder, d8: ObjectId) {
    serialization::install(b, d8)
}

/** Prefix only engine-produced compile errors once; author getter exceptions retain their exact identity. */
fn api_prefix(vm: &mut Vm, error: Thrown, api: &str) -> Thrown {
    if let Thrown::Value(JsValue::Object(id)) = &error
        && get(vm, *id, "wasm.compile.native") == Some(JsValue::Boolean(true))
    {
        if let Some(JsValue::String(message)) = vm
            .heap
            .get(*id)
            .and_then(|o| o.props.get(&key("message")))
            .and_then(|p| match &p.slot {
                Slot::Data(value) => Some(value.clone()),
                _ => None,
            })
            && let Ok(message) = JsValue::string(&format!("{api}: {}", message.lossy()))
        {
            vm.heap.put(
                *id,
                key("message"),
                Prop {
                    slot: Slot::Data(message),
                    attrs: HIDDEN_ATTRS,
                },
            );
            set(vm, *id, "wasm.compile.native", JsValue::Boolean(false));
        }
    }
    error
}

/** Every restored shared graph is typed and bounded before it can be used by scripts. */
pub(in crate::v9) fn validate_shared_gc(
    heap: &crate::v9::vm::heap::Heap,
) -> crate::protocol::EngineResult<()> {
    gc::shared::validate(heap)
}
/** Mixed shared arenas validate every Wasm record before cross-domain restoration. */
#[cfg(feature = "conformance")]
pub(in crate::v9) fn validate_shared_wasm_arena(arena: &crate::v9::vm::data::SharedGcArena) -> crate::protocol::EngineResult<()> {
    gc::shared::validate_arena(arena)
}
/** Native sharing is restored only through grants; all other decoded graphs remain owned inert copies. */
pub(in crate::v9) fn initialize_shared_gc(vm: &mut Vm) -> crate::protocol::EngineResult<()> {
    gc::shared::initialize(vm)
}
/** Native Worker packets carry an actual root handle and create a separately branded receiving wrapper. */
#[cfg(feature = "conformance")]
pub(in crate::v9) fn wasm_worker_restore_shared_gc(
    vm: &mut Vm,
    root: &crate::v9::vm::data::SharedGc,
) -> Res<JsValue> {
    gc::shared::wrap(vm, root)
}

/** Structured cloning preserves a shared node's real native graph and local wrapper identity. */
pub(super) fn restore_shared_gc(vm: &mut Vm, root: &crate::v9::vm::data::SharedGc) -> Res<JsValue> {
    gc::shared::wrap(vm, root)
}

/** Resizing an engine-owned ArrayBuffer preserves its memory's page and maximum invariants. */
pub(super) fn resize_memory_buffer(vm: &mut Vm, buffer: ObjectId, length: usize) -> Res<bool> {
    memory::resize_buffer(vm, buffer, length)
}
