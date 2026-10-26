/** Stack extension entry checks admit only genuine own trace records and define public capture accessors before private data. */
use super::*;
use crate::v9::vm::error::{FRAMES_KEY, TRACE_KEY};
use crate::v9::vm::heap::Slot;

/** Native accessor receiver conversion rejects nullish values; other primitives and Proxy receivers have no own ErrorStackData. */
pub(super) fn receiver(vm: &mut Vm, value: &JsValue) -> Res<Option<ObjectId>> {
    if value.nullish() {
        return Err(vm.type_error("Cannot convert undefined or null to object"));
    }
    Ok(value
        .object()
        .filter(|id| vm.proxy_of(*id).is_none() && own_trace(vm, *id)))
}

/** Private own lookup cannot consult prototype objects, user descriptors, interceptors, or Proxy traps. */
fn own_trace(vm: &Vm, id: ObjectId) -> bool {
    vm.heap.get(id).is_some_and(|object| {
        [FRAMES_KEY, TRACE_KEY].iter().any(|name| {
            object
                .props
                .get(&Key::private(name))
                .is_some_and(|prop| matches!(&prop.slot, Slot::Data(_)))
        })
    })
}

/** Public descriptor failure must happen before any old trace is replaced or new trace becomes observable. */
pub(super) fn prepare_capture(vm: &mut Vm, value: &JsValue) -> Res<ObjectId> {
    let id = value
        .object()
        .filter(|id| vm.proxy_of(*id).is_none())
        .ok_or_else(|| vm.type_error("invalid_argument"))?;
    if vm.heap.get(id).is_some_and(|object| !object.extensible) {
        return Err(vm.type_error("Cannot define property stack, object is not extensible"));
    }
    let Some((_, crate::v9::vm::realm::Static::Accessor(get, set), _)) = realm()
        .object(realm().ids.error_prototype)
        .and_then(|definition| definition.get(&Key::private("errorStackAccessor")))
    else {
        return Err(
            crate::protocol::EngineError::invalid("Missing native Error stack accessor.").into(),
        );
    };
    let descriptor = Descriptor {
        get: Some(*get),
        set: Some(*set),
        enumerable: Some(false),
        configurable: Some(true),
        ..Descriptor::default()
    };
    if !vm.define(id, key("stack"), descriptor)? {
        return Err(vm.type_error("Cannot redefine property: stack"));
    }
    Ok(id)
}
