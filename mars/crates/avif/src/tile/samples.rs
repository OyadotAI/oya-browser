/** Sample reconstruction inside the tile walk (spec transform_block's predict calls and 7.12.3): for one transform block, the intra prediction its mode names (palette, DC-based chroma-from-luma, or a directional, smooth, Paeth or filter-intra predictor), then the residual read for it, added to the prediction. */
use super::block::Block;
use super::constants::{COLOR_MAP_STRIDE, DC_PRED, MI_SIZE, UV_CFL_PRED};
use super::decoder::TileDecoder;
use super::lr::plane_subsampling;
use super::tables::{TX_HEIGHT, TX_HEIGHT_LOG2, TX_WIDTH, TX_WIDTH_LOG2};
use crate::recon::intra::{self, CflBlock, PaletteBlock, Request};
use crate::recon::inverse::{Geometry, row_shift};
use crate::recon::{ResidualSpec, reconstruct};

/** The dequantised values of a transform block that has coefficients, with the transform type that gives them their shape. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Coded {
    pub(super) tx_type: u8,
    pub(super) coeffs: Vec<(u16, i32)>,
}

/** One transform block's place: its plane, its top-left sample in that plane, its offset inside the block in 4x4 units (which says whether a neighbour inside the block is decoded), and its transform size. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TxAt {
    pub(super) plane: usize,
    pub(super) start_x: usize,
    pub(super) start_y: usize,
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) tx_size: usize,
}

impl<'a> TileDecoder<'a> {
    /** The intra prediction of one transform block: the palette when the plane has one, otherwise the mode's predictor and, for chroma-from-luma, the luma correction on top of the DC prediction. */
    pub(super) fn predict_intra_tx(&mut self, at: TxAt) {
        if self.plane_has_palette(at.plane) {
            self.predict_palette_tx(at);
        } else {
            self.predict_mode_tx(at);
            if self.is_cfl(at.plane) {
                self.predict_cfl_tx(at);
            }
        }
    }

    /** Whether the plane of the block carries a palette. */
    fn plane_has_palette(&self, plane: usize) -> bool {
        if plane == 0 {
            self.blk.palette_size_y > 0
        } else {
            self.blk.palette_size_uv > 0
        }
    }

    /** Whether a chroma plane predicts from luma: its mode is the chroma-from-luma mode. */
    fn is_cfl(&self, plane: usize) -> bool {
        plane > 0 && self.blk.uv_mode == UV_CFL_PRED
    }

    /** Run the mode's predictor into the plane, with the edge availability the transform block sees. */
    fn predict_mode_tx(&mut self, at: TxAt) {
        let req = self.intra_request(at);
        intra::predict(&mut self.fs.planes[at.plane], &req);
    }

    /** The request for a transform block's intra prediction: its neighbours' availability (from the block and the decoded flags of the superblock), its mode and size, and the tool settings of the mode. */
    fn intra_request(&self, at: TxAt) -> Request {
        let (sub_x, sub_y) = plane_subsampling(self.seq, at.plane);
        let blk = &self.blk;
        let row = (at.start_y << sub_y) >> 2;
        let col = (at.start_x << sub_x) >> 2;
        let sb_mask = self.sb_mi() - 1;
        let sub_row = ((row & sb_mask) >> sub_y) as isize;
        let sub_col = ((col & sb_mask) >> sub_x) as isize;
        let step_x = (TX_WIDTH[at.tx_size] as usize >> 2) as isize;
        let step_y = (TX_HEIGHT[at.tx_size] as usize >> 2) as isize;
        let (avail_l, avail_u) = if at.plane == 0 {
            (blk.avail_l, blk.avail_u)
        } else {
            (blk.avail_l_chroma, blk.avail_u_chroma)
        };
        let mode = match at.plane {
            0 => blk.y_mode,
            _ if self.is_cfl(at.plane) => DC_PRED,
            _ => blk.uv_mode,
        };
        Request {
            x: at.start_x,
            y: at.start_y,
            have_left: avail_l || at.x > 0,
            have_above: avail_u || at.y > 0,
            have_above_right: self.decoded.get(at.plane, sub_row - 1, sub_col + step_x),
            have_below_left: self.decoded.get(at.plane, sub_row + step_y, sub_col - 1),
            mode,
            log2w: u32::from(TX_WIDTH_LOG2[at.tx_size]),
            log2h: u32::from(TX_HEIGHT_LOG2[at.tx_size]),
            angle_delta: if at.plane == 0 {
                blk.angle_delta_y
            } else {
                blk.angle_delta_uv
            },
            filter_intra: (at.plane == 0 && blk.use_filter_intra).then_some(blk.filter_intra_mode),
            edge_filter: self.seq.enable_intra_edge_filter,
            smooth_neighbour: self.filter_type(at.plane),
            max_x: self.plane_last_column(at.plane),
            max_y: self.plane_last_row(at.plane),
            bit_depth: u32::from(self.seq.color.bit_depth),
        }
    }

    /** The last sample column of a plane inside the frame's mode-info area. */
    fn plane_last_column(&self, plane: usize) -> usize {
        let (sub_x, _) = plane_subsampling(self.seq, plane);
        ((self.mi_cols() * MI_SIZE) >> sub_x) - 1
    }

    /** The last sample row of a plane inside the frame's mode-info area. */
    fn plane_last_row(&self, plane: usize) -> usize {
        let (_, sub_y) = plane_subsampling(self.seq, plane);
        ((self.mi_rows() * MI_SIZE) >> sub_y) - 1
    }

    /** get_filter_type(plane): whether the block above or to the left is smooth predicted, which selects the stronger edge filter. For chroma the neighbour is found at the position the chroma block actually borders. */
    fn filter_type(&self, plane: usize) -> bool {
        let (sub_x, sub_y) = plane_subsampling(self.seq, plane);
        let blk: &Block = &self.blk;
        let (r0, c0) = (blk.mi_row, blk.mi_col);
        let (avail_u, avail_l) = if plane == 0 {
            (blk.avail_u, blk.avail_l)
        } else {
            (blk.avail_u_chroma, blk.avail_l_chroma)
        };
        let above = avail_u && {
            let (mut r, mut c) = (r0 - 1, c0);
            if plane > 0 {
                if sub_x == 1 && c0 & 1 == 0 {
                    c += 1;
                }
                if sub_y == 1 && r0 & 1 == 1 {
                    r -= 1;
                }
            }
            self.is_smooth(r, c, plane)
        };
        let left = avail_l && {
            let (mut r, mut c) = (r0, c0 - 1);
            if plane > 0 {
                if sub_x == 1 && c0 & 1 == 1 {
                    c -= 1;
                }
                if sub_y == 1 && r0 & 1 == 0 {
                    r += 1;
                }
            }
            self.is_smooth(r, c, plane)
        };
        above || left
    }

    /** is_smooth(row, col, plane): whether the mode-info unit at a position predicts with one of the three smooth modes. */
    fn is_smooth(&self, row: usize, col: usize, plane: usize) -> bool {
        let info = self.fs.mi.at(row, col);
        let mode = if plane == 0 {
            info.y_mode
        } else {
            info.uv_mode
        };
        matches!(mode, 9..=11)
    }

    /** The palette prediction of a transform block: each sample is the palette colour its map entry names. */
    fn predict_palette_tx(&mut self, at: TxAt) {
        let blk = &self.blk;
        let (map, colors) = if at.plane == 0 {
            (&blk.color_map_y, blk.palette_colors[0])
        } else {
            (&blk.color_map_uv, blk.palette_colors[at.plane])
        };
        let block = PaletteBlock {
            colors: &colors,
            map,
            stride: COLOR_MAP_STRIDE,
            x4: at.x,
            y4: at.y,
        };
        let (w, h) = (
            TX_WIDTH[at.tx_size] as usize,
            TX_HEIGHT[at.tx_size] as usize,
        );
        intra::predict_palette(
            &mut self.fs.planes[at.plane],
            at.start_x,
            at.start_y,
            w,
            h,
            &block,
        );
    }

    /** The chroma-from-luma correction of a chroma transform block, read from the luma that was reconstructed before it. */
    fn predict_cfl_tx(&mut self, at: TxAt) {
        let (sub_x, sub_y) = plane_subsampling(self.seq, at.plane);
        let alpha = if at.plane == 1 {
            self.blk.cfl_alpha_u
        } else {
            self.blk.cfl_alpha_v
        };
        let block = CflBlock {
            x: at.start_x,
            y: at.start_y,
            log2w: u32::from(TX_WIDTH_LOG2[at.tx_size]),
            log2h: u32::from(TX_HEIGHT_LOG2[at.tx_size]),
            sub_x,
            sub_y,
            max_luma_w: self.blk.max_luma_w,
            max_luma_h: self.blk.max_luma_h,
        };
        let clip = (1i32 << self.seq.color.bit_depth) - 1;
        let (luma, rest) = self.fs.planes.split_at_mut(1);
        intra::predict_cfl(&mut rest[at.plane - 1], &luma[0], &block, alpha, clip);
    }

    /** Add the residual of a transform block, decoded from its coefficients, to the prediction already in the plane. */
    pub(super) fn reconstruct_tx(&mut self, at: TxAt, coded: &Coded) {
        let spec = ResidualSpec {
            x: at.start_x,
            y: at.start_y,
            geom: Geometry {
                log2w: u32::from(TX_WIDTH_LOG2[at.tx_size]),
                log2h: u32::from(TX_HEIGHT_LOG2[at.tx_size]),
                row_shift: row_shift(at.tx_size),
            },
            tx_type: coded.tx_type,
            lossless: self.blk.lossless,
            bit_depth: u32::from(self.seq.color.bit_depth),
        };
        reconstruct(&mut self.fs.planes[at.plane], &spec, &coded.coeffs);
    }
}
