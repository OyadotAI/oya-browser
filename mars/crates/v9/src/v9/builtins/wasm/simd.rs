/** Scalar-owned v128 SIMD: exact bytes, checked lanes, typed operands and ordinary memory bounds. */
use super::*;
use types::{Constant, Instruction};
mod extended;
mod half;

/** Vectors stay as eight opaque UTF-16 units inside Wasm; JS boundaries reject v128 signatures. */
pub(super) fn value(bytes: [u8; 16]) -> Res<JsValue> {
    let units = bytes
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    Ok(JsValue::String(JsString::from_units(units)?))
}
/** Native code never accepts an incorrectly sized vector from restored state. */
pub(super) fn vector(value: &JsValue) -> Res<[u8; 16]> {
    let JsValue::String(units) = value else {
        return Err(
            crate::protocol::EngineError::invalid("Invalid WebAssembly v128 operand.").into(),
        );
    };
    if units.len() != 8 {
        return Err(
            crate::protocol::EngineError::invalid("Invalid WebAssembly v128 width.").into(),
        );
    }
    let mut bytes = [0; 16];
    for (i, unit) in units.code_units().enumerate() {
        bytes[i * 2..i * 2 + 2].copy_from_slice(&unit.to_le_bytes())
    }
    Ok(bytes)
}
/** Lane reads are little-endian and bounded by opcode-specific validation. */
fn lane(bytes: &[u8; 16], size: usize, index: usize) -> u64 {
    let mut out = [0; 8];
    out[..size].copy_from_slice(&bytes[index * size..(index + 1) * size]);
    u64::from_le_bytes(out)
}
/** Lane writes preserve every other byte, including NaN payloads and zero sign bits. */
fn put(bytes: &mut [u8; 16], size: usize, index: usize, bits: u64) {
    bytes[index * size..(index + 1) * size].copy_from_slice(&bits.to_le_bytes()[..size]);
}
/** Signed lanes use their own width rather than the host's 64-bit sign bit. */
fn signed(bits: u64, size: usize) -> i64 {
    let shift = (8 - size) * 8;
    ((bits << shift) as i64) >> shift
}
/** Decode only implemented SIMD operations, retaining immediate lane bounds and memory alignment. */
pub(super) fn decode(r: &mut binary::Reader, m: &Module) -> Result<Instruction, Error> {
    let op = r.prefixed_opcode()?;
    if half::signature(op).is_some() && !r.fp16_enabled {
        return Err(Error::invalid("f16 SIMD instructions require --wasm-fp16"));
    }
    if op == 12 {
        return Ok(Instruction::Constant(Constant::V128(
            r.take(16)?.try_into().unwrap(),
        )));
    }
    let mut args = Vec::new();
    if matches!(op,0..=11|0x54..=0x5d) {
        let (align, index, offset) = r.memarg(m)?;
        args.extend([align, offset as u32, (offset >> 32) as u32, index]);
    }
    if op == 13 {
        for _ in 0..16 {
            let lane = r.byte()?;
            if lane >= 32 {
                return Err(Error::invalid("SIMD shuffle lane out of range"));
            }
            args.push(u32::from(lane))
        }
    }
    if matches!(op,0x15..=0x22|0x54..=0x5b|0x121..=0x122) {
        let index = r.byte()?;
        let size = if op >= 0x121 {
            2
        } else if op >= 0x54 {
            1usize << ((op - 0x54) % 4)
        } else {
            lane_type(op).0
        };
        if index as usize >= 16 / size {
            return Err(Error::invalid("SIMD lane index out of range"));
        }
        args.push(u32::from(index))
    }
    let _ = operand_types(op)?;
    Ok(Instruction::Simd(op, args))
}
/** Every accepted SIMD instruction has one concrete static stack signature. */
pub(super) fn signature(
    m: &Module,
    op: u32,
    args: &[u32],
) -> Result<(Vec<Type>, Vec<Type>), Error> {
    if matches!(op,0..=11|0x54..=0x5d) {
        if validate::memory_count(m) == 0 {
            return Err(Error::invalid("SIMD memory instruction without memory"));
        }
        let size = memory_width(op);
        if args[0] > size.trailing_zeros() {
            return Err(Error::invalid("SIMD alignment exceeds natural alignment"));
        }
    }
    let (mut params, results) = operand_types(op)?;
    if matches!(op,0..=11|0x54..=0x5d) {
        params[0] = address::ty(m, args[3])?
    }
    Ok((params, results))
}
/** Scalar lane type and byte size for extract and replace opcodes. */
fn lane_type(op: u32) -> (usize, Type, bool) {
    match op {
        0x15..=0x17 => (1, Type::I32, op == 0x17),
        0x18..=0x1a => (2, Type::I32, op == 0x1a),
        0x1b..=0x1c => (4, Type::I32, op == 0x1c),
        0x1d..=0x1e => (8, Type::I64, op == 0x1e),
        0x1f..=0x20 => (4, Type::F32, op == 0x20),
        _ => (8, Type::F64, op == 0x22),
    }
}
/** Memory widths describe the access, not the 16-byte result width. */
fn memory_width(op: u32) -> u32 {
    match op {
        0 | 11 => 16,
        1..=6 => 8,
        7 => 1,
        8 => 2,
        9 | 0x5c => 4,
        10 | 0x5d => 8,
        0x54..=0x5b => 1 << ((op - 0x54) % 4),
        _ => 0,
    }
}
/** Unknown SIMD extensions remain unsupported; they never get an invented stack contract. */
fn operand_types(op: u32) -> Result<(Vec<Type>, Vec<Type>), Error> {
    use Type::*;
    let result = match op {
        0..=10 | 0x5c..=0x5d => (vec![I32], vec![V128]),
        11 | 0x58..=0x5b => (vec![I32, V128], vec![]),
        0x54..=0x57 => (vec![I32, V128], vec![V128]),
        13 | 14 | 0x23..=0x4c | 0x4e..=0x51 => (vec![V128, V128], vec![V128]),
        0x4d => (vec![V128], vec![V128]),
        0x52 => (vec![V128, V128, V128], vec![V128]),
        0x53 => (vec![V128], vec![I32]),
        15..=20 => (
            vec![match op {
                15..=17 => I32,
                18 => I64,
                19 => F32,
                _ => F64,
            }],
            vec![V128],
        ),
        0x15..=0x22 => {
            let (_, ty, replace) = lane_type(op);
            if replace {
                (vec![V128, ty], vec![V128])
            } else {
                (vec![V128], vec![ty])
            }
        }
        0x5e..=0x62
        | 0x67..=0x6a
        | 0x74..=0x75
        | 0x7a
        | 0x80..=0x81
        | 0x94
        | 0xa0..=0xa1
        | 0xc0..=0xc1
        | 0xe0..=0xe1
        | 0xe3
        | 0xec..=0xed
        | 0xef => (vec![V128], vec![V128]),
        0x63..=0x64 | 0x83..=0x84 | 0xa3..=0xa4 | 0xc3..=0xc4 => (vec![V128], vec![I32]),
        0x6b..=0x6d | 0x8b..=0x8d | 0xab..=0xad | 0xcb..=0xcd => (vec![V128, I32], vec![V128]),
        0x6e..=0x73
        | 0x76..=0x79
        | 0x7b
        | 0x8e..=0x93
        | 0x95..=0x99
        | 0x9b
        | 0xae
        | 0xb1
        | 0xb5..=0xb9
        | 0xce
        | 0xd1
        | 0xd5..=0xdb
        | 0xe4..=0xeb
        | 0xf0..=0xf7 => (vec![V128, V128], vec![V128]),
        _ => {
            return extended::signature(op)
                .or_else(|| half::signature(op))
                .ok_or_else(|| Error::unsupported(&format!("WebAssembly SIMD opcode 0x{op:x}")));
        }
    };
    Ok(result)
}

