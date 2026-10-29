/** Wasm GC references are opaque V9 heap objects whose fields participate in ordinary precise GC. */
use super::*;
use gc_types::{Field, Kind};
use types::Instruction;
pub(super) mod atomic;
mod descriptors;
mod segments;
pub(super) mod shared;
mod typing;
/** Expose only the checked static contract through the GC domain facade. */
pub(super) fn signature(
    m: &Module,
    op: u32,
    args: &[i64],
) -> Result<(Vec<Type>, Vec<Type>), Error> {
    if matches!(op, 0 | 1 | 32..=36) {
        return descriptors::signature(m, op, args[0] as u32, args.get(1) == Some(&1));
    }
    if op >= 0x80 {
        return strings::signature(m, op, args);
    }
    typing::signature(m, op, args)
}

/** Casts preserve the function, external, exception and internal reference hierarchy boundaries. */
pub(super) fn same_hierarchy(m: &Module, a: Type, b: Type) -> Result<bool, Error> {
    if gc_types::is_shared(m, a) != gc_types::is_shared(m, b) {
        return Ok(false);
    }
    let a = a
        .reference()
        .ok_or_else(|| Error::invalid("cast source must be a reference"))?
        .0;
    let b = b
        .reference()
        .ok_or_else(|| Error::invalid("cast target must be a reference"))?
        .0;
    Ok(typing::root(m, a)? == typing::root(m, b)?)
}

