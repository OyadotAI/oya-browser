/** The inverse DCT of lengths 4 to 64 (spec 7.13.2.2 and 7.13.2.3). The program of butterfly steps is built once per length from the specification's numbered steps, then run over the input in bit-reversed order. */
use super::butterfly::{Op, brev, run};

/** Program steps for the first group of stages, which apply to lengths of 8 and up. Each push mirrors one numbered step of the specification. */
fn stage_outer(n: u32, ops: &mut Vec<Op>) {
    if n == 6 {
        for i in 0..16 {
            ops.push(rot(32 + i, 63 - i, 63 - 4 * brev(4, i) as i32, false));
        }
    }
    if n >= 5 {
        for i in 0..8 {
            ops.push(rot(
                16 + i,
                31 - i,
                6 + ((brev(3, 7 - i) as i32) << 3),
                false,
            ));
        }
    }
    if n == 6 {
        for i in 0..16 {
            ops.push(had(32 + i * 2, 33 + i * 2, i & 1 == 1));
        }
    }
    if n >= 4 {
        for i in 0..4 {
            ops.push(rot(
                8 + i,
                15 - i,
                12 + ((brev(2, 3 - i) as i32) << 4),
                false,
            ));
        }
    }
    if n >= 5 {
        for i in 0..8 {
            ops.push(had(16 + 2 * i, 17 + 2 * i, i & 1 == 1));
        }
    }
    if n == 6 {
        for i in 0..4 {
            for j in 0..2 {
                ops.push(rot(
                    62 - i * 4 - j,
                    33 + i * 4 + j,
                    60 - 16 * brev(2, i) as i32 + 64 * j as i32,
                    true,
                ));
            }
        }
    }
}

/** Program steps for the middle stages (steps 8 to 16 of the specification). */
fn stage_middle(n: u32, ops: &mut Vec<Op>) {
    if n >= 3 {
        for i in 0..2 {
            ops.push(rot(4 + i, 7 - i, 56 - 32 * i as i32, false));
        }
    }
    if n >= 4 {
        for i in 0..4 {
            ops.push(had(8 + 2 * i, 9 + 2 * i, i & 1 == 1));
        }
    }
    if n >= 5 {
        for i in 0..2 {
            for j in 0..2 {
                ops.push(rot(
                    30 - 4 * i - j,
                    17 + 4 * i + j,
                    24 + ((j as i32) << 6) + (((1 - i) as i32) << 5),
                    true,
                ));
            }
        }
    }
    if n == 6 {
        for i in 0..8 {
            for j in 0..2 {
                ops.push(had(32 + i * 4 + j, 35 + i * 4 - j, i & 1 == 1));
            }
        }
    }
    for i in 0..2 {
        ops.push(rot(2 * i, 2 * i + 1, 32 + 16 * i as i32, i == 0));
    }
    if n >= 3 {
        for i in 0..2 {
            ops.push(had(4 + 2 * i, 5 + 2 * i, i == 1));
        }
    }
    if n >= 4 {
        for i in 0..2 {
            ops.push(rot(14 - i, 9 + i, 48 + 64 * i as i32, true));
        }
    }
    if n >= 5 {
        for i in 0..4 {
            for j in 0..2 {
                ops.push(had(16 + 4 * i + j, 19 + 4 * i - j, i & 1 == 1));
            }
        }
    }
    if n == 6 {
        for i in 0..2 {
            for j in 0..4 {
                ops.push(rot(
                    61 - i * 8 - j,
                    34 + i * 8 + j,
                    56 - i as i32 * 32 + ((j >> 1) as i32) * 64,
                    true,
                ));
            }
        }
    }
}

/** Program steps for the last stages (steps 17 to 31 of the specification). */
fn stage_last(n: u32, ops: &mut Vec<Op>) {
    for i in 0..2 {
        ops.push(had(i, 3 - i, false));
    }
    if n >= 3 {
        ops.push(rot(6, 5, 32, true));
    }
    if n >= 4 {
        for i in 0..2 {
            for j in 0..2 {
                ops.push(had(8 + 4 * i + j, 11 + 4 * i - j, i == 1));
            }
        }
    }
    if n >= 5 {
        for i in 0..4 {
            ops.push(rot(29 - i, 18 + i, 48 + ((i >> 1) as i32) * 64, true));
        }
    }
    if n == 6 {
        for i in 0..4 {
            for j in 0..4 {
                ops.push(had(32 + 8 * i + j, 39 + 8 * i - j, i & 1 == 1));
            }
        }
    }
    if n >= 3 {
        for i in 0..4 {
            ops.push(had(i, 7 - i, false));
        }
    }
    if n >= 4 {
        for i in 0..2 {
            ops.push(rot(13 - i, 10 + i, 32, true));
        }
    }
    if n >= 5 {
        for i in 0..2 {
            for j in 0..4 {
                ops.push(had(16 + i * 8 + j, 23 + i * 8 - j, i == 1));
            }
        }
    }
    if n == 6 {
        for i in 0..8 {
            ops.push(rot(59 - i, 36 + i, if i < 4 { 48 } else { 112 }, true));
        }
    }
    if n >= 4 {
        for i in 0..8 {
            ops.push(had(i, 15 - i, false));
        }
    }
    if n >= 5 {
        for i in 0..4 {
            ops.push(rot(27 - i, 20 + i, 32, true));
        }
    }
    if n == 6 {
        for i in 0..8 {
            ops.push(had(32 + i, 47 - i, false));
            ops.push(had(48 + i, 63 - i, true));
        }
    }
    if n >= 5 {
        for i in 0..16 {
            ops.push(had(i, 31 - i, false));
        }
    }
    if n == 6 {
        for i in 0..8 {
            ops.push(rot(55 - i, 40 + i, 32, true));
        }
        for i in 0..32 {
            ops.push(had(i, 63 - i, false));
        }
    }
}

/** A rotation step with the given operands, written in the form the program builders use. */
fn rot(a: usize, b: usize, angle: i32, flip: bool) -> Op {
    Op::Rot { a, b, angle, flip }
}

/** A Hadamard step with the given operands. */
fn had(a: usize, b: usize, flip: bool) -> Op {
    Op::Had { a, b, flip }
}

/** The butterfly program of the inverse DCT of length 2^n: the input permutation, then the numbered stages. */
fn program(n: u32) -> Vec<Op> {
    let mut ops = Vec::new();
    stage_outer(n, &mut ops);
    stage_middle(n, &mut ops);
    stage_last(n, &mut ops);
    ops
}

/** Inverse DCT of length 2^n, in place on t, with r as the Hadamard clamping range. The input is first permuted into bit-reversed order, as in the specification. */
pub(crate) fn inverse(t: &mut [i32], n: u32, r: u32) {
    let len = 1usize << n;
    let copy: [i32; 64] = std::array::from_fn(|i| if i < len { t[i] } else { 0 });
    for (i, slot) in t[..len].iter_mut().enumerate() {
        *slot = copy[brev(n, i)];
    }
    run(t, &program(n), r);
}
