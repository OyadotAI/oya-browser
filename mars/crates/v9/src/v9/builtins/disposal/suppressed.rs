/** SuppressedError is a genuine Error subclass with non-enumerable error and suppressed data properties. */
use super::*;
/** A stable intrinsic prototype is mapped to the current disposal realm when constructing suppressed failures. */
static PROTOTYPE: OnceLock<ObjectId> = OnceLock::new();

/** The constructor is callable with or without new and has the standard length of three. */
pub(super) fn install(builder: &mut Builder) {
    let prototype = builder.object(Some(builder.ids.error_prototype));
    let _ = PROTOTYPE.set(prototype);
    let constructor = builder.constructor(
        "SuppressedError",
        3,
        construct,
        prototype,
        Some(builder.ids.error),
    );
    builder.value(
        prototype,
        "name",
        string("SuppressedError"),
        crate::v9::vm::heap::HIDDEN_ATTRS,
    );
    builder.value(
        prototype,
        "message",
        string(""),
        crate::v9::vm::heap::HIDDEN_ATTRS,
    );
    builder.global("SuppressedError", constructor);
}
/** Message conversion precedes storing error values and ignores an extra options/cause argument. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let fallback = vm.intrinsic_id(*PROTOTYPE.get().expect("SuppressedError installed"));
    let prototype = match &call.new_target {
        Some(target) => vm.prototype_for(target, fallback)?,
        None => fallback,
    };
    let id = allocate(vm, prototype, call.arg(0), call.arg(1))?;
    if !matches!(call.arg(2), JsValue::Undefined) {
        let message = vm.to_string(&call.arg(2))?;
        define(vm, id, "message", JsValue::String(message))?;
    }
    vm.record_trace(id)?;
    Ok(JsValue::Object(id))
}
/** Intrinsic suppression bypasses an authored global constructor or mutated prototype constructor property. */
pub(super) fn make(vm: &mut Vm, error: JsValue, suppressed: JsValue) -> Res<JsValue> {
    let prototype = vm.intrinsic_id(*PROTOTYPE.get().expect("SuppressedError installed"));
    let id = allocate(vm, prototype, error, suppressed)?;
    define(
        vm,
        id,
        "message",
        JsValue::string("An error was suppressed during disposal")?,
    )?;
    vm.record_trace(id)?;
    Ok(JsValue::Object(id))
}
/** Error class identity retains Error.prototype behavior and normal stack capture. */
fn allocate(
    vm: &mut Vm,
    prototype: ObjectId,
    error: JsValue,
    suppressed: JsValue,
) -> Res<ObjectId> {
    let id = vm.alloc(Object::new(Class::Error, Some(prototype)))?;
    define(vm, id, "error", error)?;
    define(vm, id, "suppressed", suppressed)?;
    Ok(id)
}
/** Standard error data fields are writable and configurable, but never enumerable. */
fn define(vm: &mut Vm, id: ObjectId, name: &str, value: JsValue) -> Res<()> {
    vm.define(
        id,
        Key::from(name),
        Descriptor {
            value: Some(value),
            writable: Some(true),
            configurable: Some(true),
            enumerable: Some(false),
            ..Descriptor::default()
        },
    )?;
    Ok(())
}
