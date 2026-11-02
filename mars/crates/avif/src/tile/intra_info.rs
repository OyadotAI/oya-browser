/** Intra prediction modes of a block (spec intra_frame_mode_info after the skip and delta reads): the luma and chroma modes with their contexts, the angle deltas, the chroma-from-luma alphas, and the filter-intra mode. */
use super::constants::{
    BLOCK_4X4, BLOCK_8X8, CFL_SIGN_COUNT, CFL_SIGN_NEG, CFL_SIGN_ZERO, D67_PRED, DC_PRED,
    MAX_ANGLE_DELTA, UV_CFL_PRED, V_PRED,
};
use super::decoder::TileDecoder;
use super::lr::plane_subsampling;
use super::tables::{INTRA_MODE_CONTEXT, SUBSAMPLED_SIZE};
use crate::error::Result;

/** The largest block dimension, in samples, for which chroma-from-luma and filter intra are allowed in a lossy block. */
const CFL_MAX_DIM: usize = 32;

/** Whether an intra mode has a direction, so that it carries an angle delta. */
pub(crate) fn is_directional(mode: u8) -> bool {
    (V_PRED..=D67_PRED).contains(&mode)
}

impl<'a> TileDecoder<'a> {
    /** The prediction modes of an intra block: luma, then chroma when the block has it, then the palette and filter-intra choices that depend on the luma mode. */
    pub(super) fn intra_modes(&mut self) -> Result<()> {
        self.intra_y_mode()?;
        if self.blk.has_chroma {
            self.uv_mode()?;
        }
        self.blk.palette_size_y = 0;
        self.blk.palette_size_uv = 0;
        if self.hdr.allow_screen_content_tools && self.palette_allowed() {
            self.palette_mode_info()?;
        }
        self.filter_intra_mode_info()
    }

    /** intra_frame_y_mode with its above and left context, then the luma angle delta. */
    fn intra_y_mode(&mut self) -> Result<()> {
        let (r, c) = (self.blk.mi_row, self.blk.mi_col);
        let above = if self.blk.avail_u {
            self.fs.mi.at(r - 1, c).y_mode
        } else {
            DC_PRED
        };
        let left = if self.blk.avail_l {
            self.fs.mi.at(r, c - 1).y_mode
        } else {
            DC_PRED
        };
        let above_ctx = INTRA_MODE_CONTEXT[usize::from(above)] as usize;
        let left_ctx = INTRA_MODE_CONTEXT[usize::from(left)] as usize;
        let mode = self
            .sym
            .read_symbol(&mut self.cdf.intra_frame_y_mode[above_ctx][left_ctx])?;
        self.blk.y_mode = mode as u8;
        self.blk.angle_delta_y = self.read_angle_delta(self.blk.y_mode)?;
        Ok(())
    }

    /** The chroma mode, chosen from the chroma-from-luma-allowed table or the other one, then the chroma alphas and angle delta. */
    fn uv_mode(&mut self) -> Result<()> {
        let mode = if self.cfl_allowed() {
            self.sym
                .read_symbol(&mut self.cdf.uv_mode_cfl_allowed[usize::from(self.blk.y_mode)])?
        } else {
            self.sym
                .read_symbol(&mut self.cdf.uv_mode_cfl_not_allowed[usize::from(self.blk.y_mode)])?
        };
        self.blk.uv_mode = mode as u8;
        if self.blk.uv_mode == UV_CFL_PRED {
            self.read_cfl_alphas()?;
        }
        self.blk.angle_delta_uv = self.read_angle_delta(self.blk.uv_mode)?;
        Ok(())
    }

    /** Whether chroma-from-luma may be coded for this block: a lossless block only at 4x4 chroma, a lossy one when it is at most 32 samples in each direction. */
    fn cfl_allowed(&self) -> bool {
        let (sub_x, sub_y) = plane_subsampling(self.seq, 1);
        let chroma = SUBSAMPLED_SIZE[self.blk.mi_size][sub_x][sub_y] as usize;
        if self.blk.lossless {
            return chroma == BLOCK_4X4;
        }
        self.blk.width().max(self.blk.height()) <= CFL_MAX_DIM
    }

    /** The angle delta of a directional mode, read for blocks of at least 8x8; other blocks and modes have none. */
    fn read_angle_delta(&mut self, mode: u8) -> Result<i32> {
        if self.blk.mi_size < BLOCK_8X8 || !is_directional(mode) {
            return Ok(0);
        }
        let ctx = usize::from(mode - V_PRED);
        let symbol = self.sym.read_symbol(&mut self.cdf.angle_delta[ctx])?;
        Ok(symbol as i32 - MAX_ANGLE_DELTA)
    }

    /** read_cfl_alphas(): the joint sign of the two chroma alphas, then each non-zero alpha's magnitude. */
    fn read_cfl_alphas(&mut self) -> Result<()> {
        let signs = self.sym.read_symbol(&mut self.cdf.cfl_sign)?;
        let sign_u = (signs + 1) / CFL_SIGN_COUNT;
        let sign_v = (signs + 1) % CFL_SIGN_COUNT;
        self.blk.cfl_alpha_u =
            self.read_cfl_alpha(sign_u, sign_u.saturating_sub(1) * CFL_SIGN_COUNT + sign_v)?;
        self.blk.cfl_alpha_v =
            self.read_cfl_alpha(sign_v, sign_v.saturating_sub(1) * CFL_SIGN_COUNT + sign_u)?;
        Ok(())
    }

    /** One chroma alpha: zero when its sign is zero, otherwise one plus a coded magnitude, negated for a negative sign. */
    fn read_cfl_alpha(&mut self, sign: usize, ctx: usize) -> Result<i32> {
        if sign == CFL_SIGN_ZERO {
            return Ok(0);
        }
        let magnitude = self.sym.read_symbol(&mut self.cdf.cfl_alpha[ctx])? as i32 + 1;
        Ok(if sign == CFL_SIGN_NEG {
            -magnitude
        } else {
            magnitude
        })
    }

    /** filter_intra_mode_info(): a DC block of at most 32 samples may use one of five recursive filters instead. */
    fn filter_intra_mode_info(&mut self) -> Result<()> {
        self.blk.use_filter_intra = false;
        let size = self.blk.width().max(self.blk.height());
        if self.seq.enable_filter_intra
            && self.blk.y_mode == DC_PRED
            && self.blk.palette_size_y == 0
            && size <= CFL_MAX_DIM
        {
            self.blk.use_filter_intra = self
                .sym
                .read_symbol(&mut self.cdf.filter_intra[self.blk.mi_size])?
                == 1;
            if self.blk.use_filter_intra {
                self.blk.filter_intra_mode =
                    self.sym.read_symbol(&mut self.cdf.filter_intra_mode)? as u8;
            }
        }
        Ok(())
    }
}