/** Execute a validated SIMD instruction over exact bytes and charge native work by lane count. */
pub(super) fn run(
    vm: &mut Vm,
    instance: ObjectId,
    op: u32,
    args: &[u32],
    stack: &mut Vec<JsValue>,
) -> Res<()> {
    vm.charge(16)?;
    if half::scalar(op, args, stack)? {
        return Ok(());
    }
    if matches!(op,0..=11|0x54..=0x5d) {
        return memory_op(vm, instance, op, args, stack);
    }
    if (15..=20).contains(&op) {
        let scalar = numeric::pop(stack)?;
        let (size, bits) = scalar_bits(op, &scalar)?;
        let mut bytes = [0; 16];
        for i in 0..16 / size {
            put(&mut bytes, size, i, bits)
        }
        stack.push(value(bytes)?);
        return Ok(());
    }
    if (0x15..=0x22).contains(&op) {
        return lane_op(op, args[0] as usize, stack);
    }
    let (types, _) = operand_types(op).map_err(|e| compile_failure(vm, e))?;
    let mut shift = 0;
    let mut operands = Vec::new();
    for ty in types.iter().rev() {
        let operand = numeric::pop(stack)?;
        if *ty == Type::I32 {
            shift = numeric::i32_value(&operand)? as u32
        } else {
            operands.push(vector(&operand)?);
        }
    }
    operands.reverse();
    let a = operands[0];
    if op == 0x53 {
        stack.push(JsValue::Number(if a.iter().any(|b| *b != 0) {
            1.0
        } else {
            0.0
        }));
        return Ok(());
    }
    if matches!(op,0x63..=0x64|0x83..=0x84|0xa3..=0xa4|0xc3..=0xc4) {
        let size = 1usize << ((op - 0x63) / 0x20);
        let mut result = if op & 1 == 1 { 1 } else { 0 };
        for i in 0..16 / size {
            let bits = lane(&a, size, i);
            if op & 1 == 1 {
                result &= u32::from(bits != 0)
            } else {
                result |= (((bits >> (size * 8 - 1)) & 1) as u32) << i
            }
        }
        stack.push(JsValue::Number(f64::from(result)));
        return Ok(());
    }
    let mut out = [0; 16];
    if extended::run(vm, op, &operands, &mut out)? || half::run(vm, op, &operands, &mut out)? {
        stack.push(value(out)?);
        return Ok(());
    }
    match op {
        13 => {
            let b = operands[1];
            for (i, index) in args.iter().enumerate() {
                out[i] = if *index < 16 {
                    a[*index as usize]
                } else {
                    b[*index as usize - 16]
                }
            }
        }
        14 => {
            for (i, index) in operands[1].iter().enumerate() {
                out[i] = a.get(*index as usize).copied().unwrap_or(0)
            }
        }
        0x4d..=0x52 => {
            for i in 0..16 {
                out[i] = match op {
                    0x4d => !a[i],
                    0x4e => a[i] & operands[1][i],
                    0x4f => a[i] & !operands[1][i],
                    0x50 => a[i] | operands[1][i],
                    0x51 => a[i] ^ operands[1][i],
                    _ => (a[i] & operands[2][i]) | (operands[1][i] & !operands[2][i]),
                }
            }
        }
        0x23..=0x4c | 0xd6..=0xdb => compare(op, &a, &operands[1], &mut out),
        0x5e => {
            for i in 0..2 {
                let n = f64::from_bits(lane(&a, 8, i)) as f32;
                put(&mut out, 4, i, u64::from(n.to_bits()))
            }
        }
        0x5f => {
            for i in 0..2 {
                put(
                    &mut out,
                    8,
                    i,
                    numeric::promote_f32(lane(&a, 4, i) as u32).to_bits(),
                )
            }
        }
        0x67..=0x6a | 0x74..=0x75 | 0x7a | 0x94 | 0xe0..=0xeb | 0xec..=0xf7 => {
            float_op(op, &a, operands.get(1), &mut out)
        }
        _ => integer_op(op, &a, operands.get(1), shift, &mut out),
    }
    stack.push(value(out)?);
    Ok(())
}

