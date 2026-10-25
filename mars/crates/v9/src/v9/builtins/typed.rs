/**
 * ArrayBuffer, the eleven typed array kinds and DataView. Elements are little-endian bytes in the
 * buffer object; reads and writes go through the integer-indexed hooks property access calls, so
 * views over one buffer share their bytes. Buffer bytes count against the string budget (two bytes
 * per UTF-16 unit) and one buffer holds at most BUFFER_BYTES.
 */
use super::*;
use crate::v9::value::{to_int32, to_uint32};
use crate::v9::vm::data::{Bytes, Class, ElementKind, ViewData};
use crate::v9::vm::heap::{CONFIGURABLE, Object};
use crate::v9::vm::realm::{NativeFn, Static, realm, string, symbol};

/** Most bytes one ArrayBuffer holds: the size of the longest string V9 allows. */
const BUFFER_BYTES: usize = 2 * crate::constants::JS_STRING_UNITS;

/** Typed array kinds, constructor names and element sizes, in `ElementKind` order. */
const KINDS: [(ElementKind, &str, usize); 11] = [
    (ElementKind::Int8, "Int8Array", 1),
    (ElementKind::Uint8, "Uint8Array", 1),
    (ElementKind::Uint8Clamped, "Uint8ClampedArray", 1),
    (ElementKind::Int16, "Int16Array", 2),
    (ElementKind::Uint16, "Uint16Array", 2),
    (ElementKind::Int32, "Int32Array", 4),
    (ElementKind::Uint32, "Uint32Array", 4),
    (ElementKind::Float32, "Float32Array", 4),
    (ElementKind::Float64, "Float64Array", 8),
    (ElementKind::BigInt64, "BigInt64Array", 8),
    (ElementKind::BigUint64, "BigUint64Array", 8),
];

/** Bytes per element; a DataView addresses single bytes. */
pub(in crate::v9) fn element_size(kind: ElementKind) -> usize {
    KINDS
        .iter()
        .find(|(each, ..)| *each == kind)
        .map_or(1, |(_, _, size)| *size)
}

