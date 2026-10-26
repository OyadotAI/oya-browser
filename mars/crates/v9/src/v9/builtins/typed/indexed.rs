/** Integer-indexed typed arrays stop canonical numeric keys at their own storage boundary. */
use super::*;
use crate::v9::vm::property::Descriptor;

/** CanonicalNumericIndexString recognizes negative zero, infinities and NaN without coercion. */
pub(in crate::v9) fn numeric_index(key: &Key) -> Option<f64> {
    let Key::Str(text) = key else {
        return None;
    };
    if text.as_units() == [u16::from(b'-'), u16::from(b'0')] {
        return Some(-0.0);
    }
    let number = crate::v9::value::string_number(&text.lossy());
    let canonical = crate::v9::value::number_to_string(number);
    text.as_units()
        .iter()
        .copied()
        .eq(canonical.encode_utf16())
        .then_some(number)
}

/** None is an ordinary key; Some(None) is numeric but invalid, including detached storage. */
pub(in crate::v9) fn index(vm: &Vm, id: ObjectId, key: &Key) -> Option<Option<usize>> {
    let view = view_of(vm, id).filter(|view| view.kind != ElementKind::DataView)?;
    let number = numeric_index(key)?;
    let valid = number.is_finite()
        && number >= 0.0
        && !number.is_sign_negative()
        && number.fract() == 0.0
        && number < view.length as f64;
    Some(if valid && view_in_bounds(vm, &view) {
        Some(number as usize)
    } else {
        None
    })
}

/** A detached or out-of-bounds fixed view has no valid integer indices. */
fn view_in_bounds(vm: &Vm, view: &ViewData) -> bool {
    view.length
        .checked_mul(element_size(view.kind))
        .and_then(|length| view.offset.checked_add(length))
        .is_some_and(|end| {
            !view.invalid && storage_len(vm, view.buffer).is_some_and(|length| end <= length)
        })
}

/** Set converts even invalid-index values, then checks storage again after arbitrary valueOf. */
pub(in crate::v9) fn set(vm: &mut Vm, id: ObjectId, key: &Key, value: &JsValue) -> Res<bool> {
    let Some(view) = view_of(vm, id) else {
        return Ok(true);
    };
    if buffer_state::immutable(vm, view.buffer) && matches!(self::index(vm, id, key), Some(Some(_)))
    {
        return Ok(false);
    }
    let bytes = encode(vm, view.kind, value)?;
    if let Some(Some(index)) = self::index(vm, id, key) {
        if let Some(current) = view_of(vm, id) {
            write_bytes(vm, &current, index * element_size(current.kind), &bytes);
        }
    }
    Ok(true)
}

/** Integer-indexed definitions cannot replace the element's data or attribute semantics. */
pub(in crate::v9) fn define(
    vm: &mut Vm,
    id: ObjectId,
    key: &Key,
    descriptor: &Descriptor,
) -> Option<Res<bool>> {
    let index = self::index(vm, id, key)?;
    if view_of(vm, id).is_some_and(|view| buffer_state::immutable(vm, view.buffer)) {
        let compatible = index
            .and_then(|index| load(vm, id, index))
            .is_some_and(|value| {
                !descriptor.accessor()
                    && descriptor.configurable != Some(true)
                    && descriptor.enumerable != Some(false)
                    && descriptor.writable != Some(true)
                    && descriptor
                        .value
                        .as_ref()
                        .is_none_or(|next| value.same_value(next))
            });
        return Some(Ok(compatible));
    }
    if index.is_none()
        || descriptor.accessor()
        || descriptor.configurable == Some(false)
        || descriptor.enumerable == Some(false)
        || descriptor.writable == Some(false)
    {
        return Some(Ok(false));
    }
    Some(match &descriptor.value {
        Some(value) => set(vm, id, key, value),
        None => Ok(true),
    })
}

/** Enumeration uses the live buffer rather than stale length metadata after detachment. */
pub(in crate::v9) fn keys_length(vm: &Vm, id: ObjectId) -> usize {
    view_of(vm, id)
        .filter(|view| view.kind != ElementKind::DataView && view_in_bounds(vm, view))
        .map_or(0, |view| view.length)
}
