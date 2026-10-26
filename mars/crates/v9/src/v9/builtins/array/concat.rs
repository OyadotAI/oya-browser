/** Concat copies present indexed values while preserving species, proxy traps and sparse holes. */
use super::*;
use crate::v9::vm::heap::Slot;
use crate::v9::vm::realm::Static;
use std::collections::BTreeMap;

/** Result definitions are callback-free only on an ordinary array or ordinary object. */
fn ordinary_target(vm: &Vm, result: ObjectId) -> bool {
    matches!(
        vm.heap.get(result).map(|object| &object.class),
        Some(Class::Array(_) | Class::Plain)
    )
}

/** Global object access retains its own authorization checks and never uses a raw data shortcut. */
fn global(vm: &Vm, id: ObjectId) -> bool {
    #[cfg(feature = "conformance")]
    {
        vm.realms
            .contexts
            .iter()
            .any(|context| context.global == Some(id))
    }
    #[cfg(not(feature = "conformance"))]
    {
        let _ = vm;
        id == crate::v9::vm::GLOBAL
    }
}

/** Generic array-likes may have integer property names beyond the uint32 array-index domain. */
pub(super) fn integer(key: &Key) -> Option<usize> {
    let Key::Str(text) = key else {
        return None;
    };
    let text = text.lossy();
    let index = text.parse::<usize>().ok()?;
    (index.to_string() == text).then_some(index)
}

/** Only data-only chains can skip absent positions without suppressing author code. */
fn data(
    vm: &Vm,
    source: ObjectId,
    target: Option<ObjectId>,
    length: usize,
) -> Option<Vec<(usize, JsValue)>> {
    if target.is_some_and(|target| !ordinary_target(vm, target)) {
        return None;
    }
    let mut values = BTreeMap::new();
    let mut current = Some(source);
    for _ in 0..vm.limits.frames {
        let Some(id) = current else {
            return Some(values.into_iter().collect());
        };
        if Some(id) == target || global(vm, id) {
            return None;
        }
        if let Some(object) = vm.heap.get(id) {
            match &object.class {
                Class::Array(elements) => {
                    for (index, value) in elements
                        .resident_entries()
                        .filter(|(index, _)| *index < length)
                    {
                        values.entry(index).or_insert_with(|| value.clone());
                    }
                }
                Class::Plain => {}
                _ => return None,
            }
            for (key, property) in object.props.iter() {
                let Some(index) = integer(key).filter(|index| *index < length) else {
                    continue;
                };
                let Slot::Data(value) = &property.slot else {
                    return None;
                };
                values.entry(index).or_insert_with(|| value.clone());
            }
            current = object.proto;
        } else {
            let definition = vm.object_intrinsics(id).object(id)?;
            for (key, property, _) in &definition.props {
                let Some(index) = integer(key).filter(|index| *index < length) else {
                    continue;
                };
                let Static::Value(value) = property else {
                    return None;
                };
                values.entry(index).or_insert_with(|| value.clone());
            }
            current = definition.proto;
        }
    }
    None
}

/** In-place sort may snapshot pure data before it starts writes or comparator callbacks. */
pub(super) fn indexed_data(
    vm: &Vm,
    source: ObjectId,
    length: usize,
) -> Option<Vec<(usize, JsValue)>> {
    data(vm, source, None, length)
}

/** Slice can skip absent positions only when source reads and output definitions call no author code. */
pub(super) fn slice_range(
    vm: &mut Vm,
    result: ObjectId,
    source: &JsValue,
    start: usize,
    end: usize,
) -> Res<bool> {
    let Some(id) = source.object() else {
        return Ok(false);
    };
    let Some(values) = data(vm, id, Some(result), end) else {
        return Ok(false);
    };
    for (index, value) in values.into_iter().filter(|(index, _)| *index >= start) {
        species::define(vm, result, index - start, value)?;
    }
    Ok(true)
}

/** Fast sparse copies and generic proxy/getter copies share the exact same result definitions. */
fn spread(vm: &mut Vm, result: ObjectId, item: &JsValue, offset: usize, length: usize) -> Res<()> {
    if let Some(source) = item.object()
        && let Some(values) = data(vm, source, Some(result), length)
    {
        for (index, value) in values {
            species::define(vm, result, offset + index, value)?;
        }
        return Ok(());
    }
    for index in 0..length {
        vm.charge(1)?;
        if has_index(vm, item, index)? {
            let value = get_index(vm, item, index)?;
            species::define(vm, result, offset + index, value)?;
        }
    }
    Ok(())
}

/** Length overflow is checked before source reads, and final Set obeys the result's true brand. */
pub(super) fn run(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let original = JsValue::Object(this_object(vm, &call, "Array.prototype.concat")?);
    let result = JsValue::Object(species::create(vm, &original, 0)?);
    vm.pin(result.clone());
    let target = result
        .object()
        .ok_or_else(|| vm.type_error("Array species did not create an object"))?;
    let mut items = vec![original];
    items.extend(call.args.iter().cloned());
    let mut count = 0usize;
    for item in items {
        if spreadable(vm, &item)? {
            let length = vm.length_of(&item)?;
            let next = safe_growth(vm, count, length)?;
            spread(vm, target, &item, count, length)?;
            count = next;
        } else {
            let next = safe_growth(vm, count, 1)?;
            species::define(vm, target, count, item)?;
            count = next;
        }
    }
    set_length(vm, &result, count)?;
    Ok(result)
}
