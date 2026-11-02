/** FP16 SIMD uses explicit IEEE binary16 conversion with round-to-nearest, ties-to-even. */
use super::*;

use crate::v9::value::{decode_f16 as decode, encode_f16 as encode};

/** FP16 has f32 scalar operands at the Wasm instruction boundary. */
pub(super) fn signature(op: u32) -> Option<(Vec<Type>, Vec<Type>)> {
    use Type::*;
    Some(match op {
        0x120 => (vec![F32], vec![V128]),
        0x121 => (vec![V128], vec![F32]),
        0x122 => (vec![V128, F32], vec![V128]),
        0x130..=0x136 | 0x145..=0x14b => (vec![V128], vec![V128]),
        0x137..=0x144 => (vec![V128, V128], vec![V128]),
        0x14e..=0x14f => (vec![V128, V128, V128], vec![V128]),
        _ => return None,
    })
}
/** Splat and scalar lane access are handled before the generic vector-only dispatcher. */
pub(super) fn scalar(op: u32, args: &[u32], stack: &mut Vec<JsValue>) -> Res<bool> {
    if !(0x120..=0x122).contains(&op) {
        return Ok(false);
    }
    let replacement = if op != 0x121 {
        let JsValue::Number(n) = numeric::pop(stack)? else {
            return Err(crate::protocol::EngineError::invalid("Invalid FP16 scalar.").into());
        };
        Some(encode(n))
    } else {
        None
    };
    let mut bytes = if op == 0x120 {
        [0; 16]
    } else {
        vector(&numeric::pop(stack)?)?
    };
    match op {
        0x120 => {
            for i in 0..8 {
                put(&mut bytes, 2, i, u64::from(replacement.unwrap()))
            }
            stack.push(value(bytes)?)
        }
        0x121 => stack.push(JsValue::Number(decode(
            lane(&bytes, 2, args[0] as usize) as u16
        ))),
        _ => {
            put(
                &mut bytes,
                2,
                args[0] as usize,
                u64::from(replacement.unwrap()),
            );
            stack.push(value(bytes)?);
        }
    }
    Ok(true)
}
/** All FP16 arithmetic is rounded once to a concrete binary16 result per lane. */
pub(super) fn run(vm: &mut Vm, op: u32, inputs: &[[u8; 16]], out: &mut [u8; 16]) -> Res<bool> {
    if signature(op).is_none() {
        return Ok(false);
    }
    let a = &inputs[0];
    if (0x145..=0x14b).contains(&op) {
        conversion(vm, op, a, out)?;
        return Ok(true);
    }
    for i in 0..8 {
        let x = decode(lane(a, 2, i) as u16);
        let y = inputs.get(1).map(|b| decode(lane(b, 2, i) as u16));
        if (0x137..=0x13c).contains(&op) {
            let y = y.unwrap();
            let yes = match op {
                0x137 => x == y,
                0x138 => x != y,
                0x139 => x < y,
                0x13a => x > y,
                0x13b => x <= y,
                _ => x >= y,
            };
            put(out, 2, i, if yes { u16::MAX as u64 } else { 0 });
            continue;
        }
        let result = match op {
            0x130 => x.abs(),
            0x131 => -x,
            0x132 => x.sqrt(),
            0x133 => x.ceil(),
            0x134 => x.floor(),
            0x135 => x.trunc(),
            0x136 => x.round_ties_even(),
            0x13d => x + y.unwrap(),
            0x13e => x - y.unwrap(),
            0x13f => x * y.unwrap(),
            0x140 => x / y.unwrap(),
            0x141 => numeric::minimum(x, y.unwrap(), false),
            0x142 => numeric::minimum(x, y.unwrap(), true),
            0x143 => {
                if y.unwrap() < x {
                    y.unwrap()
                } else {
                    x
                }
            }
            0x144 => {
                if x < y.unwrap() {
                    y.unwrap()
                } else {
                    x
                }
            }
            0x14e => x.mul_add(y.unwrap(), decode(lane(&inputs[2], 2, i) as u16)),
            0x14f => (-x).mul_add(y.unwrap(), decode(lane(&inputs[2], 2, i) as u16)),
            _ => unreachable!("FP16 scalar operations are dispatched before vector arithmetic"),
        };
        put(out, 2, i, u64::from(encode(result)))
    }
    Ok(true)
}
/** Narrowing and promotion touch only the specified low lanes and leave the rest zero. */
fn conversion(vm: &mut Vm, op: u32, a: &[u8; 16], out: &mut [u8; 16]) -> Res<()> {
    match op {
        0x145..=0x146 => {
            for i in 0..8 {
                let n = decode(lane(a, 2, i) as u16);
                let bits = numeric::truncate(vm, n, 16, op == 0x146, true)? as u16;
                put(out, 2, i, u64::from(bits))
            }
        }
        0x147..=0x148 => {
            for i in 0..8 {
                let bits = lane(a, 2, i);
                let n = if op == 0x147 {
                    f64::from(bits as i16)
                } else {
                    bits as f64
                };
                put(out, 2, i, u64::from(encode(n)))
            }
        }
        0x149..=0x14a => {
            let size = if op == 0x149 { 4 } else { 8 };
            for i in 0..16 / size {
                let n = if size == 4 {
                    numeric::promote_f32(lane(a, size, i) as u32)
                } else {
                    f64::from_bits(lane(a, size, i))
                };
                put(out, 2, i, u64::from(encode(n)))
            }
        }
        _ => {
            for i in 0..4 {
                let n = decode(lane(a, 2, i) as u16) as f32;
                put(out, 4, i, u64::from(n.to_bits()))
            }
        }
    }
    Ok(())
}