/** Install ArrayBuffer, %TypedArray%.prototype, every kind and DataView. */
pub(super) fn install(b: &mut Builder, array_methods: &[(&'static str, u32, NativeFn)]) {
    let buffer = b.object(Some(b.ids.object_prototype));
    b.ids.array_buffer_prototype = buffer;
    let ctor = b.constructor("ArrayBuffer", 1, construct_buffer, buffer, None);
    b.global("ArrayBuffer", ctor);
    b.method(ctor, "isView", 1, is_view);
    b.builtin_accessor(buffer, "byteLength", Some(buffer_length), None);
    b.method(buffer, "slice", 2, buffer_slice);
    tag(b, buffer, "ArrayBuffer");
    species_getter(b, ctor);
    let typed = b.object(Some(b.ids.object_prototype));
    for name in ["length", "byteLength", "byteOffset", "buffer"] {
        b.builtin_accessor(typed, name, Some(view_get), None);
    }
    for (name, length, function) in array_methods {
        let method = b.method(typed, name, *length, *function);
        if *name == "values" {
            b.put(
                typed,
                Key::Sym(symbol::ITERATOR),
                Static::Value(JsValue::Object(method)),
                crate::v9::vm::heap::HIDDEN_ATTRS,
            );
        }
    }
    let methods: [(&'static str, u32, NativeFn); 9] = [
        ("toReversed", 0, typed_to_reversed),
        ("toSorted", 1, typed_to_sorted),
        ("with", 2, typed_with),
        ("set", 1, typed_set),
        ("subarray", 2, subarray),
        ("slice", 2, typed_slice),
        ("map", 1, typed_map),
        ("filter", 1, typed_filter),
        ("sort", 1, typed_sort),
    ];
    for (name, length, function) in methods {
        b.method(typed, name, length, function);
    }
    let base = b.constructor("TypedArray", 0, abstract_typed, typed, None);
    species_getter(b, base);
    let tag_getter = b.function("get [Symbol.toStringTag]", 0, typed_tag);
    b.put(
        typed,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Accessor(Some(tag_getter), None),
        CONFIGURABLE,
    );
    for (index, (_, name, size)) in KINDS.iter().enumerate() {
        let proto = b.object(Some(typed));
        b.ids.view_prototypes[index] = proto;
        let ctor = b.constructor(name, 3, construct_typed, proto, Some(base));
        b.value(ctor, "BYTES_PER_ELEMENT", JsValue::Number(*size as f64), 0);
        b.value(proto, "BYTES_PER_ELEMENT", JsValue::Number(*size as f64), 0);
        b.method(ctor, "from", 1, typed_from);
        b.method(ctor, "of", 0, typed_of);
        b.global(name, ctor);
    }
    data_view(b);
}

/** Symbol.toStringTag on a prototype. */
fn tag(b: &mut Builder, target: u32, name: &'static str) {
    b.put(
        target,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string(name)),
        CONFIGURABLE,
    );
}

/** `get [Symbol.species]() { return this }` on a constructor (ArraySpeciesCreate's hook). */
pub(super) fn species_getter(b: &mut Builder, ctor: ObjectId) {
    let getter = b.function("get [Symbol.species]", 0, species);
    b.put(
        ctor,
        Key::Sym(symbol::SPECIES),
        Static::Accessor(Some(getter), None),
        CONFIGURABLE,
    );
}

/** The species getter: the receiver itself. */
fn species(_: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(call.this)
}

/** %TypedArray%.prototype[Symbol.toStringTag]: the kind's name, or undefined for non-views. */
fn typed_tag(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let view = call.this.object().and_then(|id| view_of(vm, id));
    Ok(
        match view.and_then(|view| KINDS.iter().find(|(kind, ..)| *kind == view.kind)) {
            Some((_, name, _)) => string(name),
            None => JsValue::Undefined,
        },
    )
}

/** DataView and its typed accessors. */
fn data_view(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.ids.view_prototypes[KINDS.len()] = proto;
    let ctor = b.constructor("DataView", 1, construct_data_view, proto, None);
    b.global("DataView", ctor);
    for name in ["byteLength", "byteOffset", "buffer"] {
        b.builtin_accessor(proto, name, Some(view_get), None);
    }
    for name in [
        "getInt8",
        "getUint8",
        "getInt16",
        "getUint16",
        "getInt32",
        "getUint32",
        "getFloat32",
        "getFloat64",
        "getBigInt64",
        "getBigUint64",
    ] {
        b.method(proto, name, 1, data_view_get);
    }
    for name in [
        "setInt8",
        "setUint8",
        "setInt16",
        "setUint16",
        "setInt32",
        "setUint32",
        "setFloat32",
        "setFloat64",
        "setBigInt64",
        "setBigUint64",
    ] {
        b.method(proto, name, 2, data_view_set);
    }
    tag(b, proto, "DataView");
}

/** The name of the native being called (constructors are called through their intrinsic). */
fn callee_name(call: &Call) -> &'static str {
    let realm = realm();
    let native = realm.native(call.callee).map(|def| def.name).or_else(|| {
        let index = realm.object(call.callee)?.call?;
        Some(realm.natives[index as usize].name)
    });
    native.unwrap_or("")
}

/** A new ArrayBuffer of `length` zero bytes. */
fn new_buffer(vm: &mut Vm, bytes: Vec<u8>) -> Res<ObjectId> {
    if bytes.len() > BUFFER_BYTES {
        return Err(vm.range_error("Array buffer allocation failed"));
    }
    let proto = realm().ids.array_buffer_prototype;
    vm.alloc(Object::new(Class::ArrayBuffer(Bytes(bytes)), Some(proto)))
}

/** ToIndex: a non-negative integer, or a RangeError. */
fn to_index(vm: &mut Vm, value: &JsValue, what: &str) -> Res<usize> {
    if matches!(value, JsValue::Undefined) {
        return Ok(0);
    }
    let integer = vm.to_integer(value)?;
    if !(0.0..=crate::constants::VM_MAX_SAFE_LENGTH).contains(&integer) {
        return Err(vm.range_error(&format!("Invalid {what}")));
    }
    Ok(integer as usize)
}

/** `new ArrayBuffer(length)`. */
fn construct_buffer(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(vm.type_error("Constructor ArrayBuffer requires 'new'"));
    }
    let length = to_index(vm, &call.arg(0), "array buffer length")?;
    if length > BUFFER_BYTES {
        return Err(vm.range_error("Array buffer allocation failed"));
    }
    Ok(JsValue::Object(new_buffer(vm, vec![0; length])?))
}

/** ArrayBuffer.isView. */
fn is_view(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let view = call.arg(0).object().and_then(|id| view_of(vm, id));
    Ok(JsValue::Boolean(view.is_some()))
}

