/** The neighbour scans of the intra block copy motion vector stack (spec scan row, scan col, scan point and add reference motion vector, 7.10.2.2 to 7.10.2.8): which above and left positions are visited, how far each one's block extends along the edge, and the weight each candidate carries. */
use super::constants::{INTRA_FRAME, POINT_WEIGHT};
use super::decoder::TileDecoder;
use super::mv_stack::{MvStack, to_whole_sample};
use super::tables::{NUM_4X4_BLOCKS_HIGH, NUM_4X4_BLOCKS_WIDE};

/** Weight of one unit of a scanned run: a run of len 4x4 units weighs twice len. */
const RUN_WEIGHT_PER_UNIT: u32 = 2;
/** The smallest run a candidate covers when it lies more than one unit from the block's edge, and when the block is at least 16 units long. */
const FAR_RUN_MIN: usize = 2;
const LONG_RUN_MIN: usize = 4;
/** Units along an edge at which a scan is limited, and the block length from which the long-run minimum applies. */
const SCAN_EXTENT_MAX: usize = 16;
const LONG_BLOCK_MI: usize = 16;

/** Which way a scan runs: along a row (the scan moves across the block's width) or down a column. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Axis {
    Row,
    Col,
}

impl<'a> TileDecoder<'a> {
    /** scan row (deltaRow): walk the row `delta_row` above the block, across its width. */
    pub(super) fn scan_row(&self, stack: &mut MvStack, delta_row: isize) {
        self.scan_line(stack, Axis::Row, delta_row);
    }

    /** scan col (deltaCol): walk the column `delta_col` left of the block, down its height. */
    pub(super) fn scan_col(&self, stack: &mut MvStack, delta_col: isize) {
        self.scan_line(stack, Axis::Col, delta_col);
    }

    /** The walk shared by both scans: the offset of the first position (a far scan shifts it onto an even unit), then one candidate per run until the edge or the tile ends the walk. */
    fn scan_line(&self, stack: &mut MvStack, axis: Axis, offset: isize) {
        let far = offset.abs() > 1;
        let (along, across) = self.scan_extent(axis);
        let end = along.min(SCAN_EXTENT_MAX);
        let (row_step, col_step) = self.scan_origin(axis, offset, far);
        let mut i = 0;
        while i < end {
            let (dr, dc) = match axis {
                Axis::Row => (row_step, col_step + i as isize),
                Axis::Col => (row_step + i as isize, col_step),
            };
            let (mv_row, mv_col) = (self.blk.mi_row as isize + dr, self.blk.mi_col as isize + dc);
            if !self.is_inside(mv_row, mv_col) {
                break;
            }
            let (row, col) = (mv_row as usize, mv_col as usize);
            let cand = self.fs.mi.at(row, col);
            let cand_side = match axis {
                Axis::Row => NUM_4X4_BLOCKS_WIDE[cand.mi_size as usize],
                Axis::Col => NUM_4X4_BLOCKS_HIGH[cand.mi_size as usize],
            } as usize;
            let len = run_len(across, cand_side, far, across >= LONG_BLOCK_MI);
            self.add_candidate(stack, row, col, len as u32 * RUN_WEIGHT_PER_UNIT);
            i += len;
        }
    }

    /** The block's extent along and across a scan, in 4x4 units: the walk covers the block's side, bounded by the frame's edge. */
    fn scan_extent(&self, axis: Axis) -> (usize, usize) {
        match axis {
            Axis::Row => (
                self.blk
                    .bw4
                    .min(self.mi_cols().saturating_sub(self.blk.mi_col)),
                self.blk.bw4,
            ),
            Axis::Col => (
                self.blk
                    .bh4
                    .min(self.mi_rows().saturating_sub(self.blk.mi_row)),
                self.blk.bh4,
            ),
        }
    }

    /** The row and column offsets a scan starts from. A far scan moves its line by the block's parity so the candidates pair up across the grid. */
    fn scan_origin(&self, axis: Axis, offset: isize, far: bool) -> (isize, isize) {
        let parity_row = (self.blk.mi_row & 1) as isize;
        let parity_col = (self.blk.mi_col & 1) as isize;
        match (axis, far) {
            (Axis::Row, true) => (offset + parity_row, 1 - parity_col),
            (Axis::Row, false) => (offset, 0),
            (Axis::Col, true) => (1 - parity_row, offset + parity_col),
            (Axis::Col, false) => (0, offset),
        }
    }

    /** scan point (deltaRow, deltaCol): the single position at that offset, if it is inside the tile and already decoded. */
    pub(super) fn scan_point(&self, stack: &mut MvStack, delta_row: isize, delta_col: isize) {
        let mv_row = self.blk.mi_row as isize + delta_row;
        let mv_col = self.blk.mi_col as isize + delta_col;
        if !self.is_inside(mv_row, mv_col) {
            return;
        }
        let (row, col) = (mv_row as usize, mv_col as usize);
        if self.fs.mi.at(row, col).written {
            self.add_candidate(stack, row, col, POINT_WEIGHT);
        }
    }

    /** add reference motion vector: a candidate counts only when it is an inter block (an intra block copy block is one) that copies from the intra frame, and then its list-0 vector is added. */
    fn add_candidate(&self, stack: &mut MvStack, row: usize, col: usize, weight: u32) {
        let cand = self.fs.mi.at(row, col);
        if !cand.is_inter || cand.ref_frame[0] != INTRA_FRAME {
            return;
        }
        let mv = cand.mv[0].map(i32::from);
        stack.add(mv.map(to_whole_sample), weight);
    }
}

/** The length of one candidate's run along the edge: the smaller of the block's and the neighbour's sides, raised to the far minimum for a far scan and to the long minimum for a long block. */
fn run_len(block_side: usize, cand_side: usize, far: bool, long_block: bool) -> usize {
    let mut len = block_side.min(cand_side);
    if far {
        len = len.max(FAR_RUN_MIN);
    }
    if long_block {
        len = len.max(LONG_RUN_MIN);
    }
    len
}
