/** CDEF direction search (spec 7.15.2): for one 8x8 luma block, the dominant edge direction out of eight and the variance of the block along it. Costs use 64-bit sums because squared partial sums overflow 32 bits on 8-bit content. */
use crate::recon::Plane;

/** The size of the block the search measures, and the number of directions it compares. */
const BLOCK: usize = 8;
const DIRS: usize = 8;
/** The partial-sum rows: one per direction, each long enough for the diagonal sums. */
const PARTIAL_LEN: usize = 15;
/** The normalising table of the spec: entry n is 840 / n, for the diagonal sums of length n. */
const DIV_TABLE: [i64; 9] = [0, 840, 420, 280, 210, 168, 140, 120, 105];
/** The sample is centred on zero by subtracting this after the bit depth is reduced to eight bits. */
const CENTRE: i32 = 128;
/** The variance is the best cost less the opposite direction's cost, scaled down by this shift. */
const VAR_SHIFT: u32 = 10;

/** The direction of one 8x8 luma block at 4x4 position (r, c) and its variance (spec 7.15.2). */
pub(super) fn search(plane: &Plane, r: usize, c: usize, bit_depth: u32) -> (usize, i64) {
    let partial = partial_sums(plane, r, c, bit_depth);
    let cost = costs(&partial);
    let mut best = 0;
    let mut best_cost = 0;
    for (dir, &value) in cost.iter().enumerate() {
        if value > best_cost {
            best_cost = value;
            best = dir;
        }
    }
    let var = (best_cost - cost[(best + 4) & 7]) >> VAR_SHIFT;
    (best, var)
}

/** The sums of the centred samples along each of the eight directions. */
fn partial_sums(plane: &Plane, r: usize, c: usize, bit_depth: u32) -> [[i64; PARTIAL_LEN]; DIRS] {
    let mut partial = [[0i64; PARTIAL_LEN]; DIRS];
    let (x0, y0) = (c << 2, r << 2);
    for i in 0..BLOCK {
        for j in 0..BLOCK {
            let x = (plane.get(x0 + j, y0 + i) >> (bit_depth - 8)) - CENTRE;
            let x = i64::from(x);
            for (dir, index) in line_indices(i, j).into_iter().enumerate() {
                partial[dir][index] += x;
            }
        }
    }
    partial
}

/** The partial-sum slot that sample (i, j) of the block adds to, for each of the eight directions. */
fn line_indices(i: usize, j: usize) -> [usize; DIRS] {
    [
        i + j,
        i + j / 2,
        i,
        3 + i - j / 2,
        7 + i - j,
        3 - i / 2 + j,
        j,
        i / 2 + j,
    ]
}

/** The cost of each direction from its partial sums (spec 7.15.2). The axis directions 2 and 6 sum eight rows, the diagonals sum a triangle of lengths 1 to 8, and 1, 3, 5 and 7 are the off-axis directions. */
fn costs(partial: &[[i64; PARTIAL_LEN]; DIRS]) -> [i64; DIRS] {
    let mut cost = [0i64; DIRS];
    for (&a, &b) in partial[2].iter().zip(&partial[6]).take(BLOCK) {
        cost[2] += sq(a);
        cost[6] += sq(b);
    }
    cost[2] *= DIV_TABLE[8];
    cost[6] *= DIV_TABLE[8];
    for i in 0..7 {
        cost[0] += (sq(partial[0][i]) + sq(partial[0][14 - i])) * DIV_TABLE[i + 1];
        cost[4] += (sq(partial[4][i]) + sq(partial[4][14 - i])) * DIV_TABLE[i + 1];
    }
    cost[0] += sq(partial[0][7]) * DIV_TABLE[8];
    cost[4] += sq(partial[4][7]) * DIV_TABLE[8];
    for dir in [1, 3, 5, 7] {
        cost[dir] = off_axis_cost(&partial[dir]);
    }
    cost
}

/** The cost of one of the off-axis directions: a centre run of five, then pairs of shrinking runs on either side. */
fn off_axis_cost(line: &[i64; PARTIAL_LEN]) -> i64 {
    let mut cost: i64 = (3..=7).map(|k| sq(line[k])).sum::<i64>() * DIV_TABLE[8];
    for j in 0..3 {
        cost += (sq(line[j]) + sq(line[10 - j])) * DIV_TABLE[2 * j + 2];
    }
    cost
}

/** The square of a partial sum. */
fn sq(v: i64) -> i64 {
    v * v
}
