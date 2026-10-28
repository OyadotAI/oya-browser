/** D8 condition variables implement timed waits on the real synchronized waiter queue. */
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::{Class, SharedGc};
use crate::v9::vm::heap::{Key, Object, Prop};
use crate::v9::vm::realm::{Builder, banks};
use crate::v9::vm::{Call, Res, Vm};
use std::sync::OnceLock;
use std::time::Duration;

pub(super) const CONDITION_BRAND: &str = "conformance-local-condition";
const CONDITION_STORE: &str = "conformance-condition-store";
static CONSTRUCTOR: OnceLock<ObjectId> = OnceLock::new();
/** Resolve a constructor identity in the requesting intrinsic bank. */
pub(super) fn constructor_id(bank: u32) -> ObjectId { *CONSTRUCTOR.get().expect("condition installed") + bank * banks::BANK_STRIDE }

/** Install the native constructor and operations without exposing the feature globally. */
pub(super) fn install(builder: &mut Builder) {
    let proto = builder.object(Some(builder.ids.object_prototype));
    let constructor = builder.constructor("Condition", 0, construct, proto, None);
    builder.symbol_method(
        constructor,
        crate::v9::vm::realm::symbol::HAS_INSTANCE,
        "[Symbol.hasInstance]",
        1,
        has_instance,
    );
    builder.method(constructor, "isCondition", 1, is_condition_method);
    builder.method(constructor, "wait", 3, wait);
    builder.method(constructor, "notify", 2, notify);
    let _ = CONSTRUCTOR.set(constructor);
}

/** Expose the constructor only in realms explicitly requesting harmony shared objects. */
pub(super) fn expose(vm: &mut Vm, global: ObjectId, bank: u32) {
    if !vm.page.environment.conformance_d8 || !vm.page.environment.conformance_harmony_struct {
        return;
    }
    let id = *CONSTRUCTOR.get().expect("condition installed") + bank * banks::BANK_STRIDE;
    let atomics = vm
        .heap
        .get(global)
        .and_then(|object| object.props.get(&Key::from("Atomics")))
        .and_then(|prop| match &prop.slot {
            crate::v9::vm::heap::Slot::Data(JsValue::Object(id)) => Some(*id),
            _ => None,
        });
    let Some(atomics) = atomics else { return };
    vm.materialize(id).expect("condition constructor");
    vm.materialize(atomics).expect("Atomics intrinsic");
    vm.heap.put(
        atomics,
        Key::from("Condition"),
        Prop::data(JsValue::Object(id)),
    );
}

