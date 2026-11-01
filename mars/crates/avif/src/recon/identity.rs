/** The identity transforms and the Walsh-Hadamard transform used by lossless blocks (spec 7.13.2.10 to 7.13.2.14). Identity transforms scale each sample by a size-dependent factor; the WHT is an exact integer transform with a pre-shift. */
use super::math::round2;

/** Inverse identity transform of length 2^n for n in 2 to 5, in place on t. Each size has its own scale: 4 and 16 are fixed-point multiplies by sqrt(2) and 2*sqrt(2), 8 and 32 are plain multiplies by 2 and 4. */
pub(crate) fn inverse(t: &mut [i32], n: u32) {
    let len = 1usize << n;
    for v in &mut t[..len] {
        *v = match n {
            2 => round2(i64::from(*v) * 5793, 12) as i32,
            3 => *v * 2,
            4 => round2(i64::from(*v) * 11586, 12) as i32,
            _ => *v * 4,
        };
    }
}

/** Inverse Walsh-Hadamard transform of length 4 in place on t, after an arithmetic right shift of every input by `shift` (spec 7.13.2.10). */
pub(crate) fn wht(t: &mut [i32], shift: u32) {
    let mut a = t[0] >> shift;
    let mut c = t[1] >> shift;
    let mut d = t[2] >> shift;
    let mut b = t[3] >> shift;
    a += c;
    d -= b;
    let e = (a - d) >> 1;
    b = e - b;
    c = e - c;
    a -= b;
    d += c;
    t[0] = a;
    t[1] = b;
    t[2] = c;
    t[3] = d;
}