/** Splat converts already-typed scalar bits without any JavaScript callback. */
fn scalar_bits(op: u32, value: &JsValue) -> Res<(usize, u64)> {
    Ok(match op {
        15 => (1, numeric::i32_value(value)? as u64),
        16 => (2, numeric::i32_value(value)? as u64),
        17 => (4, numeric::i32_value(value)? as u64),
        18 => match value {
            JsValue::BigInt(n) => (8, *n as u64),
            _ => {
                return Err(crate::protocol::EngineError::invalid("Invalid i64 SIMD lane.").into());
            }
        },
        19 => match value {
            #[cfg(feature = "conformance")]
            JsValue::Smi(n) => (4, u64::from((*n as f32).to_bits())),
            JsValue::Number(n) => (4, u64::from((*n as f32).to_bits())),
            _ => {
                return Err(crate::protocol::EngineError::invalid("Invalid f32 SIMD lane.").into());
            }
        },
        _ => match value {
            #[cfg(feature = "conformance")]
            JsValue::Smi(n) => (8, n.to_bits()),
            JsValue::Number(n) => (8, n.to_bits()),
            _ => {
                return Err(crate::protocol::EngineError::invalid("Invalid f64 SIMD lane.").into());
            }
        },
    })
}
/** Lane extraction preserves signedness and floating payloads; replacement changes one lane. */
fn lane_op(op: u32, index: usize, stack: &mut Vec<JsValue>) -> Res<()> {
    let (size, _, replace) = lane_type(op);
    let replacement = if replace {
        Some(numeric::pop(stack)?)
    } else {
        None
    };
    let mut bytes = vector(&numeric::pop(stack)?)?;
    if let Some(replacement) = replacement {
        let scalar = match op {
            0x17 => 15,
            0x1a => 16,
            0x1c => 17,
            0x1e => 18,
            0x20 => 19,
            _ => 20,
        };
        let (_, bits) = scalar_bits(scalar, &replacement)?;
        put(&mut bytes, size, index, bits);
        stack.push(value(bytes)?)
    } else {
        let bits = lane(&bytes, size, index);
        stack.push(match op {
            0x15 | 0x18 => JsValue::Number(signed(bits, size) as f64),
            0x16 | 0x19 => JsValue::Number(bits as f64),
            0x1b => JsValue::Number((bits as i32) as f64),
            0x1d => JsValue::BigInt((bits as i64) as i128),
            0x1f => JsValue::Number(numeric::promote_f32(bits as u32)),
            _ => JsValue::Number(f64::from_bits(bits)),
        })
    }
    Ok(())
}
/** Loads, stores, splats and lane accesses all reuse the same checked memory boundary. */
fn memory_op(
    vm: &mut Vm,
    instance: ObjectId,
    op: u32,
    args: &[u32],
    stack: &mut Vec<JsValue>,
) -> Res<()> {
    let mut bytes = if op == 11 || (0x54..=0x5b).contains(&op) {
        vector(&numeric::pop(stack)?)?
    } else {
        [0; 16]
    };
    let address_value = numeric::pop(stack)?;
    let memory = instance::item(vm, instance, MEMORIES, args[3])?;
    let offset = u64::from(args[1]) | (u64::from(args[2]) << 32);
    let at = address::effective(vm, &memory, &address_value, offset)?;
    let width = memory_width(op) as usize;
    if op == 11 {
        memory::write(vm, &memory, at, &bytes)?;
        return Ok(());
    }
    if (0x58..=0x5b).contains(&op) {
        let lane = args[4] as usize;
        memory::write(vm, &memory, at, &bytes[lane * width..(lane + 1) * width])?;
        return Ok(());
    }
    let source = memory::read(vm, &memory, at, width)?;
    match op {
        0 => bytes.copy_from_slice(&source),
        1..=6 => {
            let size = 1usize << ((op - 1) / 2);
            for (i, chunk) in source.chunks_exact(size).enumerate() {
                let mut input = [0; 16];
                input[..size].copy_from_slice(chunk);
                let bits = lane(&input, size, 0);
                put(
                    &mut bytes,
                    size * 2,
                    i,
                    if op & 1 == 1 {
                        signed(bits, size) as u64
                    } else {
                        bits
                    },
                )
            }
        }
        7..=10 => {
            for chunk in bytes.chunks_exact_mut(width) {
                chunk.copy_from_slice(&source)
            }
        }
        0x54..=0x57 => {
            let lane = args[4] as usize;
            bytes[lane * width..(lane + 1) * width].copy_from_slice(&source)
        }
        _ => bytes[..width].copy_from_slice(&source),
    }
    stack.push(value(bytes)?);
    Ok(())
}

