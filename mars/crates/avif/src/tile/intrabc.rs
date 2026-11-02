/** Intra block copy mode info (spec intra_frame_mode_info's use_intrabc branch and assign_mv for that case): the block is recorded as an inter block that copies from the frame it is decoding, its vector is its predicted vector plus the coded difference, and a vector that would read samples not yet decoded is refused. */
use super::constants::{DC_PRED, INTRABC_DELAY_PIXELS, MI_SIZE, MV_UNIT};
use super::decoder::TileDecoder;
use crate::error::{Error, Result};

impl<'a> TileDecoder<'a> {
    /** The mode info of an intra block copy block: its modes are fixed, its stack is searched, and its vector is read and checked. Nothing else of an intra block is read. */
    pub(super) fn intrabc_mode_info(&mut self) -> Result<()> {
        self.blk.is_inter = true;
        self.blk.use_intrabc = true;
        self.blk.y_mode = DC_PRED;
        self.blk.uv_mode = DC_PRED;
        self.blk.angle_delta_y = 0;
        self.blk.angle_delta_uv = 0;
        self.blk.use_filter_intra = false;
        self.blk.palette_size_y = 0;
        self.blk.palette_size_uv = 0;
        let stack = self.intrabc_mv_stack();
        let pred = self.intrabc_pred_mv(stack.mv_or_zero(0), stack.mv_or_zero(1));
        let diff = self.read_intrabc_mv_diff()?;
        let mv = [pred[0] + diff[0], pred[1] + diff[1]];
        if !self.intrabc_mv_is_valid(mv) {
            return Err(Error::Malformed("intra block copy vector"));
        }
        self.blk.mv = mv;
        Ok(())
    }

    /** The predicted vector of an intra block copy block: the first stack candidate that is not zero, then the second, and failing both a default that points one superblock row up, or one superblock to the left, from the block. */
    fn intrabc_pred_mv(&self, first: [i32; 2], second: [i32; 2]) -> [i32; 2] {
        if first != [0, 0] {
            first
        } else if second != [0, 0] {
            second
        } else {
            self.intrabc_default_mv()
        }
    }

    /** The default vector: one superblock up when the tile has a superblock row above the block, shifted past the delay the decoder keeps; otherwise one superblock to the left. */
    fn intrabc_default_mv(&self) -> [i32; 2] {
        let sb = self.sb_mi() as i32;
        let sb_samples = sb * MI_SIZE as i32;
        if self.blk.mi_row as isize - (sb as isize) < self.tile.mi_row_start as isize {
            [0, -(sb_samples + INTRABC_DELAY_PIXELS) * MV_UNIT]
        } else {
            [-sb_samples * MV_UNIT, 0]
        }
    }
}
