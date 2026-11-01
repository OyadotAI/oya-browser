/** Residual syntax of an intra block (spec residual 5.11.34, transform_block 5.11.35, get_tx_size 5.11.37 and get_plane_residual_size): the walk over the block's planes and 64x64 chunks, and over the transform blocks of each plane, with the block-decoded flags that intra prediction later reads. Prediction itself is not run here. */
use super::constants::{BLOCK_64X64, MAX_TX_DIM, MI_SIZE, TX_4X4, TX_16X32, TX_32X16, TX_32X32};
use super::decoder::TileDecoder;
use super::lr::plane_subsampling;
use super::samples::TxAt;
use super::tables::{
    MAX_TX_SIZE_RECT, NUM_4X4_BLOCKS_HIGH, NUM_4X4_BLOCKS_WIDE, SUBSAMPLED_SIZE, TX_HEIGHT,
    TX_WIDTH,
};
use crate::error::Result;

/** Width or height of a block in samples above which it is coded as 64-sample chunks, and the chunk size as a log2 of 4x4 units. */
const CHUNK_SAMPLES: usize = 64;
const CHUNK_MI_LOG2: usize = 4;
/** Shifts a 4x4 count by this to get the 4-sample step of the transform walk. */
const TX_UNIT_LOG2: usize = 2;

impl<'a> TileDecoder<'a> {
    /** residual(): every transform block of the block, plane by plane within each 64x64 chunk, in the order the spec reads them. */
    pub(super) fn residual(&mut self) -> Result<()> {
        let (bw, bh) = (self.blk.width(), self.blk.height());
        let width_chunks = (bw / CHUNK_SAMPLES).max(1);
        let height_chunks = (bh / CHUNK_SAMPLES).max(1);
        let chunk_size = if width_chunks > 1 || height_chunks > 1 {
            BLOCK_64X64
        } else {
            self.blk.mi_size
        };
        for chunk_y in 0..height_chunks {
            for chunk_x in 0..width_chunks {
                self.residual_chunk(chunk_size, chunk_x, chunk_y)?;
            }
        }
        Ok(())
    }

    /** One 64x64 chunk of the block: each plane's transform blocks that fall inside it. An inter block's luma is walked as its transform tree instead. */
    fn residual_chunk(&mut self, chunk_size: usize, chunk_x: usize, chunk_y: usize) -> Result<()> {
        let planes = if self.blk.has_chroma { 3 } else { 1 };
        for plane in 0..planes {
            if plane == 0 && self.blk.is_inter && !self.blk.lossless {
                self.luma_tree_chunk(chunk_size, chunk_x, chunk_y)?;
                continue;
            }
            let tx_size = if self.blk.lossless {
                TX_4X4
            } else {
                self.get_tx_size(plane, self.blk.tx_size)
            };
            let step_x = (TX_WIDTH[tx_size] as usize) >> TX_UNIT_LOG2;
            let step_y = (TX_HEIGHT[tx_size] as usize) >> TX_UNIT_LOG2;
            let (sub_x, sub_y) = plane_subsampling(self.seq, plane);
            let plane_size = self.plane_residual_size(chunk_size, plane);
            let num4x4_w = NUM_4X4_BLOCKS_WIDE[plane_size] as usize;
            let num4x4_h = NUM_4X4_BLOCKS_HIGH[plane_size] as usize;
            let base_x = (self.blk.mi_col >> sub_x) * MI_SIZE;
            let base_y = (self.blk.mi_row >> sub_y) * MI_SIZE;
            for y in (0..num4x4_h).step_by(step_y) {
                for x in (0..num4x4_w).step_by(step_x) {
                    let offset_x = x + ((chunk_x << CHUNK_MI_LOG2) >> sub_x);
                    let offset_y = y + ((chunk_y << CHUNK_MI_LOG2) >> sub_y);
                    self.transform_block(plane, base_x, base_y, tx_size, offset_x, offset_y)?;
                }
            }
        }
        Ok(())
    }

