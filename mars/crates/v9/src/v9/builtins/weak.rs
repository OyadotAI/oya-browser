/**
 * WeakRef and FinalizationRegistry. V9 collects only between requests and a page's state is
 * small, so a WeakRef holds its target strongly (deref() always finds it) and finalization
 * callbacks never run; both are behaviours the specification allows.
 */
use super::*;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{HIDDEN_ATTRS, Object, Prop, Slot};
use crate::v9::vm::realm::realm;

/** WeakRef and FinalizationRegistry. */
pub(super) fn install(b: &mut Builder) {
    let weak = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("WeakRef", 1, construct_weak_ref, weak, None);
    b.global("WeakRef", ctor);
    b.method(weak, "deref", 0, deref);
    let registry = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor(
        "FinalizationRegistry",
        1,
        construct_registry,
        registry,
        None,
    );
    b.global("FinalizationRegistry", ctor);
    b.method(registry, "register", 2, |_, _| Ok(JsValue::Undefined));
    b.method(registry, "unregister", 1, |_, _| {
        Ok(JsValue::Boolean(false))
    });
}

/** The hidden slot a WeakRef keeps its target in. */
fn target_key() -> Key {
    Key::private("weakRefTarget")
}

/** A new ordinary object for `new_target`. */
fn instance(vm: &mut Vm, call: &Call, name: &str) -> Res<ObjectId> {
    let Some(target) = &call.new_target else {
        return Err(vm.type_error(&format!("Constructor {name} requires 'new'")));
    };
    let proto = vm.prototype_for(target, realm().ids.object_prototype)?;
    vm.alloc(Object::new(Class::Plain, Some(proto)))
}

/** `new WeakRef(target)`: the target must be an object. */
fn construct_weak_ref(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = call.arg(0);
    if !matches!(target, JsValue::Object(_)) {
        return Err(vm.type_error("WeakRef: invalid target"));
    }
    let id = instance(vm, &call, "WeakRef")?;
    vm.heap.put(
        id,
        target_key(),
        Prop {
            slot: Slot::Data(target),
            attrs: HIDDEN_ATTRS,
        },
    );
    Ok(JsValue::Object(id))
}

/** WeakRef.prototype.deref. */
fn deref(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(id) = call.this.object() else {
        return Err(vm.type_error("WeakRef.prototype.deref called on incompatible receiver"));
    };
    match vm
        .heap
        .get(id)
        .and_then(|object| object.props.get(&target_key()))
    {
        Some(Prop {
            slot: Slot::Data(value),
            ..
        }) => Ok(value.clone()),
        _ => Err(vm.type_error("WeakRef.prototype.deref called on incompatible receiver")),
    }
}

/** `new FinalizationRegistry(callback)`. */
fn construct_registry(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !vm.is_callable(&call.arg(0)) {
        return Err(vm.type_error("FinalizationRegistry: cleanup must be callable"));
    }
    Ok(JsValue::Object(instance(
        vm,
        &call,
        "FinalizationRegistry",
    )?))
}
