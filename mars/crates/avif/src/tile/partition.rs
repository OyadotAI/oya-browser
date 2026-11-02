/** Partition syntax (spec decode_partition, 5.11.4): the partition symbol and its context, the split-or-horizontal and split-or-vertical flags at the frame edge, and the order in which each partition shape visits its blocks. */
use super::constants::{
    BLOCK_8X8, BLOCK_128X128, PARTITION_HORZ, PARTITION_HORZ_4, PARTITION_HORZ_A, PARTITION_HORZ_B,
    PARTITION_NONE, PARTITION_SPLIT, PARTITION_VERT, PARTITION_VERT_4, PARTITION_VERT_A,
    PARTITION_VERT_B,
};
use super::decoder::TileDecoder;
use super::tables::{MI_HEIGHT_LOG2, MI_WIDTH_LOG2, NUM_4X4_BLOCKS_WIDE, PARTITION_SUBSIZE};
use crate::context::TileCdfs;
use crate::error::Result;

/** The partition CDF for a block-size class (1 for 8x8 up to 5 for 128x128) and context, read-only. */
fn partition_cdf(cdf: &TileCdfs, bsl: usize, ctx: usize) -> &[u16] {
    match bsl {
        1 => &cdf.partition_w8[ctx][..],
        2 => &cdf.partition_w16[ctx][..],
        3 => &cdf.partition_w32[ctx][..],
        4 => &cdf.partition_w64[ctx][..],
        _ => &cdf.partition_w128[ctx][..],
    }
}

/** The partition CDF for a block-size class and context, for adaptation. */
fn partition_cdf_mut(cdf: &mut TileCdfs, bsl: usize, ctx: usize) -> &mut [u16] {
    match bsl {
        1 => &mut cdf.partition_w8[ctx][..],
        2 => &mut cdf.partition_w16[ctx][..],
        3 => &mut cdf.partition_w32[ctx][..],
        4 => &mut cdf.partition_w64[ctx][..],
        _ => &mut cdf.partition_w128[ctx][..],
    }
}

/** Probability mass of one symbol, read from a cumulative CDF: its value minus the value before it. */
fn mass(cdf: &[u16], symbol: usize) -> i32 {
    i32::from(cdf[symbol]) - i32::from(cdf[symbol - 1])
}

/** One step of a partition: a block to read, or a quarter to partition again. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Block(usize, usize, usize),
    Split(usize, usize, usize),
}

/** The geometry a partition's steps are built from. */
#[derive(Clone, Copy, Debug)]
struct Shape {
    r: usize,
    c: usize,
    half: usize,
    quarter: usize,
    sub: usize,
    split: usize,
    has_rows: bool,
    has_cols: bool,
    mi_rows: usize,
    mi_cols: usize,
}

impl<'a> TileDecoder<'a> {
    /** decode_partition(r, c, bSize): read the partition of a square block, then its blocks or quarters. Positions outside the frame are skipped. */
    pub(super) fn decode_partition(&mut self, r: usize, c: usize, bsize: usize) -> Result<()> {
        if r >= self.mi_rows() || c >= self.mi_cols() {
            return Ok(());
        }
        let half = NUM_4X4_BLOCKS_WIDE[bsize] as usize >> 1;
        let has_rows = r + half < self.mi_rows();
        let has_cols = c + half < self.mi_cols();
        let partition = self.read_partition(r, c, bsize, has_rows, has_cols)?;
        let shape = Shape {
            r,
            c,
            half,
            quarter: half >> 1,
            sub: PARTITION_SUBSIZE[partition][bsize] as usize,
            split: PARTITION_SUBSIZE[PARTITION_SPLIT][bsize] as usize,
            has_rows,
            has_cols,
            mi_rows: self.mi_rows(),
            mi_cols: self.mi_cols(),
        };
        for step in partition_steps(partition, &shape) {
            match step {
                Step::Block(r, c, size) => self.decode_block(r, c, size)?,
                Step::Split(r, c, size) => self.decode_partition(r, c, size)?,
            }
        }
        Ok(())
    }

