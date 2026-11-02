/** Filter intra prediction (spec 7.11.2.3): a luma block of at most 32 samples is predicted in 4x2 cells, each from seven neighbours already predicted, by one of five trained tap sets. */
use super::edges::Edges;
use crate::recon::math::round2_signed;
use crate::recon::tables::INTRA_FILTER_TAPS;

/** Output cells are 4 samples wide and 2 high; each cell has 8 outputs and 7 taps. */
const CELL_W: usize = 4;
const CELL_H: usize = 2;
const TAPS: usize = 7;
const OUTPUTS: usize = CELL_W * CELL_H;
/** Fractional bits of the filter weights, which are scaled by 16. */
const SCALE_BITS: u32 = 4;

/** Filter intra prediction of a w by h block with one of the five filter modes; `clip` is the largest sample value. */
pub(super) fn predict(e: &Edges, w: usize, h: usize, filter_mode: u8, clip: i32) -> Vec<i32> {
    let mut pred = vec![0i32; w * h];
    for i2 in 0..h / CELL_H {
        for j4 in 0..w / CELL_W {
            let p = neighbours(e, &pred, w, i2, j4);
            let outputs = cell(&p, filter_mode, clip);
            for (k, value) in outputs.iter().enumerate() {
                let (i1, j1) = (k / CELL_W, k % CELL_W);
                pred[(i2 * CELL_H + i1) * w + j4 * CELL_W + j1] = *value;
            }
        }
    }
    pred
}

/** The seven neighbours of a cell: five along the row above it (the corner included) and two down the left, read from the edges at the block border and from the predictions already made inside the block. */
fn neighbours(e: &Edges, pred: &[i32], w: usize, i2: usize, j4: usize) -> [i32; TAPS] {
    let mut p = [0i32; TAPS];
    let (row, col) = ((i2 * CELL_H) as isize, (j4 * CELL_W) as isize);
    for (i, slot) in p.iter_mut().enumerate().take(5) {
        *slot = if i2 == 0 {
            e.above(col + i as isize - 1)
        } else if j4 == 0 && i == 0 {
            e.left(row - 1)
        } else {
            pred[(row as usize - 1) * w + (col as usize + i - 1)]
        };
    }
    for i in 5..TAPS {
        p[i] = if j4 == 0 {
            e.left(row + i as isize - 5)
        } else {
            pred[(row as usize + i - 5) * w + (col as usize - 1)]
        };
    }
    p
}

/** The eight outputs of a cell: each is a weighted sum of the seven neighbours, rounded and clipped. */
fn cell(p: &[i32; TAPS], filter_mode: u8, clip: i32) -> [i32; OUTPUTS] {
    let mut out = [0i32; OUTPUTS];
    for (k, slot) in out.iter_mut().enumerate() {
        let base = usize::from(filter_mode) * OUTPUTS * TAPS + k * TAPS;
        let sum: i64 = (0..TAPS)
            .map(|i| i64::from(INTRA_FILTER_TAPS[base + i]) * i64::from(p[i]))
            .sum();
        *slot = round2_signed(sum, SCALE_BITS).clamp(0, i64::from(clip)) as i32;
    }
    out
}
