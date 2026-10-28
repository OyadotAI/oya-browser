/** Stack capture may follow a genuine unresolved native resolve/reject pair without invoking either callback or any property getter. */
use super::{Class, JsValue, ObjectId, PromiseState, RESOLVER_TARGET, Vm, natives};
use crate::v9::vm::{
    data::FunctionKind,
    heap::{Key, Slot},
};

/** Native thenable adoption forwards into the captured promise; both handlers must share the same actual flag. */
pub(in crate::v9) fn forwarding_promise(
    vm: &Vm,
    fulfill: &JsValue,
    reject: &JsValue,
) -> Option<ObjectId> {
    let (promise, flag) = resolver(vm, fulfill, natives().resolve)?;
    if resolver(vm, reject, natives().reject)? != (promise, flag) {
        return None;
    }
    pending_target(vm, promise, flag)
}

/** A genuine native resolve capture names the capability promise used by a native combinator. */
pub(in crate::v9) fn resolving_promise(vm: &Vm, resolve: &JsValue) -> Option<ObjectId> {
    let (promise, flag) = resolver(vm, resolve, natives().resolve)?;
    pending_target(vm, promise, flag)
}

/** Claimed functions no longer describe an unresolved continuation. */
fn pending_target(vm: &Vm, promise: ObjectId, flag: ObjectId) -> Option<ObjectId> {
    let flag = vm.heap.get(flag)?;
    if !matches!(flag.class, Class::Plain) {
        return None;
    }
    if let Some(prop) = flag.props.get(&Key::from("done")) {
        if !matches!(
            &prop.slot,
            Slot::Data(JsValue::Boolean(false) | JsValue::Undefined)
        ) {
            return None;
        }
    }
    matches!(vm.heap.get(promise).map(|object|&object.class),Some(Class::Promise(data))if matches!(data.state,PromiseState::Pending)).then_some(promise)
}

/** Identity is the immutable native body and its typed captured slots, never a mutable function name. */
fn resolver(vm: &Vm, function: &JsValue, expected: u32) -> Option<(ObjectId, ObjectId)> {
    let Class::Function(data) = &vm.heap.get(function.object()?)?.class else {
        return None;
    };
    let FunctionKind::Native { id, slots } = &data.kind else {
        return None;
    };
    if *id != expected {
        return None;
    }
    match slots.as_slice() {
        [JsValue::Object(flag)] => match &vm.heap.get(*flag)?.props.get(&Key::private(RESOLVER_TARGET))?.slot {
            Slot::Data(JsValue::Object(promise)) => Some((*promise, *flag)),
            _ => None,
        },
        [JsValue::Object(promise), JsValue::Object(flag)] => Some((*promise, *flag)),
        _ => None,
    }
}
