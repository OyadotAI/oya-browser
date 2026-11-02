/** The self-guided filter (spec 7.17.2 and 7.17.3): two box-filter passes of different radius build guided estimates of the block, and a projection blends them with the CDEF output by two weights the unit carries. */
use super::source::Window;
use crate::recon::Plane;
use crate::tile::{SGR_PARAMS, SGRPROJ_PRJ_BITS};

/** Internal precisions of the box filter, named as the spec names them. */
const RST_BITS: u32 = 4;
const SGR_BITS: u32 = 8;
const MTABLE_BITS: u32 = 20;
const RECIP_BITS: u32 = 12;
/** The largest z before the ratio saturates, and the value it saturates to. */
const Z_MAX: i32 = 255;
const A_MAX: i32 = 256;
/** The radius is in samples; the box is 2r+1 wide, and the first pass reads only odd rows. */
const PASS0: usize = 0;
const ODD_MASK: isize = 1;
const SHIFT_EVEN: u32 = 5;
const SHIFT_ODD: u32 = 4;

/** The self-guided output of a block (spec 7.17.2): the blend of the CDEF samples with the two filtered estimates by the unit's weights. */
pub(super) fn filter_block(
    win: &Window,
    cdef: &Plane,
    out: &mut Plane,
    rect: (isize, isize, usize, usize),
    unit: (usize, [i32; 2]),
    bit_depth: u32,
) {
    let (x0, y0, w, h) = rect;
    let (set, xqd) = unit;
    let flt0 = box_filter(win, (x0, y0), (w, h), set, 0, bit_depth);
    let flt1 = box_filter(win, (x0, y0), (w, h), set, 1, bit_depth);
    let w0 = xqd[0];
    let w1 = xqd[1];
    let w2 = (1 << SGRPROJ_PRJ_BITS) - w0 - w1;
    let (r0, r1) = (SGR_PARAMS[set][0], SGR_PARAMS[set][2]);
    let max = (1i32 << bit_depth) - 1;
    for i in 0..h {
        for j in 0..w {
            let (x, y) = (x0 as usize + j, y0 as usize + i);
            let u = cdef.get(x, y) << RST_BITS;
            let mut v = w1 * u;
            v += w0 * if r0 != 0 { flt0[i * w + j] } else { u };
            v += w2 * if r1 != 0 { flt1[i * w + j] } else { u };
            let s = (v + (1 << (RST_BITS + SGRPROJ_PRJ_BITS - 1))) >> (RST_BITS + SGRPROJ_PRJ_BITS);
            out.set(x, y, s.clamp(0, max) as u16);
        }
    }
}

/** The box filter of one pass (spec 7.17.3): per-position sums of squares and of samples over a window of radius r, turned into a and b coefficients and blended over a 3x3 neighbourhood. An empty result means the pass is off for this set. */
fn box_filter(
    win: &Window,
    (x0, y0): (isize, isize),
    (w, h): (usize, usize),
    set: usize,
    pass: usize,
    bit_depth: u32,
) -> Vec<i32> {
    let r = SGR_PARAMS[set][pass * 2] as isize;
    if r == 0 {
        return Vec::new();
    }
    let eps = i64::from(SGR_PARAMS[set][pass * 2 + 1]);
    let ab = coefficients(win, (x0, y0), (w, h), r, eps, bit_depth);
    let stride = w + 2;
    let mut f = vec![0; w * h];
    for i in 0..h {
        let shift = if pass == PASS0 && (i as isize & ODD_MASK) != 0 {
            SHIFT_ODD
        } else {
            SHIFT_EVEN
        };
        for j in 0..w {
            let (a, b) = blend(&ab, stride, (i, j), pass);
            let v = a * win.at(x0 + j as isize, y0 + i as isize) as i64 + b;
            f[i * w + j] = ((v + (1 << (SGR_BITS + shift - RST_BITS - 1)))
                >> (SGR_BITS + shift - RST_BITS)) as i32;
        }
    }
    f
}

