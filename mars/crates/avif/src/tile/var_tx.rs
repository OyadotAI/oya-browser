/** Variable transform size syntax of an inter block (spec read_block_tx_size's tree branch, read_var_tx_size and the txfm_split context of 8.3.2): the block's largest transform is split by coded flags into a tree of sizes, and each leaf is recorded for the 4x4 units it covers. Intra block copy blocks read their transform size this way too. */
use super::constants::{BLOCK_4X4, MAX_VARTX_DEPTH, MI_SIZE, TX_4X4, TX_SIZES};
use super::decoder::TileDecoder;
use super::tables::{MAX_TX_SIZE_RECT, SPLIT_TX_SIZE, TX_HEIGHT, TX_SIZE_SQR_UP, TX_WIDTH};
use crate::av1::TX_MODE_SELECT;
use crate::error::Result;

/** The largest block dimension, in samples, that a transform tree is sized against. */
const MAX_TREE_DIM: usize = 64;
/** log2 of the smallest square transform (4x4), so a square's log2 minus this is its size index. */
const SQUARE_TX_LOG2_BASE: u32 = 2;
/** Weight of the size-mismatch term in the txfm_split context, and of the depth term. */
const CTX_SIZE_MISMATCH: usize = 3;
const CTX_DEPTH_STEP: usize = 6;

impl<'a> TileDecoder<'a> {
    /** Whether the block reads a transform tree: an inter block of a select-mode frame, larger than 4x4, with coefficients, that is not lossless. */
    pub(super) fn uses_var_tx(&self) -> bool {
        self.hdr.tx_mode == TX_MODE_SELECT
            && self.blk.mi_size > BLOCK_4X4
            && self.blk.is_inter
            && !self.blk.skip
            && !self.blk.lossless
    }

    /** The trees of a block: its largest transform rectangle tiles the block, and each tile is read as one tree. */
    pub(super) fn read_var_tx_blocks(&mut self) -> Result<()> {
        let max_tx = MAX_TX_SIZE_RECT[self.blk.mi_size] as usize;
        let step_w4 = TX_WIDTH[max_tx] as usize / MI_SIZE;
        let step_h4 = TX_HEIGHT[max_tx] as usize / MI_SIZE;
        let (r0, c0) = (self.blk.mi_row, self.blk.mi_col);
        for row in (r0..r0 + self.blk.bh4).step_by(step_h4) {
            for col in (c0..c0 + self.blk.bw4).step_by(step_w4) {
                self.read_var_tx_size(row, col, max_tx, 0)?;
            }
        }
        Ok(())
    }

    /** read_var_tx_size(row, col, txSz, depth): one node of a tree. A node past the frame edge is skipped; a leaf records its size over the units it covers; a split node recurses into its children. */
    fn read_var_tx_size(&mut self, row: usize, col: usize, tx: usize, depth: usize) -> Result<()> {
        if row >= self.mi_rows() || col >= self.mi_cols() {
            return Ok(());
        }
        let split =
            tx != TX_4X4 && depth != MAX_VARTX_DEPTH && self.read_txfm_split(row, col, tx)?;
        if split {
            self.split_var_tx(row, col, tx, depth)
        } else {
            self.set_var_tx_leaf(row, col, tx);
            Ok(())
        }
    }

    /** The children of a split node: the next size down, tiled over the node in its own steps. */
    fn split_var_tx(&mut self, row: usize, col: usize, tx: usize, depth: usize) -> Result<()> {
        let sub = SPLIT_TX_SIZE[tx] as usize;
        let (w4, h4) = (
            TX_WIDTH[tx] as usize / MI_SIZE,
            TX_HEIGHT[tx] as usize / MI_SIZE,
        );
        let step_w4 = TX_WIDTH[sub] as usize / MI_SIZE;
        let step_h4 = TX_HEIGHT[sub] as usize / MI_SIZE;
        for i in (0..h4).step_by(step_h4) {
            for j in (0..w4).step_by(step_w4) {
                self.read_var_tx_size(row + i, col + j, sub, depth + 1)?;
            }
        }
        Ok(())
    }

    /** Record a leaf's size over the 4x4 units it covers, and make it the block's transform size. */
    fn set_var_tx_leaf(&mut self, row: usize, col: usize, tx: usize) {
        let (w4, h4) = (
            TX_WIDTH[tx] as usize / MI_SIZE,
            TX_HEIGHT[tx] as usize / MI_SIZE,
        );
        self.fs
            .mi
            .fill(row, col, h4, w4, |m| m.inter_tx_size = tx as u8);
        self.blk.tx_size = tx;
    }

    /** The txfm_split flag of a node, read with the context its neighbours give. */
    fn read_txfm_split(&mut self, row: usize, col: usize, tx: usize) -> Result<bool> {
        let ctx = self.txfm_split_ctx(row, col, tx);
        Ok(self.sym.read_symbol(&mut self.cdf.txfm_split[ctx])? == 1)
    }

    /** The txfm_split context: whether the neighbours above and left are narrower or shorter than the node, and how far the node's square is from the block's largest square. */
    fn txfm_split_ctx(&self, row: usize, col: usize, tx: usize) -> usize {
        let above = usize::from(self.above_tx_width(row, col) < TX_WIDTH[tx] as usize);
        let left = usize::from(self.left_tx_height(row, col) < TX_HEIGHT[tx] as usize);
        let max_sq = self.block_square_tx();
        let mismatch = usize::from(TX_SIZE_SQR_UP[tx] as usize != max_sq);
        mismatch * CTX_SIZE_MISMATCH + (TX_SIZES - 1 - max_sq) * CTX_DEPTH_STEP + above + left
    }

    /** The index of the square transform that matches the block: its larger dimension, capped at 64 samples. */
    fn block_square_tx(&self) -> usize {
        let size = MAX_TREE_DIM.min(self.blk.width().max(self.blk.height()));
        (size.trailing_zeros() - SQUARE_TX_LOG2_BASE) as usize
    }
}
