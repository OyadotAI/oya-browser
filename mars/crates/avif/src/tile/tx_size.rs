/** Transform size syntax for intra blocks (spec read_block_tx_size and read_tx_size, with the tx_depth and neighbour contexts of 8.3.2): the largest transform a block may use, split by a coded depth when the frame selects per block, and recorded for the 4x4 units the block covers. */
use super::constants::{BLOCK_4X4, MAX_TX_DEPTH, MI_SIZE, TX_4X4};
use super::decoder::TileDecoder;
use super::tables::{
    MAX_TX_DEPTH as MAX_TX_DEPTH_BY_SIZE, MAX_TX_SIZE_RECT, NUM_4X4_BLOCKS_HIGH,
    NUM_4X4_BLOCKS_WIDE, SPLIT_TX_SIZE, TX_HEIGHT, TX_WIDTH,
};
use crate::av1::TX_MODE_SELECT;
use crate::error::Result;

/** The largest transform dimension, which is what an unavailable neighbour contributes to the context. */
const MAX_TX_DIM: usize = 64;
/** The maximum tx_depth of each CDF family, by the largest depth a block size can code. */
const DEPTH_FAMILY_64: usize = 4;
const DEPTH_FAMILY_32: usize = 3;
const DEPTH_FAMILY_16: usize = 2;

impl<'a> TileDecoder<'a> {
    /** read_block_tx_size() for an intra block: read its transform size, then record that size for every 4x4 unit the block covers. */
    pub(super) fn read_block_tx_size(&mut self) -> Result<()> {
        self.read_tx_size()?;
        let size = self.blk.tx_size as u8;
        let (r, c, h, w) = (self.blk.mi_row, self.blk.mi_col, self.blk.bh4, self.blk.bw4);
        self.fs.mi.fill(r, c, h, w, |m| m.inter_tx_size = size);
        Ok(())
    }

    /** read_tx_size(allowSelect = 1): a lossless block uses 4x4; otherwise the largest rectangle for the block, split tx_depth times when the frame selects per block. */
    fn read_tx_size(&mut self) -> Result<()> {
        if self.blk.lossless {
            self.blk.tx_size = TX_4X4;
            return Ok(());
        }
        let max_rect = MAX_TX_SIZE_RECT[self.blk.mi_size] as usize;
        self.blk.tx_size = max_rect;
        if self.blk.mi_size > BLOCK_4X4 && self.hdr.tx_mode == TX_MODE_SELECT {
            let max_depth = MAX_TX_DEPTH_BY_SIZE[self.blk.mi_size] as usize;
            let depth = self.read_tx_depth(max_rect, max_depth)?;
            for _ in 0..depth {
                self.blk.tx_size = SPLIT_TX_SIZE[self.blk.tx_size] as usize;
            }
        }
        Ok(())
    }

    /** tx_depth: the number of splits, read with the CDF of the block's depth family and the neighbour context. */
    fn read_tx_depth(&mut self, max_rect: usize, max_depth: usize) -> Result<usize> {
        let ctx = self.tx_depth_ctx(max_rect);
        let symbol = match max_depth {
            DEPTH_FAMILY_64 => self.sym.read_symbol(&mut self.cdf.tx_64x64[ctx])?,
            DEPTH_FAMILY_32 => self.sym.read_symbol(&mut self.cdf.tx_32x32[ctx])?,
            DEPTH_FAMILY_16 => self.sym.read_symbol(&mut self.cdf.tx_16x16[ctx])?,
            _ => self.sym.read_symbol(&mut self.cdf.tx_8x8[ctx])?,
        };
        Ok(symbol.min(MAX_TX_DEPTH))
    }

    /** The tx_depth context: one point for each neighbour whose transform (or block, for an inter neighbour) is at least as large as the block's largest transform in that direction. */
    fn tx_depth_ctx(&self, max_rect: usize) -> usize {
        let (r, c) = (self.blk.mi_row, self.blk.mi_col);
        let above_w = if !self.blk.avail_u {
            0
        } else if self.fs.mi.at(r - 1, c).is_inter {
            self.block_width_at(r - 1, c)
        } else {
            self.above_tx_width(r, c)
        };
        let left_h = if !self.blk.avail_l {
            0
        } else if self.fs.mi.at(r, c - 1).is_inter {
            self.block_height_at(r, c - 1)
        } else {
            self.left_tx_height(r, c)
        };
        usize::from(above_w >= TX_WIDTH[max_rect] as usize)
            + usize::from(left_h >= TX_HEIGHT[max_rect] as usize)
    }

    /** get_above_tx_width(row, col): the width of the transform above a 4x4 unit. On the block's first row an unavailable row counts as the largest size, and a skipped inter block counts as its whole width. */
    pub(super) fn above_tx_width(&self, row: usize, col: usize) -> usize {
        if row == self.blk.mi_row {
            if !self.blk.avail_u {
                return MAX_TX_DIM;
            }
            let above = self.fs.mi.at(row - 1, col);
            if above.skip && above.is_inter {
                return self.block_width_at(row - 1, col);
            }
        }
        TX_WIDTH[self.fs.mi.at(row - 1, col).inter_tx_size as usize] as usize
    }

    /** get_left_tx_height(row, col): the height of the transform to the left of a 4x4 unit, with the same edge rules as the width. */
    pub(super) fn left_tx_height(&self, row: usize, col: usize) -> usize {
        if col == self.blk.mi_col {
            if !self.blk.avail_l {
                return MAX_TX_DIM;
            }
            let left = self.fs.mi.at(row, col - 1);
            if left.skip && left.is_inter {
                return self.block_height_at(row, col - 1);
            }
        }
        TX_HEIGHT[self.fs.mi.at(row, col - 1).inter_tx_size as usize] as usize
    }

    /** Width in samples of the block whose top-left unit is at (r, c). */
    fn block_width_at(&self, r: usize, c: usize) -> usize {
        NUM_4X4_BLOCKS_WIDE[self.fs.mi.at(r, c).mi_size as usize] as usize * MI_SIZE
    }

    /** Height in samples of the block whose top-left unit is at (r, c). */
    fn block_height_at(&self, r: usize, c: usize) -> usize {
        NUM_4X4_BLOCKS_HIGH[self.fs.mi.at(r, c).mi_size as usize] as usize * MI_SIZE
    }
}