/** GC opcodes retain checked type/field/segment indices or a signed heap-type immediate. */
pub(super) fn decode(r: &mut binary::Reader) -> Result<Instruction, Error> {
    let op = r.prefixed_opcode()?;
    if (32..=38).contains(&op) && !r.custom_descriptors_enabled {
        return Err(Error::invalid(
            "custom descriptor instruction requires --wasm-custom-descriptors",
        ));
    }
    if op == 31 && !r.shared_enabled {
        return Err(Error::invalid(
            "shared i31 instruction requires --wasm-shared",
        ));
    }
    if op == 0x4c {
        if !r.ref_cast_nop_enabled {
            return Err(Error::invalid(
                "Invalid opcode 0xfb4c (enable with --wasm-ref-cast-nop)",
            ));
        }
        let ty = gc_types::heap(r, false)?;
        let (heap, _) = ty.reference().unwrap();
        return Ok(Instruction::Gc(
            op,
            vec![
                heap,
                i64::from(matches!(ty, Type::Exact(..))),
                i64::from(matches!(ty, Type::Shared(..))),
            ],
        ));
    }
    if op > u8::MAX as u32 {
        return Err(Error::invalid(&format!(
            "invalid stringref opcode: fb{op:x}"
        )));
    }
    if op >= 0x80 {
        if !r.stringref_enabled {
            return Err(Error::invalid(
                "stringref instruction requires --wasm-stringref",
            ));
        }
        return strings::decode(r, op);
    }
    let count = match op {
        0 | 1 | 6 | 7 | 11..=14 | 16 | 32..=34 => 1,
        2..=5 | 8..=10 | 17..=19 => 2,
        15 | 26..=31 => 0,
        20..=23 | 35 | 36 => {
            let ty = gc_types::heap(r, false)?.with_nullability(if op <= 23 {
                op % 2 == 1
            } else {
                op == 36
            });
            let (heap, _) = ty.reference().unwrap();
            return Ok(Instruction::Gc(
                op,
                vec![
                    heap,
                    i64::from(matches!(ty, Type::Exact(..))),
                    i64::from(matches!(ty, Type::Shared(..))),
                ],
            ));
        }
        24 | 25 | 37 | 38 => {
            let flags = r.byte()?;
            if flags > 3 {
                return Err(Error::invalid("invalid br_on_cast flags"));
            }
            let depth = r.u32()?;
            let source = gc_types::heap(r, flags & 1 != 0)?;
            let target = gc_types::heap(r, flags & 2 != 0)?;
            return Ok(if op >= 37 {
                Instruction::BranchDescriptor(op, depth, source, target)
            } else {
                Instruction::BranchCast(op, depth, source, target)
            });
        }
        _ => return Err(Error::unsupported("WebAssembly GC instruction")),
    };
    let args = (0..count)
        .map(|_| r.u32().map(i64::from))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Instruction::Gc(op, args))
}
/** Nullability and the abstract roots are checked even when crossing JS or restored-state boundaries. */
pub(super) fn coerce(vm: &mut Vm, ty: Type, value: &JsValue) -> Res<JsValue> {
    let (heap, nullable) = ty
        .reference()
        .ok_or_else(|| vm.type_error("expected a Wasm reference type"))?;
    if *value == JsValue::Null {
        return if nullable {
            Ok(JsValue::Null)
        } else {
            Err(vm.type_error("type incompatibility when transforming from/to JS"))
        };
    }
    if heap == -17 && !matches!(ty, Type::Shared(..)) {
        return Ok(value.clone());
    }
    let valid = if matches!(ty, Type::Shared(..)) {
        if heap == -17 {
            shared_extern(vm, value)
        } else if let Some(root) = shared::handle(vm, value) {
            let (metadata, _, array, _) = root.metadata()?;
            if metadata.as_ref() == shared::WAIT_QUEUE_META {
                heap == -36
            } else {
                matches!(heap, -18 | -19) || heap == -21 && !array || heap == -22 && array
            }
        } else {
            (heap == -18 && shared_extern(vm, value)) || matches!(heap, -20..=-18) && i31(value)
        }
    } else {
        reference_allowed(vm, heap, value)
    };
    if valid {
        if matches!(ty, Type::Shared(..)) && matches!(heap, -17 | -18) {
            #[cfg(feature = "conformance")]
            if let JsValue::String(text) = value {
                return Ok(JsValue::String(vm.share_string(text)?));
            }
            #[cfg(feature = "conformance")]
            if !i31(value)
                && let Some(number) = value.number_value()
            {
                return crate::v9::builtins::new_shared_number(vm, number);
            }
        }
        Ok(value.clone())
    } else {
        Err(vm.type_error("type incompatibility when transforming from/to JS"))
    }
}
/** Testing a reference performs no error allocation and cannot hide a fatal allocation failure. */
fn reference_allowed(vm: &Vm, heap: i64, value: &JsValue) -> bool {
    if heap >= 0 {
        return instance::is_function(vm, value)
            || shared::handle(vm, value).is_some()
            || value
                .object()
                .is_some_and(|id| get(vm, id, "wasm.gc.module").is_some());
    }
    if matches!(heap, -19 | -21 | -22) && shared::handle(vm, value).is_some() {
        return false;
    }
    match heap {
        -17 | -18 => true,
        -19 => i31(value) || kind(vm, value).is_some(),
        -20 => i31(value),
        -21 => kind(vm, value) == Some(false),
        -22 => kind(vm, value) == Some(true),
        -23 => exceptions::is_reference(vm, value),
        -24 => value
            .object()
            .is_some_and(|id| get(vm, id, "wasm.cont.function").is_some()),
        -26 | -32 | -25 => matches!(value, JsValue::String(_)),
        -31 => value
            .object()
            .is_some_and(|id| get(vm, id, "wasm.string.iterator").is_some()),
        -16 => instance::is_function(vm, value),
        _ => false,
    }
}
/** Shared extern references must contain values that can cross an agent boundary. */
fn shared_extern(vm: &Vm, value: &JsValue) -> bool {
    if value.number_value().is_some()
        || matches!(
            value,
            JsValue::Undefined | JsValue::Boolean(_) | JsValue::String(_)
        )
        || shared::handle(vm, value).is_some()
    {
        return true;
    }
    #[cfg(feature = "conformance")]
    if value
        .object()
        .is_some_and(|id| crate::v9::builtins::is_shared_object(vm, id))
    {
        return true;
    }
    false
}
/** The immediate i31 representation preserves value identity and cannot allocate a foreign object. */
fn i31(value: &JsValue) -> bool {
    value.number_value().is_some_and(|n| {
        n.fract() == 0.0
            && (n != 0.0 || !n.is_sign_negative())
            && (-1_073_741_824.0..1_073_741_824.0).contains(&n)
    })
}
/** Crossing back into JavaScript exposes the original externref inside any.convert_extern. */
pub(super) fn expose(vm: &Vm, value: JsValue) -> JsValue {
    value
        .object()
        .and_then(|id| get(vm, id, "wasm.gc.extern"))
        .unwrap_or(value)
}
/** Objects record whether their inaccessible fields implement a struct or an array. */
fn kind(vm: &Vm, value: &JsValue) -> Option<bool> {
    let id = value.object()?;
    if let Some(root) = shared::handle(vm, value) {
        return root.metadata().ok().map(|(_, _, array, _)| array);
    }
    match get(vm, id, "wasm.gc.array") {
        Some(JsValue::Boolean(v)) => Some(v),
        _ => None,
    }
}
/** Concrete type checks reconstruct both module graphs and compare canonical shapes. */
pub(super) fn matches(vm: &mut Vm, m: &Module, ty: Type, value: &JsValue) -> Res<bool> {
    let Some((heap, nullable)) = ty.reference() else {
        return Ok(false);
    };
    if *value == JsValue::Null {
        return Ok(nullable);
    }
    if let Some(root) = shared::handle(vm, value) {
        if matches!(heap, -17 | -18) && !gc_types::is_shared(m, ty) {
            return Ok(true);
        }
        return shared::matches(vm, m, ty, &root);
    }
    if gc_types::is_shared(m, ty) {
        return Ok(if heap == -17 {
            shared_extern(vm, value)
        } else {
            (heap == -18 && shared_extern(vm, value)) || matches!(heap, -20..=-18) && i31(value)
        });
    }
    if heap < 0 {
        return Ok(reference_allowed(vm, heap, value));
    }
    if instance::is_function(vm, value) {
        if matches!(ty, Type::Exact(..)) {
            return instance::function_exact(vm, m, heap as u32, value);
        }
        return instance::function_matches(vm, m, heap as u32, value);
    }
    let Some(id) = value.object() else {
        return Ok(false);
    };
    let Some(source) = get(vm, id, "wasm.gc.module") else {
        return Ok(false);
    };
    let (_, actual) = module(vm, &source)?;
    let index = type_index(vm, id)?;
    if matches!(ty, Type::Exact(..)) {
        return Ok(gc_types::equivalent(&actual, index, m, heap as u32));
    }
    let mut current = Some(index);
    for _ in 0..=actual.definitions.len() {
        let Some(index) = current else {
            return Ok(false);
        };
        if gc_types::equivalent(&actual, index, m, heap as u32) {
            return Ok(true);
        }
        current = actual
            .definitions
            .get(index as usize)
            .and_then(|d| d.parent)
    }
    Ok(false)
}
/** A forged type index cannot choose executable code or index unchecked into a module graph. */
fn type_index(vm: &mut Vm, id: ObjectId) -> Res<u32> {
    match get(vm, id, "wasm.gc.type") {
        Some(JsValue::Number(n))
            if n.is_finite() && n >= 0.0 && n.fract() == 0.0 && n <= u32::MAX as f64 =>
        {
            Ok(n as u32)
        }
        _ => Err(vm.type_error("invalid Wasm GC type record")),
    }
}
/** Field arrays are concrete bounded heap arrays; author prototypes and accessors never run. */
fn fields_id(vm: &mut Vm, value: &JsValue) -> Res<ObjectId> {
    if *value == JsValue::Null {
        return Err(trap(vm, "dereferencing a null pointer"));
    }
    let Some(id) = value.object() else {
        return Err(vm.type_error("invalid Wasm GC reference"));
    };
    let fields = get(vm, id, "wasm.gc.fields")
        .and_then(|v| v.object())
        .ok_or_else(|| vm.type_error("invalid Wasm GC fields"))?;
    if !matches!(vm.heap.get(fields).map(|o| &o.class), Some(Class::Array(_))) {
        return Err(vm.type_error("invalid Wasm GC field storage"));
    }
    Ok(fields)
}
/** Inert field count is never taken from an authored length getter. */
fn length(vm: &mut Vm, value: &JsValue) -> Res<usize> {
    if let Some(root) = shared::handle(vm, value) {
        return Ok(root.metadata()?.3);
    }
    let fields = fields_id(vm, value)?;
    let Class::Array(values) = &vm.heap.get(fields).unwrap().class else {
        unreachable!()
    };
    Ok(values.len())
}
/** Every field access is checked, including state restored from a serialized candidate. */
fn read(vm: &mut Vm, value: &JsValue, index: usize) -> Res<JsValue> {
    if let Some(root) = shared::handle(vm, value) {
        return shared::read(vm, &root, index);
    }
    let fields = fields_id(vm, value)?;
    let Class::Array(values) = &vm.heap.get(fields).unwrap().class else {
        unreachable!()
    };
    values
        .get(index)
        .cloned()
        .ok_or_else(|| trap(vm, "array element access out of bounds"))
}
/** Internal writes use data-property definition instead of JavaScript setters. */
fn write(vm: &mut Vm, value: &JsValue, index: usize, item: JsValue) -> Res<()> {
    if let Some(root) = shared::handle(vm, value) {
        return shared::write(vm, &root, index, item);
    }
    let fields = fields_id(vm, value)?;
    if index >= length(vm, value)? {
        return Err(trap(vm, "array element access out of bounds"));
    }
    vm.define(fields, key(&index.to_string()), Descriptor::data(item))?;
    Ok(())
}
/** Packed fields truncate bits at storage time, independently of later signed/unsigned reads. */
fn packed(vm: &mut Vm, field: &Field, value: &JsValue) -> Res<JsValue> {
    let value = global::convert(vm, field.ty, value)?;
    Ok(match field.packed {
        Some(8) => JsValue::Number(f64::from(numeric::i32_value(&value)? as u8)),
        Some(16) => JsValue::Number(f64::from(numeric::i32_value(&value)? as u16)),
        _ => value,
    })
}
/** Allocation records retain the immutable module bytes so later casts can revalidate their types. */
fn allocate(
    vm: &mut Vm,
    instance: ObjectId,
    index: u32,
    array: bool,
    fields: Vec<JsValue>,
) -> Res<JsValue> {
    let module =
        get(vm, instance, INSTANCE).ok_or_else(|| vm.type_error("invalid Wasm GC instance"))?;
    allocate_record(vm, module, index, array, fields)
}
/** Field storage remains opaque and traced for both Wasm instructions and native string encoders. */
pub(in crate::v9::builtins::wasm) fn allocate_record(
    vm: &mut Vm,
    module: JsValue,
    index: u32,
    array: bool,
    fields: Vec<JsValue>,
) -> Res<JsValue> {
    if fields.len() > vm.limits.array_length {
        return Err(trap(vm, "requested new array is too large"));
    }
    vm.charge(fields.len())?;
    let (_, m) = self::module(vm, &module)?;
    if m.definitions.get(index as usize).is_some_and(|d| d.shared) {
        return shared::allocate(vm, &module, &m, index, array, fields);
    }
    let fields = vm.new_array(fields)?;
    let mut record = Object::new(Class::Plain, None);
    record.extensible = false;
    let object = vm.alloc(record)?;
    set(vm, object, "wasm.gc.module", module);
    set(
        vm,
        object,
        "wasm.gc.type",
        JsValue::Number(f64::from(index)),
    );
    set(vm, object, "wasm.gc.array", JsValue::Boolean(array));
    set(vm, object, "wasm.gc.fields", JsValue::Object(fields));
    Ok(JsValue::Object(object))
}
/** A vector of initializer operands is popped in declaration order. */
fn take(stack: &mut Vec<JsValue>, count: usize) -> Res<Vec<JsValue>> {
    if count > stack.len() {
        return Err(crate::protocol::EngineError::invalid("Invalid Wasm GC operand stack.").into());
    }
    Ok(stack.split_off(stack.len() - count))
}
/** Typed accesses reject nulls and unexpected object shapes before reading a native field. */
fn check(vm: &mut Vm, m: &Module, value: &JsValue, index: u32) -> Res<()> {
    if *value == JsValue::Null {
        return Err(trap(vm, "dereferencing a null pointer"));
    }
    if !matches(vm, m, Type::Ref(i64::from(index), false), value)? {
        return Err(vm.type_error("Wasm GC object type mismatch"));
    }
    Ok(())
}
/** Execute only statically checked GC instructions over owned heap values. */
pub(super) fn run(
    vm: &mut Vm,
    instance: ObjectId,
    m: &Module,
    op: u32,
    args: &[i64],
    stack: &mut Vec<JsValue>,
) -> Res<()> {
    if matches!(op, 0 | 1 | 32..=36) {
        let value = if op <= 1 {
            descriptors::construct(vm, instance, m, op, args[0] as u32, stack)?
        } else {
            return descriptors::run(vm, instance, m, op, args, stack);
        };
        stack.push(value);
        return Ok(());
    }
    if op >= 0x80 {
        return strings::run(vm, instance, m, op, args, stack);
    }
    if matches!(op, 9 | 10 | 18 | 19) {
        return segments::run(vm, instance, m, op, args, stack);
    }
    vm.charge(1)?;
    match op {
        0 | 1 => {
            let index = args[0] as u32;
            let fields = gc_types::fields(m, index).map_err(|e| compile_failure(vm, e))?;
            let values = if op == 0 {
                take(stack, fields.len())?
            } else {
                fields.iter().map(|f| global::default(f.ty)).collect()
            };
            let values = fields
                .iter()
                .zip(values)
                .map(|(f, v)| packed(vm, f, &v))
                .collect::<Res<Vec<_>>>()?;
            stack.push(allocate(vm, instance, index, false, values)?)
        }
        2..=4 => {
            let object = numeric::pop(stack)?;
            check(vm, m, &object, args[0] as u32)?;
            let field = &gc_types::fields(m, args[0] as u32).map_err(|e| compile_failure(vm, e))?
                [args[1] as usize];
            let value = read(vm, &object, args[1] as usize)?;
            stack.push(unpack(op == 3, field, value)?)
        }
        5 => {
            let value = numeric::pop(stack)?;
            let object = numeric::pop(stack)?;
            check(vm, m, &object, args[0] as u32)?;
            let field = &gc_types::fields(m, args[0] as u32).map_err(|e| compile_failure(vm, e))?
                [args[1] as usize];
            let value = packed(vm, field, &value)?;
            write(vm, &object, args[1] as usize, value)?
        }
        6 | 7 => {
            let index = args[0] as u32;
            let field = gc_types::element(m, index).map_err(|e| compile_failure(vm, e))?;
            let count = numeric::i32_value(&numeric::pop(stack)?)? as u32 as usize;
            if count > vm.limits.array_length {
                return Err(trap(vm, "requested new array is too large"));
            }
            let value = if op == 6 {
                numeric::pop(stack)?
            } else {
                global::default(field.ty)
            };
            let value = packed(vm, field, &value)?;
            stack.push(allocate(vm, instance, index, true, vec![value; count])?)
        }
        8 => {
            let index = args[0] as u32;
            let field = gc_types::element(m, index).map_err(|e| compile_failure(vm, e))?;
            let values = take(stack, args[1] as usize)?
                .iter()
                .map(|v| packed(vm, field, v))
                .collect::<Res<Vec<_>>>()?;
            stack.push(allocate(vm, instance, index, true, values)?)
        }
        11..=13 => {
            let index = numeric::i32_value(&numeric::pop(stack)?)? as u32 as usize;
            let object = numeric::pop(stack)?;
            check(vm, m, &object, args[0] as u32)?;
            let field = gc_types::element(m, args[0] as u32).map_err(|e| compile_failure(vm, e))?;
            let value = read(vm, &object, index)?;
            stack.push(unpack(op == 12, field, value)?)
        }
        14 => {
            let value = numeric::pop(stack)?;
            let index = numeric::i32_value(&numeric::pop(stack)?)? as u32 as usize;
            let object = numeric::pop(stack)?;
            check(vm, m, &object, args[0] as u32)?;
            let field = gc_types::element(m, args[0] as u32).map_err(|e| compile_failure(vm, e))?;
            let value = packed(vm, field, &value)?;
            write(vm, &object, index, value)?
        }
        15 => {
            let value = numeric::pop(stack)?;
            stack.push(JsValue::Number(length(vm, &value)? as f64))
        }
        16 => fill(vm, m, args[0] as u32, stack)?,
        17 => copy(vm, m, args, stack)?,
        0x4c => {}
        20..=23 => {
            let value = numeric::pop(stack)?;
            let target = if args.get(1) == Some(&1) {
                Type::Exact(args[0], op % 2 == 1)
            } else if args.get(2) == Some(&1) {
                Type::Shared(args[0], op % 2 == 1)
            } else {
                gc_types::reference(args[0], op % 2 == 1)
            };
            let yes = matches(vm, m, target, &value)?;
            if op <= 21 {
                stack.push(JsValue::Number(if yes { 1.0 } else { 0.0 }))
            } else if yes {
                stack.push(value)
            } else {
                return Err(trap(vm, "illegal cast"));
            }
        }
        26 => {
            let value = numeric::pop(stack)?;
            #[cfg(feature = "conformance")]
            let shared_number = matches!(value, JsValue::SharedNumber(_));
            #[cfg(not(feature = "conformance"))]
            let shared_number = false;
            if matches!(
                value,
                JsValue::Null
                    | JsValue::Undefined
                    | JsValue::Boolean(_)
                    | JsValue::BigInt(_)
                    | JsValue::BigIntWide(_)
            ) || i31(&value)
                || shared_number
                || matches!(value, JsValue::String(_))
                || kind(vm, &value).is_some()
            {
                stack.push(value)
            } else {
                let object = vm.new_object()?;
                set(vm, object, "wasm.gc.extern", value);
                stack.push(JsValue::Object(object))
            }
        }
        27 => {
            let value = numeric::pop(stack)?;
            stack.push(
                value
                    .object()
                    .and_then(|id| get(vm, id, "wasm.gc.extern"))
                    .unwrap_or(value),
            )
        }
        28 | 31 => {
            let value = numeric::i32_value(&numeric::pop(stack)?)?;
            stack.push(JsValue::Number(f64::from(value.wrapping_shl(1) >> 1)))
        }
        29 | 30 => {
            let value = numeric::pop(stack)?;
            if value == JsValue::Null {
                return Err(trap(vm, "dereferencing a null pointer"));
            }
            let value = numeric::i32_value(&value)?;
            stack.push(JsValue::Number(f64::from(if op == 29 {
                value
            } else {
                value & 0x7fff_ffff
            })))
        }
        _ => return Err(vm.unsupported_error("WebAssembly GC data/element segment instruction")),
    }
    Ok(())
}
/** Packed reads choose signed extension only for the signed opcode. */
fn unpack(signed: bool, field: &Field, value: JsValue) -> Res<JsValue> {
    if !signed {
        return Ok(value);
    }
    Ok(match field.packed {
        Some(8) => JsValue::Number((numeric::i32_value(&value)? as i8) as f64),
        Some(16) => JsValue::Number((numeric::i32_value(&value)? as i16) as f64),
        _ => value,
    })
}
/** Fill validates the whole destination before its first write, including the zero-length case. */
fn fill(vm: &mut Vm, m: &Module, index: u32, stack: &mut Vec<JsValue>) -> Res<()> {
    let count = numeric::i32_value(&numeric::pop(stack)?)? as u32 as usize;
    let value = numeric::pop(stack)?;
    let start = numeric::i32_value(&numeric::pop(stack)?)? as u32 as usize;
    let object = numeric::pop(stack)?;
    check(vm, m, &object, index)?;
    if start
        .checked_add(count)
        .is_none_or(|end| end > length(vm, &object).unwrap_or(0))
    {
        return Err(trap(vm, "array element access out of bounds"));
    }
    let field = gc_types::element(m, index).map_err(|e| compile_failure(vm, e))?;
    let value = packed(vm, field, &value)?;
    vm.charge(count)?;
    for at in start..start + count {
        write(vm, &object, at, value.clone())?
    }
    Ok(())
}
/** Copy snapshots the source so overlapping copies preserve memmove semantics. */
fn copy(vm: &mut Vm, m: &Module, args: &[i64], stack: &mut Vec<JsValue>) -> Res<()> {
    let count = numeric::i32_value(&numeric::pop(stack)?)? as u32 as usize;
    let from = numeric::i32_value(&numeric::pop(stack)?)? as u32 as usize;
    let source = numeric::pop(stack)?;
    let to = numeric::i32_value(&numeric::pop(stack)?)? as u32 as usize;
    let target = numeric::pop(stack)?;
    check(vm, m, &target, args[0] as u32)?;
    let target_len = length(vm, &target)?;
    if to.checked_add(count).is_none_or(|end| end > target_len) {
        return Err(trap(vm, "array element access out of bounds"));
    }
    check(vm, m, &source, args[1] as u32)?;
    let source_len = length(vm, &source)?;
    if from.checked_add(count).is_none_or(|end| end > source_len) {
        return Err(trap(vm, "array element access out of bounds"));
    }
    vm.charge(count)?;
    let values = (from..from + count)
        .map(|i| read(vm, &source, i))
        .collect::<Res<Vec<_>>>()?;
    for (i, value) in values.into_iter().enumerate() {
        write(vm, &target, to + i, value)?
    }
    Ok(())
}

