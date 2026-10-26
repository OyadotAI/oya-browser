/** Resizable and detached buffer metadata stays inert and is shared by every view of its bytes. */
use super::*;

/** ECMAScript indices fit exactly in a binary64 integer. */
const MAXIMUM_LENGTH: f64 = ((1u64 << f64::MANTISSA_DIGITS) - 1) as f64;

/** The maximum is a private internal slot, never a writable JavaScript property. */
fn maximum_key() -> Key {
    Key::private("array-buffer-maximum")
}
/** Detachment differs from an attached zero-byte buffer. */
fn detached_key() -> Key {
    Key::private("array-buffer-detached")
}

/** Native engines retain authority over memory buffers that scripts may not detach. */
fn protected_key() -> Key {
    Key::private("array-buffer-detach-protected")
}

/** This internal capability is shared by every alias and cannot be supplied through a JS property. */
fn immutable_key() -> Key {
    Key::private("array-buffer-immutable")
}

/** Reads, clones and view construction may use immutable bytes; writes and detachment may not. */
pub(in crate::v9) fn immutable(vm: &Vm, id: ObjectId) -> bool {
    vm.heap
        .get(id)
        .is_some_and(|object| object.props.get(&immutable_key()).is_some())
}

/** Only freshly initialized fixed ordinary buffers receive this capability. */
pub(in crate::v9) fn make_immutable(vm: &mut Vm, id: ObjectId) {
    vm.heap.put(
        id,
        immutable_key(),
        crate::v9::vm::heap::Prop::data(JsValue::Boolean(true)),
    );
}

/** An engine-owned memory buffer cannot be transferred by author code. */
pub(in crate::v9) fn protect_buffer(vm: &mut Vm, id: ObjectId) {
    vm.heap.put(
        id,
        protected_key(),
        crate::v9::vm::heap::Prop::data(JsValue::Boolean(true)),
    );
}

/** Internal memory growth may still detach a protected old backing through detach_buffer. */
pub(in crate::v9) fn buffer_detachable(vm: &Vm, id: ObjectId) -> bool {
    vm.heap.get(id).is_some_and(|object| {
        object.props.get(&protected_key()).is_none() && object.props.get(&immutable_key()).is_none()
    })
}

/** Fixed buffers have no maximum internal slot; resizable buffers retain it after detachment. */
pub(in crate::v9) fn maximum(vm: &Vm, id: ObjectId) -> Option<usize> {
    if let Some(Class::SharedArrayBuffer(buffer)) = vm.heap.get(id).map(|object| &object.class) {
        return buffer.maximum();
    }
    match &vm.heap.get(id)?.props.get(&maximum_key())?.slot {
        crate::v9::vm::heap::Slot::Data(JsValue::Number(value)) => Some(*value as usize),
        _ => None,
    }
}

/** Detachment is recorded independently of current byte length. */
pub(in crate::v9) fn detached(vm: &Vm, id: ObjectId) -> bool {
    if matches!(
        vm.heap.get(id).map(|object| &object.class),
        Some(Class::SharedArrayBuffer(_))
    ) {
        return false;
    }
    vm.heap
        .get(id)
        .is_some_and(|object| object.props.get(&detached_key()).is_some())
}

/** Setting an internal maximum occurs only after receiver construction and option coercion succeed. */
pub(in crate::v9) fn set_maximum(vm: &mut Vm, id: ObjectId, maximum: usize) {
    vm.heap.put(
        id,
        maximum_key(),
        crate::v9::vm::heap::Prop::data(JsValue::Number(maximum as f64)),
    );
}

/** A length-tracking view re-evaluates remaining bytes each time the buffer is resized. */
pub(super) fn tracking(vm: &mut Vm, id: ObjectId) {
    if let Some(Class::View(view)) = vm.heap.get_mut(id).map(|object| &mut object.class) {
        view.tracking = true;
    }
}

/** Effective length does not overwrite original bounds, so fixed views recover after a later grow. */
pub(super) fn effective(vm: &Vm, mut view: ViewData) -> ViewData {
    let bytes = storage_len(vm, view.buffer).unwrap_or(0);
    let size = element_size(view.kind);
    let end = view.offset.checked_add(view.length.saturating_mul(size));
    view.invalid = detached(vm, view.buffer)
        || view.offset > bytes
        || !view.tracking && end.is_none_or(|end| end > bytes);
    if view.invalid {
        view.length = 0;
    } else if view.tracking {
        view.length = (bytes - view.offset) / size;
    }
    view
}

