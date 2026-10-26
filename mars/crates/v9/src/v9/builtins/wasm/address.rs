/** Memory32 and memory64 keep distinct numeric contracts while sharing checked bounded byte storage. */
use super::*;
pub(super) const MAX_MEMORY64_PAGES: u32 = 262_144;
const ADDRESS64: &str = "wasm.memory.address64";

/** Binary memory indices include imports before definitions and never use a table's address type. */
pub(super) fn limits(m: &Module, index: u32) -> Result<types::Limits, Error> {
    m.imports
        .iter()
        .filter_map(|i| match i.kind {
            types::External::Memory(l) => Some(l),
            _ => None,
        })
        .chain(m.memories.iter().copied())
        .nth(index as usize)
        .ok_or_else(|| Error::invalid("unknown memory index"))
}
/** Memory operands have the same width as their selected memory's declared index space. */
pub(super) fn ty(m: &Module, index: u32) -> Result<Type, Error> {
    Ok(if limits(m, index)?.address64 {
        Type::I64
    } else {
        Type::I32
    })
}
/** Private address metadata survives serialization and does not consult mutable public properties. */
pub(super) fn mark(vm: &mut Vm, memory: ObjectId, address64: bool) {
    set(vm, memory, ADDRESS64, JsValue::Boolean(address64))
}
/** A missing flag denotes the original memory32 representation for backwards-compatible inert state. */
pub(super) fn is64(vm: &Vm, memory: ObjectId) -> bool {
    get(vm, memory, ADDRESS64) == Some(JsValue::Boolean(true))
}
/** Descriptors read address before initial/maximum, and reject spellings other than i32 or i64. */
pub(super) fn descriptor(vm: &mut Vm, value: &JsValue) -> Res<bool> {
    if value.object().is_none() {
        return Err(vm.type_error("WebAssembly descriptor must be an object"));
    }
    let value = vm.get(value, &key("address"))?;
    if value == JsValue::Undefined {
        return Ok(false);
    }
    match vm.to_text(&value)?.as_str() {
        "i32" => Ok(false),
        "i64" => Ok(true),
        _ => Err(vm.type_error("WebAssembly address must be i32 or i64")),
    }
}
/** JS index conversion enforces the original unsigned range before narrowing to the page quota. */
pub(super) fn index(vm: &mut Vm, value: &JsValue, address64: bool) -> Res<u64> {
    if !address64 {
        return memory::index(vm, value).map(u64::from);
    }
    let value = super::super::bigint::to_bigint_full(vm, value)?.to_i128();
    let Some(value) = value.filter(|value| (0..=u64::MAX as i128).contains(value)) else {
        return Err(vm.type_error("WebAssembly index must be an unsigned 64-bit BigInt"));
    };
    Ok(value as u64)
}
/** JS constructors retain exact getter order and reject declared bounds beyond the binary type's limit. */
pub(super) fn read_limits(
    vm: &mut Vm,
    value: &JsValue,
    address64: bool,
    bound: u32,
) -> Res<types::Limits> {
    if value.object().is_none() {
        return Err(vm.type_error("WebAssembly descriptor must be an object"));
    }
    let initial = vm.get(value, &key("initial"))?;
    if initial == JsValue::Undefined {
        return Err(vm.type_error("WebAssembly descriptor requires initial"));
    }
    let min = index(vm, &initial, address64)?;
    let maximum = vm.get(value, &key("maximum"))?;
    let max = if maximum == JsValue::Undefined {
        None
    } else {
        Some(index(vm, &maximum, address64)?)
    };
    if min > u64::from(bound) || max.is_some_and(|max| max > u64::from(bound) || max < min) {
        return Err(vm.range_error("WebAssembly descriptor has invalid limits"));
    }
    Ok(types::Limits {
        min: min as u32,
        max: max.map(|v| v as u32),
        shared: false,
        address64,
    })
}
/** Runtime operands preserve unsigned i64 bits, including addresses greater than the signed range. */
pub(super) fn operand(vm: &Vm, memory: ObjectId, value: &JsValue) -> Res<u64> {
    if is64(vm, memory) {
        numeric::i64_value(value).map(|n| n as u64)
    } else {
        numeric::i32_value(value).map(|n| u64::from(n as u32))
    }
}
/** Address arithmetic checks overflow before converting to host usize or touching memory. */
pub(super) fn effective(vm: &mut Vm, memory: &JsValue, value: &JsValue, offset: u64) -> Res<usize> {
    let memory = brand(vm, memory, MEMORY)?;
    let base = operand(vm, memory, value)?;
    base.checked_add(offset)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| trap(vm, "memory access out of bounds"))
}
/** JS growth failures are RangeErrors; Wasm callers translate them into the typed minus-one result. */
pub(super) fn grow(vm: &mut Vm, memory: &JsValue, delta: u64) -> Res<u32> {
    if delta > MAX_PAGES as u64 {
        return Err(vm.range_error("WebAssembly.Memory.grow(): Unable to grow instance memory"));
    }
    memory::resize(vm, memory, delta as u32)
}
