/** DisposableStack and AsyncDisposableStack expose branded registration, transfer and disposal over native capabilities. */
use super::*;
/** Public intrinsic prototypes are kept distinct from compiler-only capabilities. */
static PROTOTYPES: OnceLock<(ObjectId, ObjectId)> = OnceLock::new();

/** Each prototype has the specified tag, disposal alias, getter and common registration methods. */
pub(super) fn install(builder: &mut Builder) {
    let sync = install_one(builder, false);
    let asynchronous = install_one(builder, true);
    let _ = PROTOTYPES.set((sync, asynchronous));
}
/** Constructor wrappers select a fixed brand rather than trusting receiver or prototype properties. */
fn install_one(builder: &mut Builder, is_async: bool) -> ObjectId {
    let prototype = builder.object(Some(builder.ids.object_prototype));
    let name = if is_async {
        "AsyncDisposableStack"
    } else {
        "DisposableStack"
    };
    let constructor = builder.constructor(
        name,
        0,
        if is_async {
            construct_async
        } else {
            construct_sync
        },
        prototype,
        None,
    );
    builder.global(name, constructor);
    builder.builtin_accessor(
        prototype,
        "disposed",
        Some(if is_async {
            disposed_async
        } else {
            disposed_sync
        }),
        None,
    );
    builder.method(
        prototype,
        "use",
        1,
        if is_async { use_async } else { use_sync },
    );
    builder.method(
        prototype,
        "adopt",
        2,
        if is_async { adopt_async } else { adopt_sync },
    );
    builder.method(
        prototype,
        "defer",
        1,
        if is_async { defer_async } else { defer_sync },
    );
    builder.method(
        prototype,
        "move",
        0,
        if is_async { move_async } else { move_sync },
    );
    let dispose = builder.method(
        prototype,
        if is_async { "disposeAsync" } else { "dispose" },
        0,
        if is_async {
            dispose_async
        } else {
            dispose_sync
        },
    );
    builder.put(
        prototype,
        Key::Sym(if is_async {
            symbol::ASYNC_DISPOSE
        } else {
            symbol::DISPOSE
        }),
        Static::Value(JsValue::Object(dispose)),
        crate::v9::vm::heap::HIDDEN_ATTRS,
    );
    builder.put(
        prototype,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string(name)),
        crate::v9::vm::heap::CONFIGURABLE,
    );
    prototype
}
/** A public stack object stores its exact kind and private capability without author-observable initialization. */
fn allocate(vm: &mut Vm, prototype: ObjectId, is_async: bool) -> Res<ObjectId> {
    let capability = state::create(vm)?;
    let object = vm.alloc(Object::new(Class::Plain, Some(prototype)))?;
    state::put(
        vm,
        object,
        "disposal-capability",
        JsValue::Object(capability),
    );
    state::put(vm, object, "disposal-async", JsValue::Boolean(is_async));
    Ok(object)
}
/** newTarget controls only the prototype; construction always establishes the selected stack's own brand. */
fn construct(vm: &mut Vm, call: Call, is_async: bool) -> Res<JsValue> {
    let Some(target) = &call.new_target else {
        return Err(vm.type_error("DisposableStack constructor requires new"));
    };
    let prototypes = PROTOTYPES.get().expect("disposal installed");
    let fallback = vm.intrinsic_id(if is_async { prototypes.1 } else { prototypes.0 });
    let prototype = vm.prototype_for(target, fallback)?;
    allocate(vm, prototype, is_async).map(JsValue::Object)
}
/** Borrowed methods reject the other stack kind and proxies without invoking traps. */
fn receiver(vm: &mut Vm, call: &Call, is_async: bool) -> Res<ObjectId> {
    let Some(id) = call.this.object() else {
        return Err(vm.type_error("Incompatible disposal stack receiver"));
    };
    if state::get(vm, id, "disposal-async").ok() != Some(JsValue::Boolean(is_async)) {
        return Err(vm.type_error("Incompatible disposal stack receiver"));
    }
    let value = state::get(vm, id, "disposal-capability")?;
    state::identity(vm, &value)
}
/** Active registration checks occur before reading resource methods or validating callbacks. */
fn use_value(vm: &mut Vm, call: Call, is_async: bool) -> Res<JsValue> {
    let id = receiver(vm, &call, is_async)?;
    add(vm, JsValue::Object(id), call.arg(0), is_async)
}
/** Adopt and defer capture their callback now with exact receiver-versus-argument invocation semantics. */
fn callback(vm: &mut Vm, call: Call, is_async: bool, defer: bool) -> Res<JsValue> {
    let id = receiver(vm, &call, is_async)?;
    state::active(vm, id)?;
    let method = call.arg(if defer { 0 } else { 1 });
    if !vm.is_callable(&method) {
        return Err(vm.type_error("Disposal callback must be callable"));
    }
    let value = if defer {
        JsValue::Undefined
    } else {
        call.arg(0)
    };
    state::append(
        vm,
        id,
        value.clone(),
        method,
        (if is_async { ASYNC_FLAG } else { 0 }) | if defer { DEFER_FLAG } else { ARGUMENT_FLAG },
    )?;
    Ok(value)
}
/** move allocates the intrinsic stack, transfers records and marks the original disposed before returning. */
fn move_stack(vm: &mut Vm, call: Call, is_async: bool) -> Res<JsValue> {
    let old = receiver(vm, &call, is_async)?;
    state::active(vm, old)?;
    let prototypes = PROTOTYPES.get().expect("disposal installed");
    let object = allocate(
        vm,
        vm.intrinsic_id(if is_async { prototypes.1 } else { prototypes.0 }),
        is_async,
    )?;
    let new = state::get(vm, object, "disposal-capability")?
        .object()
        .expect("fresh capability");
    let records = state::get(vm, old, "records")?;
    let empty = vm.new_array(Vec::new())?;
    state::put(vm, new, "records", records);
    state::put(vm, old, "records", JsValue::Object(empty));
    state::put(vm, old, "disposed", JsValue::Boolean(true));
    Ok(JsValue::Object(object))
}
/** Async receiver errors become rejected promises, while native failures remain fatal. */
fn dispose_stack(vm: &mut Vm, call: Call, is_async: bool) -> Res<JsValue> {
    let receiver = receiver(vm, &call, is_async);
    match receiver {
        Ok(id) => dispose(vm, JsValue::Object(id), is_async),
        Err(Thrown::Value(error)) if is_async => {
            let promise = vm.new_promise()?;
            vm.settle(promise, PromiseState::Rejected(error))?;
            Ok(JsValue::Object(promise))
        }
        Err(error) => Err(error),
    }
}
/** Exact-kind disposed getters read the private capability state. */
fn disposed(vm: &mut Vm, call: Call, is_async: bool) -> Res<JsValue> {
    let id = receiver(vm, &call, is_async)?;
    state::boolean(vm, id, "disposed").map(JsValue::Boolean)
}
/** Synchronous construction. */
fn construct_sync(vm: &mut Vm, call: Call) -> Res<JsValue> {
    construct(vm, call, false)
}
/** Asynchronous construction. */
fn construct_async(vm: &mut Vm, call: Call) -> Res<JsValue> {
    construct(vm, call, true)
}
/** Synchronous resource registration. */
fn use_sync(vm: &mut Vm, call: Call) -> Res<JsValue> {
    use_value(vm, call, false)
}
/** Asynchronous resource registration. */
fn use_async(vm: &mut Vm, call: Call) -> Res<JsValue> {
    use_value(vm, call, true)
}
/** Synchronous adoption. */
fn adopt_sync(vm: &mut Vm, call: Call) -> Res<JsValue> {
    callback(vm, call, false, false)
}
/** Asynchronous adoption. */
fn adopt_async(vm: &mut Vm, call: Call) -> Res<JsValue> {
    callback(vm, call, true, false)
}
/** Synchronous deferred cleanup. */
fn defer_sync(vm: &mut Vm, call: Call) -> Res<JsValue> {
    callback(vm, call, false, true)
}
/** Asynchronous deferred cleanup. */
fn defer_async(vm: &mut Vm, call: Call) -> Res<JsValue> {
    callback(vm, call, true, true)
}
/** Synchronous ownership transfer. */
fn move_sync(vm: &mut Vm, call: Call) -> Res<JsValue> {
    move_stack(vm, call, false)
}
/** Asynchronous ownership transfer. */
fn move_async(vm: &mut Vm, call: Call) -> Res<JsValue> {
    move_stack(vm, call, true)
}
/** Synchronous cleanup. */
fn dispose_sync(vm: &mut Vm, call: Call) -> Res<JsValue> {
    dispose_stack(vm, call, false)
}
/** Asynchronous cleanup. */
fn dispose_async(vm: &mut Vm, call: Call) -> Res<JsValue> {
    dispose_stack(vm, call, true)
}
/** Synchronous state observation. */
fn disposed_sync(vm: &mut Vm, call: Call) -> Res<JsValue> {
    disposed(vm, call, false)
}
/** Asynchronous state observation. */
fn disposed_async(vm: &mut Vm, call: Call) -> Res<JsValue> {
    disposed(vm, call, true)
}
