/** WebAssembly.Table retains genuine references in a heap array traced by V9's collector. */
use super::*;

/** Install table construction, indexed access, growth and length. */
pub(super) fn install(b: &mut Builder, namespace: ObjectId) -> ObjectId {
    let proto = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("Table", 1, construct, proto, None);
    b.alias(namespace, "Table", ctor);
    b.builtin_accessor(proto, "length", Some(length_get), None);
    b.method(proto, "get", 1, get_entry);
    b.method(proto, "set", 2, set_entry);
    b.method(proto, "grow", 1, grow);
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
    if limits.min as usize > capacity(vm) {
        return Err(vm.range_error("WebAssembly table exceeds V9 array limit"));
    }
    let value = global::convert(vm, ty, &value)?;
    let values = vm.new_array(vec![value; limits.min as usize])?;
    let object = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    set(vm, object, TABLE, JsValue::Object(values));
    set(vm, object, VALUE_TYPE, global::type_value(ty)?);
    set(
        vm,
        object,
        MAXIMUM,
        limits
            .max
            .map_or(JsValue::Undefined, |n| JsValue::Number(f64::from(n))),
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
        return Err(vm.type_error("WebAssembly.Table descriptor must be an object"));
    }
    let element = vm.get(&descriptor, &key("element"))?;
    let ty = global::value_type(vm, &element)?;
    if ty.reference().is_none() {
        return Err(vm.type_error("WebAssembly.Table requires a reference element type"));
    }
    let address64 = address::descriptor(vm, &descriptor)?;
    if address64 {
        return Err(vm.unsupported_error("WebAssembly table64"));
    }
    let limits = memory::limits(vm, &descriptor, u32::MAX)?;
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
/** Validate the array and reference type before every native table operation. */
fn record(vm: &mut Vm, table: &JsValue) -> Res<(ObjectId, ObjectId, Type)> {
    let object = brand(vm, table, TABLE)?;
    let values = get(vm, object, TABLE)
        .and_then(|v| v.object())
        .ok_or_else(|| vm.type_error("invalid WebAssembly table storage"))?;
    if !matches!(vm.heap.get(values).map(|o|&o.class),Some(Class::Array(v))if v.len()<=MAX_TABLE) {
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
    type_context::convert(vm, object, ty, &value)
}
/** Internal stores use CreateDataProperty, so Array.prototype setters never intercept Wasm state. */
pub(super) fn write(vm: &mut Vm, table: &JsValue, index: u32, value: &JsValue) -> Res<()> {
    let (object, values, ty) = record(vm, table)?;
    let value = type_context::convert(vm, object, ty, value)?;
    if index as usize >= length(vm, table)? {
        return Err(vm.range_error("WebAssembly.Table index out of bounds"));
    }
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
    let maximum = match get(vm, object, MAXIMUM) {
        Some(JsValue::Number(n)) if n.is_finite() && n >= 0.0 && n.fract() == 0.0 => n as usize,
        Some(JsValue::Undefined) => u32::MAX as usize,
        _ => return Err(vm.type_error("invalid WebAssembly table maximum")),
    };
    if next > maximum || next > capacity(vm) {
        return Err(vm.range_error("WebAssembly.Table.grow exceeds maximum or V9 array limit"));
    }
    for _ in length..next {
        vm.charge(1)?;
        vm.array_append(&JsValue::Object(values), value.clone())?;
    }
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
    let maximum = match get(vm, object, MAXIMUM) {
        Some(JsValue::Number(n)) => Some(n as u32),
        _ => None,
    };
    Ok(type_context::equivalent(vm, object, actual, m, ty)?
        && length >= required.min as usize
        && required.max.is_none_or(|r| maximum.is_some_and(|m| m <= r)))
}
/** Length accessor. */
fn length_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    length(vm, &call.this).map(|n| JsValue::Number(n as f64))
}
/** JS get accepts an unsigned index and returns the stored reference. */
fn get_entry(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let index = memory::index(vm, &call.arg(0))?;
    read(vm, &call.this, index)
}
/** JS set converts before storing. */
fn set_entry(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let index = memory::index(vm, &call.arg(0))?;
    write(vm, &call.this, index, &call.arg(1))?;
    Ok(JsValue::Undefined)
}
/** A missing fill value is undefined for externref and null for funcref. */
fn grow(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (_, _, ty) = record(vm, &call.this)?;
    let delta = memory::index(vm, &call.arg(0))?;
    let value = if call.args.len() > 1 {
        call.arg(1)
    } else if ty == Type::Extern {
        JsValue::Undefined
    } else {
        JsValue::Null
    };
    resize(vm, &call.this, delta, &value).map(|n| JsValue::Number(f64::from(n)))
}

/** Original conformance metadata only lowers the already bounded native table allocation ceiling. */
fn capacity(vm: &Vm) -> usize {
    #[cfg(feature = "conformance")]
    if let Some(limit) = vm.page.environment.conformance_wasm_max_table_entries {
        return MAX_TABLE.min(limit);
    }
    let _ = vm;
    MAX_TABLE
}
