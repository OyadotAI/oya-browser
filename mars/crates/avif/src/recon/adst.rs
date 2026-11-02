/** The inverse ADST of lengths 4, 8 and 16 (spec 7.13.2.4 to 7.13.2.9). ADST4 is a direct formula; ADST8 and ADST16 are butterfly programs between fixed input and output permutations. */
use super::butterfly::{Op, run};
use super::math::round2;

/** The sine constants of the 4-point ADST, scaled to 12 bits (SINPI_k_9 in the specification). */
const SINPI_1_9: i64 = 1321;
const SINPI_2_9: i64 = 2482;
const SINPI_3_9: i64 = 3344;
const SINPI_4_9: i64 = 3803;

/** Inverse ADST4 in place on t (spec 7.13.2.6). The formula has no clamping of its own, so no clamping range is taken. */
pub(crate) fn inverse4(t: &mut [i32]) {
    let x: [i64; 4] = std::array::from_fn(|i| i64::from(t[i]));
    let mut s = [0i64; 7];
    s[0] = SINPI_1_9 * x[0];
    s[1] = SINPI_2_9 * x[0];
    s[2] = SINPI_3_9 * x[1];
    s[3] = SINPI_4_9 * x[2];
    s[4] = SINPI_1_9 * x[2];
    s[5] = SINPI_2_9 * x[3];
    s[6] = SINPI_4_9 * x[3];
    let a7 = x[0] - x[2];
    let b7 = a7 + x[3];
    s[0] += s[3];
    s[1] -= s[4];
    s[3] = s[2];
    s[2] = SINPI_3_9 * b7;
    s[0] += s[5];
    s[1] -= s[6];
    let out0 = s[0] + s[3];
    let out1 = s[1] + s[3];
    let out2 = s[2];
    let out3 = s[0] + s[1] - s[3];
    t[0] = round2(out0, 12) as i32;
    t[1] = round2(out1, 12) as i32;
    t[2] = round2(out2, 12) as i32;
    t[3] = round2(out3, 12) as i32;
}

/** The input permutation of the ADST of length 2^n: even positions read from the far end, odd positions from the one before them (spec 7.13.2.4). */
fn input_permute(t: &mut [i32], n: u32) {
    let n0 = 1usize << n;
    let copy: [i32; 16] = std::array::from_fn(|i| if i < n0 { t[i] } else { 0 });
    for (i, slot) in t[..n0].iter_mut().enumerate() {
        let idx = if i & 1 == 1 { i - 1 } else { n0 - i - 1 };
        *slot = copy[idx];
    }
}

/** The output permutation of the ADST of length 2^n, with a sign on every odd output (spec 7.13.2.5). */
fn output_permute(t: &mut [i32], n: u32) {
    let n0 = 1usize << n;
    let copy: [i32; 16] = std::array::from_fn(|i| if i < n0 { t[i] } else { 0 });
    for (i, slot) in t[..n0].iter_mut().enumerate() {
        let a = (i >> 3) & 1;
        let b = ((i >> 2) & 1) ^ ((i >> 3) & 1);
        let c = ((i >> 1) & 1) ^ ((i >> 2) & 1);
        let d = (i & 1) ^ ((i >> 1) & 1);
        let idx = ((d << 3) | (c << 2) | (b << 1) | a) >> (4 - n);
        *slot = if i & 1 == 1 { -copy[idx] } else { copy[idx] };
    }
}

/** Butterfly steps of the ADST8 program, between its permutations (spec 7.13.2.7). */
fn adst8_program() -> Vec<Op> {
    let mut ops = Vec::new();
    for i in 0..4 {
        ops.push(Op::Rot {
            a: 2 * i,
            b: 2 * i + 1,
            angle: 60 - 16 * i as i32,
            flip: true,
        });
    }
    for i in 0..4 {
        ops.push(Op::Had {
            a: i,
            b: 4 + i,
            flip: false,
        });
    }
    for i in 0..2 {
        ops.push(Op::Rot {
            a: 4 + 3 * i,
            b: 5 + i,
            angle: 48 - 32 * i as i32,
            flip: true,
        });
    }
    for i in 0..2 {
        for j in 0..2 {
            ops.push(Op::Had {
                a: 4 * j + i,
                b: 2 + 4 * j + i,
                flip: false,
            });
        }
    }
    for i in 0..2 {
        ops.push(Op::Rot {
            a: 2 + 4 * i,
            b: 3 + 4 * i,
            angle: 32,
            flip: true,
        });
    }
    ops
}

/** Butterfly steps of the ADST16 program, between its permutations (spec 7.13.2.8). */
fn adst16_program() -> Vec<Op> {
    let mut ops = Vec::new();
    for i in 0..8 {
        ops.push(Op::Rot {
            a: 2 * i,
            b: 2 * i + 1,
            angle: 62 - 8 * i as i32,
            flip: true,
        });
    }
    for i in 0..8 {
        ops.push(Op::Had {
            a: i,
            b: 8 + i,
            flip: false,
        });
    }
    for i in 0..2 {
        ops.push(Op::Rot {
            a: 8 + 2 * i,
            b: 9 + 2 * i,
            angle: 56 - 32 * i as i32,
            flip: true,
        });
        ops.push(Op::Rot {
            a: 13 + 2 * i,
            b: 12 + 2 * i,
            angle: 8 + 32 * i as i32,
            flip: true,
        });
    }
    for i in 0..4 {
        for j in 0..2 {
            ops.push(Op::Had {
                a: 8 * j + i,
                b: 4 + 8 * j + i,
                flip: false,
            });
        }
    }
    for i in 0..2 {
        for j in 0..2 {
            ops.push(Op::Rot {
                a: 4 + 8 * j + 3 * i,
                b: 5 + 8 * j + i,
                angle: 48 - 32 * i as i32,
                flip: true,
            });
        }
    }
    for i in 0..2 {
        for j in 0..4 {
            ops.push(Op::Had {
                a: 4 * j + i,
                b: 2 + 4 * j + i,
                flip: false,
            });
        }
    }
    for i in 0..4 {
        ops.push(Op::Rot {
            a: 2 + 4 * i,
            b: 3 + 4 * i,
            angle: 32,
            flip: true,
        });
    }
    ops
}

/** Inverse ADST of length 2^n for n of 3 or 4, in place on t, with r as the Hadamard clamping range. */
pub(crate) fn inverse(t: &mut [i32], n: u32, r: u32) {
    input_permute(t, n);
    let program = if n == 3 {
        adst8_program()
    } else {
        adst16_program()
    };
    run(t, &program, r);
    output_permute(t, n);
}

/** Inverse ADST of length 2^n for any n in 2 to 4, dispatching to the size-specific transform. */
pub(crate) fn inverse_any(t: &mut [i32], n: u32, r: u32) {
    if n == 2 {
        inverse4(t);
    } else {
        inverse(t, n, r);
    }
}