/** Detachment invalidates all views through their shared buffer identity and releases byte accounting. */
pub(in crate::v9) fn detach(vm: &mut Vm, id: ObjectId) {
    debug_assert!(!immutable(vm, id), "immutable buffers cannot detach");
    if let Some(Class::ArrayBuffer(bytes)) = vm.heap.get_mut(id).map(|object| &mut object.class) {
        let units = bytes.0.len().div_ceil(2);
        bytes.0.clear();
        vm.heap.adjust(0, -(units as isize));
    }
    vm.heap.put(
        id,
        detached_key(),
        crate::v9::vm::heap::Prop::data(JsValue::Boolean(true)),
    );
}

/** Option parsing happens even when the initial length is zero and can invoke user getters. */
pub(super) fn options(vm: &mut Vm, value: &JsValue) -> Res<Option<usize>> {
    if matches!(value, JsValue::Undefined) {
        return Ok(None);
    }
    let object = vm.to_object(value)?;
    let maximum = vm.get(&JsValue::Object(object), &Key::from("maxByteLength"))?;
    if matches!(maximum, JsValue::Undefined) {
        return Ok(None);
    }
    to_index(vm, &maximum, "ArrayBuffer maximum byte length").map(Some)
}

/** Private slot validation rejects forged maximums and detached buffers that still contain bytes. */
pub(in crate::v9) fn valid(vm: &Vm, id: ObjectId) -> bool {
    let Some(object) = vm.heap.get(id) else {
        return false;
    };
    let length = buffer_bytes(vm, id).map_or(0, Vec::len);
    let maximum = object.props.get(&maximum_key()).is_none_or(|prop| matches!(prop.slot, crate::v9::vm::heap::Slot::Data(JsValue::Number(value)) if value.is_finite() && value >= length as f64 && value <= MAXIMUM_LENGTH && value.fract() == 0.0));
    let protection = object.props.get(&protected_key()).is_none_or(|prop| {
        matches!(
            prop.slot,
            crate::v9::vm::heap::Slot::Data(JsValue::Boolean(true))
        )
    });
    let immutable = object.props.get(&immutable_key()).is_none_or(|prop| {
        matches!(
            prop.slot,
            crate::v9::vm::heap::Slot::Data(JsValue::Boolean(true))
        ) && object.props.get(&maximum_key()).is_none()
            && object.props.get(&detached_key()).is_none()
            && object.props.get(&protected_key()).is_none()
    });
    maximum
        && protection
        && immutable
        && object.props.get(&detached_key()).is_none_or(|prop| {
            length == 0
                && matches!(
                    prop.slot,
                    crate::v9::vm::heap::Slot::Data(JsValue::Boolean(true))
                )
        })
}

/** An inert saved record cannot attach the backing capability to any other object brand. */
pub(in crate::v9) fn valid_immutable_brand(vm: &Vm, id: ObjectId) -> bool {
    !immutable(vm, id)
        || matches!(
            vm.heap.get(id).map(|object| &object.class),
            Some(Class::ArrayBuffer(_))
        )
}

/** Saved views retain their original bounds but cannot forge tracking on fixed storage. */
pub(in crate::v9) fn valid_view(vm: &Vm, id: ObjectId) -> bool {
    let Some(Class::View(view)) = vm.heap.get(id).map(|object| &object.class) else {
        return false;
    };
    let Some(bytes) = storage_len(vm, view.buffer) else {
        return false;
    };
    let Some(end) = view
        .length
        .checked_mul(element_size(view.kind))
        .and_then(|length| view.offset.checked_add(length))
    else {
        return false;
    };
    if end > BUFFER_BYTES {
        return false;
    }
    if matches!(
        vm.heap.get(view.buffer).map(|object| &object.class),
        Some(Class::SharedArrayBuffer(_))
    ) && end > bytes
    {
        return false;
    }
    match maximum(vm, view.buffer) {
        Some(maximum) => end <= maximum,
        None => !view.tracking && (detached(vm, view.buffer) || end <= bytes),
    }
}