/** String imports accept only owned mutable packed arrays and check the complete requested range. */
pub(super) fn string_storage(vm: &mut Vm, value: &JsValue, bits: u8, mutable: bool) -> Res<usize> {
    let Some(id) = value.object() else {
        return if *value == JsValue::Null {
            Err(trap(vm, "dereferencing a null pointer"))
        } else {
            Err(vm.type_error("expected a packed Wasm array"))
        };
    };
    if let Some(root) = shared::handle(vm, value) {
        let (bytes, index, array, length) = root.metadata()?;
        let module = binary::parse(&bytes).map_err(|e| compile_failure(vm, e))?;
        if !array
            || !matches!(module.definitions.get(index as usize).map(|d| &d.kind), Some(Kind::Array(f)) if f.packed == Some(bits) && (!mutable || f.mutable))
        {
            return Err(vm.type_error("expected a packed Wasm array"));
        }
        return Ok(length);
    }
    let source = get(vm, id, "wasm.gc.module")
        .ok_or_else(|| vm.type_error("expected a packed Wasm array"))?;
    let (_, m) = module(vm, &source)?;
    let index = type_index(vm, id)?;
    if kind(vm, value) != Some(true)
        || !matches!(m.definitions.get(index as usize).map(|d|&d.kind),Some(Kind::Array(f))if f.packed==Some(bits)&&(!mutable||f.mutable))
    {
        return Err(vm.type_error("expected a packed Wasm array"));
    }
    length(vm, value)
}
/** UTF decoding reads end-exclusive code units without invoking authored properties. */
pub(super) fn string_array(
    vm: &mut Vm,
    value: &JsValue,
    start: usize,
    end: usize,
    bits: u8,
) -> Res<Vec<u16>> {
    let length = string_storage(vm, value, bits, false)?;
    if start > end || end > length {
        return Err(trap(vm, "array element access out of bounds"));
    }
    vm.charge(end - start)?;
    (start..end)
        .map(|index| read(vm, value, index).and_then(|v| numeric::i32_value(&v).map(|n| n as u16)))
        .collect()
}
/** Encoders validate all destination bounds before mutating a packed array. */
pub(super) fn write_string_array(
    vm: &mut Vm,
    value: &JsValue,
    start: usize,
    units: &[u16],
    bits: u8,
) -> Res<()> {
    let length = string_storage(vm, value, bits, true)?;
    if start > length || units.len() > length - start {
        return Err(trap(vm, "array element access out of bounds"));
    }
    vm.charge(units.len())?;
    for (index, unit) in units.iter().enumerate() {
        write(vm, value, start + index, JsValue::Number(f64::from(*unit)))?
    }
    Ok(())
}

