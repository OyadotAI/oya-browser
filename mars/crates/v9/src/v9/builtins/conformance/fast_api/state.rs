/** Per-isolate callback state is rooted and traced independently of visible native receiver objects. */
use super::*;
/** Metadata cannot invoke getters or follow a prototype to impersonate an embedded native receiver. */
fn get(vm: &Vm, id: ObjectId, name: &str) -> Option<JsValue> {
    match &vm.heap.get(id)?.props.get(&Key::private(name))?.slot {
        Slot::Data(value) => Some(value.clone()),
        _ => None,
    }
}
pub(super) fn put(vm: &mut Vm, id: ObjectId, name: &str, value: JsValue) {
    vm.heap.put(id, Key::private(name), Prop::data(value));
}
/** Untrusted snapshot metadata fails as an engine state error rather than indexing unchecked values. */
fn invalid() -> crate::v9::vm::Thrown {
    crate::protocol::EngineError::invalid("Invalid FastCAPI isolate state.").into()
}
/** Original d8 native objects share one thread/isolate instance; its data survives genuine V9 public requests. */
pub(super) fn root(vm: &mut Vm) -> Res<ObjectId> {
    if let Some(value) = get(vm, crate::v9::vm::GLOBAL, "fast-api-state") {
        let id = value.object().ok_or_else(invalid)?;
        if !matches!(vm.heap.get(id),Some(object)if object.realm==0&&object.proto.is_none()&&matches!(object.class,Class::Plain))
        {
            return Err(invalid());
        }
        return Ok(id);
    }
    let mut object = Object::new(Class::Plain, None);
    object.realm = 0;
    let id = vm.alloc(object)?;
    vm.pin(JsValue::Object(id));
    put(
        vm,
        crate::v9::vm::GLOBAL,
        "fast-api-state",
        JsValue::Object(id),
    );
    for field in ["fast", "slow", "attribute"] {
        put(vm, id, field, JsValue::Number(0.0));
    }
    Ok(id)
}
/** Receiver identity is a direct native private field belonging to this exact isolate record. */
pub(super) fn branded(vm: &Vm, value: &JsValue) -> bool {
    value
        .object()
        .and_then(|id| get(vm, id, "fast-api-brand"))
        .is_some_and(|brand| get(vm, crate::v9::vm::GLOBAL, "fast-api-state") == Some(brand))
}
pub(super) fn receiver(vm: &mut Vm, value: &JsValue) -> Res<ObjectId> {
    if !branded(vm, value) {
        return Err(error(
            vm,
            "This method is not defined on objects inheriting from FastCAPI.",
        ));
    }
    root(vm)
}
/** Stored values retain exact finite numeric data and never derive counts from source names or call results. */
pub(super) fn number(vm: &Vm, id: ObjectId, name: &str) -> Res<f64> {
    match get(vm, id, name) {
        Some(JsValue::Number(value)) if value.is_finite() => Ok(value),
        _ => Err(invalid()),
    }
}
/** Only an actual callback increments slow count; querying capabilities and counts does not. */
pub(super) fn enter(vm: &mut Vm, value: &JsValue) -> Res<ObjectId> {
    let id = receiver(vm, value)?;
    let count = number(vm, id, "slow")?;
    if !(0.0..=f64::from(i32::MAX)).contains(&count) || count.fract() != 0.0 {
        return Err(invalid());
    }
    put(vm, id, "slow", JsValue::Number(count + 1.0));
    Ok(id)
}
