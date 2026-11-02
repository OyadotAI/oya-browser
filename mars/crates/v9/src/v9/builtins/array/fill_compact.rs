/** Compact a full fill only when ordinary indexed writes cannot call author code. */
use super::*;

/** Every indexed property in the chain must be absent, including accessors and readonly data. */
fn inert_chain(vm: &Vm, source: ObjectId) -> bool {
    let mut current = Some(source);
    for _ in 0..vm.limits.frames {
        let Some(id) = current else {
            return true;
        };
        if id == crate::v9::vm::GLOBAL {
            return false;
        }
        #[cfg(feature = "conformance")]
        if crate::v9::builtins::shared_container_active(vm, id) {
            return false;
        }
        if let Some(object) = vm.heap.get(id) {
            #[cfg(feature = "conformance")]
            if object.global {
                return false;
            }
            if object.props.iter().any(|(key, _)| key.index().is_some()) {
                return false;
            }
            match &object.class {
                Class::Array(elements) if elements.logical_present() == 0 => {}
                Class::Plain => {}
                _ => return false,
            }
            current = object.proto;
        } else {
            let Some(object) = vm.object_intrinsics(id).object(id) else {
                return false;
            };
            if object.props.iter().any(|(key, _, _)| key.index().is_some()) {
                return false;
            }
            current = object.proto;
        }
    }
    false
}

/** Reuse a single actual value when filling every slot of a large, extensible hole array. */
pub(super) fn run(
    vm: &mut Vm,
    receiver: &JsValue,
    value: JsValue,
    start: usize,
    end: usize,
    length: usize,
) -> Res<bool> {
    let Some(id) = receiver.object() else {
        return Ok(false);
    };
    if start != 0 || end != length || length <= vm.limits.array_length {
        return Ok(false);
    }
    if !matches!(vm.heap.get(id), Some(object) if object.extensible && matches!(&object.class, Class::Array(elements) if elements.len() == length && elements.logical_present() == 0))
        || !inert_chain(vm, id)
    {
        return Ok(false);
    }
    let seed = crate::v9::vm::array_elements::ArrayElements::from(vec![value.clone()]);
    let Some(elements) = crate::v9::vm::array_elements::ArrayElements::repeated_copy(&seed, length)
    else {
        return Ok(false);
    };
    let slots = elements.resident_len();
    let units = value.string_units();
    if vm.heap.counts.properties.saturating_add(slots) > vm.limits.properties
        || vm.heap.counts.units.saturating_add(units) > vm.limits.units
    {
        return Ok(false);
    }
    vm.charge(length)?;
    vm.heap.track_value(&value);
    let Some(Class::Array(destination)) = vm.heap.get_mut(id).map(|object| &mut object.class)
    else {
        return Ok(false);
    };
    *destination = elements;
    vm.heap.adjust(slots as isize, units as isize);
    Ok(true)
}