/** Encoder allocation validates the actual return array's packed type before retaining byte values. */
pub(super) fn byte_array(
    vm: &mut Vm,
    module_id: ObjectId,
    index: u32,
    bytes: &[u8],
) -> Res<JsValue> {
    let (_, m) = module(vm, &JsValue::Object(module_id))?;
    if !matches!(m.definitions.get(index as usize).map(|d|&d.kind),Some(Kind::Array(f))if f.packed==Some(8)&&f.mutable)
    {
        return Err(vm.type_error("invalid Wasm UTF8 array type"));
    }
    allocate_record(
        vm,
        JsValue::Object(module_id),
        index,
        true,
        bytes
            .iter()
            .map(|b| JsValue::Number(f64::from(*b)))
            .collect(),
    )
}

/** Branch casts compare the same checked descriptor identity as the unary cast operation. */
pub(super) fn descriptor_match(
    vm: &mut Vm,
    m: &Module,
    value: &JsValue,
    descriptor: &JsValue,
    target: Type,
) -> Res<bool> {
    descriptors::equality(vm, m, value, descriptor, target)
}
/** The static descriptor operand follows the indexed target and preserves requested exactness. */
pub(super) fn descriptor_type(m: &Module, target: Type) -> Result<Type, Error> {
    descriptors::descriptor_type(m, target)
}

