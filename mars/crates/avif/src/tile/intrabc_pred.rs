/** Intra block copy prediction (spec 7.11.3.4 with refIdx -1, and the rounding of 7.11.3.2): each plane of the block is predicted from the samples already reconstructed at its vector, through the bilinear filter. At whole-sample positions the filter is an exact copy; chroma can land on half samples, where it averages. The copy runs before the residual, and reads samples before any loop filter, which intra block copy disables. */
use super::constants::{
    BITDEPTH_HIGH, FILTER_ROWS_ABOVE, FILTER_ROWS_EXTRA, FILTER_TAP_NEXT, FILTER_TAP_SELF,
    FILTER_TOTAL, HALF_UNIT_SCALE, HIGH_BITDEPTH_ROUND, INTER_ROUND0, INTER_ROUND1, MI_SIZE,
    SUBPEL_BITS, SUBPEL_MASK,
};
use super::decoder::TileDecoder;
use super::lr::plane_subsampling;
use super::tables::{NUM_4X4_BLOCKS_HIGH, NUM_4X4_BLOCKS_WIDE};
use crate::recon::Plane;

/** The bilinear weight step per sub-sample position: the filter's total weight spread over its sixteen positions. */
const TAP_STEP: i32 = FILTER_TOTAL >> SUBPEL_BITS;

