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
                    if elements.logical_present() > vm.limits.array_length {
                        return None;
                    }
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

/** Only ordinary, data-only chains can skip all spreadability and indexed lookups. */
fn inert_chain(vm: &Vm, source: ObjectId, target: ObjectId) -> bool {
    let mut current = Some(source);
    for _ in 0..vm.limits.frames {
        let Some(id) = current else {
            return true;
        };
        if id == target || global(vm, id) {
            return false;
        }
        if let Some(object) = vm.heap.get(id) {
            if !matches!(object.class, Class::Array(_) | Class::Plain) {
                return false;
            }
            if object.props.iter().any(|(key, _)| {
                integer(key).is_some() || *key == Key::Sym(symbol::IS_CONCAT_SPREADABLE)
            }) {
                return false;
            }
            current = object.proto;
        } else {
            let Some(definition) = vm.object_intrinsics(id).object(id) else {
                return false;
            };
            if definition.props.iter().any(|(key, _, _)| {
                integer(key).is_some() || *key == Key::Sym(symbol::IS_CONCAT_SPREADABLE)
            }) {
                return false;
            }
            current = definition.proto;
        }
    }
    false
}

/** Share a dense repeated pattern only after every observable lookup has been proven inert. */
fn compact(
    vm: &mut Vm,
    target: ObjectId,
    original: &JsValue,
    args: &[JsValue],
    fresh: bool,
) -> Res<bool> {
    let Some(source) = original.object() else {
        return Ok(false);
    };
    if !fresh
        || args.is_empty()
        || args.iter().any(|item| item.object() != Some(source))
        || !inert_chain(vm, source, target)
    {
        return Ok(false);
    }
    let Some(Class::Array(elements)) = vm.heap.get(source).map(|object| &object.class) else {
        return Ok(false);
    };
    if !elements.fully_present() {
        return Ok(false);
    }
    let count = args.len() + 1;
    if elements
        .len()
        .checked_mul(count)
        .is_none_or(|length| length > u32::MAX as usize)
    {
        return Err(vm.range_error("Invalid array length"));
    }
    // Reserve for the pattern before cloning it; later collections cannot make a
    // temporary allocation that exceeds the live heap budgets retroactively safe.
    let slots = elements.resident_len().saturating_add(1);
    let units = elements
        .resident_values()
        .map(JsValue::string_units)
        .sum::<usize>();
    if slots > vm.limits.array_length
        || vm.heap.counts.properties.saturating_add(slots) > vm.limits.properties
        || vm.heap.counts.units.saturating_add(units) > vm.limits.units
    {
        return Ok(false);
    }
    let Some(repeated) =
        crate::v9::vm::array_elements::ArrayElements::repeated_copy(elements, count)
    else {
        return Ok(false);
    };
    if repeated.resident_len() > vm.limits.array_length {
        return Ok(false);
    }
    let units = repeated
        .resident_values()
        .map(JsValue::string_units)
        .sum::<usize>();
    let slots = repeated.resident_len();
    for value in repeated.resident_values() {
        vm.heap.track_value(value);
    }
    let Some(Class::Array(elements)) = vm.heap.get_mut(target).map(|object| &mut object.class)
    else {
        return Ok(false);
    };
    *elements = repeated;
    vm.heap.adjust(slots as isize, units as isize);
    Ok(true)
}

/** Length overflow is checked before source reads, and final Set obeys the result's true brand. */
pub(super) fn run(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let original = JsValue::Object(this_object(vm, &call, "Array.prototype.concat")?);
    let (created, fresh) = species::create_with_fresh(vm, &original, 0)?;
    let result = JsValue::Object(created);
    vm.pin(result.clone());
    let target = result
        .object()
        .ok_or_else(|| vm.type_error("Array species did not create an object"))?;
    if compact(vm, target, &original, &call.args, fresh)? {
        return Ok(result);
    }
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
