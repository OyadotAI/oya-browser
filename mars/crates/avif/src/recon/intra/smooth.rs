use super::edges::Edges;
/** Smooth intra prediction (spec 7.11.2.6): the three smooth modes blend the edges with weights that fall off across the block, SMOOTH in both directions, SMOOTH_V down the columns and SMOOTH_H across the rows. */
use super::{Request, SMOOTH_PRED, SMOOTH_V_PRED};
use crate::recon::math::round2;
use crate::recon::tables::{
    SM_WEIGHTS_TX_4X4, SM_WEIGHTS_TX_8X8, SM_WEIGHTS_TX_16X16, SM_WEIGHTS_TX_32X32,
    SM_WEIGHTS_TX_64X64,
};

/** The weights of a smooth edge of 2^log2 samples, from the table for that size. */
fn weights(log2: u32) -> &'static [u8] {
    match log2 {
        2 => &SM_WEIGHTS_TX_4X4,
        3 => &SM_WEIGHTS_TX_8X8,
        4 => &SM_WEIGHTS_TX_16X16,
        5 => &SM_WEIGHTS_TX_32X32,
        _ => &SM_WEIGHTS_TX_64X64,
    }
}

/** Weighted blend of two values with weight `wt` on the first and 256 - wt on the second. */
fn mix(a: i32, b: i32, wt: i32) -> i32 {
    wt * a + (256 - wt) * b
}

/** Smooth prediction of a w by h block, in the direction the mode names. */
pub(super) fn predict(e: &Edges, req: &Request, mode: u8, w: usize, h: usize) -> Vec<i32> {
    let wx = weights(req.log2w);
    let wy = weights(req.log2h);
    let bottom = e.left(h as isize - 1);
    let right = e.above(w as isize - 1);
    let mut pred = vec![0i32; w * h];
    for i in 0..h {
        for j in 0..w {
            let (above, left) = (e.above(j as isize), e.left(i as isize));
            let vertical = mix(above, bottom, i32::from(wy[i]));
            let horizontal = mix(left, right, i32::from(wx[j]));
            pred[i * w + j] = match mode {
                SMOOTH_PRED => round2(i64::from(vertical + horizontal), 9),
                SMOOTH_V_PRED => round2(i64::from(vertical), 8),
                _ => round2(i64::from(horizontal), 8),
            } as i32;
        }
    }
    pred
}
