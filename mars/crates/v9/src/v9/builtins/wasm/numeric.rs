/** WebAssembly scalar operations keep integer wrapping, float widths and traps independent of JS. */
use super::*;

/** Bit-preserving widening keeps signaling NaN payloads intact until an arithmetic instruction. */
pub(super) fn promote_f32(bits: u32) -> f64 {
    if bits & 0x7f80_0000 == 0x7f80_0000 && bits & 0x007f_ffff != 0 {
        f64::from_bits(
            (u64::from(bits & 0x8000_0000) << 32)
                | 0x7ff0_0000_0000_0000
                | (u64::from(bits & 0x007f_ffff) << 29),
        )
    } else {
        f64::from(f32::from_bits(bits))
    }
}
/** Reinterpretation narrows NaN payload bits without the hardware conversion's quieting step. */
fn f32_bits(number: f64) -> u32 {
    if number.is_nan() {
        let bits = number.to_bits();
        let mantissa = ((bits & 0x000f_ffff_ffff_ffff) >> 29) as u32;
        ((bits >> 32) as u32 & 0x8000_0000) | 0x7f80_0000 | mantissa.max(1)
    } else {
        (number as f32).to_bits()
    }
}

/** A checked internal operand; malformed inert state cannot make native code index an empty stack. */
pub(super) fn pop(stack: &mut Vec<JsValue>) -> Res<JsValue> {
    stack.pop().ok_or_else(|| {
        crate::protocol::EngineError::invalid("Invalid WebAssembly operand stack.").into()
    })
}
/** i32 values remain exact signed Numbers throughout interpretation. */
pub(super) fn i32_value(value: &JsValue) -> Res<i32> {
    if let Some(n) = value.number_value()
        && n.is_finite()
        && n.fract() == 0.0
        && n >= i32::MIN as f64
        && n <= i32::MAX as f64
    {
        return Ok(n as i32);
    }
    Err(crate::protocol::EngineError::invalid("Invalid WebAssembly i32 value.").into())
}
/** i64 values use the existing compact BigInt representation. */
pub(super) fn i64_value(value: &JsValue) -> Res<i64> {
    match value {
        JsValue::BigInt(n) if *n >= i64::MIN as i128 && *n <= i64::MAX as i128 => Ok(*n as i64),
        _ => Err(crate::protocol::EngineError::invalid("Invalid WebAssembly i64 value.").into()),
    }
}
/** Wasm floating values never coerce objects or invoke JavaScript. */
fn number(value: &JsValue) -> Res<f64> {
    match value {
        #[cfg(feature = "conformance")]
        JsValue::Smi(n) => Ok(*n),
        JsValue::Number(n) => Ok(*n),
        _ => Err(crate::protocol::EngineError::invalid("Invalid WebAssembly float value.").into()),
    }
}
/** Comparison results are integer booleans. */
fn boolean(value: bool) -> JsValue {
    JsValue::Number(if value { 1.0 } else { 0.0 })
}
/** Run the complete supported scalar numeric opcode family. */
pub(super) fn run(vm: &mut Vm, op: u8, stack: &mut Vec<JsValue>) -> Res<()> {
    let (_, count, _) = validate::numeric_type(op)
        .ok_or_else(|| vm.unsupported_error("WebAssembly numeric opcode"))?;
    let right = pop(stack)?;
    let left = if count == 2 { Some(pop(stack)?) } else { None };
    let result = match op {
        0x45..=0x4f => {
            let b = i32_value(&right)?;
            if op == 0x45 {
                boolean(b == 0)
            } else {
                let a = i32_value(left.as_ref().unwrap())?;
                boolean(match op {
                    0x46 => a == b,
                    0x47 => a != b,
                    0x48 => a < b,
                    0x49 => (a as u32) < b as u32,
                    0x4a => a > b,
                    0x4b => (a as u32) > b as u32,
                    0x4c => a <= b,
                    0x4d => (a as u32) <= b as u32,
                    0x4e => a >= b,
                    _ => (a as u32) >= b as u32,
                })
            }
        }
        0x50..=0x5a => {
            let b = i64_value(&right)?;
            if op == 0x50 {
                boolean(b == 0)
            } else {
                let a = i64_value(left.as_ref().unwrap())?;
                boolean(match op {
                    0x51 => a == b,
                    0x52 => a != b,
                    0x53 => a < b,
                    0x54 => (a as u64) < b as u64,
                    0x55 => a > b,
                    0x56 => (a as u64) > b as u64,
                    0x57 => a <= b,
                    0x58 => (a as u64) <= b as u64,
                    0x59 => a >= b,
                    _ => (a as u64) >= b as u64,
                })
            }
        }
        0x5b..=0x66 => {
            let b = number(&right)?;
            let a = number(left.as_ref().unwrap())?;
            let relative = if op < 0x61 { op - 0x5b } else { op - 0x61 };
            boolean(match relative {
                0 => a == b,
                1 => a != b,
                2 => a < b,
                3 => a > b,
                4 => a <= b,
                _ => a >= b,
            })
        }
        0x67..=0x78 => {
            let b = i32_value(&right)?;
            let result = if op <= 0x69 {
                match op {
                    0x67 => (b as u32).leading_zeros() as i32,
                    0x68 => (b as u32).trailing_zeros() as i32,
                    _ => b.count_ones() as i32,
                }
            } else {
                let a = i32_value(left.as_ref().unwrap())?;
                integer32(vm, op, a, b)?
            };
            JsValue::Number(f64::from(result))
        }
        0x79..=0x8a => {
            let b = i64_value(&right)?;
            let result = if op <= 0x7b {
                match op {
                    0x79 => (b as u64).leading_zeros() as i64,
                    0x7a => (b as u64).trailing_zeros() as i64,
                    _ => b.count_ones() as i64,
                }
            } else {
                let a = i64_value(left.as_ref().unwrap())?;
                integer64(vm, op, a, b)?
            };
            JsValue::BigInt(i128::from(result))
        }
        0x8b..=0xa6 => {
            let b = number(&right)?;
            let base = if op < 0x99 { 0x8b } else { 0x99 };
            let relative = op - base;
            let a = left.as_ref().map(number).transpose()?;
            let value = float(relative, a, b);
            JsValue::Number(if op < 0x99 {
                f64::from(value as f32)
            } else {
                value
            })
        }
        _ => conversion(vm, op, right)?,
    };
    stack.push(result);
    Ok(())
}
/** i32 division has two distinct traps; remainder of MIN/-1 is defined as zero. */
fn integer32(vm: &mut Vm, op: u8, a: i32, b: i32) -> Res<i32> {
    if matches!(op, 0x6d..=0x70) && b == 0 {
        return Err(trap(
            vm,
            if op >= 0x6f {
                "remainder by zero"
            } else {
                "divide by zero"
            },
        ));
    }
    if op == 0x6d && a == i32::MIN && b == -1 {
        return Err(trap(vm, "divide result unrepresentable"));
    }
    Ok(match op {
        0x6a => a.wrapping_add(b),
        0x6b => a.wrapping_sub(b),
        0x6c => a.wrapping_mul(b),
        0x6d => a / b,
        0x6e => ((a as u32) / (b as u32)) as i32,
        0x6f => a.checked_rem(b).unwrap_or(0),
        0x70 => ((a as u32) % (b as u32)) as i32,
        0x71 => a & b,
        0x72 => a | b,
        0x73 => a ^ b,
        0x74 => a.wrapping_shl(b as u32),
        0x75 => a.wrapping_shr(b as u32),
        0x76 => (a as u32).wrapping_shr(b as u32) as i32,
        0x77 => a.rotate_left(b as u32),
        _ => a.rotate_right(b as u32),
    })
}
/** i64 arithmetic uses explicit wrapping and unsigned views of the same bits. */
fn integer64(vm: &mut Vm, op: u8, a: i64, b: i64) -> Res<i64> {
    if matches!(op, 0x7f..=0x82) && b == 0 {
        return Err(trap(
            vm,
            if op >= 0x81 {
                "remainder by zero"
            } else {
                "divide by zero"
            },
        ));
    }
    if op == 0x7f && a == i64::MIN && b == -1 {
        return Err(trap(vm, "divide result unrepresentable"));
    }
    Ok(match op {
        0x7c => a.wrapping_add(b),
        0x7d => a.wrapping_sub(b),
        0x7e => a.wrapping_mul(b),
        0x7f => a / b,
        0x80 => ((a as u64) / (b as u64)) as i64,
        0x81 => a.checked_rem(b).unwrap_or(0),
        0x82 => ((a as u64) % (b as u64)) as i64,
        0x83 => a & b,
        0x84 => a | b,
        0x85 => a ^ b,
        0x86 => a.wrapping_shl(b as u32),
        0x87 => a.wrapping_shr(b as u32),
        0x88 => (a as u64).wrapping_shr(b as u32) as i64,
        0x89 => a.rotate_left(b as u32),
        _ => a.rotate_right(b as u32),
    })
}
/** IEEE min/max propagate NaNs and choose the prescribed sign when both inputs are zero. */
pub(super) fn minimum(a: f64, b: f64, maximum: bool) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a == 0.0 && b == 0.0 {
        return f64::from_bits(if maximum {
            a.to_bits() & b.to_bits()
        } else {
            a.to_bits() | b.to_bits()
        });
    }
    if maximum { a.max(b) } else { a.min(b) }
}
/** f32 arithmetic is rounded by the caller after each instruction. */
fn float(op: u8, a: Option<f64>, b: f64) -> f64 {
    match op {
        0 => b.abs(),
        1 => -b,
        2 => b.ceil(),
        3 => b.floor(),
        4 => b.trunc(),
        5 => b.round_ties_even(),
        6 => b.sqrt(),
        7 => a.unwrap() + b,
        8 => a.unwrap() - b,
        9 => a.unwrap() * b,
        10 => a.unwrap() / b,
        11 => minimum(a.unwrap(), b, false),
        12 => minimum(a.unwrap(), b, true),
        _ => a.unwrap().copysign(b),
    }
}
/** Truncation checks the rounded integer against exact powers of two, then preserves unsigned bits. */
pub(super) fn truncate(
    vm: &mut Vm,
    value: f64,
    bits: u32,
    unsigned: bool,
    saturate: bool,
) -> Res<i128> {
    let value = value.trunc();
    let lower = if unsigned {
        0.0
    } else {
        -(2.0f64).powi(bits as i32 - 1)
    };
    let upper = (2.0f64).powi(if unsigned {
        bits as i32
    } else {
        bits as i32 - 1
    });
    if value.is_nan() {
        return if saturate {
            Ok(0)
        } else {
            Err(trap(vm, "float unrepresentable in integer range"))
        };
    }
    if value < lower || value >= upper {
        if !saturate {
            return Err(trap(vm, "float unrepresentable in integer range"));
        }
        return Ok(if value < lower {
            if unsigned { 0 } else { -(1i128 << (bits - 1)) }
        } else if unsigned {
            (1i128 << bits) - 1
        } else {
            (1i128 << (bits - 1)) - 1
        });
    }
    Ok(value as i128)
}
/** Numeric conversions and bit reinterpretations never route through JS Number/BigInt coercions. */
fn conversion(vm: &mut Vm, op: u8, value: JsValue) -> Res<JsValue> {
    Ok(match op {
        0xa7 => JsValue::Number((i64_value(&value)? as i32) as f64),
        0xa8..=0xab => {
            JsValue::Number((truncate(vm, number(&value)?, 32, op % 2 == 1, false)? as i32) as f64)
        }
        0xac => JsValue::BigInt(i128::from(i32_value(&value)?)),
        0xad => JsValue::BigInt(i128::from(i32_value(&value)? as u32)),
        0xae..=0xb1 => {
            JsValue::BigInt((truncate(vm, number(&value)?, 64, op % 2 == 1, false)? as i64) as i128)
        }
        0xb2 => JsValue::Number(f64::from(i32_value(&value)? as f32)),
        0xb3 => JsValue::Number(f64::from((i32_value(&value)? as u32) as f32)),
        0xb4 => JsValue::Number(f64::from(i64_value(&value)? as f32)),
        0xb5 => JsValue::Number(f64::from((i64_value(&value)? as u64) as f32)),
        0xb6 => JsValue::Number(f64::from(number(&value)? as f32)),
        0xb7 => JsValue::Number(f64::from(i32_value(&value)?)),
        0xb8 => JsValue::Number(f64::from(i32_value(&value)? as u32)),
        0xb9 => JsValue::Number(i64_value(&value)? as f64),
        0xba => JsValue::Number((i64_value(&value)? as u64) as f64),
        0xbb => {
            let number = number(&value)?;
            JsValue::Number(if number.is_nan() {
                f64::from_bits(number.to_bits() | F64_QUIET_NAN_BIT)
            } else {
                number
            })
        }
        0xbc => JsValue::Number((f32_bits(number(&value)?) as i32) as f64),
        0xbd => JsValue::BigInt((number(&value)?.to_bits() as i64) as i128),
        0xbe => JsValue::Number(promote_f32(i32_value(&value)? as u32)),
        0xbf => JsValue::Number(f64::from_bits(i64_value(&value)? as u64)),
        0xc0 => JsValue::Number((i32_value(&value)? as i8) as f64),
        0xc1 => JsValue::Number((i32_value(&value)? as i16) as f64),
        0xc2 => JsValue::BigInt((i64_value(&value)? as i8) as i128),
        0xc3 => JsValue::BigInt((i64_value(&value)? as i16) as i128),
        0xc4 => JsValue::BigInt((i64_value(&value)? as i32) as i128),
        _ => return Err(vm.unsupported_error("WebAssembly conversion opcode")),
    })
}
/** Saturating conversions share the exact same range logic as their trapping counterparts. */
pub(super) fn saturating(vm: &mut Vm, sub: u32, stack: &mut Vec<JsValue>) -> Res<()> {
    let value = number(&pop(stack)?)?;
    let bits = if sub < 4 { 32 } else { 64 };
    let n = truncate(vm, value, bits, sub % 2 == 1, true)?;
    stack.push(if bits == 32 {
        JsValue::Number((n as i32) as f64)
    } else {
        JsValue::BigInt((n as i64) as i128)
    });
    Ok(())
}
/** Load a little-endian scalar or its prescribed signed/unsigned narrow representation. */
pub(super) fn load(op: u8, bytes: &[u8]) -> JsValue {
    let mut data = [0u8; 8];
    data[..bytes.len()].copy_from_slice(bytes);
    let unsigned = u64::from_le_bytes(data);
    let signed = match bytes.len() {
        1 => i64::from(data[0] as i8),
        2 => i64::from(i16::from_le_bytes(data[..2].try_into().unwrap())),
        4 => i64::from(i32::from_le_bytes(data[..4].try_into().unwrap())),
        _ => unsigned as i64,
    };
    match op {
        0xc5 => JsValue::Number(crate::v9::value::decode_f16(unsigned as u16)),
        0x2a => JsValue::Number(promote_f32(unsigned as u32)),
        0x2b => JsValue::Number(f64::from_bits(unsigned)),
        0x28 | 0x2c | 0x2e => JsValue::Number((signed as i32) as f64),
        0x2d | 0x2f => JsValue::Number(unsigned as f64),
        0x30 | 0x32 | 0x34 | 0x29 => JsValue::BigInt(i128::from(signed)),
        _ => JsValue::BigInt(i128::from(unsigned)),
    }
}
/** Store width is validated by the opcode table before a slice is taken. */
pub(super) fn store(op: u8, value: &JsValue) -> Res<[u8; 8]> {
    Ok(match op {
        0xc6 => u64::from(crate::v9::value::encode_f16(number(value)?)),
        0x38 => f32_bits(number(value)?) as u64,
        0x39 => number(value)?.to_bits(),
        0x36 | 0x3a | 0x3b => i32_value(value)? as u32 as u64,
        _ => i64_value(value)? as u64,
    }
    .to_le_bytes())
}

/** Wide arithmetic returns low then high i64 halves and preserves modulo-128 overflow semantics. */
pub(super) fn wide(op: u32, stack: &mut Vec<JsValue>) -> Res<()> {
    let last = i64_value(&pop(stack)?)?;
    let prior = i64_value(&pop(stack)?)?;
    let bits = if op <= 20 {
        let low = i64_value(&pop(stack)?)? as u64;
        let first = i64_value(&pop(stack)?)? as u64;
        let a = u128::from(first) | (u128::from(low) << 64);
        let b = u128::from(prior as u64) | (u128::from(last as u64) << 64);
        if op == 19 {
            a.wrapping_add(b)
        } else {
            a.wrapping_sub(b)
        }
    } else if op == 21 {
        ((prior as i128) * (last as i128)) as u128
    } else {
        u128::from(prior as u64) * u128::from(last as u64)
    };
    stack.push(JsValue::BigInt((bits as u64 as i64) as i128));
    stack.push(JsValue::BigInt(((bits >> 64) as u64 as i64) as i128));
    Ok(())
}
