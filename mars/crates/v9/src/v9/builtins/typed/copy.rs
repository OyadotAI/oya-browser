/** Typed array construction copies internal element data without consulting user iteration hooks. */
use super::*;

/** Number and BigInt content types cannot mix, including for empty input views. */
pub(super) fn from_view(
    vm: &mut Vm,
    kind: ElementKind,
    source: ViewData,
    prototype: ObjectId,
) -> Res<JsValue> {
    if source.invalid {
        return Err(
            vm.type_error("Cannot construct a typed array from a detached or out-of-bounds view")
        );
    }
    let big = |kind| matches!(kind, ElementKind::BigInt64 | ElementKind::BigUint64);
    if big(kind) != big(source.kind) {
        return Err(vm.type_error("Cannot mix BigInt and other types, use explicit conversions"));
    }
    let end = source.offset + source.length * element_size(source.kind);
    let bytes = read_range(vm, source.buffer, source.offset, end - source.offset)
        .ok_or_else(|| vm.type_error("Cannot construct a typed array from a detached buffer"))?;
    if kind == source.kind {
        return from_bytes(vm, kind, bytes, source.length, prototype);
    }
    let size = element_size(source.kind);
    let values: Vec<_> = bytes
        .chunks_exact(size)
        .map(|bytes| decode(source.kind, bytes, true))
        .collect();
    Ok(JsValue::Object(typed_from_values(
        vm,
        kind,
        &values,
        Some(prototype),
    )?))
}

/** Same-kind copies retain every NaN payload bit and allocate a distinct buffer. */
fn from_bytes(
    vm: &mut Vm,
    kind: ElementKind,
    bytes: Vec<u8>,
    length: usize,
    prototype: ObjectId,
) -> Res<JsValue> {
    let buffer = new_buffer(vm, bytes)?;
    Ok(JsValue::Object(new_view(
        vm,
        kind,
        buffer,
        0,
        length,
        Some(prototype),
    )?))
}