/** The inaccessible native store provides real timed-wait registration and cancellation. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = call.new_target.unwrap_or(JsValue::Object(call.callee));
    let fallback = crate::v9::vm::realm::realm().ids.object_prototype;
    let prototype = vm.prototype_for(&target, fallback)?;
    let store = super::arena_transport::new_synchronizer(vm, true)?;
    create(vm, store, prototype).map(JsValue::Object)
}
/** A local wrapper retains one shared, brokered condition waiter queue. */
fn create(vm: &mut Vm, store: SharedGc, prototype: ObjectId) -> Res<ObjectId> {
    let store = super::arena_transport::grant(vm, store)?;
    let mut native = Object::new(Class::SharedContainerRoot(store), None);
    native.extensible = false;
    let backing = vm.alloc(native)?;
    vm.pin(JsValue::Object(backing));
    let object = vm.alloc(Object::new(Class::Plain, Some(prototype)))?;
    vm.heap.put(
        object,
        Key::private(CONDITION_BRAND),
        Prop::data(JsValue::Boolean(true)),
    );
    vm.heap.put(
        object,
        Key::private(CONDITION_STORE),
        Prop::data(JsValue::Object(backing)),
    );
    crate::v9::builtins::integrity::prevent_extensions(vm, object)?;
    Ok(object)
}
/** Worker decoding restores a local wrapper around the original condition queue. */
pub(super) fn restore(vm: &mut Vm, store: SharedGc) -> Res<ObjectId> {
    let ids: Vec<_> = vm.heap.objects.keys().copied().filter(|id| is_condition(vm, *id)).collect();
    for id in ids {
        let other = storage(vm, &JsValue::Object(id))?;
        if other.node() == store.node() && other.arena().same_store(&store.arena()) { return Ok(id); }
    }
    let constructor =
        *CONSTRUCTOR.get().expect("condition installed") + vm.active_realm * banks::BANK_STRIDE;
    let fallback = crate::v9::vm::realm::realm().ids.object_prototype;
    let prototype = vm.prototype_for(&JsValue::Object(constructor), fallback)?;
    create(vm, store, prototype)
}
/** Branded condition objects retain their identity across same-isolate realms. */
pub(super) fn is_condition(vm: &Vm, object: ObjectId) -> bool {
    vm.heap
        .get(object)
        .is_some_and(|object| object.props.get(&Key::private(CONDITION_BRAND)).is_some())
}
fn is_condition_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Boolean(
        call.arg(0).object().is_some_and(|id| is_condition(vm, id)),
    ))
}
/** Branded conditions remain recognizable through same-isolate realm and prototype boundaries. */
fn has_instance(vm: &mut Vm, call: Call) -> Res<JsValue> {
    super::shared::shared_has_instance(vm, call)
}
/** Only engine-owned condition storage can register notifications. */
pub(super) fn storage(vm: &mut Vm, value: &JsValue) -> Res<SharedGc> {
    let Some(id) = value.object().filter(|id| is_condition(vm, *id)) else {
        return Err(vm.type_error("Invalid Atomics.Condition"));
    };
    let backing = vm
        .heap
        .get(id)
        .and_then(|object| object.props.get(&Key::private(CONDITION_STORE)))
        .and_then(|prop| match &prop.slot {
            crate::v9::vm::heap::Slot::Data(JsValue::Object(id)) => Some(*id),
            _ => None,
        });
    match backing
        .and_then(|id| vm.heap.get(id))
        .map(|object| &object.class)
    {
        Some(Class::SharedContainerRoot(store)) if store.metadata()?.0.as_ref() == super::arena_transport::CONDITION_META => Ok(store.clone()),
        _ => Err(vm.type_error("Invalid Atomics.Condition storage")),
    }
}
/** Wait temporarily releases the held local mutex, reacquiring even after cancellation. */
fn wait(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if vm.page.state.atomics_wait_disallowed { return Err(vm.type_error("Atomics wait is not allowed")); }
    let store = storage(vm, &call.arg(0))?;
    let Some(mutex) = call
        .arg(1)
        .object()
        .filter(|id| super::mutex::is_mutex(vm, *id))
    else {
        return Err(vm.type_error("Invalid Atomics.Mutex"));
    };
    if !super::mutex::locked(vm, mutex)? {
        return Err(vm.type_error("Mutex is not locked"));
    }
    let duration = match call.arg(2) {
        JsValue::Undefined => None,
        value => {
            let Some(value) = value.number_value() else { return Err(vm.type_error("Timeout must be a number")); };
            if value.is_infinite() && value.is_sign_positive() { None } else { Some(Duration::from_millis(value.max(0.0) as u64)) }
        },
    };
    let mutex_store = super::mutex::storage(vm, mutex)?;
    let owner = super::mutex::owner(vm);
    let mut poll_error = None;
    let result = store.condition_wait(&mutex_store, owner as i128, duration, || match vm.wait_safepoint() {
        Ok(expired) => expired,
        Err(error) => { poll_error = Some(error); true }
    });
    if let Err(failure @ (crate::v9::vm::data::SharedWaitFailure::Limit | crate::v9::vm::data::SharedWaitFailure::Invalid)) = result {
        return Err(super::mutex::wait_failure(vm, failure));
    }
    super::mutex::acquire(vm, mutex, None, true)?;
    if let Some(error) = poll_error { return Err(error); }
    let result = result.map_err(|failure| super::mutex::wait_failure(vm, failure))?;
    Ok(JsValue::Boolean(result))
}
/** Notifications wake real registered waiters and return the number awakened. */
fn notify(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let store = storage(vm, &call.arg(0))?;
    let count = match call.arg(1) {
        JsValue::Undefined => usize::MAX,
        value => vm.to_integer(&value)?.max(0.0).min(usize::MAX as f64) as usize,
    };
    let count = store.notify_waiters(count);
    Ok(JsValue::Number(count as f64))
}
