/** The butterfly primitives of the AV1 1-D transforms (spec 7.13.2.1): the rotation B and the Hadamard step H, the cos128 and sin128 lookups, and the bit reversal. A transform is written as a program of these steps, so the DCT and ADST sequences read like the specification. */
use super::math::{clip3, round2};
use super::tables::COS128_LOOKUP;

/** One step of a 1-D transform program: a rotation of two samples by an angle (B), or a sum and difference of two samples (H). A flip swaps the outputs of a rotation, or the indices of a Hadamard step. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Op {
    Rot {
        a: usize,
        b: usize,
        angle: i32,
        flip: bool,
    },
    Had {
        a: usize,
        b: usize,
        flip: bool,
    },
}

/** cos128(angle): 4096 times the cosine of angle * pi / 128, read from a quarter-wave table with the sign and symmetry of the angle. */
pub(crate) fn cos128(angle: i32) -> i64 {
    let a = angle & 255;
    let (index, sign) = match a {
        0..=64 => (a, 1),
        65..=128 => (128 - a, -1),
        129..=192 => (a - 128, -1),
        _ => (256 - a, 1),
    };
    i64::from(sign * COS128_LOOKUP[index as usize])
}

/** sin128(angle), which is cos128 shifted by a quarter turn. */
pub(crate) fn sin128(angle: i32) -> i64 {
    cos128(angle - 64)
}

/** brev(numBits, x): the low numBits bits of x in reverse order. */
pub(crate) fn brev(num_bits: u32, x: usize) -> usize {
    (0..num_bits).fold(0, |t, i| t | (((x >> i) & 1) << (num_bits - 1 - i)))
}

/** B(a, b, angle, flip): rotate the pair (t[a], t[b]) by angle, rounding each product sum to 12 fractional bits, and swap the pair when flipped. */
pub(crate) fn rotate(t: &mut [i32], a: usize, b: usize, angle: i32, flip: bool) {
    let (ta, tb) = (i64::from(t[a]), i64::from(t[b]));
    let (c, s) = (cos128(angle), sin128(angle));
    let x = round2(ta * c - tb * s, 12);
    let y = round2(ta * s + tb * c, 12);
    t[a] = x as i32;
    t[b] = y as i32;
    if flip {
        t.swap(a, b);
    }
}

/** H(a, b, flip, r): replace the pair with its sum and difference, each clamped to a signed r-bit range. A flip exchanges a and b first. */
pub(crate) fn hadamard(t: &mut [i32], a: usize, b: usize, flip: bool, r: u32) {
    let (a, b) = if flip { (b, a) } else { (a, b) };
    let (x, y) = (i64::from(t[a]), i64::from(t[b]));
    let high = (1i64 << (r - 1)) - 1;
    let low = -(1i64 << (r - 1));
    t[a] = clip3(low, high, x + y) as i32;
    t[b] = clip3(low, high, x - y) as i32;
}

/** Run a transform program over the array, step by step, with r as the clamping range of the Hadamard steps. */
pub(crate) fn run(t: &mut [i32], program: &[Op], r: u32) {
    for op in program {
        match *op {
            Op::Rot { a, b, angle, flip } => rotate(t, a, b, angle, flip),
            Op::Had { a, b, flip } => hadamard(t, a, b, flip, r),
        }
    }
}