/** The bytes of a buffer object. */
fn buffer_bytes<'a>(vm: &'a Vm<'_>, id: ObjectId) -> Option<&'a Vec<u8>> {
    match vm.heap.get(id).map(|object| &object.class) {
        Some(Class::ArrayBuffer(bytes)) => Some(&bytes.0),
        _ => None,
    }
}

/** The receiver's buffer, or a TypeError. */
fn this_buffer(vm: &mut Vm, call: &Call) -> Res<ObjectId> {
    match call
        .this
        .object()
        .filter(|id| buffer_bytes(vm, *id).is_some())
    {
        Some(id) => Ok(id),
        None => Err(vm.type_error("Receiver is not an ArrayBuffer")),
    }
}

/** ArrayBuffer.prototype.byteLength. */
fn buffer_length(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_buffer(vm, &call)?;
    Ok(JsValue::Number(
        buffer_bytes(vm, id).map_or(0, Vec::len) as f64
    ))
}

/** ArrayBuffer.prototype.slice(begin, end). */
fn buffer_slice(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_buffer(vm, &call)?;
    let length = buffer_bytes(vm, id).map_or(0, Vec::len);
    let start = vm.relative_index(&call.arg(0), length, 0)?;
    let end = vm.relative_index(&call.arg(1), length, length)?.max(start);
    let bytes = buffer_bytes(vm, id)
        .map(|bytes| bytes[start..end].to_vec())
        .unwrap_or_default();
    Ok(JsValue::Object(new_buffer(vm, bytes)?))
}

/** The view data of an object. */
pub(in crate::v9) fn view_of(vm: &Vm, id: ObjectId) -> Option<ViewData> {
    match vm.heap.get(id).map(|object| &object.class) {
        Some(Class::View(view)) => Some((**view).clone()),
        _ => None,
    }
}

/** A typed array (not a DataView) receiver. */
fn this_typed(vm: &mut Vm, call: &Call) -> Res<(ObjectId, ViewData)> {
    let found = call
        .this
        .object()
        .and_then(|id| view_of(vm, id).map(|view| (id, view)))
        .filter(|(_, view)| view.kind != ElementKind::DataView);
    found.ok_or_else(|| vm.type_error("this is not a typed array."))
}

/** length, byteLength, byteOffset and buffer of a view. */
fn view_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(view) = call.this.object().and_then(|id| view_of(vm, id)) else {
        return Err(vm.type_error("Receiver is not a typed array or DataView"));
    };
    let size = element_size(view.kind);
    Ok(match callee_name(&call) {
        "length" => JsValue::Number(view.length as f64),
        "byteLength" => JsValue::Number((view.length * size) as f64),
        "byteOffset" => JsValue::Number(view.offset as f64),
        _ => JsValue::Object(view.buffer),
    })
}

/** A new view of `kind` over `buffer`. */
fn new_view(
    vm: &mut Vm,
    kind: ElementKind,
    buffer: ObjectId,
    offset: usize,
    length: usize,
    proto: Option<ObjectId>,
) -> Res<ObjectId> {
    let index = KINDS
        .iter()
        .position(|(each, ..)| *each == kind)
        .unwrap_or(KINDS.len());
    let proto = proto.unwrap_or(realm().ids.view_prototypes[index]);
    let data = ViewData {
        kind,
        buffer,
        offset,
        length,
    };
    vm.pin(JsValue::Object(buffer));
    vm.alloc(Object::new(Class::View(Box::new(data)), Some(proto)))
}

/** A fresh typed array of `kind` holding `values`, converted element by element. */
fn typed_from_values(
    vm: &mut Vm,
    kind: ElementKind,
    values: &[JsValue],
    proto: Option<ObjectId>,
) -> Res<ObjectId> {
    let size = element_size(kind);
    let length = values.len();
    if length.saturating_mul(size) > BUFFER_BYTES {
        return Err(vm.range_error("Invalid typed array length"));
    }
    let buffer = new_buffer(vm, vec![0; length * size])?;
    let id = new_view(vm, kind, buffer, 0, length, proto)?;
    for (index, value) in values.iter().enumerate() {
        store(vm, id, index, value)?;
    }
    Ok(id)
}

/** %TypedArray% itself cannot be called or constructed. */
fn abstract_typed(vm: &mut Vm, _: Call) -> Res<JsValue> {
    Err(vm.type_error("Abstract class TypedArray not directly constructable"))
}

