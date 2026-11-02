/** WebAssembly.Table retains genuine references in a heap array traced by V9's collector. */
use super::*;
use crate::v9::EngineError;
const ADDRESS64: &str = "wasm.table.address64";
/** Sparse table64 defaults retain one traced value per growth range. */
const RANGES: &str = "wasm.table.ranges";

/** Public table methods reject unbranded receivers before converting their arguments. */
fn check_receiver(vm: &mut Vm, value: &JsValue) -> Res<()> {
    if value
        .object()
        .is_some_and(|id| get(vm, id, TABLE).is_some())
    {
        Ok(())
    } else {
        Err(vm.type_error("Receiver is not a WebAssembly.Table"))
    }
}

/** Table indices include imports before module-defined tables. */
pub(super) fn address_type(m: &Module, index: u32) -> Result<Type, Error> {
    let limits = m
        .imports
        .iter()
        .filter_map(|i| match i.kind {
            types::External::Table(_, limits) => Some(limits),
            _ => None,
        })
        .chain(m.tables.iter().map(|(_, limits, _)| *limits))
        .nth(index as usize)
        .ok_or_else(|| Error::invalid("unknown table index"))?;
    Ok(if limits.address64 {
        Type::I64
    } else {
        Type::I32
    })
}

/** Runtime table operands use the unsigned bit pattern of their address type. */
pub(super) fn operand(vm: &mut Vm, table: &JsValue, value: &JsValue) -> Res<u64> {
    let object = brand(vm, table, TABLE)?;
    if is64(vm, object) {
        Ok(numeric::i64_value(value)? as u64)
    } else {
        Ok(u64::from(numeric::i32_value(value)? as u32))
    }
}

/** Narrow only after checking the entire operand, so large indices always trap. */
pub(super) fn operand_index(vm: &mut Vm, table: &JsValue, value: &JsValue) -> Res<u32> {
    let index = operand(vm, table, value)?;
    u32::try_from(index).map_err(|_| trap(vm, "table index is out of bounds"))
}

