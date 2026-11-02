/** Promise wrappers run synchronous Wasm faithfully; asynchronous stack continuations remain explicit. */
use super::*;
const CALLBACK: &str = "wasm.suspending.callback";
pub(super) const CONTEXT: &str = "wasm.promising.context";

/** Synchronous promising calls use genuine Promise resolution, including thenable adoption. */
pub(super) fn install(b: &mut Builder, namespace: ObjectId) -> (ObjectId, u32) {
    let proto = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("Suspending", 1, suspending, proto, None);
    b.alias(namespace, "Suspending", ctor);
    b.method(namespace, "promising", 1, promising);
    (proto, b.native("wrapped", 0, invoke))
}
/** Suspender objects retain an actual callable without pretending that the wrapper is itself callable. */
fn suspending(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target.as_ref() else {
        return Err(vm.type_error("WebAssembly.Suspending must be invoked with 'new'"));
    };
    let callback = call.arg(0);
    if !vm.is_callable(&callback) {
        return Err(vm.type_error("WebAssembly.Suspending requires a callable"));
    }
    let proto = vm.prototype_for(target, ids(vm).suspending)?;
    let object = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    set(vm, object, CALLBACK, callback);
    Ok(JsValue::Object(object))
}
/** The linker accepts a suspender only in a function import slot with its declared Wasm signature. */
pub(super) fn is_suspending(vm: &Vm, value: &JsValue) -> bool {
    value
        .object()
        .is_some_and(|id| get(vm, id, CALLBACK).is_some())
}
/** Only genuine Wasm functions can acquire a Promise-returning boundary wrapper. */
fn promising(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let function = call.arg(0);
    let (_, m, index) = instance::target(vm, &function)?;
    let length = m
        .signature(index)
        .map_err(|e| compile_failure(vm, e))?
        .params
        .len();
    let wrapper = vm.native_closure(ids(vm).promise_invoke, vec![function])?;
    vm.define(
        wrapper,
        key("length"),
        Descriptor {
            value: Some(JsValue::Number(length as f64)),
            configurable: Some(true),
            ..Descriptor::default()
        },
    )?;
    Ok(JsValue::Object(wrapper))
}
/** A lexical context records intervening JS frames and is restored for nested/reentrant wrappers. */
fn invoke(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let function = vm
        .slots(call.callee)
        .first()
        .cloned()
        .ok_or_else(|| vm.type_error("invalid promising function capture"))?;
    let (instance, _, _) = instance::target(vm, &function)?;
    let previous = get(vm, instance, CONTEXT).unwrap_or(JsValue::Undefined);
    let promise = vm.new_promise()?;
    set(
        vm,
        instance,
        CONTEXT,
        JsValue::Number(vm.frames.len() as f64),
    );
    let result = execute::start_promising(vm, function, call.args.to_vec(), promise);
    set(vm, instance, CONTEXT, previous);
    match result {
        Ok(()) => (),
        Err(Thrown::Value(value)) => vm.settle(promise, PromiseState::Rejected(value))?,
        Err(fatal) => return Err(fatal),
    }
    Ok(JsValue::Object(promise))
}
/** Plain return values do not suspend; Promise returns require a live continuation-capable stack. */
pub(super) fn call(
    vm: &mut Vm,
    instance: ObjectId,
    wrapper: &JsValue,
    args: Vec<JsValue>,
) -> Res<JsValue> {
    let callback = wrapper
        .object()
        .and_then(|id| get(vm, id, CALLBACK))
        .ok_or_else(|| vm.type_error("invalid suspending function"))?;
    if super::wasmfx_instance_enabled(vm, instance) || super::wasmfx_active(vm) {
        let context = get(vm, instance, CONTEXT);
        if !matches!(context, Some(JsValue::Number(depth)) if depth == vm.frames.len() as f64) {
            let message = if matches!(context, Some(JsValue::Number(_))) {
                "trying to suspend JS frames"
            } else {
                "trying to suspend without WebAssembly.promising"
            };
            return Err(failure(vm, ids(vm).suspend_error, message));
        }
    }
    let value = vm.call(callback, JsValue::Undefined, args)?;
    if !value
        .object()
        .is_some_and(|id| matches!(vm.heap.get(id).map(|o| &o.class), Some(Class::Promise(_))))
    {
        return Ok(value);
    }
    if matches!(get(vm,instance,CONTEXT),Some(JsValue::Number(depth))if depth==vm.frames.len()as f64)
    {
        return Err(vm.unsupported_error("asynchronous WebAssembly stack continuation"));
    }
    let message = if matches!(get(vm, instance, CONTEXT), Some(JsValue::Number(_))) {
        "trying to suspend JS frames"
    } else {
        "trying to suspend without WebAssembly.promising"
    };
    Err(failure(vm, ids(vm).suspend_error, message))
}

/** An inaccessible suspender slot retains the exact callable selected during construction. */
pub(super) fn callback(vm: &mut Vm, wrapper: &JsValue) -> Res<JsValue> {
    wrapper
        .object()
        .and_then(|id| get(vm, id, CALLBACK))
        .ok_or_else(|| vm.type_error("invalid suspending function"))
}