/** `new Uint8Array(length | buffer, byteOffset, length | array-like | iterable)`. */
fn construct_typed(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let name = callee_name(&call);
    let Some(target) = call.new_target.clone() else {
        return Err(vm.type_error(&format!("Constructor {name} requires 'new'")));
    };
    let (kind, _, size) = KINDS
        .iter()
        .copied()
        .find(|(_, each, _)| *each == name)
        .unwrap_or(KINDS[1]);
    let index = KINDS
        .iter()
        .position(|(each, ..)| *each == kind)
        .unwrap_or(0);
    let proto = vm.prototype_for(&target, realm().ids.view_prototypes[index])?;
    let first = call.arg(0);
    let Some(source) = first.object() else {
        let length = to_index(vm, &first, "typed array length")?;
        if length.saturating_mul(size) > BUFFER_BYTES {
            return Err(vm.range_error("Invalid typed array length"));
        }
        let buffer = new_buffer(vm, vec![0; length * size])?;
        return Ok(JsValue::Object(new_view(
            vm,
            kind,
            buffer,
            0,
            length,
            Some(proto),
        )?));
    };
    if let Some(bytes) = buffer_bytes(vm, source).map(Vec::len) {
        let offset = to_index(vm, &call.arg(1), "typed array offset")?;
        if offset % size != 0 || offset > bytes {
            return Err(vm.range_error(&format!(
                "start offset of {name} should be a multiple of {size}"
            )));
        }
        let length = match call.arg(2) {
            JsValue::Undefined if (bytes - offset) % size != 0 => {
                return Err(vm.range_error(&format!(
                    "byte length of {name} should be a multiple of {size}"
                )));
            }
            JsValue::Undefined => (bytes - offset) / size,
            value => to_index(vm, &value, "typed array length")?,
        };
        if offset + length * size > bytes {
            return Err(vm.range_error(&format!("Invalid typed array length: {length}")));
        }
        return Ok(JsValue::Object(new_view(
            vm,
            kind,
            source,
            offset,
            length,
            Some(proto),
        )?));
    }
    let values = elements_of(vm, &first)?;
    Ok(JsValue::Object(typed_from_values(
        vm,
        kind,
        &values,
        Some(proto),
    )?))
}

/** The elements of an iterable or array-like, as typed array construction reads them. */
fn elements_of(vm: &mut Vm, source: &JsValue) -> Res<Vec<JsValue>> {
    let iterator = vm.get(source, &Key::Sym(symbol::ITERATOR))?;
    if !iterator.nullish() {
        return vm.iterate_to_vec(source);
    }
    let length = vm.length_of(source)?;
    if length > vm.limits.array_length {
        return Err(vm.range_error("Invalid typed array length"));
    }
    let mut out = Vec::with_capacity(length);
    for index in 0..length {
        let value = vm.get(source, &key(&index.to_string()))?;
        vm.pin(value.clone());
        out.push(value);
    }
    Ok(out)
}

/** The kind a typed array constructor (the receiver of from/of) makes. */
fn kind_of_constructor(vm: &mut Vm, constructor: &JsValue) -> Res<(ElementKind, ObjectId)> {
    let proto = vm.get(constructor, &key("prototype"))?;
    let found = realm()
        .ids
        .view_prototypes
        .iter()
        .take(KINDS.len())
        .position(|each| proto == JsValue::Object(*each));
    match (found, proto) {
        (Some(index), JsValue::Object(proto)) => Ok((KINDS[index].0, proto)),
        _ => Err(vm.type_error("this is not a typed array constructor")),
    }
}

/** Uint8Array.from(source, mapFn, thisArg). */
fn typed_from(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (kind, proto) = kind_of_constructor(vm, &call.this)?;
    let mut values = elements_of(vm, &call.arg(0))?;
    let mapper = call.arg(1);
    if !mapper.nullish() {
        for (index, value) in values.iter_mut().enumerate() {
            *value = vm.call(
                mapper.clone(),
                call.arg(2),
                vec![value.clone(), JsValue::Number(index as f64)],
            )?;
            vm.pin(value.clone());
        }
    }
    Ok(JsValue::Object(typed_from_values(
        vm,
        kind,
        &values,
        Some(proto),
    )?))
}

/** Uint8Array.of(...values). */
fn typed_of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (kind, proto) = kind_of_constructor(vm, &call.this)?;
    Ok(JsValue::Object(typed_from_values(
        vm,
        kind,
        call.args,
        Some(proto),
    )?))
}

