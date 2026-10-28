/** Shared memories use synchronized backing stores and fixed JavaScript wrapper windows across growth. */
use super::*;
use crate::v9::vm::data::SharedBytes;

/** Reserve only the existing VM byte allowance; the declared Wasm maximum remains separate metadata. */
pub(super) fn create(vm: &mut Vm, limits: types::Limits) -> Res<ObjectId> {
    let maximum = limits
        .max
        .ok_or_else(|| vm.type_error("Shared WebAssembly memory requires a maximum"))?;
    let length = limits.min as usize * PAGE_BYTES;
    let capacity = (maximum as usize).min(super::memory::page_limit(vm, limits.address64))
        .checked_mul(PAGE_BYTES)
        .ok_or_else(|| vm.range_error("WebAssembly memory allocation failed"))?;
    let backing = if capacity > BUFFER_BYTES {
        SharedBytes::new_sparse_wasm(length, capacity)?
    } else {
        SharedBytes::new(length, Some(capacity))?
    };
    wrapper(vm, backing.fixed_view(length).unwrap())
}
/** Native imports pass through the same broker grant boundary as SharedArrayBuffer cloning. */
pub(super) fn wrapper(vm: &mut Vm, backing: SharedBytes) -> Res<ObjectId> {
    let storage = super::super::import_shared_buffer(vm, backing)?;
    super::super::object::freeze_object(vm, storage)?;
    Ok(storage)
}
/** Access to the full backing belongs only to native memory instructions, never old fixed JS wrappers. */
pub(super) fn backing(vm: &Vm, storage: ObjectId) -> Option<SharedBytes> {
    match vm.heap.get(storage).map(|o| &o.class) {
        Some(Class::SharedArrayBuffer(bytes)) => Some(bytes.engine_view()),
        _ => None,
    }
}
/** A fresh fixed wrapper exposes growth while old wrappers retain their own immutable byte length. */
pub(super) fn refresh(vm: &mut Vm, memory: ObjectId, storage: ObjectId) -> Res<ObjectId> {
    let Some(Class::SharedArrayBuffer(bytes)) = vm.heap.get(storage).map(|o| &o.class) else {
        return Ok(storage);
    };
    let bytes = bytes.clone();
    if bytes.len() == bytes.backing_len() {
        return Ok(storage);
    }
    let next = wrapper(vm, bytes.fixed_view(bytes.backing_len()).unwrap())?;
    set(vm, memory, MEMORY, JsValue::Object(next));
    Ok(next)
}
/** Atomic growth returns the previous page count and never detaches shared wrappers. */
pub(super) fn grow(
    vm: &mut Vm,
    memory: ObjectId,
    storage: ObjectId,
    delta: u32,
    maximum: usize,
) -> Res<u32> {
    let bytes =
        backing(vm, storage).ok_or_else(|| vm.type_error("invalid shared memory backing"))?;
    if delta == 0 {
        refresh(vm, memory, storage)?;
        return Ok((bytes.backing_len() / PAGE_BYTES) as u32);
    }
    let old = bytes
        .grow_by(delta as usize * PAGE_BYTES, maximum * PAGE_BYTES)
        .ok_or_else(|| {
            vm.range_error("WebAssembly.Memory.grow exceeds maximum or V9 buffer limit")
        })?;
    if matches!(vm.heap.get(storage).map(|o|&o.class),Some(Class::SharedArrayBuffer(bytes)) if bytes.wasm_maximum().is_some())
    {
        return Ok((old / PAGE_BYTES) as u32);
    }
    let next = wrapper(vm, bytes.fixed_view(bytes.len()).unwrap())?;
    set(vm, memory, MEMORY, JsValue::Object(next));
    Ok((old / PAGE_BYTES) as u32)
}
