/** Async-hook metadata lives in private traced heap objects, so snapshots and GC preserve actual resources. */
use super::*;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{Key, Object, Prop, Slot};

/** Async identifiers are exactly representable Numbers, never reused within this isolate. */
const MAX_ASYNC_ID: u64 = (1u64 << f64::MANTISSA_DIGITS) - 1;
/** A private isolate record has no author prototype and does not retain its first caller's foreign realm. */
pub(super) fn object(vm: &mut Vm, class: Class) -> Res<ObjectId> {
    let mut object = Object::new(class, None);
    object.realm = 0;
    vm.alloc(object)
}
/** Native fields never invoke getters, proxies or inherited setters. */
pub(super) fn get(vm: &Vm, id: ObjectId, name: &str) -> Option<JsValue> {
    match &vm.heap.get(id)?.props.get(&Key::private(name))?.slot {
        Slot::Data(value) => Some(value.clone()),
        _ => None,
    }
}
/** All metadata contributes to the ordinary heap property and string budgets. */
pub(super) fn put(vm: &mut Vm, id: ObjectId, name: &str, value: JsValue) {
    vm.heap.put(id, Key::private(name), Prop::data(value));
}
/** Untrusted restored metadata is validated before any native indexing or counter arithmetic. */
pub(super) fn invalid() -> crate::v9::vm::Thrown {
    crate::protocol::EngineError::invalid("Invalid async_hooks heap metadata.").into()
}
/** Numbers in private records cannot forge NaN, fractional or lossy resource identities. */
pub(super) fn number(value: Option<JsValue>) -> Option<u64> {
    match value {
        Some(JsValue::Number(value))
            if value.is_finite()
                && value >= 0.0
                && value.fract() == 0.0
                && value <= MAX_ASYNC_ID as f64 =>
        {
            Some(value as u64)
        }
        _ => None,
    }
}
/** An internal queue must be a native dense array with no prototype or user-visible accessors. */
pub(super) fn array<'a>(
    vm: &'a Vm,
    id: ObjectId,
) -> Res<&'a crate::v9::vm::array_elements::ArrayElements> {
    match vm.heap.get(id) {
        Some(object) if object.proto.is_none() && object.realm == 0 => match &object.class {
            Class::Array(values) => Ok(values),
            _ => Err(invalid()),
        },
        _ => Err(invalid()),
    }
}
/** The singleton is rooted on the real isolate-global object, independently of the namespace property. */
pub(super) fn root(vm: &mut Vm) -> Res<ObjectId> {
    if let Some(value) = get(vm, crate::v9::vm::GLOBAL, "async-hooks-state") {
        let id = value.object().ok_or_else(invalid)?;
        if !matches!(vm.heap.get(id),Some(object)if matches!(object.class,Class::Plain)&&object.proto.is_none()&&object.realm==0)
        {
            return Err(invalid());
        }
        return Ok(id);
    }
    let id = object(vm, Class::Plain)?;
    vm.pin(JsValue::Object(id));
    put(
        vm,
        crate::v9::vm::GLOBAL,
        "async-hooks-state",
        JsValue::Object(id),
    );
    let records = object(vm, Class::Array(Vec::new().into()))?;
    put(vm, id, "records", JsValue::Object(records));
    let stack = object(
        vm,
        Class::Array(vec![JsValue::Number(1.0), JsValue::Number(0.0)].into()),
    )?;
    put(vm, id, "stack", JsValue::Object(stack));
    put(vm, id, "next", JsValue::Number(1.0));
    Ok(id)
}
/** Metadata queues remain ordinary bounded heap arrays, without invoking Array.prototype methods. */
pub(super) fn queue(vm: &mut Vm, name: &str) -> Res<ObjectId> {
    let root = root(vm)?;
    let id = get(vm, root, name)
        .and_then(|value| value.object())
        .ok_or_else(invalid)?;
    array(vm, id)?;
    Ok(id)
}
/** Allocate one id at genuine Promise creation and persist it before any author callback runs. */
pub(super) fn next(vm: &mut Vm) -> Res<u64> {
    let root = root(vm)?;
    let previous = number(get(vm, root, "next")).ok_or_else(invalid)?;
    let next = previous
        .checked_add(1)
        .filter(|value| *value <= MAX_ASYNC_ID)
        .ok_or_else(invalid)?;
    put(vm, root, "next", JsValue::Number(next as f64));
    Ok(next)
}
/** Promise metadata names only the actual promise and numeric parent identity, never a synthetic resource. */
pub(super) fn ids(vm: &Vm, promise: ObjectId) -> Res<(u64, u64)> {
    if !matches!(
        vm.heap.get(promise).map(|object| &object.class),
        Some(Class::Promise(_))
    ) {
        return Err(invalid());
    }
    let id = number(get(vm, promise, "async-hook-id"))
        .filter(|id| *id >= 2)
        .ok_or_else(invalid)?;
    let trigger = number(get(vm, promise, "async-hook-trigger")).ok_or_else(invalid)?;
    Ok((id, trigger))
}
/** Context stacks start at (execution=1, trigger=0) and preserve every real nested promise job. */
pub(super) fn context(vm: &mut Vm) -> Res<(u64, u64)> {
    let stack = queue(vm, "stack")?;
    let values = array(vm, stack)?;
    let length = values.len();
    if length < 2 || length % 2 != 0 {
        return Err(invalid());
    }
    let id = number(values.get(length - 2).cloned()).ok_or_else(invalid)?;
    let trigger = number(values.get(length - 1).cloned()).ok_or_else(invalid)?;
    Ok((id, trigger))
}
/** A before event pushes its actual resource context before callbacks and user code execute. */
pub(super) fn push(vm: &mut Vm, id: u64, trigger: u64) -> Res<()> {
    let stack = queue(vm, "stack")?;
    vm.array_append(&JsValue::Object(stack), JsValue::Number(id as f64))?;
    vm.array_append(&JsValue::Object(stack), JsValue::Number(trigger as f64))
}
/** After hooks observe the restored parent context, matching d8's promise-hook wrapper. */
pub(super) fn pop(vm: &mut Vm) -> Res<()> {
    let stack = queue(vm, "stack")?;
    let length = array(vm, stack)?.len();
    if length < 4 || length % 2 != 0 {
        return Err(invalid());
    }
    if let Some(Class::Array(values)) = vm.heap.get_mut(stack).map(|object| &mut object.class) {
        values.truncate_collect(length - 2);
        vm.heap.adjust(-2, 0);
    }
    Ok(())
}
/** Branded hook methods accept another real hook receiver but cannot be borrowed onto arbitrary data. */
pub(super) fn record(vm: &mut Vm, this: &JsValue) -> Res<ObjectId> {
    let record = this
        .object()
        .and_then(|id| get(vm, id, "async-hook-record"))
        .and_then(|value| value.object())
        .ok_or_else(|| vm.type_error("Invalid 'this' passed instead of AsyncHooks instance"))?;
    let records = queue(vm, "records")?;
    if !array(vm, records)?
        .resident_values()
        .any(|value| *value == JsValue::Object(record))
    {
        return Err(invalid());
    }
    Ok(record)
}