/** Element `index` of a typed array, or None past its length (or for a forged, short buffer). */
pub(in crate::v9) fn load(vm: &Vm, id: ObjectId, index: usize) -> Option<JsValue> {
    let view = match vm.heap.get(id).map(|object| &object.class) {
        Some(Class::View(view)) if view.kind != ElementKind::DataView => view,
        _ => return None,
    };
    if index >= view.length {
        return None;
    }
    let size = element_size(view.kind);
    let start = view.offset.checked_add(index * size)?;
    let bytes = buffer_bytes(vm, view.buffer)?.get(start..start + size)?;
    Some(decode(view.kind, bytes, true))
}

/** Decode one element from `bytes` (little-endian unless `little` is false). */
fn decode(kind: ElementKind, bytes: &[u8], little: bool) -> JsValue {
    let mut raw = [0u8; 8];
    raw[..bytes.len()].copy_from_slice(bytes);
    if !little {
        raw[..bytes.len()].reverse();
    }
    let number = |value: f64| JsValue::Number(value);
    match kind {
        ElementKind::Int8 => number(f64::from(raw[0] as i8)),
        ElementKind::Uint8 | ElementKind::Uint8Clamped | ElementKind::DataView => {
            number(f64::from(raw[0]))
        }
        ElementKind::Int16 => number(f64::from(i16::from_le_bytes([raw[0], raw[1]]))),
        ElementKind::Uint16 => number(f64::from(u16::from_le_bytes([raw[0], raw[1]]))),
        ElementKind::Int32 => number(f64::from(i32::from_le_bytes([
            raw[0], raw[1], raw[2], raw[3],
        ]))),
        ElementKind::Uint32 => number(f64::from(u32::from_le_bytes([
            raw[0], raw[1], raw[2], raw[3],
        ]))),
        ElementKind::Float32 => number(f64::from(f32::from_le_bytes([
            raw[0], raw[1], raw[2], raw[3],
        ]))),
        ElementKind::Float64 => number(f64::from_le_bytes(raw)),
        ElementKind::BigInt64 => JsValue::BigInt(i128::from(i64::from_le_bytes(raw))),
        ElementKind::BigUint64 => JsValue::BigInt(i128::from(u64::from_le_bytes(raw))),
    }
}

/** Convert a value to one element's little-endian bytes (this may run script valueOf). */
fn encode(vm: &mut Vm, kind: ElementKind, value: &JsValue) -> Res<Vec<u8>> {
    if matches!(kind, ElementKind::BigInt64 | ElementKind::BigUint64) {
        let big = super::bigint::to_bigint(vm, value)?;
        return Ok((big as i64).to_le_bytes().to_vec());
    }
    let number = vm.to_number(value)?;
    Ok(match kind {
        ElementKind::Int8 | ElementKind::Uint8 | ElementKind::DataView => {
            vec![to_int32(number) as u8]
        }
        ElementKind::Uint8Clamped => vec![clamp(number)],
        ElementKind::Int16 | ElementKind::Uint16 => {
            (to_int32(number) as u16).to_le_bytes().to_vec()
        }
        ElementKind::Int32 | ElementKind::Uint32 => to_uint32(number).to_le_bytes().to_vec(),
        ElementKind::Float32 => (number as f32).to_le_bytes().to_vec(),
        _ => number.to_le_bytes().to_vec(),
    })
}

/** ToUint8Clamp: round half to even into 0..=255. */
fn clamp(number: f64) -> u8 {
    if number.is_nan() || number <= 0.0 {
        return 0;
    }
    if number >= 255.0 {
        return 255;
    }
    let floor = number.floor();
    let rounded = match number - floor {
        half if half > 0.5 => floor + 1.0,
        half if half < 0.5 => floor,
        _ if floor % 2.0 == 0.0 => floor,
        _ => floor + 1.0,
    };
    rounded as u8
}

/** Write element `index`; out-of-range writes are ignored, as integer-indexed objects do. */
pub(in crate::v9) fn store(vm: &mut Vm, id: ObjectId, index: usize, value: &JsValue) -> Res<()> {
    let Some(view) = view_of(vm, id) else {
        return Ok(());
    };
    let encoded = encode(vm, view.kind, value)?;
    write_bytes(vm, &view, index * element_size(view.kind), &encoded);
    Ok(())
}

