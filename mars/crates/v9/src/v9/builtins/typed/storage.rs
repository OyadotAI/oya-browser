/** Buffer storage operations share synchronization while ArrayBuffer brand checks stay separate. */
use super::*;

/** Views accept either backing kind and read the current synchronized length of shared storage. */
pub(in crate::v9) fn storage_len(vm: &Vm, id: ObjectId) -> Option<usize> {
    match vm.heap.get(id).map(|object| &object.class) {
        Some(Class::ArrayBuffer(bytes)) => Some(bytes.0.len()),
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
        _ => None,
    }
}
/** Normal and shared writes use the same range checks, with synchronization only where necessary. */
pub(in crate::v9) fn write_range(vm: &mut Vm, id: ObjectId, start: usize, bytes: &[u8]) -> bool {
    match vm.heap.get_mut(id).map(|object| &mut object.class) {
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
        _ => false,
    }
}
