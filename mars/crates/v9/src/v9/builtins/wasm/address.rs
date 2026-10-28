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
        .ok_or_else(|| {
            let count = m.imports.iter().filter(|import| matches!(import.kind, types::External::Memory(_))).count() + m.memories.len();
            Error::invalid(&format!("memory index {index} exceeds number of declared memories ({count})"))
        })
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
    index_with_range(
        vm,
        value,
        address64,
        "WebAssembly index must be an unsigned 64-bit BigInt",
    )
}
/** A descriptor can name the property whose converted BigInt is outside the supported range. */
pub(super) fn index_with_range(
    vm: &mut Vm,
    value: &JsValue,
    address64: bool,
    range_message: &str,
) -> Res<u64> {
    if !address64 {
        return memory::index(vm, value).map(u64::from);
    }
    if value.number_value().is_some() {
        let value = vm.to_text(value)?;
        return Err(vm.type_error(&format!("Cannot convert {value} to a BigInt")));
    }
    let value = super::super::bigint::to_bigint_full(vm, value)?.to_i128();
    let Some(value) = value.filter(|value| (0..=u64::MAX as i128).contains(value)) else {
        return Err(vm.type_error(range_message));
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
        return Err(vm.type_error("WebAssembly descriptor 'initial' is required"));
    }
    let min = index(vm, &initial, address64)?;
    let maximum = vm.get(value, &key("maximum"))?;
    let max = if maximum == JsValue::Undefined {
        None
    } else {
        Some(index(vm, &maximum, address64)?)
    };
    if min > u64::from(bound) || max.is_some_and(|max| max > u64::from(bound)) {
        return Err(vm.range_error("WebAssembly descriptor limit is above the upper bound"));
    }
    if max.is_some_and(|max| max < min) {
        return Err(vm.range_error("WebAssembly descriptor maximum is below the lower bound"));
    }
    Ok(types::Limits {
        min,
        max,
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
    if delta > u64::from(u32::MAX) {
        return Err(vm.range_error("WebAssembly.Memory.grow(): Unable to grow instance memory"));
    }
    memory::resize(vm, memory, delta as u32)
}