/** Copy `bytes` into a view at a byte position inside it, if they fit. */
fn write_bytes(vm: &mut Vm, view: &ViewData, at: usize, bytes: &[u8]) {
    let limit = view.length * element_size(view.kind);
    if at + bytes.len() > limit {
        return;
    }
    let start = view.offset + at;
    if let Some(Class::ArrayBuffer(buffer)) =
        vm.heap.get_mut(view.buffer).map(|object| &mut object.class)
        && let Some(target) = buffer.0.get_mut(start..start + bytes.len())
    {
        target.copy_from_slice(bytes);
    }
}

/** Every element of a typed array, in order. */
fn values_of(vm: &Vm, id: ObjectId, length: usize) -> Vec<JsValue> {
    (0..length)
        .filter_map(|index| load(vm, id, index))
        .collect()
}

/** %TypedArray%.prototype.set(source, offset). */
fn typed_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    let offset = to_index(vm, &call.arg(1), "offset")?;
    let source = call.arg(0);
    let values = match source
        .object()
        .and_then(|other| view_of(vm, other).map(|v| (other, v)))
    {
        Some((other, other_view)) => values_of(vm, other, other_view.length),
        None => elements_of(vm, &source)?,
    };
    if offset + values.len() > view.length {
        return Err(vm.range_error("offset is out of bounds"));
    }
    for (index, value) in values.iter().enumerate() {
        store(vm, id, offset + index, value)?;
    }
    Ok(JsValue::Undefined)
}

/** %TypedArray%.prototype.toReversed: a fresh array of the same kind, reversed. */
fn typed_to_reversed(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    let mut values = values_of(vm, id, view.length);
    values.reverse();
    Ok(JsValue::Object(typed_from_values(
        vm, view.kind, &values, None,
    )?))
}

/** %TypedArray%.prototype.toSorted: validates the comparator first, then sorts a copy. */
fn typed_to_sorted(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let comparator = call.arg(0);
    if !matches!(comparator, JsValue::Undefined) && !vm.is_callable(&comparator) {
        return Err(vm.type_error("The comparison function must be either a function or undefined"));
    }
    let (id, view) = this_typed(vm, &call)?;
    let values = values_of(vm, id, view.length);
    let copy = typed_from_values(vm, view.kind, &values, None)?;
    let sort_call = Call {
        this: JsValue::Object(copy),
        ..call
    };
    typed_sort(vm, sort_call)
}

/**
 * %TypedArray%.prototype.with(index, value): the value converts (ToNumber or ToBigInt) before
 * the index is checked against the length, and an out-of-range index is a RangeError.
 */
fn typed_with(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    let relative = vm.to_integer(&call.arg(0))?;
    let length = view.length as f64;
    let actual = if relative >= 0.0 {
        relative
    } else {
        length + relative
    };
    let value = match view.kind {
        ElementKind::BigInt64 | ElementKind::BigUint64 => {
            JsValue::BigInt(super::bigint::to_bigint(vm, &call.arg(1))?)
        }
        _ => JsValue::Number(vm.to_number(&call.arg(1))?),
    };
    let length = view_of(vm, id).map_or(0, |view| view.length);
    if !(0.0..length as f64).contains(&actual) {
        return Err(vm.range_error("Invalid typed array index"));
    }
    let mut values = values_of(vm, id, length);
    values[actual as usize] = value;
    Ok(JsValue::Object(typed_from_values(
        vm, view.kind, &values, None,
    )?))
}

/** %TypedArray%.prototype.subarray(begin, end): a view sharing this one's bytes. */
fn subarray(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    let start = vm.relative_index(&call.arg(0), view.length, 0)?;
    let end = vm
        .relative_index(&call.arg(1), view.length, view.length)?
        .max(start);
    let proto = vm.proto_of(id);
    let offset = view.offset + start * element_size(view.kind);
    Ok(JsValue::Object(new_view(
        vm,
        view.kind,
        view.buffer,
        offset,
        end - start,
        proto,
    )?))
}

/** %TypedArray%.prototype.slice(begin, end): a copy. */
fn typed_slice(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    let start = vm.relative_index(&call.arg(0), view.length, 0)?;
    let end = vm
        .relative_index(&call.arg(1), view.length, view.length)?
        .max(start);
    let values: Vec<JsValue> = (start..end)
        .filter_map(|index| load(vm, id, index))
        .collect();
    let proto = vm.proto_of(id);
    Ok(JsValue::Object(typed_from_values(
        vm, view.kind, &values, proto,
    )?))
}

