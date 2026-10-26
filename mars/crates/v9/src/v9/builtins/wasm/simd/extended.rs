/** SIMD narrowing, extension, pairwise/dot products and deterministic allowed relaxed variants. */
use super::*;

/** This set's exact stack signatures are shared by parsing, validation and execution. */
pub(super) fn signature(op: u32) -> Option<(Vec<Type>, Vec<Type>)> {
    use Type::*;
    Some(match op {
        0x7c..=0x7f | 0x87..=0x8a | 0xa7..=0xaa | 0xc7..=0xca | 0xf8..=0xff | 0x101..=0x104 => {
            (vec![V128], vec![V128])
        }
        0x65..=0x66
        | 0x85..=0x86
        | 0x82
        | 0x9c..=0x9f
        | 0xba
        | 0xbc..=0xbf
        | 0xdc..=0xdf
        | 0x100
        | 0x10d..=0x112 => (vec![V128, V128], vec![V128]),
        0x105..=0x10c | 0x113 => (vec![V128, V128, V128], vec![V128]),
        _ => return None,
    })
}
/** Return false for instructions owned by the base SIMD executor. */
pub(super) fn run(vm: &mut Vm, op: u32, inputs: &[[u8; 16]], out: &mut [u8; 16]) -> Res<bool> {
    if signature(op).is_none() {
        return Ok(false);
    }
    let a = inputs[0];
    match op {
        0x65..=0x66 | 0x85..=0x86 => narrow(op, &a, &inputs[1], out),
        0x87..=0x8a | 0xa7..=0xaa | 0xc7..=0xca => extend(op, &a, None, out),
        0x9c..=0x9f | 0xbc..=0xbf | 0xdc..=0xdf => extend(op, &a, Some(&inputs[1]), out),
        0x7c..=0x7f => pairwise(op, &a, out),
        0x82 | 0x111 => {
            for i in 0..8 {
                let x = signed(lane(&a, 2, i), 2);
                let y = signed(lane(&inputs[1], 2, i), 2);
                put(
                    out,
                    2,
                    i,
                    ((x * y + 0x4000) >> 15).clamp(i16::MIN as i64, i16::MAX as i64) as u64,
                )
            }
        }
        0xba | 0x112 | 0x113 => dot(op, inputs, out),
        0xf8..=0xff | 0x101..=0x104 => conversion(vm, op, &a, out)?,
        0x100 => {
            for (i, index) in inputs[1].iter().enumerate() {
                out[i] = a.get(*index as usize).copied().unwrap_or(0)
            }
        }
        0x105..=0x108 => {
            let size = if op <= 0x106 { 4 } else { 8 };
            for i in 0..16 / size {
                let x = read_float(&a, size, i);
                let y = read_float(&inputs[1], size, i);
                let z = read_float(&inputs[2], size, i);
                let value = if op % 2 == 1 {
                    x.mul_add(y, z)
                } else {
                    (-x).mul_add(y, z)
                };
                write_float(out, size, i, value)
            }
        }
        0x109..=0x10c => {
            for i in 0..16 {
                out[i] = (a[i] & inputs[2][i]) | (inputs[1][i] & !inputs[2][i])
            }
        }
        0x10d..=0x110 => {
            let size = if op <= 0x10e { 4 } else { 8 };
            for i in 0..16 / size {
                let x = read_float(&a, size, i);
                let y = read_float(&inputs[1], size, i);
                write_float(out, size, i, numeric::minimum(x, y, op % 2 == 0))
            }
        }
        _ => unreachable!("extended SIMD signature and executor agree"),
    }
    Ok(true)
}
/** Float lanes retain their declared precision when a result is written. */
fn read_float(bytes: &[u8; 16], size: usize, index: usize) -> f64 {
    if size == 4 {
        numeric::promote_f32(lane(bytes, size, index) as u32)
    } else {
        f64::from_bits(lane(bytes, size, index))
    }
}
/** Storage rounds only f32, with f64 lanes preserving the full binary64 result. */
fn write_float(bytes: &mut [u8; 16], size: usize, index: usize, value: f64) {
    put(
        bytes,
        size,
        index,
        if size == 4 {
            u64::from((value as f32).to_bits())
        } else {
            value.to_bits()
        },
    )
}
/** Saturating narrowing always interprets the wide input as signed before clamping. */
fn narrow(op: u32, a: &[u8; 16], b: &[u8; 16], out: &mut [u8; 16]) {
    let size = if op < 0x80 { 1 } else { 2 };
    let unsigned = op % 2 == 0;
    let lower = if unsigned {
        0
    } else {
        -(1i64 << (size * 8 - 1))
    };
    let upper = if unsigned {
        (1i64 << (size * 8)) - 1
    } else {
        (1i64 << (size * 8 - 1)) - 1
    };
    let count = 16 / (size * 2);
    for i in 0..16 / size {
        let input = if i < count { a } else { b };
        let value = signed(lane(input, size * 2, i % count), size * 2);
        put(out, size, i, value.clamp(lower, upper) as u64)
    }
}
/** Extension and widening multiplication differ only in whether a second input is present. */
fn extend(op: u32, a: &[u8; 16], b: Option<&[u8; 16]>, out: &mut [u8; 16]) {
    let (size, base) = match op {
        0x87..=0x8a => (2, 0x87),
        0xa7..=0xaa => (4, 0xa7),
        0xc7..=0xca => (8, 0xc7),
        0x9c..=0x9f => (2, 0x9c),
        0xbc..=0xbf => (4, 0xbc),
        _ => (8, 0xdc),
    };
    let relative = op - base;
    let count = 16 / size;
    let start = if relative % 2 == 1 { count } else { 0 };
    for i in 0..count {
        let bits = lane(a, size / 2, start + i);
        let x = if relative < 2 {
            signed(bits, size / 2) as u64
        } else {
            bits
        };
        let value = if let Some(b) = b {
            let bits = lane(b, size / 2, start + i);
            let y = if relative < 2 {
                signed(bits, size / 2) as u64
            } else {
                bits
            };
            x.wrapping_mul(y)
        } else {
            x
        };
        put(out, size, i, value)
    }
}
/** Adjacent narrow lanes add into one widened result without overflow at the wider width. */
fn pairwise(op: u32, a: &[u8; 16], out: &mut [u8; 16]) {
    let size = if op < 0x7e { 2 } else { 4 };
    for i in 0..16 / size {
        let x = lane(a, size / 2, i * 2);
        let y = lane(a, size / 2, i * 2 + 1);
        let result = if op % 2 == 0 {
            signed(x, size / 2) + signed(y, size / 2)
        } else {
            (x + y) as i64
        };
        put(out, size, i, result as u64)
    }
}
/** Dot products sum signed products; the relaxed add form adds a real i32 accumulator lane. */
fn dot(op: u32, inputs: &[[u8; 16]], out: &mut [u8; 16]) {
    let (source, destination) = if op == 0xba {
        (2, 4)
    } else if op == 0x112 {
        (1, 2)
    } else {
        (1, 4)
    };
    let count = destination / source;
    for i in 0..16 / destination {
        let mut sum = if op == 0x113 {
            lane(&inputs[2], 4, i) as i64
        } else {
            0
        };
        for j in 0..count {
            let index = i * count + j;
            sum = sum.wrapping_add(
                signed(lane(&inputs[0], source, index), source)
                    .wrapping_mul(signed(lane(&inputs[1], source, index), source)),
            )
        }
        put(out, destination, i, sum as u64)
    }
}
/** Standard saturating truncation is a permitted deterministic relaxed truncation result. */
fn conversion(vm: &mut Vm, op: u32, a: &[u8; 16], out: &mut [u8; 16]) -> Res<()> {
    let code = match op {
        0x101 => 0xf8,
        0x102 => 0xf9,
        0x103 => 0xfc,
        0x104 => 0xfd,
        n => n,
    };
    match code {
        0xf8..=0xf9 | 0xfc..=0xfd => {
            let size = if code >= 0xfc { 8 } else { 4 };
            for i in 0..16 / size {
                let n = read_float(a, size, i);
                let bits = numeric::truncate(vm, n, 32, code % 2 == 1, true)? as u32;
                put(out, 4, i, u64::from(bits))
            }
        }
        0xfa..=0xfb | 0xfe..=0xff => {
            let size = if code >= 0xfe { 8 } else { 4 };
            for i in 0..16 / size {
                let bits = lane(a, 4, i);
                let n = if code % 2 == 0 {
                    f64::from(bits as i32)
                } else {
                    bits as f64
                };
                write_float(out, size, i, n)
            }
        }
        _ => unreachable!("conversion signatures only admit implemented forms"),
    }
    Ok(())
}