/** The a and b arrays of one box pass over the block and a one-sample border, as (a, b) pairs in row-major order with a stride of w + 2. */
fn coefficients(
    win: &Window,
    (x0, y0): (isize, isize),
    (w, h): (usize, usize),
    r: isize,
    eps: i64,
    bit_depth: u32,
) -> Vec<(i64, i64)> {
    let n = ((2 * r + 1) * (2 * r + 1)) as i64;
    let n2e = n * n * eps;
    let s = ((1i64 << MTABLE_BITS) + n2e / 2) / n2e;
    let one_over_n = ((1i64 << RECIP_BITS) + n / 2) / n;
    let mut ab = Vec::with_capacity((w + 2) * (h + 2));
    for i in -1..=h as isize {
        for j in -1..=w as isize {
            let (sum_sq, sum) = window_sums(win, (x0 + j, y0 + i), r);
            ab.push(ab_pair(sum_sq, sum, (n, s, one_over_n), bit_depth));
        }
    }
    ab
}

/** The sum of squares and the sum of the samples over the (2r+1) square centred on a position. */
fn window_sums(win: &Window, (x, y): (isize, isize), r: isize) -> (i64, i64) {
    let mut sum_sq = 0i64;
    let mut sum = 0i64;
    for dy in -r..=r {
        for dx in -r..=r {
            let c = i64::from(win.at(x + dx, y + dy));
            sum_sq += c * c;
            sum += c;
        }
    }
    (sum_sq, sum)
}

/** One position's a and b, from its window sums (spec 7.17.3): the variance ratio is looked up through z, and b is the weight of the local mean. */
fn ab_pair(
    sum_sq: i64,
    sum: i64,
    (n, s, one_over_n): (i64, i64, i64),
    bit_depth: u32,
) -> (i64, i64) {
    let shift = bit_depth - 8;
    let a = round2(sum_sq, 2 * shift);
    let d = round2(sum, shift);
    let p = (a * n - d * d).max(0);
    let z = round2(p * s, MTABLE_BITS);
    let a2 = if z >= i64::from(Z_MAX) {
        i64::from(A_MAX)
    } else if z == 0 {
        1
    } else {
        ((z << SGR_BITS) + z / 2) / (z + 1)
    };
    let b2 = ((1i64 << SGR_BITS) - a2) * sum * one_over_n;
    (a2, round2(b2, RECIP_BITS))
}

/** The weighted a and b of one output position: the 3x3 neighbourhood with the spec's weights for the pass. Pass 0 reads only odd rows, so even rows weigh nothing. */
fn blend(ab: &[(i64, i64)], stride: usize, (i, j): (usize, usize), pass: usize) -> (i64, i64) {
    let mut a = 0i64;
    let mut b = 0i64;
    for dy in -1isize..=1 {
        for dx in -1isize..=1 {
            let weight = weight(pass, (i as isize + dy, dx, dy));
            let idx = (i as isize + dy + 1) as usize * stride + (j as isize + dx + 1) as usize;
            a += weight * ab[idx].0;
            b += weight * ab[idx].1;
        }
    }
    (a, b)
}

/** The weight of a neighbour in the blend: for pass 0 only odd rows count (6 straight, 5 diagonal); for pass 1 the cross weighs 4 and the corners 3. */
fn weight(pass: usize, (row, dx, dy): (isize, isize, isize)) -> i64 {
    if pass == PASS0 {
        if row & ODD_MASK == 0 {
            0
        } else if dx == 0 {
            6
        } else {
            5
        }
    } else if dx == 0 || dy == 0 {
        4
    } else {
        3
    }
}

/** Round2 for a signed value: add half the divisor and shift right, which rounds ties upwards. */
fn round2(x: i64, n: u32) -> i64 {
    if n == 0 { x } else { (x + (1 << (n - 1))) >> n }
}