/** Apply `callback(value, index, array)` to each element. */
fn each_result(
    vm: &mut Vm,
    call: &Call,
    id: ObjectId,
    length: usize,
) -> Res<Vec<(JsValue, JsValue)>> {
    let callback = call.arg(0);
    if !vm.is_callable(&callback) {
        return Err(vm.type_error("callback is not a function"));
    }
    let mut out = Vec::with_capacity(length);
    for index in 0..length {
        let value = load(vm, id, index).unwrap_or(JsValue::Undefined);
        let result = vm.call(
            callback.clone(),
            call.arg(1),
            vec![
                value.clone(),
                JsValue::Number(index as f64),
                JsValue::Object(id),
            ],
        )?;
        vm.pin(result.clone());
        out.push((value, result));
    }
    Ok(out)
}

/** %TypedArray%.prototype.map: the same kind, holding the callback's results. */
fn typed_map(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    let results: Vec<JsValue> = each_result(vm, &call, id, view.length)?
        .into_iter()
        .map(|(_, result)| result)
        .collect();
    let proto = vm.proto_of(id);
    Ok(JsValue::Object(typed_from_values(
        vm, view.kind, &results, proto,
    )?))
}

/** %TypedArray%.prototype.filter: the same kind, holding the kept elements. */
fn typed_filter(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    let kept: Vec<JsValue> = each_result(vm, &call, id, view.length)?
        .into_iter()
        .filter(|(_, keep)| keep.truthy())
        .map(|(value, _)| value)
        .collect();
    let proto = vm.proto_of(id);
    Ok(JsValue::Object(typed_from_values(
        vm, view.kind, &kept, proto,
    )?))
}

/** %TypedArray%.prototype.sort: numeric order by default, else the comparator's. */
fn typed_sort(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    let mut values = values_of(vm, id, view.length);
    let comparator = call.arg(0);
    if comparator.nullish() {
        values.sort_by(|a, b| match (a, b) {
            (JsValue::Number(a), JsValue::Number(b)) => a.total_cmp(b),
            (JsValue::BigInt(a), JsValue::BigInt(b)) => a.cmp(b),
            _ => std::cmp::Ordering::Equal,
        });
    } else {
        // Insertion sort keeps every comparator call fallible and bounded by the operation budget.
        for index in 1..values.len() {
            let mut at = index;
            while at > 0 {
                let order = vm.call(
                    comparator.clone(),
                    JsValue::Undefined,
                    vec![values[at - 1].clone(), values[at].clone()],
                )?;
                if vm.to_number(&order)? <= 0.0 {
                    break;
                }
                values.swap(at - 1, at);
                at -= 1;
            }
        }
    }
    for (index, value) in values.iter().enumerate() {
        store(vm, id, index, value)?;
    }
    Ok(JsValue::Object(id))
}

/** `new DataView(buffer, byteOffset, byteLength)`. */
fn construct_data_view(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target.clone() else {
        return Err(vm.type_error("Constructor DataView requires 'new'"));
    };
    let Some((buffer, bytes)) = call
        .arg(0)
        .object()
        .and_then(|id| buffer_bytes(vm, id).map(|b| (id, b.len())))
    else {
        return Err(vm.type_error("First argument to DataView constructor must be an ArrayBuffer"));
    };
    let offset = to_index(vm, &call.arg(1), "DataView offset")?;
    if offset > bytes {
        return Err(vm.range_error("Start offset is outside the bounds of the buffer"));
    }
    let length = match call.arg(2) {
        JsValue::Undefined => bytes - offset,
        value => to_index(vm, &value, "DataView length")?,
    };
    if offset + length > bytes {
        return Err(vm.range_error("Invalid DataView length"));
    }
    let proto = vm.prototype_for(&target, realm().ids.view_prototypes[KINDS.len()])?;
    Ok(JsValue::Object(new_view(
        vm,
        ElementKind::DataView,
        buffer,
        offset,
        length,
        Some(proto),
    )?))
}

/** The element kind a DataView accessor name (getInt16, setFloat64...) addresses. */
fn accessor_kind(name: &str) -> ElementKind {
    let kind = &name[3..];
    KINDS
        .iter()
        .find(|(_, each, _)| each.strip_suffix("Array") == Some(kind))
        .map_or(ElementKind::Uint8, |(each, ..)| *each)
}