/** Prototype setup accepts only the real final mutable primitive/reference array shapes. */
pub(super) fn interop_array(
    vm: &mut Vm,
    value: &JsValue,
    ty: Type,
    storage: Option<u8>,
) -> Res<()> {
    let Some(id) = value.object() else {
        return Err(trap(vm, "illegal cast"));
    };
    let Some(source) = get(vm, id, "wasm.gc.module") else {
        return Err(trap(vm, "illegal cast"));
    };
    let (_, m) = module(vm, &source)?;
    let index = type_index(vm, id)?;
    if kind(vm, value) != Some(true)
        || !matches!(m.definitions.get(index as usize),Some(d)if d.final_type&&d.parent.is_none()&&matches!(&d.kind,Kind::Array(f)if f.mutable&&f.ty==ty&&f.packed==storage))
    {
        return Err(trap(vm, "illegal cast"));
    }
    Ok(())
}
/** Live array reads preserve mutations performed by earlier proxy traps during prototype setup. */
pub(super) fn interop_read(vm: &mut Vm, value: &JsValue, index: usize) -> Res<JsValue> {
    read(vm, value, index)
}
/** Opaque array lengths use resident checked fields without observable JavaScript property reads. */
pub(super) fn interop_length(vm: &mut Vm, value: &JsValue) -> Res<usize> {
    length(vm, value)
}