    /** The luma of an inter block's chunk, walked as the transform tree its variable transform sizes name. */
    fn luma_tree_chunk(&mut self, chunk_size: usize, chunk_x: usize, chunk_y: usize) -> Result<()> {
        let x = (self.blk.mi_col + (chunk_x << CHUNK_MI_LOG2)) * MI_SIZE;
        let y = (self.blk.mi_row + (chunk_y << CHUNK_MI_LOG2)) * MI_SIZE;
        let w = NUM_4X4_BLOCKS_WIDE[chunk_size] as usize * MI_SIZE;
        let h = NUM_4X4_BLOCKS_HIGH[chunk_size] as usize * MI_SIZE;
        self.transform_tree(x, y, w, h)
    }

    /** transform_block(plane, baseX, baseY, txSz, x, y): one transform block at a 4x4 offset from the base. Blocks that start past the frame edge are skipped. An intra block is predicted first; then the block's residual is read and added, and its edges are recorded for the blocks after it. */
    pub(super) fn transform_block(
        &mut self,
        plane: usize,
        base_x: usize,
        base_y: usize,
        tx_size: usize,
        x: usize,
        y: usize,
    ) -> Result<()> {
        let start_x = base_x + MI_SIZE * x;
        let start_y = base_y + MI_SIZE * y;
        let (sub_x, sub_y) = plane_subsampling(self.seq, plane);
        let max_x = (self.mi_cols() * MI_SIZE) >> sub_x;
        let max_y = (self.mi_rows() * MI_SIZE) >> sub_y;
        if start_x >= max_x || start_y >= max_y {
            return Ok(());
        }
        let at = TxAt {
            plane,
            start_x,
            start_y,
            x,
            y,
            tx_size,
        };
        if !self.blk.is_inter {
            self.predict_intra_tx(at);
            if plane == 0 {
                self.blk.max_luma_w = start_x + (TX_WIDTH[tx_size] as usize);
                self.blk.max_luma_h = start_y + (TX_HEIGHT[tx_size] as usize);
            }
        }
        let coded = if self.blk.skip {
            None
        } else {
            self.coeffs(plane, start_x, start_y, tx_size)?
        };
        if let Some(coded) = coded {
            self.reconstruct_tx(at, &coded);
        }
        self.record_transform_block(at, sub_x, sub_y);
        Ok(())
    }

    /** The edges a transform block leaves for later blocks: its 4x4 units are marked decoded for intra prediction, and its size is recorded for the loop filter. */
    fn record_transform_block(&mut self, at: TxAt, sub_x: usize, sub_y: usize) {
        let row = (at.start_y << sub_y) >> TX_UNIT_LOG2;
        let col = (at.start_x << sub_x) >> TX_UNIT_LOG2;
        let sb_mask = self.sb_mi() - 1;
        let sub_row = (row & sb_mask) >> sub_y;
        let sub_col = (col & sb_mask) >> sub_x;
        let step_x = (TX_WIDTH[at.tx_size] as usize) >> TX_UNIT_LOG2;
        let step_y = (TX_HEIGHT[at.tx_size] as usize) >> TX_UNIT_LOG2;
        for i in 0..step_y {
            for j in 0..step_x {
                self.decoded.set(
                    at.plane,
                    (sub_row + i) as isize,
                    (sub_col + j) as isize,
                    true,
                );
            }
        }
        self.fs.lf_tx[at.plane].fill(row >> sub_y, col >> sub_x, step_y, step_x, at.tx_size as u8);
    }

    /** get_tx_size(plane, txSz): luma keeps its transform; chroma uses the largest rectangle for its residual block, capped at 32 samples. */
    pub(super) fn get_tx_size(&self, plane: usize, tx_size: usize) -> usize {
        if plane == 0 {
            return tx_size;
        }
        let uv_tx = MAX_TX_SIZE_RECT[self.plane_residual_size(self.blk.mi_size, plane)] as usize;
        if TX_WIDTH[uv_tx] as usize == MAX_TX_DIM || TX_HEIGHT[uv_tx] as usize == MAX_TX_DIM {
            if TX_WIDTH[uv_tx] == 16 {
                return TX_16X32;
            }
            if TX_HEIGHT[uv_tx] == 16 {
                return TX_32X16;
            }
            return TX_32X32;
        }
        uv_tx
    }

    /** get_plane_residual_size(subsize, plane): the block size a plane's residual covers, after the plane's subsampling. */
    pub(super) fn plane_residual_size(&self, size: usize, plane: usize) -> usize {
        let (sub_x, sub_y) = plane_subsampling(self.seq, plane);
        SUBSAMPLED_SIZE[size][sub_x][sub_y] as usize
    }
}