/** The receiver DataView, the addressed kind and the checked byte position. */
fn data_view_target(vm: &mut Vm, call: &Call) -> Res<(ViewData, ElementKind, usize)> {
    let Some(view) = call
        .this
        .object()
        .and_then(|id| view_of(vm, id))
        .filter(|view| view.kind == ElementKind::DataView)
    else {
        return Err(vm.type_error("Receiver is not a DataView"));
    };
    let kind = accessor_kind(callee_name(call));
    let at = to_index(vm, &call.arg(0), "DataView offset")?;
    if at + element_size(kind) > view.length {
        return Err(vm.range_error("Offset is outside the bounds of the DataView"));
    }
    Ok((view, kind, at))
}

/** DataView.prototype.getInt8 ... getBigUint64(byteOffset, littleEndian). */
fn data_view_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (view, kind, at) = data_view_target(vm, &call)?;
    let little = call.arg(1).truthy();
    let start = view.offset + at;
    let size = element_size(kind);
    let bytes = buffer_bytes(vm, view.buffer)
        .and_then(|bytes| bytes.get(start..start + size))
        .map(<[u8]>::to_vec);
    match bytes {
        Some(bytes) => Ok(decode(kind, &bytes, little)),
        None => Err(vm.range_error("Offset is outside the bounds of the DataView")),
    }
}

/** DataView.prototype.setInt8 ... setBigUint64(byteOffset, value, littleEndian). */
fn data_view_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (view, kind, at) = data_view_target(vm, &call)?;
    let mut bytes = encode(vm, kind, &call.arg(1))?;
    bytes.truncate(element_size(kind));
    if !call.arg(2).truthy() {
        bytes.reverse();
    }
    write_bytes(vm, &view, at, &bytes);
    Ok(JsValue::Undefined)
}

/** Most bytes one getRandomValues call fills, as browsers bound it. */
const RANDOM_BYTES: usize = 65_536;

/**
 * crypto.getRandomValues(integerTypedArray): fills the view from the operating system's CSPRNG,
 * never from the page's seeded Math.random, and returns the same array.
 */
pub(in crate::v9) fn random_values(vm: &mut Vm, call: Call) -> Res<JsValue> {
    use ring::rand::SecureRandom;
    let target = call.arg(0);
    let view = target
        .object()
        .and_then(|id| view_of(vm, id))
        .filter(|view| {
            !matches!(
                view.kind,
                ElementKind::Float32 | ElementKind::Float64 | ElementKind::DataView
            )
        });
    let Some(view) = view else {
        return Err(vm.type_error("Failed to execute 'getRandomValues' on 'Crypto': parameter 1 is not an integer typed array."));
    };
    let length = view.length * element_size(view.kind);
    if length > RANDOM_BYTES {
        return Err(vm.dom_exception("QuotaExceededError", "Failed to execute 'getRandomValues' on 'Crypto': the requested length exceeds 65536 bytes."));
    }
    let mut bytes = vec![0u8; length];
    ring::rand::SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| {
            crate::protocol::EngineError::unsupported(
                "The operating system random source is unavailable to V9.",
            )
        })?;
    write_bytes(vm, &view, 0, &bytes);
    Ok(target)
}

/** A new Uint8Array holding `bytes`. */
pub(in crate::v9) fn new_uint8(vm: &mut Vm, bytes: Vec<u8>) -> Res<ObjectId> {
    let length = bytes.len();
    let buffer = new_buffer(vm, bytes)?;
    new_view(vm, ElementKind::Uint8, buffer, 0, length, None)
}

/** The bytes a BufferSource (ArrayBuffer, typed array or DataView) covers. */
pub(in crate::v9) fn source_bytes(vm: &Vm, value: &JsValue) -> Option<Vec<u8>> {
    let id = value.object()?;
    if let Some(bytes) = buffer_bytes(vm, id) {
        return Some(bytes.clone());
    }
    let view = view_of(vm, id)?;
    let length = view.length * element_size(view.kind);
    buffer_bytes(vm, view.buffer)?
        .get(view.offset..view.offset + length)
        .map(<[u8]>::to_vec)
}

/** Write `bytes` at the start of a Uint8Array, as far as they fit; returns how many fit. */
pub(in crate::v9) fn fill_uint8(vm: &mut Vm, value: &JsValue, bytes: &[u8]) -> Option<usize> {
    let view = value.object().and_then(|id| view_of(vm, id))?;
    if !matches!(view.kind, ElementKind::Uint8 | ElementKind::Uint8Clamped) {
        return None;
    }
    let count = bytes.len().min(view.length);
    write_bytes(vm, &view, 0, &bytes[..count]);
    Some(count)
}
