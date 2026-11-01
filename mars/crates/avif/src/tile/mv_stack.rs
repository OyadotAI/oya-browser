/** Motion vector candidate stack of an intra block copy block (spec find_mv_stack, 7.10.2, as it applies to an intra frame): the candidate vectors, their weights, the stable sort by weight, and the clamp to the frame. An intra frame has no other reference frame, so the temporal, extra-search and global candidates never apply. */
use super::constants::{
    INTEGER_ROUND_BIAS, MAX_REF_MV_STACK_SIZE, MI_SIZE, MV_BORDER, MV_FRAC_BITS, MV_SCAN_FAR_COL,
    MV_SCAN_FAR_ROW, MV_SCAN_FARTHER_COL, MV_SCAN_FARTHER_ROW, MV_SCAN_NEAR, MV_UNIT,
    POINT_SKIP_MI, REF_CAT_LEVEL,
};
use super::decoder::TileDecoder;

/** The candidate vectors of one block, in stack order, with the weight each has gathered. Only list 0 exists for intra block copy. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MvStack {
    len: usize,
    mvs: [[i32; 2]; MAX_REF_MV_STACK_SIZE],
    weights: [u32; MAX_REF_MV_STACK_SIZE],
}

impl MvStack {
    /** The number of candidates found. */
    pub(super) fn len(&self) -> usize {
        self.len
    }

    /** The candidate at `idx`, or zero past the candidates found (the spec fills those slots with the zero global vector). */
    pub(crate) fn mv_or_zero(&self, idx: usize) -> [i32; 2] {
        if idx < self.len {
            self.mvs[idx]
        } else {
            [0, 0]
        }
    }

    /** Add a vector with a weight: a vector already in the stack gains the weight, otherwise it is appended while there is room. A full stack ignores new vectors. */
    pub(super) fn add(&mut self, mv: [i32; 2], weight: u32) {
        if let Some(idx) = (0..self.len).find(|&i| self.mvs[i] == mv) {
            self.weights[idx] += weight;
        } else if self.len < MAX_REF_MV_STACK_SIZE {
            self.mvs[self.len] = mv;
            self.weights[self.len] = weight;
            self.len += 1;
        }
    }

    /** Add REF_CAT_LEVEL to the weight of the first `count` candidates, so the immediate neighbourhood outranks the rest. */
    pub(super) fn boost_nearest(&mut self, count: usize) {
        for weight in &mut self.weights[..count] {
            *weight += REF_CAT_LEVEL;
        }
    }

    /** The stable sort of entries [start, end) by descending weight (spec sorting process): repeated passes swap each adjacent pair that is out of order. */
    pub(super) fn sort(&mut self, start: usize, end: usize) {
        let mut end = end;
        while end > start {
            let mut new_end = start;
            for idx in start + 1..end {
                if self.weights[idx - 1] < self.weights[idx] {
                    self.weights.swap(idx - 1, idx);
                    self.mvs.swap(idx - 1, idx);
                    new_end = idx;
                }
            }
            end = new_end;
        }
    }
}

/** Round a candidate to whole samples (spec lower precision process with force_integer_mv): the magnitude is rounded up past half a sample and the sign kept. */
pub(super) fn to_whole_sample(component: i32) -> i32 {
    let whole = (component.abs() + INTEGER_ROUND_BIAS) >> MV_FRAC_BITS;
    if component > 0 {
        whole << MV_FRAC_BITS
    } else {
        -(whole << MV_FRAC_BITS)
    }
}

impl<'a> TileDecoder<'a> {
    /** find_mv_stack for an intra block copy block: the spatial scans, boosted and sorted, then clamped to the frame. The result is what assign_mv takes its predicted vector from. */
    pub(super) fn intrabc_mv_stack(&self) -> MvStack {
        let mut stack = MvStack::default();
        self.scan_row(&mut stack, MV_SCAN_NEAR);
        self.scan_col(&mut stack, MV_SCAN_NEAR);
        if self.blk.bw4.max(self.blk.bh4) <= POINT_SKIP_MI {
            self.scan_point(&mut stack, -1, self.blk.bw4 as isize);
        }
        let nearest = stack.len();
        stack.boost_nearest(nearest);
        self.scan_point(&mut stack, -1, -1);
        self.scan_farther(&mut stack);
        stack.sort(0, nearest);
        stack.sort(nearest, stack.len());
        self.clamp_stack(&mut stack);
        stack
    }

    /** The farther rows and columns the search reaches: three back, and five back for blocks that are more than one 4x4 unit across or down. */
    fn scan_farther(&self, stack: &mut MvStack) {
        self.scan_row(stack, MV_SCAN_FAR_ROW);
        self.scan_col(stack, MV_SCAN_FAR_COL);
        if self.blk.bh4 > 1 {
            self.scan_row(stack, MV_SCAN_FARTHER_ROW);
        }
        if self.blk.bw4 > 1 {
            self.scan_col(stack, MV_SCAN_FARTHER_COL);
        }
    }

    /** Clamp each candidate so the block stays within MV_BORDER plus its own size of the frame edges (spec context and clamping process). */
    fn clamp_stack(&self, stack: &mut MvStack) {
        let (bw, bh) = (self.blk.width() as i32, self.blk.height() as i32);
        for idx in 0..stack.len() {
            let mv = stack.mvs[idx];
            stack.mvs[idx] = [
                self.clamp_mv_row(mv[0], MV_BORDER + bh * MV_UNIT),
                self.clamp_mv_col(mv[1], MV_BORDER + bw * MV_UNIT),
            ];
        }
    }

    /** clamp_mv_row(mvec, border): clip a row component to the block's distance from the top and bottom of the frame. */
    fn clamp_mv_row(&self, mvec: i32, border: i32) -> i32 {
        let to_top = -(self.blk.mi_row as i32 * MI_SIZE as i32 * MV_UNIT);
        let to_bottom = (self.mi_rows() as i32 - self.blk.bh4 as i32 - self.blk.mi_row as i32)
            * MI_SIZE as i32
            * MV_UNIT;
        mvec.clamp(to_top - border, to_bottom + border)
    }

    /** clamp_mv_col(mvec, border): clip a column component to the block's distance from the left and right of the frame. */
    fn clamp_mv_col(&self, mvec: i32, border: i32) -> i32 {
        let to_left = -(self.blk.mi_col as i32 * MI_SIZE as i32 * MV_UNIT);
        let to_right = (self.mi_cols() as i32 - self.blk.bw4 as i32 - self.blk.mi_col as i32)
            * MI_SIZE as i32
            * MV_UNIT;
        mvec.clamp(to_left - border, to_right + border)
    }
}
