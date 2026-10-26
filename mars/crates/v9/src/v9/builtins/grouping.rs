/** Grouping consumes live iterators and invokes its callback between steps, closing on callback failures. */
use super::*;
use crate::v9::vm::Thrown;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{Object, Slot};
use crate::v9::vm::property::Descriptor;
use crate::v9::vm::realm::realm;

/** Property grouping creates a null-prototype object; value grouping creates an intrinsic Map. */
pub(super) fn group(vm: &mut Vm, call: Call, property_keys: bool) -> Res<JsValue> {
    if call.arg(0).nullish() {
        return Err(vm.type_error("Cannot convert undefined or null to object"));
    }
    if !vm.is_callable(&call.arg(1)) {
        return Err(vm.type_error("Grouping callback must be callable"));
    }
    let result = if property_keys {
        vm.alloc(Object::new(Class::Plain, None))?
    } else {
        vm.alloc(Object::new(
            Class::Map(Box::default()),
            Some(realm().ids.map_prototype),
        ))?
    };
    let record = vm.get_iterator(&call.arg(0))?;
    let mut index = 0.0;
    let roots = vm.pins.len();
    while let Some(item) = vm.iter_step(record)? {
        vm.pin(item.clone());
        let operation = append(vm, result, property_keys, &call.arg(1), item, index);
        if let Err(error) = operation {
            return Err(close_error(vm, record, error));
        }
        vm.pins.truncate(roots);
        index += 1.0;
        vm.charge(1)?;
    }
    Ok(JsValue::Object(result))
}

/** Captured items remain rooted while callback and property-key coercion execute author code. */
fn append(
    vm: &mut Vm,
    result: ObjectId,
    property_keys: bool,
    callback: &JsValue,
    item: JsValue,
    index: f64,
) -> Res<()> {
    let group = vm.call(
        callback.clone(),
        JsValue::Undefined,
        vec![item.clone(), JsValue::Number(index)],
    )?;
    vm.pin(group.clone());
    let list = if property_keys {
        object_list(vm, result, group)?
    } else {
        map_list(vm, result, group)?
    };
    vm.array_append(&list, item)
}

/** New property groups bypass prototype setters and capture ToPropertyKey before selecting a list. */
fn object_list(vm: &mut Vm, result: ObjectId, group: JsValue) -> Res<JsValue> {
    let property = vm.to_key(&group)?;
    if let Some(prop) = vm.own(result, &property)?
        && let Slot::Data(list) = prop.slot
    {
        return Ok(list);
    }
    let list = JsValue::Object(vm.new_array(Vec::new())?);
    vm.define(result, property, Descriptor::data(list.clone()))?;
    Ok(list)
}

/** Map insertion normalizes keys through the native table and charges newly retained keys to the heap. */
fn map_list(vm: &mut Vm, result: ObjectId, group: JsValue) -> Res<JsValue> {
    if let Some(Class::Map(table)) = vm.heap.get(result).map(|object| &object.class)
        && let Some(list) = table.get(&group)
    {
        return Ok(list.clone());
    }
    let list = JsValue::Object(vm.new_array(Vec::new())?);
    let units = group.string_units();
    if let Some(Class::Map(table)) = vm.heap.get_mut(result).map(|object| &mut object.class) {
        table.set(group, list.clone());
    }
    vm.heap.adjust(1, units as isize);
    vm.check_heap_bounds()?;
    Ok(list)
}

/** Script cleanup failures do not replace the original callback error; native termination still escapes. */
pub(super) fn close_error(vm: &mut Vm, record: ObjectId, error: Thrown) -> Thrown {
    if matches!(error, Thrown::Fatal(_)) {
        return error;
    }
    match vm.iter_close(record) {
        Err(Thrown::Fatal(fatal)) => Thrown::Fatal(fatal),
        _ => error,
    }
}
