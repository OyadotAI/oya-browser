/** d8's extras binding validates trace events like V8's builtin and reports whether their category is enabled. */
use super::*;
use crate::v9::builtins::json::stringify;
use crate::v9::value::ObjectId;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{Key, Object, Prop, Slot};

/** The only category enabled by `--enable-tracing` in V9; other categories report disabled. */
const ENABLED_CATEGORY: &str = "v8";
/** Private slot on the realm's global, so every call in one realm returns the same binding object. */
const BINDING_KEY: &str = "extras-binding";
/** V8's TypeError for a non-string category, raised before any other argument is read. */
const CATEGORY_ERROR: &str = "Trace event category must be a string.";
/** V8's TypeError for a phase that is not a number. */
const PHASE_ERROR: &str = "Trace event phase must be a number.";
/** V8's TypeError for a name that is not a string. */
const NAME_ERROR: &str = "Trace event name must be a string.";
/** V8's TypeError for an id that is neither nullish nor a number. */
const ID_ERROR: &str = "Trace event id must be a number.";
/** V8's TypeError for an empty name. */
const NAME_LENGTH_ERROR: &str = "Trace event name must not be an empty string.";

/** Native identities are installed once; each realm's binding closes over them through native closures. */
static TRACE: OnceLock<u32> = OnceLock::new();
/** The category query is a separate native so its arity and errors match `isTraceCategoryEnabled`. */
static CATEGORY: OnceLock<u32> = OnceLock::new();

/** Registers the binding accessor on the d8 namespace and the two natives it hands out. */
pub(super) fn install(builder: &mut Builder, d8: ObjectId) {
    let _ = TRACE.set(builder.native("trace", 5, trace));
    let _ = CATEGORY.set(builder.native("isTraceCategoryEnabled", 1, is_enabled));
    builder.method(d8, "getExtrasBindingObject", 0, binding);
}

/** A category is enabled only under the shell's tracing flag, and never while fuzzing suppresses tracing. */
fn enabled(vm: &Vm, category: &str) -> bool {
    let environment = &vm.page.environment;
    environment.conformance_enable_tracing
        && !environment.conformance_fuzzing
        && category == ENABLED_CATEGORY
}

/** The realm's binding is created on first use and then reused, as V8's per-context extras object is. */
fn binding(vm: &mut Vm, _call: Call) -> Res<JsValue> {
    let global = vm.global_object();
    if let Some(existing) = private_binding(vm, global) {
        return Ok(JsValue::Object(existing));
    }
    let object = vm.alloc(Object::new(Class::Plain, None))?;
    for (name, index) in [
        ("trace", *TRACE.get().unwrap()),
        ("isTraceCategoryEnabled", *CATEGORY.get().unwrap()),
    ] {
        let function = vm.native_closure(index, Vec::new())?;
        vm.heap.put(
            object,
            Key::from(name),
            Prop::data(JsValue::Object(function)),
        );
    }
    vm.heap.put(
        global,
        Key::private(BINDING_KEY),
        Prop::data(JsValue::Object(object)),
    );
    vm.check_heap_bounds()?;
    Ok(JsValue::Object(object))
}

/** Reads the realm's stored binding without invoking any script-visible accessor. */
fn private_binding(vm: &Vm, global: ObjectId) -> Option<ObjectId> {
    let prop = vm.heap.get(global)?.props.get(&Key::private(BINDING_KEY))?;
    match &prop.slot {
        Slot::Data(value) => value.object(),
        _ => None,
    }
}

/** A non-string category is a TypeError; any other category answers whether tracing is enabled for it. */
fn is_enabled(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let JsValue::String(category) = call.arg(0) else {
        return Err(vm.type_error(CATEGORY_ERROR));
    };
    Ok(JsValue::Boolean(enabled(vm, &category.lossy())))
}

/** Validation follows V8's order: category, then enablement, then phase, name, id, name length and data. */
fn trace(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let JsValue::String(category) = call.arg(1) else {
        return Err(vm.type_error(CATEGORY_ERROR));
    };
    if !enabled(vm, &category.lossy()) {
        return Ok(JsValue::Boolean(false));
    }
    check_event(vm, &call)?;
    Ok(JsValue::Boolean(true))
}

/** Phase, name, id and name length are checked before the data argument is serialized. */
fn check_event(vm: &mut Vm, call: &Call) -> Res<()> {
    if call.arg(0).number_value().is_none() {
        return Err(vm.type_error(PHASE_ERROR));
    }
    let JsValue::String(name) = call.arg(2) else {
        return Err(vm.type_error(NAME_ERROR));
    };
    if !matches!(
        call.arg(3),
        JsValue::Undefined | JsValue::Null | JsValue::Number(_)
    ) {
        return Err(vm.type_error(ID_ERROR));
    }
    if name.is_empty() {
        return Err(vm.type_error(NAME_LENGTH_ERROR));
    }
    serialize_data(vm, call)
}

/** The data argument must serialize as JSON, so circular or BigInt payloads throw as they do in V8. */
fn serialize_data(vm: &mut Vm, call: &Call) -> Res<()> {
    let data = call.arg(4);
    if matches!(data, JsValue::Undefined) {
        return Ok(());
    }
    let arguments = [data];
    stringify(
        vm,
        crate::v9::vm::Call {
            this: JsValue::Undefined,
            args: &arguments,
            new_target: None,
            callee: call.callee,
        },
    )?;
    Ok(())
}
