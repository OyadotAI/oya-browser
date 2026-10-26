/** Joint iteration captures input records, enumerable own keys and padding in specification order. */
use super::*;
use crate::v9::vm::heap::ENUMERABLE;

/** Flattenability rejects primitive strings and permits ordinary direct iterator objects. */
fn flatten(vm: &mut Vm, value: JsValue) -> Res<ObjectId> {
    if !matches!(value, JsValue::Object(_)) {
        return Err(vm.type_error("Iterator.zip input is not an object"));
    }
    vm.pin(value.clone());
    let method = vm.get(&value, &Key::Sym(symbol::ITERATOR))?;
    let iterator = if method.nullish() {
        value
    } else {
        vm.call(method, value, Vec::new())?
    };
    if !matches!(iterator, JsValue::Object(_)) {
        return Err(vm.type_error("Iterator method did not return an object"));
    }
    direct(vm, &iterator)
}

/** An inner acquisition failure closes acquired inputs before the outer iterable. */
pub(super) fn positional(vm: &mut Vm, source: JsValue, records: &mut Vec<JsValue>) -> Res<()> {
    let outer = vm.get_iterator(&source)?;
    while let Some(value) = vm.iter_step(outer)? {
        let record = match flatten(vm, value) {
            Ok(record) => record,
            Err(error) => {
                let error = close_records_error(vm, records, error);
                return Err(close_error(vm, outer, error));
            }
        };
        append(vm, records, JsValue::Object(record))?;
        vm.charge(1)?;
    }
    Ok(())
}

/** Own keys include symbols and skip missing, non-enumerable and undefined-valued properties. */
pub(super) fn keyed(
    vm: &mut Vm,
    source: JsValue,
    records: &mut Vec<JsValue>,
    keys: &mut Vec<JsValue>,
) -> Res<()> {
    let id = source.object().expect("source checked");
    for key in vm.own_keys(id)? {
        if vm
            .own(id, &key)?
            .is_none_or(|property| property.attrs & ENUMERABLE == 0)
        {
            continue;
        }
        let value = vm.get(&source, &key)?;
        if matches!(value, JsValue::Undefined) {
            continue;
        }
        let record = flatten(vm, value)?;
        append(vm, records, JsValue::Object(record))?;
        keys.push(key.value());
        vm.charge(1)?;
    }
    Ok(())
}

/** Opening arbitrarily many inputs remains subject to the existing resident array allowance. */
fn append(vm: &mut Vm, records: &mut Vec<JsValue>, value: JsValue) -> Res<()> {
    if records.len() >= vm.limits.array_length {
        return Err(crate::protocol::EngineError::limit(
            "V9 resident array elements",
            vm.limits.array_length,
        )
        .into());
    }
    vm.pin(value.clone());
    records.push(value);
    Ok(())
}

/** Longest mode captures padding now; an unfinished padding iterable is closed after the required count. */
pub(super) fn padding(
    vm: &mut Vm,
    padding: JsValue,
    keys: &[JsValue],
    count: usize,
    keyed: bool,
    longest: bool,
) -> Res<Vec<JsValue>> {
    let mut output = vec![JsValue::Undefined; count];
    if !longest || matches!(padding, JsValue::Undefined) {
        return Ok(output);
    }
    if keyed {
        for (slot, key) in output.iter_mut().zip(keys) {
            let key = vm.to_key(key)?;
            *slot = vm.get(&padding, &key)?;
            vm.pin(slot.clone());
        }
        return Ok(output);
    }
    let record = vm.get_iterator(&padding)?;
    let mut active = true;
    for slot in &mut output {
        if active {
            match vm.iter_step(record)? {
                Some(value) => {
                    vm.pin(value.clone());
                    *slot = value
                }
                None => active = false,
            }
        }
    }
    if active {
        vm.iter_close(record)?;
    }
    Ok(output)
}