/** Comparison lanes become all-one or all-zero masks at the original lane width. */
fn compare(op: u32, a: &[u8; 16], b: &[u8; 16], out: &mut [u8; 16]) {
    let (size, relative, floating) = match op {
        0x23..=0x2c => (1, op - 0x23, false),
        0x2d..=0x36 => (2, op - 0x2d, false),
        0x37..=0x40 => (4, op - 0x37, false),
        0x41..=0x46 => (4, op - 0x41, true),
        0x47..=0x4c => (8, op - 0x47, true),
        _ => (
            8,
            match op {
                0xd6 => 0,
                0xd7 => 1,
                0xd8 => 2,
                0xd9 => 4,
                0xda => 6,
                _ => 8,
            },
            false,
        ),
    };
    for i in 0..16 / size {
        let x = lane(a, size, i);
        let y = lane(b, size, i);
        let yes = if floating {
            let x = if size == 4 {
                numeric::promote_f32(x as u32)
            } else {
                f64::from_bits(x)
            };
            let y = if size == 4 {
                numeric::promote_f32(y as u32)
            } else {
                f64::from_bits(y)
            };
            match relative {
                0 => x == y,
                1 => x != y,
                2 => x < y,
                3 => x > y,
                4 => x <= y,
                _ => x >= y,
            }
        } else {
            let sx = signed(x, size);
            let sy = signed(y, size);
            match relative {
                0 => x == y,
                1 => x != y,
                2 => sx < sy,
                3 => x < y,
                4 => sx > sy,
                5 => x > y,
                6 => sx <= sy,
                7 => x <= y,
                8 => sx >= sy,
                _ => x >= y,
            }
        };
        put(out, size, i, if yes { u64::MAX } else { 0 })
    }
}
/** Float SIMD shares scalar rounding and NaN/zero handling rather than reinterpreting integer bits. */
fn float_op(op: u32, a: &[u8; 16], b: Option<&[u8; 16]>, out: &mut [u8; 16]) {
    let (size, code) = match op {
        0x67..=0x6a => (4, op - 0x67 + 2),
        0x74..=0x75 => (8, op - 0x74 + 2),
        0x7a => (8, 4),
        0x94 => (8, 5),
        0xe0..=0xeb => (
            4,
            match op {
                0xe0 => 0,
                0xe1 => 1,
                0xe3 => 6,
                n => n - 0xe4 + 7,
            },
        ),
        _ => (
            8,
            match op {
                0xec => 0,
                0xed => 1,
                0xef => 6,
                n => n - 0xf0 + 7,
            },
        ),
    };
    for i in 0..16 / size {
        let x = if size == 4 {
            numeric::promote_f32(lane(a, size, i) as u32)
        } else {
            f64::from_bits(lane(a, size, i))
        };
        let y = b.map(|b| {
            if size == 4 {
                numeric::promote_f32(lane(b, size, i) as u32)
            } else {
                f64::from_bits(lane(b, size, i))
            }
        });
        let result = match code {
            0 => x.abs(),
            1 => -x,
            2 => x.ceil(),
            3 => x.floor(),
            4 => x.trunc(),
            5 => x.round_ties_even(),
            6 => x.sqrt(),
            7 => x + y.unwrap(),
            8 => x - y.unwrap(),
            9 => x * y.unwrap(),
            10 => x / y.unwrap(),
            11 => numeric::minimum(x, y.unwrap(), false),
            12 => numeric::minimum(x, y.unwrap(), true),
            13 => {
                if y.unwrap() < x {
                    y.unwrap()
                } else {
                    x
                }
            }
            _ => {
                if x < y.unwrap() {
                    y.unwrap()
                } else {
                    x
                }
            }
        };
        put(
            out,
            size,
            i,
            if size == 4 {
                u64::from((result as f32).to_bits())
            } else {
                result.to_bits()
            },
        )
    }
}
/** Integer operations derive width from the opcode family and mask each result to that width. */
fn integer_op(op: u32, a: &[u8; 16], b: Option<&[u8; 16]>, shift: u32, out: &mut [u8; 16]) {
    let size = match op {
        0x60..=0x7b => 1,
        0x80..=0x9b => 2,
        0xa0..=0xb9 => 4,
        _ => 8,
    };
    let relative = op
        - (match size {
            1 => 0x60,
            2 => 0x80,
            4 => 0xa0,
            _ => 0xc0,
        });
    for i in 0..16 / size {
        let x = lane(a, size, i);
        let sx = signed(x, size);
        let y = b.map(|b| lane(b, size, i)).unwrap_or(0);
        let sy = signed(y, size);
        let max = if size == 8 {
            u64::MAX
        } else {
            (1u64 << (size * 8)) - 1
        };
        let min_s = -(1i128 << (size * 8 - 1));
        let max_s = (1i128 << (size * 8 - 1)) - 1;
        let bits = match relative {
            0 => sx.wrapping_abs() as u64,
            1 => sx.wrapping_neg() as u64,
            2 => x.count_ones() as u64,
            11 => x.wrapping_shl(shift % (size as u32 * 8)),
            12 => (sx >> (shift % (size as u32 * 8))) as u64,
            13 => x >> (shift % (size as u32 * 8)),
            14 => x.wrapping_add(y),
            15 => (i128::from(sx) + i128::from(sy)).clamp(min_s, max_s) as u64,
            16 => x.saturating_add(y).min(max),
            17 => x.wrapping_sub(y),
            18 => (i128::from(sx) - i128::from(sy)).clamp(min_s, max_s) as u64,
            19 => x.saturating_sub(y),
            21 => x.wrapping_mul(y),
            22 => sx.min(sy) as u64,
            23 => x.min(y),
            24 => sx.max(sy) as u64,
            25 => x.max(y),
            27 => (x + y + 1) / 2,
            _ => unreachable!("SIMD operation was not admitted by its signature"),
        };
        put(out, size, i, bits)
    }
}
