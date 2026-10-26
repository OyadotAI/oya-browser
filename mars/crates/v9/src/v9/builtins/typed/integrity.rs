/** Typed-array indexed storage and ordinary named properties have separate integrity semantics. */
use super::*;
use crate::v9::vm::heap::{CONFIGURABLE, Slot, WRITABLE};

/** V8's fixed indexed storage is sealed when extension stops, but live elements remain mutable. */
pub(in crate::v9::builtins) fn integrity_status(vm: &Vm, id: ObjectId, bits: u8) -> Option<bool> {
    let view = view_of(vm, id).filter(|view| view.kind != ElementKind::DataView)?;
    let object = vm.heap.get(id)?;
    if object.extensible
        || bits & WRITABLE != 0 && view.length != 0 && !buffer_state::immutable(vm, view.buffer)
    {
        return Some(false);
    }
    Some(object.props.iter().all(|(key, prop)| {
        key.hidden()
            || numeric_index(key).is_some()
            || prop.attrs & property_mask(&prop.slot, bits) == 0
    }))
}

/** Native property attributes change before a nonempty view rejects freezing or sealing its elements. */
pub(in crate::v9::builtins) fn restrict_named_properties(
    vm: &mut Vm,
    id: ObjectId,
    clear: u8,
) -> Res<()> {
    let keys = vm
        .heap
        .get(id)
        .expect("typed receiver")
        .props
        .ordered_keys();
    for key in keys.into_iter().filter(|key| numeric_index(key).is_none()) {
        vm.charge(1)?;
        if let Some(mut prop) = vm
            .heap
            .get(id)
            .and_then(|object| object.props.get(&key))
            .cloned()
        {
            prop.attrs &= !property_mask(&prop.slot, clear);
            vm.heap.put(id, key, prop);
        }
    }
    Ok(())
}

/** Accessors do not gain a writable attribute when their configurable bit changes. */
fn property_mask(slot: &Slot, bits: u8) -> u8 {
    if matches!(slot, Slot::Accessor(..)) {
        bits & !WRITABLE
    } else {
        bits & (CONFIGURABLE | WRITABLE)
    }
}
