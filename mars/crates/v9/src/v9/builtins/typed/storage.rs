/** Buffer storage operations share synchronization while ArrayBuffer brand checks stay separate. */
use super::*;

/** Views accept either backing kind and read the current synchronized length of shared storage. */
pub(in crate::v9) fn storage_len(vm: &Vm, id: ObjectId) -> Option<usize> {
    match vm.heap.get(id).map(|object| &object.class) {
        Some(Class::ArrayBuffer(bytes)) => Some(bytes.0.len()),
        Some(Class::SparseWasmBuffer(bytes)) => Some(bytes.length),
        Some(Class::SharedArrayBuffer(bytes)) => Some(bytes.len()),
        _ => None,
    }
}
/** An entire typed element is copied under one shared lock, preserving no-tear integer access. */
pub(in crate::v9) fn read_range(
    vm: &Vm,
    id: ObjectId,
    start: usize,
    length: usize,
) -> Option<Vec<u8>> {
    match vm.heap.get(id).map(|object| &object.class) {
        Some(Class::ArrayBuffer(bytes)) => bytes
            .0
            .get(start..start.checked_add(length)?)
            .map(<[u8]>::to_vec),
        Some(Class::SharedArrayBuffer(bytes)) => bytes.read(start, length),
        Some(Class::SparseWasmBuffer(bytes)) => bytes.read(start, length),
        _ => None,
    }
}
/** Normal and shared writes use the same range checks, with synchronization only where necessary. */
pub(in crate::v9) fn write_range(vm: &mut Vm, id: ObjectId, start: usize, bytes: &[u8]) -> bool {
    if buffer_state::immutable(vm, id) {
        return false;
    }
    #[cfg(feature = "conformance")]
    let used_units = vm
        .heap
        .counts
        .units
        .saturating_add(vm.page.state.cpu_profile.units());
    #[cfg(not(feature = "conformance"))]
    let used_units = vm.heap.counts.units;
    let additional = match vm.heap.get(id).map(|object| &object.class) {
        Some(Class::SparseWasmBuffer(buffer)) => match buffer.additional(start, bytes) {
            Some(additional)
                if used_units.saturating_add(additional.div_ceil(2)) <= vm.limits.units =>
            {
                additional
            }
            _ => return false,
        },
        _ => 0,
    };
    let result = match vm.heap.get_mut(id).map(|object| &mut object.class) {
        Some(Class::ArrayBuffer(buffer)) => match start
            .checked_add(bytes.len())
            .and_then(|end| buffer.0.get_mut(start..end))
        {
            Some(target) => {
                target.copy_from_slice(bytes);
                true
            }
            None => false,
        },
        Some(Class::SharedArrayBuffer(buffer)) => buffer.write(start, bytes),
        Some(Class::SparseWasmBuffer(buffer)) => buffer.write(start, bytes),
        _ => false,
    };
    if result && additional > 0 {
        vm.heap.adjust(0, additional.div_ceil(2) as isize);
    }
    result
}
