/** Private disposal state roots captured methods and resources until reverse-order cleanup consumes them. */
use super::*;

/** A branded state references its own identity, preventing accidental confusion with public wrappers. */
pub(super) fn create(vm: &mut Vm) -> Res<ObjectId> {
    let records = vm.new_array(Vec::new())?;
    let id = vm.alloc(Object::new(Class::Plain, None))?;
    put(vm, id, "disposal-state", JsValue::Object(id));
    put(vm, id, "records", JsValue::Object(records));
    for name in [
        "disposed",
        "has-error",
        "disposal-error",
        "needs-await",
        "has-awaited",
    ] {
        put(vm, id, name, JsValue::Boolean(false));
    }
    put(vm, id, "error", JsValue::Undefined);
    Ok(id)
}

/** Internal operations accept only directly branded objects, excluding proxies and inherited brands. */
pub(super) fn identity(vm: &Vm, value: &JsValue) -> Res<ObjectId> {
    let id = value.object().ok_or_else(|| invalid())?;
    if get(vm, id, "disposal-state")? != JsValue::Object(id) {
        return Err(invalid().into());
    }
    Ok(id)
}
/** Public mutation of a disposed capability is always a ReferenceError. */
pub(super) fn active(vm: &mut Vm, id: ObjectId) -> Res<()> {
    if boolean(vm, id, "disposed")? {
        return Err(vm.reference_error("DisposableStack is disposed"));
    }
    Ok(())
}
/** Private slot reads reject accessors rather than executing forged saved getters. */
pub(super) fn get(vm: &Vm, id: ObjectId, name: &str) -> Res<JsValue> {
    match vm
        .heap
        .get(id)
        .and_then(|object| object.props.get(&Key::private(name)))
        .map(|property| &property.slot)
    {
        Some(Slot::Data(value)) => Ok(value.clone()),
        _ => Err(invalid().into()),
    }
}
/** Internal slots remain mutable even when authors freeze a public stack object. */
pub(super) fn put(vm: &mut Vm, id: ObjectId, name: &str, value: JsValue) {
    vm.heap.put(id, Key::private(name), Prop::data(value));
}
/** Exact booleans are required for persisted state flags. */
pub(super) fn boolean(vm: &Vm, id: ObjectId, name: &str) -> Res<bool> {
    match get(vm, id, name)? {
        JsValue::Boolean(value) => Ok(value),
        _ => Err(invalid().into()),
    }
}
/** Native storage arrays remain subject to ordinary heap accounting and admission limits. */
fn records(vm: &Vm, id: ObjectId) -> Res<ObjectId> {
    let record = get(vm, id, "records")?.object().ok_or_else(|| invalid())?;
    if !matches!(
        vm.heap.get(record).map(|object| &object.class),
        Some(Class::Array(_))
    ) {
        return Err(invalid().into());
    }
    Ok(record)
}
/** Each record captures its resource, method identity and invocation mode before returning to script. */
pub(super) fn append(
    vm: &mut Vm,
    id: ObjectId,
    value: JsValue,
    method: JsValue,
    flags: u32,
) -> Res<()> {
    let record = vm.new_array(vec![value, method, JsValue::Number(f64::from(flags))])?;
    let records = records(vm, id)?;
    let length = vm.length_of(&JsValue::Object(records))?;
    if !vm.define(
        records,
        Key::from(&length.to_string()),
        Descriptor::data(JsValue::Object(record)),
    )? {
        return Err(invalid().into());
    }
    Ok(())
}
/** Peek preserves the record until a deferred null await has completed before a sync disposal. */
pub(super) fn peek(vm: &mut Vm, id: ObjectId) -> Res<Option<(JsValue, JsValue, u32)>> {
    let records = records(vm, id)?;
    let array = JsValue::Object(records);
    let length = vm.length_of(&array)?;
    if length == 0 {
        return Ok(None);
    }
    let record = vm.get(&array, &Key::from(&(length - 1).to_string()))?;
    let value = vm.get(&record, &Key::from("0"))?;
    let method = vm.get(&record, &Key::from("1"))?;
    let flags = vm.get(&record, &Key::from("2"))?;
    match flags {
        JsValue::Number(flags) if flags.fract() == 0.0 && (0.0..=7.0).contains(&flags) => {
            Ok(Some((value, method, flags as u32)))
        }
        _ => Err(invalid().into()),
    }
}
/** Remove a record before calling its method so reentrant public operations see disposed state. */
pub(super) fn pop(vm: &mut Vm, id: ObjectId) -> Res<()> {
    let records = records(vm, id)?;
    let array = JsValue::Object(records);
    let length = vm.length_of(&array)?;
    vm.set(
        &array,
        Key::length(),
        JsValue::Number(length.saturating_sub(1) as f64),
        true,
    )
}
/** Earlier disposal or body errors become the suppressed cause of the latest disposal failure. */
pub(super) fn failure(vm: &mut Vm, id: ObjectId, error: JsValue) -> Res<()> {
    let error = if boolean(vm, id, "has-error")? {
        suppressed::make(vm, error, get(vm, id, "error")?)?
    } else {
        error
    };
    put(vm, id, "error", error);
    put(vm, id, "has-error", JsValue::Boolean(true));
    put(vm, id, "disposal-error", JsValue::Boolean(true));
    Ok(())
}
/** Only disposer errors replace compiler-preserved body completion; released state no longer roots failures. */
pub(super) fn finish(vm: &mut Vm, id: ObjectId) -> Res<()> {
    let failed = boolean(vm, id, "disposal-error")?;
    let error = get(vm, id, "error")?;
    put(vm, id, "error", JsValue::Undefined);
    put(vm, id, "has-error", JsValue::Boolean(false));
    if failed {
        Err(Thrown::Value(error))
    } else {
        Ok(())
    }
}
/** Corrupt private state is rejected as an admission failure rather than silently skipping cleanup. */
fn invalid() -> crate::protocol::EngineError {
    crate::protocol::EngineError::invalid("Invalid disposal capability state.")
}
