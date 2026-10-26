/** Atomic instructions execute indivisibly within V9's serialized owned-memory execution turn. */
use super::*;
use types::Instruction;
mod shared;

/** Every atomic opcode has a fixed natural width and integer result type. */
fn shape(op: u32) -> Result<(usize, Type), Error> {
    if op <= 2 {
        return Ok((if op == 2 { 8 } else { 4 }, Type::I32));
    }
    if !(0x10..=0x4e).contains(&op) {
        return Err(Error::invalid("invalid atomic opcode"));
    }
    let lane = (op - 0x10) % 7;
    Ok((
        [4, 8, 1, 2, 1, 2, 4][lane as usize],
        if matches!(lane, 0 | 2 | 3) {
            Type::I32
        } else {
            Type::I64
        },
    ))
}
/** Atomic memargs require exact natural alignment; fence has one reserved zero byte. */
pub(super) fn decode(r: &mut binary::Reader, m: &Module) -> Result<Instruction, Error> {
    let op = r.u32()?;
    if (0x5c..=0x71).contains(&op) {
        return gc::atomic::decode(r, op);
    }
    if op == 3 {
        if r.byte()? > 1 {
            return Err(Error::invalid("invalid atomic fence immediate"));
        }
        return Ok(Instruction::Atomic(op, 0, 0));
    }
    if op == 4 {
        return Ok(Instruction::Atomic(op, 0, 0));
    }
    let (width, _) = shape(op)?;
    let (align, index, offset) = r.ordered_memarg(m, Some(op >= 0x1e))?;
    if align != width.trailing_zeros() {
        return Err(Error::invalid(
            "atomic alignment must equal natural alignment",
        ));
    }
    Ok(Instruction::Atomic(op, index, offset))
}
/** Stack typing is independent of whether the memory is shared, as required by the Wasm grammar. */
pub(super) fn signature(m: &Module, op: u32, index: u32) -> Result<(Vec<Type>, Vec<Type>), Error> {
    if matches!(op, 3 | 4) {
        return Ok((vec![], vec![]));
    }
    if validate::memory_count(m) == 0 {
        return Err(Error::invalid("atomic instruction requires a memory"));
    }
    let (_, ty) = shape(op)?;
    let address = super::address::ty(m, index)?;
    let inputs = match op {
        0 => vec![address, Type::I32],
        1 => vec![address, Type::I32, Type::I64],
        2 => vec![address, Type::I64, Type::I64],
        0x10..=0x16 => vec![address],
        0x48..=0x4e => vec![address, ty, ty],
        _ => vec![address, ty],
    };
    let outputs = if (0x17..=0x1d).contains(&op) {
        vec![]
    } else {
        vec![ty]
    };
    Ok((inputs, outputs))
}
/** Integer operand extraction retains all 64 bits without passing through floating-point numbers. */
fn integer(value: &JsValue, ty: Type) -> Res<u64> {
    if ty == Type::I64 {
        numeric::i64_value(value).map(|n| n as u64)
    } else {
        numeric::i32_value(value).map(|n| n as u32 as u64)
    }
}
/** One atomic step checks alignment and bounds before a read or any mutation. */
pub(super) fn run(
    vm: &mut Vm,
    instance: ObjectId,
    op: u32,
    index: u32,
    offset: u64,
    stack: &mut Vec<JsValue>,
) -> Res<()> {
    if op == 3 {
        crate::v9::vm::data::SharedBytes::fence();
        return Ok(());
    }
    if op == 4 {
        std::thread::yield_now();
        return Ok(());
    }
    let (width, ty) = shape(op).map_err(|e| compile_failure(vm, e))?;
    let stored = if op <= 2 || op >= 0x17 {
        Some(numeric::pop(stack)?)
    } else {
        None
    };
    let expected = if matches!(op, 1 | 2 | 0x48..=0x4e) {
        Some(numeric::pop(stack)?)
    } else {
        None
    };
    let value = numeric::pop(stack)?;
    let memory = instance::item(vm, instance, MEMORIES, index)?;
    let at = address::effective(vm, &memory, &value, offset)?;
    if at % width != 0 {
        return Err(trap(vm, "operation does not support unaligned accesses"));
    }
    let storage = memory::storage(vm, &memory)?;
    if let Some(bytes) = shared_memory::backing(vm, storage) {
        return shared::run(vm, bytes, op, width, ty, at, stored, expected, stack);
    }
    let bytes = memory::read(vm, &memory, at, width)?;
    if matches!(op, 1 | 2) {
        return Err(trap(vm, "Atomics.wait cannot be called in this context"));
    }
    if op == 0 {
        stack.push(JsValue::Number(0.0));
        return Ok(());
    }
    let mut data = [0; 8];
    data[..width].copy_from_slice(&bytes);
    let old = u64::from_le_bytes(data);
    if let Some(value) = stored {
        let value = integer(&value, ty)?;
        let expected = expected
            .as_ref()
            .map(|v| integer(v, ty))
            .transpose()?
            .unwrap_or(0);
        let next = calculate(op, old, value, expected, width);
        memory::write(vm, &memory, at, &next.to_le_bytes()[..width])?;
    }
    push_result(op, ty, old, stack);
    Ok(())
}
/** Arithmetic is reduced to the addressed lane width when the resulting bytes are written. */
fn calculate(op: u32, old: u64, value: u64, expected: u64, width: usize) -> u64 {
    if op < 0x1e {
        return value;
    }
    match (op - 0x1e) / 7 {
        0 => old.wrapping_add(value),
        1 => old.wrapping_sub(value),
        2 => old & value,
        3 => old | value,
        4 => old ^ value,
        5 => value,
        6 => {
            let mask = if width == 8 {
                u64::MAX
            } else {
                (1u64 << (width * 8)) - 1
            };
            if old == (expected & mask) { value } else { old }
        }
        _ => unreachable!("validated atomic opcode"),
    }
}
/** Integer results preserve the exact 64-bit bit pattern across the BigInt JS boundary. */
fn push_result(op: u32, ty: Type, old: u64, stack: &mut Vec<JsValue>) {
    if !(0x17..=0x1d).contains(&op) {
        stack.push(if ty == Type::I64 {
            JsValue::BigInt((old as i64) as i128)
        } else {
            JsValue::Number(f64::from(old as i32))
        })
    }
}