/** The geometry of one block's source: where its first sample sits in 1/16 sample units, the fractions, the size, and the last readable column and row. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Source {
    x0: isize,
    y0: isize,
    fx: usize,
    fy: usize,
    w: usize,
    h: usize,
    last_x: isize,
    last_y: isize,
}

impl<'a> TileDecoder<'a> {
    /** compute_prediction for an intra block copy block: every plane of the block, each predicted from the block's one vector. Intra block copy reads the whole block at once, so a small chroma block takes the block's vector too. */
    pub(super) fn predict_intrabc(&mut self) {
        let planes = if self.blk.has_chroma { 3 } else { 1 };
        for plane in 0..planes {
            self.predict_intrabc_plane(plane);
        }
    }

    /** One plane of an intra block copy block: its size and place in the plane, the filtered source, and the write of the result over the block. */
    fn predict_intrabc_plane(&mut self, plane: usize) {
        let size = self.plane_residual_size(self.blk.mi_size, plane);
        let w = NUM_4X4_BLOCKS_WIDE[size] as usize * MI_SIZE;
        let h = NUM_4X4_BLOCKS_HIGH[size] as usize * MI_SIZE;
        let (sub_x, sub_y) = plane_subsampling(self.seq, plane);
        let base_x = (self.blk.mi_col >> sub_x) * MI_SIZE;
        let base_y = (self.blk.mi_row >> sub_y) * MI_SIZE;
        let geom = self.source_geometry(plane, (base_x, base_y), (w, h));
        let horizontal = self.horizontal_pass(plane, &geom);
        let samples = self.vertical_pass(&geom, &horizontal);
        let target = &mut self.fs.planes[plane];
        write_block(target, (base_x, base_y), w, &samples);
    }

    /** Where a block's source lies: its vector applied to its place, in 1/16 sample units of the plane, and the readable extent. */
    fn source_geometry(&self, plane: usize, at: (usize, usize), size: (usize, usize)) -> Source {
        let (sub_x, sub_y) = plane_subsampling(self.seq, plane);
        let [mv_row, mv_col] = self.blk.mv;
        let x16 = ((at.0 as isize) << SUBPEL_BITS) + ((HALF_UNIT_SCALE * mv_col) >> sub_x) as isize;
        let y16 = ((at.1 as isize) << SUBPEL_BITS) + ((HALF_UNIT_SCALE * mv_row) >> sub_y) as isize;
        Source {
            x0: x16 >> SUBPEL_BITS,
            y0: y16 >> SUBPEL_BITS,
            fx: (x16 & SUBPEL_MASK as isize) as usize,
            fy: (y16 & SUBPEL_MASK as isize) as usize,
            w: size.0,
            h: size.1,
            last_x: self.last_sample_column(sub_x),
            last_y: self.last_sample_row(sub_y),
        }
    }

    /** The last column of the plane the source may read: the frame's mode-info width, which intra block copy uses in place of the frame width. */
    fn last_sample_column(&self, sub_x: usize) -> isize {
        ((self.mi_cols() * MI_SIZE + sub_x) >> sub_x) as isize - 1
    }

    /** The last row of the plane the source may read, from the mode-info height. */
    fn last_sample_row(&self, sub_y: usize) -> isize {
        ((self.mi_rows() * MI_SIZE + sub_y) >> sub_y) as isize - 1
    }

    /** The horizontal pass: for each row the vertical filter reads, the bilinear blend of each block column's two source samples, rounded by the first shift. Rows past the edge repeat the edge. */
    fn horizontal_pass(&self, plane: usize, geom: &Source) -> Vec<i32> {
        let (shift, _) = self.rounds();
        let (a, b) = bilinear_taps(geom.fx);
        let source = &self.fs.planes[plane];
        let rows = geom.h + FILTER_ROWS_EXTRA;
        let mut out = Vec::with_capacity(rows * geom.w);
        for r in 0..rows {
            let y = clamp_index(
                geom.y0 + r as isize - FILTER_ROWS_ABOVE as isize,
                geom.last_y,
            );
            for c in 0..geom.w {
                let x = geom.x0 + c as isize;
                let first = source.get(clamp_index(x, geom.last_x), y);
                let second = source.get(clamp_index(x + 1, geom.last_x), y);
                out.push(round2(a * first + b * second, shift));
            }
        }
        out
    }

    /** The vertical pass: the bilinear blend of each output sample's two intermediate rows, rounded by the second shift and clipped to the bit depth. */
    fn vertical_pass(&self, geom: &Source, inter: &[i32]) -> Vec<u16> {
        let (_, shift) = self.rounds();
        let (a, b) = bilinear_taps(geom.fy);
        let max = (1i32 << self.seq.color.bit_depth) - 1;
        let mut out = Vec::with_capacity(geom.w * geom.h);
        for r in 0..geom.h {
            for c in 0..geom.w {
                let upper = inter[(r + FILTER_TAP_SELF) * geom.w + c];
                let lower = inter[(r + FILTER_TAP_NEXT) * geom.w + c];
                out.push(round2(a * upper + b * lower, shift).clamp(0, max) as u16);
            }
        }
        out
    }

    /** The two rounding shifts of the filter at this bit depth: the rounding of the first pass and of the second. At 12 bits the first pass rounds two bits more and the second two bits less. */
    fn rounds(&self) -> (u32, u32) {
        if self.seq.color.bit_depth == BITDEPTH_HIGH as u8 {
            (
                INTER_ROUND0 + HIGH_BITDEPTH_ROUND,
                INTER_ROUND1 - HIGH_BITDEPTH_ROUND,
            )
        } else {
            (INTER_ROUND0, INTER_ROUND1)
        }
    }
}

/** The two bilinear taps for a fractional position: the weight of the sample and of the one after it, which sum to the filter's total. */
fn bilinear_taps(frac: usize) -> (i32, i32) {
    let next = TAP_STEP * frac as i32;
    (FILTER_TOTAL - next, next)
}

/** Round a signed value right by `shift` bits, halves away from the floor (spec Round2). */
fn round2(value: i32, shift: u32) -> i32 {
    (value + (1 << (shift - 1))) >> shift
}

/** A position clamped into [0, last]: a source read past the plane's edge repeats the edge sample. */
fn clamp_index(value: isize, last: isize) -> usize {
    value.clamp(0, last) as usize
}

/** Store a block's w-wide rows of samples into the plane, starting at `at`. */
fn write_block(plane: &mut Plane, at: (usize, usize), w: usize, samples: &[u16]) {
    for (r, row) in samples.chunks_exact(w).enumerate() {
        plane.row_mut(at.1 + r)[at.0..at.0 + w].copy_from_slice(row);
    }
}
