/** The Wiener filter (spec 7.17.4 and 7.17.5): a separable seven-tap filter whose coefficients are symmetric with unit gain. The horizontal pass runs first over h + 6 rows, then the vertical pass, both rounded and clipped as the spec's intermediate precision requires. */
use super::source::Window;
use crate::recon::Plane;

/** The filter's precision: the taps sum to 2^FILTER_BITS. */
const FILTER_BITS: u32 = 7;
/** The rounding of the first and second pass for 8 to 10 bits and for 12 bits. */
const ROUND0: u32 = 3;
const ROUND1: u32 = 11;
const ROUND0_12: u32 = 5;
const ROUND1_12: u32 = 9;
const TAPS: usize = 7;
/** The centre tap is 128 less twice each coded coefficient. */
const CENTRE_TAP: i32 = 128;
/** The top rows and columns of the window the filter needs beyond the block: three each way. */
const HALF_TAPS: isize = 3;

/** The seven taps of a Wiener filter from its three coded coefficients (spec 7.17.5). */
pub(super) fn taps(coeff: [i32; 3]) -> [i32; TAPS] {
    let mut filter = [0; TAPS];
    filter[3] = CENTRE_TAP;
    for (i, &c) in coeff.iter().enumerate() {
        filter[i] = c;
        filter[TAPS - 1 - i] = c;
        filter[3] -= 2 * c;
    }
    filter
}

/** Filter the block at (x0, y0) of size w by h into `out`, with the vertical taps and horizontal taps the unit carries. */
pub(super) fn filter_block(
    win: &Window,
    out: &mut Plane,
    rect: (isize, isize, usize, usize),
    coeffs: [[i32; 3]; 2],
    bit_depth: u32,
) {
    let (x0, y0, w, h) = rect;
    let (vfilter, hfilter) = (taps(coeffs[0]), taps(coeffs[1]));
    let (round0, round1) = if bit_depth == 12 {
        (ROUND0_12, ROUND1_12)
    } else {
        (ROUND0, ROUND1)
    };
    let offset = 1i32 << (bit_depth + FILTER_BITS - round0 - 1);
    let limit = (1i32 << (bit_depth + 1 + FILTER_BITS - round0)) - 1;
    let inter = horizontal(win, (x0, y0), (w, h), &hfilter, round0, (offset, limit));
    let max = (1i32 << bit_depth) - 1;
    for r in 0..h {
        for c in 0..w {
            let s: i32 = (0..TAPS).map(|t| vfilter[t] * inter[(r + t) * w + c]).sum();
            let v = (s + (1 << (round1 - 1))) >> round1;
            out.set(x0 as usize + c, y0 as usize + r, v.clamp(0, max) as u16);
        }
    }
}

/** The horizontal pass over the block's rows and the three rows above and below, clipped to the intermediate range. */
fn horizontal(
    win: &Window,
    (x0, y0): (isize, isize),
    (w, h): (usize, usize),
    filter: &[i32; TAPS],
    round0: u32,
    (offset, limit): (i32, i32),
) -> Vec<i32> {
    let mut inter = vec![0; (h + 2 * HALF_TAPS as usize) * w];
    for r in 0..h + 2 * HALF_TAPS as usize {
        let y = y0 + r as isize - HALF_TAPS;
        for c in 0..w {
            let x = x0 + c as isize;
            let s: i32 = (0..TAPS)
                .map(|t| filter[t] * win.at(x + t as isize - HALF_TAPS, y))
                .sum();
            let v = (s + (1 << (round0 - 1))) >> round0;
            inter[r * w + c] = v.clamp(-offset, limit - offset);
        }
    }
    inter
}
