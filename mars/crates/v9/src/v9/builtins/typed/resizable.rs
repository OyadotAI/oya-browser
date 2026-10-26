/** ArrayBuffer resizing preserves its identity, zero-fills growth and changes every view's live bounds. */
use super::*;

/** Resizable, maximum-length, detachment, resize and transfer are real buffer operations. */
pub(super) fn install(builder: &mut Builder, prototype: ObjectId) {
    builder.builtin_accessor(prototype, "resizable", Some(resizable), None);
    builder.builtin_accessor(prototype, "maxByteLength", Some(maximum), None);
    builder.builtin_accessor(prototype, "detached", Some(detached), None);
    builder.method(prototype, "resize", 1, resize);
    super::transfer::install(builder, prototype);
}

/** Resizability records the allocation kind and remains known after detachment. */
fn resizable(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_buffer(vm, &call)?;
    Ok(JsValue::Boolean(buffer_state::maximum(vm, id).is_some()))
}

/** Detached buffers report zero maximum length, while attached fixed buffers report their length. */
fn maximum(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_buffer(vm, &call)?;
    let maximum = if buffer_state::detached(vm, id) {
        0
    } else {
        buffer_state::maximum(vm, id).unwrap_or_else(|| buffer_bytes(vm, id).map_or(0, Vec::len))
    };
    Ok(JsValue::Number(maximum as f64))
}

/** The detached accessor distinguishes a zero-byte allocation from a transferred one. */
fn detached(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_buffer(vm, &call)?;
    Ok(JsValue::Boolean(buffer_state::detached(vm, id)))
}

/** Re-check detachment after length conversion because arbitrary script may run in valueOf. */
fn resize(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_buffer(vm, &call)?;
    if buffer_state::detached(vm, id) {
        return Err(vm.type_error("Cannot resize a detached ArrayBuffer"));
    }
    let Some(maximum) = buffer_state::maximum(vm, id) else {
        return Err(vm.type_error("ArrayBuffer is not resizable"));
    };
    let length = to_index(vm, &call.arg(0), "ArrayBuffer resize length")?;
    if buffer_state::detached(vm, id) {
        return Err(vm.type_error("Cannot resize a detached ArrayBuffer"));
    }
    if length > maximum || length > BUFFER_BYTES {
        return Err(vm.range_error("ArrayBuffer resize length exceeds the maximum"));
    }
    if super::super::wasm::resize_memory_buffer(vm, id, length)? {
        return Ok(JsValue::Undefined);
    }
    resize_bytes(vm, id, length)?;
    Ok(JsValue::Undefined)
}

/** Existing storage remains subject to the same byte and total-heap limits as new buffers. */
fn resize_bytes(vm: &mut Vm, id: ObjectId, length: usize) -> Res<()> {
    if let Some(Class::ArrayBuffer(bytes)) = vm.heap.get_mut(id).map(|object| &mut object.class) {
        let previous = bytes.0.len().div_ceil(2);
        bytes.0.resize(length, 0);
        vm.heap
            .adjust(0, length.div_ceil(2) as isize - previous as isize);
    }
    vm.check_heap_bounds()
}
