/** Copying array methods create real Arrays and read only the indices their specification visits. */
use super::*;

/** ArrayCreate's uint32 check precedes both source reads and private materialization limits. */
fn length(vm: &mut Vm, length: usize) -> Res<()> {
    if length > u32::MAX as usize {
        return Err(vm.range_error("Invalid array length"));
    }
    Ok(())
}

/** Each Get can call author code, so retained earlier values are pinned before the next one. */
fn read(vm: &mut Vm, object: &JsValue, index: usize, values: &mut Vec<JsValue>) -> Res<()> {
    vm.charge(1)?;
    let value = get_index(vm, object, index)?;
    push(vm, values, value)
}

/** Private resource bounds apply when a value is actually materialized, after its observable Get. */
fn push(vm: &mut Vm, values: &mut Vec<JsValue>, value: JsValue) -> Res<()> {
    if values.len() >= vm.limits.array_length {
        return Err(
            EngineError::limit("V9 resident array elements", vm.limits.array_length).into(),
        );
    }
    vm.pin(value.clone());
    values.push(value);
    Ok(())
}

/** toSorted reads through holes, without observable HasProperty queries on a proxy. */
pub(super) fn to_sorted(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let comparator = call.arg(0);
    sort_comparator(vm, &comparator)?;
    let (object, count) = receiver(vm, &call, "Array.prototype.toSorted")?;
    length(vm, count)?;
    let mut values = Vec::new();
    for index in 0..count {
        read(vm, &object, index, &mut values)?;
    }
    let values = sorted(vm, &comparator, values)?;
    Ok(JsValue::Object(vm.new_array(values)?))
}

/** Deleted indices are never read, including when a huge removed range leaves a tiny result. */
pub(super) fn to_spliced(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, count) = receiver(vm, &call, "Array.prototype.toSpliced")?;
    let (start, removed, items) = splice_parts(vm, &call, count)?;
    let result_length = safe_growth(vm, count - removed, items.len())?;
    length(vm, result_length)?;
    let mut values = Vec::new();
    for index in 0..start {
        read(vm, &object, index, &mut values)?;
    }
    for item in items {
        push(vm, &mut values, item)?;
    }
    for index in start + removed..count {
        read(vm, &object, index, &mut values)?;
    }
    Ok(JsValue::Object(vm.new_array(values)?))
}

/** Reverse-copy access order is descending, so getter side effects match the produced values. */
pub(super) fn to_reversed(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, count) = receiver(vm, &call, "Array.prototype.toReversed")?;
    length(vm, count)?;
    let mut values = Vec::new();
    for index in (0..count).rev() {
        read(vm, &object, index, &mut values)?;
    }
    Ok(JsValue::Object(vm.new_array(values)?))
}

/** The replaced element is never read or queried, even when it is an accessor or inherited. */
pub(super) fn with(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, count) = receiver(vm, &call, "Array.prototype.with")?;
    let relative = vm.to_integer(&call.arg(0))?;
    let index = if relative < 0.0 {
        count as f64 + relative
    } else {
        relative
    };
    if index < 0.0 || index >= count as f64 {
        return Err(vm.range_error("Invalid index"));
    }
    length(vm, count)?;
    let mut values = Vec::new();
    for current in 0..count {
        if current == index as usize {
            push(vm, &mut values, call.arg(1))?;
        } else {
            read(vm, &object, current, &mut values)?;
        }
    }
    Ok(JsValue::Object(vm.new_array(values)?))
}