    /** The partition type of a block: read in full when the block fits the frame, otherwise implied or read as a one-bit split decision. */
    fn read_partition(
        &mut self,
        r: usize,
        c: usize,
        bsize: usize,
        has_rows: bool,
        has_cols: bool,
    ) -> Result<usize> {
        if bsize < BLOCK_8X8 {
            return Ok(PARTITION_NONE);
        }
        let ctx = self.partition_ctx(r, c, bsize);
        let bsl = MI_WIDTH_LOG2[bsize] as usize;
        if has_rows && has_cols {
            let cdf = partition_cdf_mut(&mut self.cdf, bsl, ctx);
            return self.sym.read_symbol(cdf);
        }
        if has_cols {
            let psum = self.split_psum(
                bsl,
                ctx,
                bsize,
                &[
                    PARTITION_VERT,
                    PARTITION_SPLIT,
                    PARTITION_HORZ_A,
                    PARTITION_VERT_A,
                    PARTITION_VERT_B,
                ],
                PARTITION_VERT_4,
            );
            return Ok(if self.read_split_flag(psum)? {
                PARTITION_SPLIT
            } else {
                PARTITION_HORZ
            });
        }
        if has_rows {
            let psum = self.split_psum(
                bsl,
                ctx,
                bsize,
                &[
                    PARTITION_HORZ,
                    PARTITION_SPLIT,
                    PARTITION_HORZ_A,
                    PARTITION_HORZ_B,
                    PARTITION_VERT_A,
                ],
                PARTITION_HORZ_4,
            );
            return Ok(if self.read_split_flag(psum)? {
                PARTITION_SPLIT
            } else {
                PARTITION_VERT
            });
        }
        Ok(PARTITION_SPLIT)
    }

    /** The partition context: whether the blocks above and to the left are smaller than this one in the relevant dimension. */
    fn partition_ctx(&self, r: usize, c: usize, bsize: usize) -> usize {
        let bsl = MI_WIDTH_LOG2[bsize];
        let (ri, ci) = (r as isize, c as isize);
        let above = self.is_inside(ri - 1, ci)
            && MI_WIDTH_LOG2[self.fs.mi.at(r - 1, c).mi_size as usize] < bsl;
        let left = self.is_inside(ri, ci - 1)
            && MI_HEIGHT_LOG2[self.fs.mi.at(r, c - 1).mi_size as usize] < bsl;
        usize::from(left) * 2 + usize::from(above)
    }

    /** The probability that a frame-edge block splits: the summed mass of the partitions that would put a block across the edge, with the four-way split added except for 128x128. */
    fn split_psum(
        &self,
        bsl: usize,
        ctx: usize,
        bsize: usize,
        types: &[usize],
        four_way: usize,
    ) -> i32 {
        let cdf = partition_cdf(&self.cdf, bsl, ctx);
        let mut psum: i32 = types.iter().map(|&t| mass(cdf, t)).sum();
        if bsize != BLOCK_128X128 {
            psum += mass(cdf, four_way);
        }
        psum
    }

    /** Read a split decision with the probability mass psum for "split". */
    fn read_split_flag(&mut self, psum: i32) -> Result<bool> {
        let one = 1u16 << 15;
        let mut cdf = [(i32::from(one) - psum) as u16, one, 0];
        Ok(self.sym.read_symbol(&mut cdf)? == 1)
    }
}

/** The steps of each partition shape, in the order the spec decodes them. Blocks past the frame edge are left out, as the spec's has_rows and has_cols tests require. */
fn partition_steps(partition: usize, s: &Shape) -> Vec<Step> {
    let (r, c, h, q) = (s.r, s.c, s.half, s.quarter);
    let (sub, split) = (s.sub, s.split);
    let b = Step::Block;
    match partition {
        PARTITION_NONE => vec![b(r, c, sub)],
        PARTITION_HORZ => [Some(b(r, c, sub)), s.has_rows.then(|| b(r + h, c, sub))]
            .into_iter()
            .flatten()
            .collect(),
        PARTITION_VERT => [Some(b(r, c, sub)), s.has_cols.then(|| b(r, c + h, sub))]
            .into_iter()
            .flatten()
            .collect(),
        PARTITION_SPLIT => vec![
            Step::Split(r, c, sub),
            Step::Split(r, c + h, sub),
            Step::Split(r + h, c, sub),
            Step::Split(r + h, c + h, sub),
        ],
        PARTITION_HORZ_A => vec![b(r, c, split), b(r, c + h, split), b(r + h, c, sub)],
        PARTITION_HORZ_B => vec![b(r, c, sub), b(r + h, c, split), b(r + h, c + h, split)],
        PARTITION_VERT_A => vec![b(r, c, split), b(r + h, c, split), b(r, c + h, sub)],
        PARTITION_VERT_B => vec![b(r, c, sub), b(r, c + h, split), b(r + h, c + h, split)],
        PARTITION_HORZ_4 => {
            let mut steps = vec![b(r, c, sub), b(r + q, c, sub), b(r + 2 * q, c, sub)];
            if r + 3 * q < s.mi_rows {
                steps.push(b(r + 3 * q, c, sub));
            }
            steps
        }
        // PARTITION_VERT_4 is the only shape left.
        _ => {
            let mut steps = vec![b(r, c, sub), b(r, c + q, sub), b(r, c + 2 * q, sub)];
            if c + 3 * q < s.mi_cols {
                steps.push(b(r, c + 3 * q, sub));
            }
            steps
        }
    }
}
