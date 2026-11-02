/** The edge conditioning of directional prediction (spec 7.11.2.7 to 7.11.2.12): the corner filter, the strength and upsampling choices, the three-tap edge filter, and the 2x upsampling of short edges. Each works on one edge array addressed from index -2. */
use super::edges::{EDGE_OFFSET, Edges};
use crate::recon::math::{clip3, round2};
use crate::recon::tables::INTRA_EDGE_KERNEL;

/** Taps of one edge kernel, and the kernel count of the table. */
const EDGE_TAPS: usize = 5;
/** Bits of rounding applied after an edge kernel, which sums to 16. */
const EDGE_ROUND_BITS: u32 = 4;
/** Largest angle difference, in degrees, that still upsamples an edge. */
const UPSAMPLE_MAX_DELTA: i32 = 40;

/** The corner sample after the corner filter: a three-tap average of the left neighbour, the top-left and the above neighbour, weighted 5, 6, 5. */
pub(super) fn corner(e: &Edges) -> i32 {
    round2(
        i64::from(e.left(0)) * 5 + i64::from(e.above(-1)) * 6 + i64::from(e.above(0)) * 5,
        4,
    ) as i32
}

/** The filter strength (0 to 3) for an edge of a w by h block whose direction differs from the nominal angle by delta degrees. The thresholds depend on the block's size sum and on whether a smooth neighbour is present. */
pub(super) fn strength(w: usize, h: usize, smooth: bool, delta: i32) -> u8 {
    let (sum, d) = (w + h, delta.abs());
    if smooth {
        smooth_strength(sum, d)
    } else {
        plain_strength(sum, d)
    }
}

/** Strength for an edge without a smooth neighbour, by the size sum and the angle difference (spec 7.11.2.9, filterType 0). */
fn plain_strength(sum: usize, d: i32) -> u8 {
    match sum {
        0..=8 => u8::from(d >= 56),
        9..=16 => u8::from(d >= 40),
        17..=24 => u8::from(d >= 8) + u8::from(d >= 16) + u8::from(d >= 32),
        25..=32 => 1 + u8::from(d >= 4) + u8::from(d >= 32),
        _ => 3,
    }
}

/** Strength for an edge with a smooth neighbour (spec 7.11.2.9, filterType 1). */
fn smooth_strength(sum: usize, d: i32) -> u8 {
    match sum {
        0..=8 => u8::from(d >= 40) + u8::from(d >= 64),
        9..=16 => u8::from(d >= 20) + u8::from(d >= 48),
        17..=24 => 3 * u8::from(d >= 4),
        _ => 3,
    }
}

/** Whether an edge of a w by h block is upsampled for a delta-degree direction difference: only for short edges, and never when the direction is exact or far off. */
pub(super) fn use_upsample(w: usize, h: usize, smooth: bool, delta: i32) -> bool {
    let d = delta.abs();
    let sum = w + h;
    if d <= 0 || d >= UPSAMPLE_MAX_DELTA {
        false
    } else if smooth {
        sum <= 8
    } else {
        sum <= 16
    }
}

/** Index of entry i of an edge array in the backing vector. */
fn at(i: isize) -> usize {
    (i + EDGE_OFFSET as isize) as usize
}

/** The edge filter (spec 7.11.2.12): smooth the first `size - 1` entries of an edge with the kernel of the strength, reading from the original values. Strength 0 leaves the edge alone. */
pub(super) fn filter_edge(buf: &mut [i32], size: usize, strength: u8) {
    if strength == 0 {
        return;
    }
    let edge: Vec<i32> = (0..size).map(|i| buf[at(i as isize - 1)]).collect();
    let kernel = usize::from(strength - 1) * EDGE_TAPS;
    for i in 1..size {
        let mut s = 0i64;
        for j in 0..EDGE_TAPS {
            let k = clip3(0, size as i64 - 1, i as i64 - 2 + j as i64) as usize;
            s += i64::from(INTRA_EDGE_KERNEL[kernel + j]) * i64::from(edge[k]);
        }
        buf[at(i as isize - 1)] = ((s + 8) >> EDGE_ROUND_BITS) as i32;
    }
}

/** The edge upsampling (spec 7.11.2.11): double the resolution of the first `count` entries of an edge, interpolating with a four-tap filter and clipping to the bit depth. The result covers entries -2 through 2 * count - 2. */
pub(super) fn upsample(buf: &mut [i32], count: usize, max: i32) {
    let mut dup = vec![0i32; count + 3];
    dup[0] = buf[at(-1)];
    for i in -1..count as isize {
        dup[(i + 2) as usize] = buf[at(i)];
    }
    dup[count + 2] = buf[at(count as isize - 1)];
    buf[at(-2)] = dup[0];
    for i in 0..count {
        let s = -dup[i] + 9 * dup[i + 1] + 9 * dup[i + 2] - dup[i + 3];
        let value = clip3(0, i64::from(max), round2(i64::from(s), 4)) as i32;
        buf[at(2 * i as isize - 1)] = value;
        buf[at(2 * i as isize)] = dup[i + 2];
    }
}