/** Install table construction, indexed access, growth and length. */
pub(super) fn install(b: &mut Builder, namespace: ObjectId) -> ObjectId {
    let proto = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("Table", 1, construct, proto, None);
    b.alias(namespace, "Table", ctor);
    let attrs = crate::v9::vm::heap::CONFIGURABLE | crate::v9::vm::heap::ENUMERABLE;
    b.builtin_accessor_with_attrs(proto, "length", Some(length_get), None, attrs);
    for (name, length, function) in [
        ("get", 1, get_entry as NativeFn),
        ("set", 1, set_entry as NativeFn),
        ("grow", 1, grow as NativeFn),
    ] {
        let method = b.function(name, length, function);
        b.value(
            proto,
            name,
            JsValue::Object(method),
            attrs | crate::v9::vm::heap::WRITABLE,
        );
    }
    proto
}
/** New tables enforce the same element capacity as ordinary V9 arrays. */
pub(super) fn create(
    vm: &mut Vm,
    ty: Type,
    limits: types::Limits,
    value: JsValue,
    proto: ObjectId,
) -> Res<ObjectId> {
    if limits.min > capacity(vm, limits.address64) as u64 {
        return Err(vm.range_error("WebAssembly table initial size is above the upper bound"));
    }
    let value = global::convert(vm, ty, &value)?;
    let values = vm.new_array_length(limits.min as usize)?;
    vm.pin(JsValue::Object(values));
    let ranges = Some(vm.new_array(vec![JsValue::Number(0.0), value])?);
    if let Some(ranges) = ranges {
        vm.pin(JsValue::Object(ranges));
    }
    let object = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    set(vm, object, TABLE, JsValue::Object(values));
    set(vm, object, VALUE_TYPE, global::type_value(ty)?);
    set(vm, object, ADDRESS64, JsValue::Boolean(limits.address64));
    if let Some(ranges) = ranges {
        set(vm, object, RANGES, JsValue::Object(ranges));
    }
    set(
        vm,
        object,
        MAXIMUM,
        limits.max.map_or(JsValue::Undefined, |n| {
            if limits.address64 {
                JsValue::BigInt(n as i128)
            } else {
                JsValue::Number(n as f64)
            }
        }),
    );
    Ok(object)
}
/** A funcref table cannot be initialized with an arbitrary JS function. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(vm.type_error("WebAssembly.Table must be invoked with 'new'"));
    }
    let descriptor = call.arg(0);
    if descriptor.object().is_none() {
        return Err(vm.type_error("WebAssembly.Table(): Argument 0 must be a table descriptor"));
    }
    let element = vm.get(&descriptor, &key("element"))?;
    let element = JsValue::String(vm.to_string(&element)?);
    let ty = match global::value_type(vm, &element) {
        Ok(ty) => ty,
        Err(Thrown::Value(_)) => {
            return Err(vm.type_error("WebAssembly.Table(): Descriptor property 'element' must be a WebAssembly reference type"));
        }
        Err(error) => return Err(error),
    };
    if ty == Type::Ref(-25, true) && !super::stringref_enabled(vm) {
        return Err(vm.type_error("WebAssembly.Table(): Descriptor property 'element' must be a WebAssembly reference type"));
    }
    if ty.reference().is_none() {
        return Err(vm.type_error("WebAssembly.Table(): Descriptor property 'element' must be a WebAssembly reference type"));
    }
    let address64 = address::descriptor(vm, &descriptor)?;
    let limits = descriptor_limits(vm, &descriptor, address64)?;
    if ty == Type::Ref(-25, true) && call.args.len() < 2 {
        return Err(vm.type_error(
            "WebAssembly.Table(): Missing initial value when creating stringref table",
        ));
    }
    let value = if call.args.len() > 1 {
        call.arg(1)
    } else if ty == Type::Extern {
        JsValue::Undefined
    } else {
        JsValue::Null
    };
    let proto = vm.prototype_for(call.new_target.as_ref().unwrap(), ids(vm).table)?;
    create(vm, ty, limits, value, proto).map(JsValue::Object)
}
/** Table limits preserve the full logical maximum without allocating it. */
fn descriptor_limits(vm: &mut Vm, descriptor: &JsValue, address64: bool) -> Res<types::Limits> {
    if !address64 {
        return memory::limits(vm, descriptor, u32::MAX);
    }
    let initial = vm.get(descriptor, &key("initial"))?;
    if initial == JsValue::Undefined {
        return Err(vm.type_error("WebAssembly descriptor requires initial"));
    }
    let min = address::index(vm, &initial, true)?;
    let maximum = vm.get(descriptor, &key("maximum"))?;
    let max = if maximum == JsValue::Undefined {
        None
    } else {
        Some(address::index_with_range(
            vm,
            &maximum,
            true,
            "WebAssembly.Table(): Property 'maximum' must be in u64 range",
        )?)
    };
    if max.is_some_and(|max| max < min) {
        return Err(vm.range_error("WebAssembly descriptor has invalid limits"));
    }
    Ok(types::Limits {
        min,
        max,
        shared: false,
        address64,
    })
}
/** Missing private metadata means the legacy table32 representation. */
pub(super) fn is64(vm: &Vm, object: ObjectId) -> bool {
    get(vm, object, ADDRESS64) == Some(JsValue::Boolean(true))
}
/** Read a private logical maximum without losing high bits to floating point. */
fn maximum(vm: &mut Vm, object: ObjectId) -> Res<Option<u64>> {
    match get(vm, object, MAXIMUM) {
        Some(JsValue::Undefined) => Ok(None),
        Some(JsValue::Number(n))
            if !is64(vm, object)
                && n.is_finite()
                && (0.0..=u32::MAX as f64).contains(&n)
                && n.fract() == 0.0 =>
        {
            Ok(Some(n as u64))
        }
        Some(JsValue::BigInt(n)) if is64(vm, object) && (0..=u64::MAX as i128).contains(&n) => {
            Ok(Some(n as u64))
        }
        _ => Err(vm.type_error("invalid WebAssembly table maximum")),
    }
}
/** Validate the array and reference type before every native table operation. */
fn record(vm: &mut Vm, table: &JsValue) -> Res<(ObjectId, ObjectId, Type)> {
    let object = brand(vm, table, TABLE)?;
    let values = get(vm, object, TABLE)
        .and_then(|v| v.object())
        .ok_or_else(|| vm.type_error("invalid WebAssembly table storage"))?;
    if !matches!(vm.heap.get(values).map(|o|&o.class),Some(Class::Array(v))if v.len()<=capacity(vm, is64(vm, object)))
    {
        return Err(vm.type_error("invalid WebAssembly table storage"));
    }
    let ty = get(vm, object, VALUE_TYPE).ok_or_else(|| vm.type_error("invalid table type"))?;
    let ty = global::record_type(vm, &ty)?;
    if ty.reference().is_none() {
        return Err(vm.type_error("invalid table reference type"));
    }
    Ok((object, values, ty))
}
/** Table indices are checked against current storage, not a serialized public length field. */
pub(super) fn length(vm: &mut Vm, table: &JsValue) -> Res<usize> {
    let (_, values, _) = record(vm, table)?;
    let Class::Array(v) = &vm.heap.get(values).unwrap().class else {
        unreachable!()
    };
    Ok(v.len())
}
/** Reads distinguish missing indices from the legal externref value undefined. */
pub(super) fn read(vm: &mut Vm, table: &JsValue, index: u32) -> Res<JsValue> {
    let (object, values, ty) = record(vm, table)?;
    let Class::Array(v) = &vm.heap.get(values).unwrap().class else {
        unreachable!()
    };
    let value = v
        .get(index as usize)
        .cloned()
        .ok_or_else(|| vm.range_error("WebAssembly.Table index out of bounds"))?;
    let value = if value == JsValue::Hole && get(vm, object, RANGES).is_some() {
        range_value(vm, object, index as usize)?
    } else {
        value
    };
    type_context::convert(vm, object, ty, &value)
}
/** Resolve an unwritten entry from the most recent traced growth range. */
fn range_value(vm: &mut Vm, object: ObjectId, index: usize) -> Res<JsValue> {
    let ranges = get(vm, object, RANGES)
        .and_then(|v| v.object())
        .ok_or_else(|| vm.type_error("invalid table ranges"))?;
    let Some(Class::Array(values)) = vm.heap.get(ranges).map(|o| &o.class) else {
        return Err(vm.type_error("invalid table ranges"));
    };
    if values.len() < 2 || values.len() % 2 != 0 {
        return Err(vm.type_error("invalid table ranges"));
    }
    let (mut low, mut high) = (0, values.len() / 2);
    while low < high {
        let mid = low + (high - low) / 2;
        let Some(JsValue::Number(start)) = values.get(mid * 2) else {
            return Err(vm.type_error("invalid table ranges"));
        };
        if !start.is_finite() || *start < 0.0 || start.fract() != 0.0 {
            return Err(vm.type_error("invalid table ranges"));
        }
        if *start <= index as f64 {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    if low == 0 {
        return Err(vm.type_error("invalid table ranges"));
    }
    Ok(values.get((low - 1) * 2 + 1).unwrap().clone())
}
/** Internal stores use CreateDataProperty, so Array.prototype setters never intercept Wasm state. */
pub(super) fn write(vm: &mut Vm, table: &JsValue, index: u32, value: &JsValue) -> Res<()> {
    let (object, values, ty) = record(vm, table)?;
    if index as usize >= length(vm, table)? {
        return Err(vm.range_error("WebAssembly.Table index out of bounds"));
    }
    let value = type_context::convert(vm, object, ty, value)?;
    vm.define(values, key(&index.to_string()), Descriptor::data(value))?;
    Ok(())
}
/** Growth is checked before any slots are appended. */
pub(super) fn resize(vm: &mut Vm, table: &JsValue, delta: u32, value: &JsValue) -> Res<u32> {
    let (object, values, ty) = record(vm, table)?;
    let value = type_context::convert(vm, object, ty, value)?;
    let length = length(vm, table)?;
    let next = length
        .checked_add(delta as usize)
        .ok_or_else(|| vm.range_error("WebAssembly.Table growth overflow"))?;
    let maximum = maximum(vm, object)?.unwrap_or(u64::MAX);
    if next as u64 > maximum || next > capacity(vm, is64(vm, object)) {
        return Err(vm.range_error("WebAssembly.Table.grow exceeds maximum or V9 array limit"));
    }
    if get(vm, object, RANGES).is_some() {
        return grow_sparse(vm, object, values, length, next, value);
    }
    if next > vm.limits.array_length {
        return Err(
            EngineError::limit("V9 resident array elements", vm.limits.array_length).into(),
        );
    }
    vm.charge(next - length)?;
    for _ in length..next {
        vm.array_append(&JsValue::Object(values), value.clone())?;
    }
    Ok(length as u32)
}
/** Grow a sparse table by recording one traced range and extending logical holes. */
fn grow_sparse(
    vm: &mut Vm,
    object: ObjectId,
    values: ObjectId,
    length: usize,
    next: usize,
    value: JsValue,
) -> Res<u32> {
    if next == length {
        return Ok(length as u32);
    }
    let ranges = get(vm, object, RANGES)
        .and_then(|v| v.object())
        .ok_or_else(|| vm.type_error("invalid table ranges"))?;
    let Some(Class::Array(entries)) = vm.heap.get(ranges).map(|o| &o.class) else {
        return Err(vm.type_error("invalid table ranges"));
    };
    if entries
        .resident_len()
        .checked_add(2)
        .is_none_or(|n| n > vm.limits.array_length)
    {
        return Err(
            EngineError::limit("V9 resident array elements", vm.limits.array_length).into(),
        );
    }
    vm.charge(2)?;
    vm.array_append(&JsValue::Object(ranges), JsValue::Number(length as f64))?;
    vm.array_append(&JsValue::Object(ranges), value)?;
    let Some(Class::Array(entries)) = vm.heap.get_mut(values).map(|o| &mut o.class) else {
        return Err(vm.type_error("invalid WebAssembly table storage"));
    };
    entries.resize_holes(next);
    Ok(length as u32)
}
/** Static table type and limits are checked at the linker boundary. */
pub(super) fn compatible(
    vm: &mut Vm,
    table: &JsValue,
    ty: Type,
    required: types::Limits,
    m: &Module,
) -> Res<bool> {
    let (object, _, actual) = record(vm, table)?;
    let length = length(vm, table)?;
    let maximum = maximum(vm, object)?;
    Ok(type_context::equivalent(vm, object, actual, m, ty)?
        && is64(vm, object) == required.address64
        && length >= required.min as usize
        && required.max.is_none_or(|r| maximum.is_some_and(|m| m <= r)))
}
/** Length accessor. */
fn length_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    check_receiver(vm, &call.this)?;
    let (object, _, _) = record(vm, &call.this)?;
    let n = length(vm, &call.this)?;
    Ok(if is64(vm, object) {
        JsValue::BigInt(n as i128)
    } else {
        JsValue::Number(n as f64)
    })
}
/** JS get accepts an unsigned index and returns the stored reference. */
fn get_entry(vm: &mut Vm, call: Call) -> Res<JsValue> {
    check_receiver(vm, &call.this)?;
    let (object, _, ty) = record(vm, &call.this)?;
    let index = address::index(vm, &call.arg(0), is64(vm, object))?;
    let size = length(vm, &call.this)?;
    if index >= size as u64 {
        let name = if ty == Type::Func {
            "funcref"
        } else {
            "externref"
        };
        return Err(vm.range_error(&format!(
            "invalid address {index} in {name} table of size {size}"
        )));
    }
    let index = u32::try_from(index)
        .map_err(|_| vm.range_error("WebAssembly.Table index out of bounds"))?;
    if !type_context::js_compatible(vm, object, ty)? {
        return Err(vm.type_error("invalid type for JavaScript WebAssembly table"));
    }
    let value = read(vm, &call.this, index)?;
    Ok(gc::expose(vm, value))
}
/** JS set converts before storing. */
fn set_entry(vm: &mut Vm, call: Call) -> Res<JsValue> {
    check_receiver(vm, &call.this)?;
    let (object, _, ty) = record(vm, &call.this)?;
    let index = address::index(vm, &call.arg(0), is64(vm, object))?;
    let size = length(vm, &call.this)?;
    if index >= size as u64 {
        let name = if ty == Type::Func {
            "funcref"
        } else {
            "externref"
        };
        return Err(vm.range_error(&format!(
            "invalid address {index} in {name} table of size {size}"
        )));
    }
    let index = u32::try_from(index)
        .map_err(|_| vm.range_error("WebAssembly.Table index out of bounds"))?;
    if !type_context::js_compatible(vm, object, ty)? {
        return Err(vm.type_error("invalid type for JavaScript WebAssembly table"));
    }
    if call.args.len() < 2
        && ty.reference().is_some_and(|(_, nullable)| !nullable)
        && (index as usize) < length(vm, &call.this)?
    {
        return Err(vm.type_error("Argument 1 must be specified for non-nullable element type"));
    }
    let value = if call.args.len() < 2 && ty == Type::Func {
        JsValue::Null
    } else {
        call.arg(1)
    };
    if (index as usize) < length(vm, &call.this)? && ty.reference().is_some() {
        match type_context::convert(vm, object, ty, &value) {
            Ok(_) => (),
            Err(Thrown::Value(_)) => return Err(vm.type_error(if instance::is_function(vm, &value) && ty.reference().is_some_and(|(heap, _)| heap >= 0) { "Argument 1 is invalid for table: assigned exported function has to be a subtype of the expected type" } else if ty == Type::Func { "Argument 1 is invalid for table: value must be null or a Wasm function object" } else { "Argument 1 is invalid for table: reference type mismatch" })),
            Err(fatal) => return Err(fatal),
        }
    }
    write(vm, &call.this, index, &value)?;
    Ok(JsValue::Undefined)
}
/** A missing fill value is undefined for externref and null for funcref. */
fn grow(vm: &mut Vm, call: Call) -> Res<JsValue> {
    check_receiver(vm, &call.this)?;
    let (object, _, ty) = record(vm, &call.this)?;
    let delta = address::index(vm, &call.arg(0), is64(vm, object))?;
    if call.args.len() < 2 && ty.reference().is_some_and(|(_, nullable)| !nullable) {
        return Err(vm.type_error("Argument 1 must be specified for non-nullable element type"));
    }
    let value = if call.args.len() > 1 {
        call.arg(1)
    } else if ty == Type::Extern {
        JsValue::Undefined
    } else {
        JsValue::Null
    };
    let value = type_context::convert(vm, object, ty, &value)?;
    let length = length(vm, &call.this)? as u64;
    let maximum = maximum(vm, object)?.unwrap_or(u64::MAX);
    if length
        .checked_add(delta)
        .is_none_or(|next| next > maximum || next > capacity(vm, is64(vm, object)) as u64)
    {
        return Err(vm.range_error(&format!(
            "WebAssembly.Table.grow(): failed to grow table by {delta}"
        )));
    }
    let delta = u32::try_from(delta)
        .map_err(|_| vm.range_error("WebAssembly.Table.grow exceeds maximum or V9 array limit"))?;
    let n = resize(vm, &call.this, delta, &value)?;
    Ok(if is64(vm, object) {
        JsValue::BigInt(n as i128)
    } else {
        JsValue::Number(f64::from(n))
    })
}

/** Original conformance metadata only lowers the already bounded native table allocation ceiling. */
fn capacity(vm: &Vm, address64: bool) -> usize {
    let maximum = MAX_DECLARED_TABLE_MINIMUM as usize;
    let _ = address64;
    #[cfg(feature = "conformance")]
    if let Some(limit) = vm.page.environment.conformance_wasm_max_table_entries {
        return maximum.min(limit);
    }
    let _ = vm;
    maximum
}
