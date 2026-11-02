/** Atomic GC operations over unshared objects execute without author callbacks in one VM turn. */
use super::*;

/** Both struct and array opcode families share identical storage and arithmetic rules. */
fn operation(op: u32) -> u32 {
    if op >= 0x67 { op - 0x67 } else { op - 0x5c }
}

/** The parser retains the concrete type and field index after validating the order enum. */
pub(in crate::v9::builtins::wasm) fn decode(
    r: &mut binary::Reader,
    op: u32,
) -> Result<Instruction, Error> {
    let ordering = r.byte()?;
    if ordering > 1 {
        return Err(Error::invalid("invalid atomic memory ordering"));
    }
    let index = r.u32()?;
    let field = if op < 0x67 { r.u32()? } else { 0 };
    Ok(Instruction::GcAtomic(op, index, field))
}

/** Fields are obtained only through the validated module graph, never authored properties. */
fn field(m: &Module, op: u32, index: u32, slot: u32) -> Result<&Field, Error> {
    if op >= 0x67 {
        return gc_types::element(m, index);
    }
    gc_types::fields(m, index)?
        .get(slot as usize)
        .ok_or_else(|| Error::invalid("unknown Wasm struct field"))
}

/** Packed get variants and reference exchanges have distinct standardized storage contracts. */
pub(in crate::v9::builtins::wasm) fn signature(
    m: &Module,
    op: u32,
    index: u32,
    slot: u32,
) -> Result<(Vec<Type>, Vec<Type>), Error> {
    let field = field(m, op, index, slot)?;
    let action = operation(op);
    let numeric = matches!(field.ty, Type::I32 | Type::I64);
    let heap = if action == 10 { -19 } else { -18 };
    let expected = if gc_types::is_shared(m, field.ty) {
        Type::Shared(heap, true)
    } else {
        Type::Ref(heap, true)
    };
    let reference = gc_types::subtype(m, field.ty, expected);
    let allowed = match action {
        1 | 2 => field.packed.is_some(),
        0 => numeric && field.packed.is_none() || reference,
        3 | 9 | 10 => numeric || reference,
        4..=8 => numeric,
        _ => false,
    };
    if !allowed {
        return Err(Error::invalid("invalid atomic GC storage type"));
    }
    if action >= 3 && !field.mutable {
        return Err(Error::invalid("atomic GC write requires mutable storage"));
    }
    let mut inputs = vec![Type::Ref(i64::from(index), true)];
    if op >= 0x67 {
        inputs.push(Type::I32);
    }
    if action == 10 {
        inputs.push(field.ty);
    }
    if action >= 3 {
        inputs.push(field.ty);
    }
    Ok((inputs, if action == 3 { vec![] } else { vec![field.ty] }))
}

/** Wrapping arithmetic uses integer bits; reference compare/exchange uses the opaque value identity. */
fn calculate(
    action: u32,
    ty: Type,
    old: &JsValue,
    value: &JsValue,
    expected: Option<&JsValue>,
) -> Res<JsValue> {
    if action == 10 {
        return Ok(if expected == Some(old) { value } else { old }.clone());
    }
    if action == 9 || action == 3 {
        return Ok(value.clone());
    }
    let wide = ty == Type::I64;
    let a = if wide {
        numeric::i64_value(old)? as u64
    } else {
        numeric::i32_value(old)? as u32 as u64
    };
    let b = if wide {
        numeric::i64_value(value)? as u64
    } else {
        numeric::i32_value(value)? as u32 as u64
    };
    let result = match action {
        4 => a.wrapping_add(b),
        5 => a.wrapping_sub(b),
        6 => a & b,
        7 => a | b,
        8 => a ^ b,
        _ => unreachable!(),
    };
    Ok(if wide {
        JsValue::BigInt(i128::from(result as i64))
    } else {
        JsValue::Number(result as u32 as i32 as f64)
    })
}

/** Bounds and null checks precede mutation; all reads/writes remain in the same owned execution turn. */
pub(in crate::v9::builtins::wasm) fn run(
    vm: &mut Vm,
    m: &Module,
    op: u32,
    index: u32,
    slot: u32,
    stack: &mut Vec<JsValue>,
) -> Res<()> {
    let action = operation(op);
    let value = if action >= 3 {
        Some(numeric::pop(stack)?)
    } else {
        None
    };
    let expected = if action == 10 {
        Some(numeric::pop(stack)?)
    } else {
        None
    };
    let position = if op >= 0x67 {
        numeric::i32_value(&numeric::pop(stack)?)? as u32 as usize
    } else {
        slot as usize
    };
    let object = numeric::pop(stack)?;
    check(vm, m, &object, index)?;
    let field = field(m, op, index, slot).map_err(|e| compile_failure(vm, e))?;
    if let Some(root) = shared::handle(vm, &object) {
        let old = shared::atomic(vm, &root, position, action, field, value, expected)?;
        if action != 3 {
            stack.push(unpack(action == 1, field, old)?);
        }
        return Ok(());
    }
    let old = read(vm, &object, position)?;
    if let Some(value) = value {
        let next = calculate(action, field.ty, &old, &value, expected.as_ref())?;
        let next = packed(vm, field, &next)?;
        write(vm, &object, position, next)?;
    }
    if action != 3 {
        stack.push(unpack(action == 1, field, old)?);
    }
    Ok(())
}
