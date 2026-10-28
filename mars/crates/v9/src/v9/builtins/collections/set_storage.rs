/** Set algebra accesses internal tables without invoking overridable receiver methods. */
use super::*;

/** Set methods require a Set internal slot, including when other is another collection kind. */
pub(super) fn receiver(vm: &mut Vm, value: &JsValue) -> Res<ObjectId> {
    match value.object().filter(|id| {
        matches!(
            vm.heap.get(*id).map(|object| &object.class),
            Some(Class::Set(_))
        )
    }) {
        Some(id) => Ok(id),
        None => Err(vm.type_error("Set method called on incompatible receiver")),
    }
}

/** Current membership stays live while user has or next callbacks mutate the receiver. */
pub(super) fn contains(vm: &Vm, id: ObjectId, value: &JsValue) -> bool {
    matches!(vm.heap.get(id).map(|object| &object.class), Some(Class::Set(table)) if table.has(value))
}

/** Size is read from internal data after the argument protocol getters have run. */
pub(super) fn size(vm: &Vm, id: ObjectId) -> usize {
    match vm.heap.get(id).map(|object| &object.class) {
        Some(Class::Set(table)) => table.live,
        _ => 0,
    }
}

/** Algebra produces an intrinsic Set, independent of receiver constructors and species. */
pub(super) fn result(vm: &mut Vm, original: Option<ObjectId>) -> Res<ObjectId> {
    let mut table = Table::default();
    if let Some(Some(Class::Set(source))) =
        original.map(|id| vm.heap.get(id).map(|object| &object.class))
    {
        for (key, _) in source.entries.iter().flatten() {
            table.set(key.clone(), JsValue::Undefined);
        }
    }
    vm.alloc(Object::new(
        Class::Set(Box::new(table)),
        Some(realm().ids.set_prototype),
    ))
}

/** Live cursors retain stable insertion positions, including deletions and additions in callbacks. */
pub(super) fn next(vm: &mut Vm, id: ObjectId, cursor: &mut usize) -> Res<Option<JsValue>> {
    loop {
        vm.charge(1)?;
        let entry = match vm.heap.get(id).map(|object| &object.class) {
            Some(Class::Set(table)) => table.entries.get(*cursor).cloned(),
            _ => None,
        };
        let Some(entry) = entry else {
            return Ok(None);
        };
        *cursor += 1;
        if let Some((value, _)) = entry {
            return Ok(Some(value));
        }
    }
}

/** New entries consume the same property and string quotas as ordinary Set.add. */
pub(super) fn insert(vm: &mut Vm, id: ObjectId, value: JsValue) -> Res<()> {
    vm.heap.track_value(&value);
    let units = value.string_units() as isize;
    if let Some(Class::Set(table)) = vm.heap.get_mut(id).map(|object| &mut object.class) {
        if table.set(value, JsValue::Undefined) {
            vm.heap.adjust(1, units);
        }
    }
    vm.check_heap_bounds()
}

/** Removing result entries updates live accounting and preserves cursor positions. */
pub(super) fn remove(vm: &mut Vm, id: ObjectId, value: &JsValue) {
    let removed = match vm.heap.get_mut(id).map(|object| &mut object.class) {
        Some(Class::Set(table)) => table.delete(value),
        _ => None,
    };
    if let Some((key, _)) = removed {
        vm.heap.adjust(-1, -(key.string_units() as isize));
    }
}
