/** One block's decoding (spec decode_block, 5.11.5): the geometry and availability of the block, its mode info and transform size, its residual, and the write of its results into the frame grid for the blocks that follow. */
use super::constants::{BLOCK_8X8, MI_SIZE, PALETTE_COLORS, PALETTE_MAX_BLOCK_DIM};
use super::decoder::TileDecoder;
use super::grid::MiInfo;
use super::lr::plane_subsampling;
use super::state::PaletteInfo;
use super::tables::NUM_4X4_BLOCKS_WIDE;
use crate::error::Result;

/** The syntax values of the block being decoded. Fields are filled in by the mode-info, palette and transform-size readers, then written to the grid by store_block. */
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Block {
    pub(crate) mi_row: usize,
    pub(crate) mi_col: usize,
    pub(crate) mi_size: usize,
    pub(crate) bw4: usize,
    pub(crate) bh4: usize,
    pub(crate) has_chroma: bool,
    pub(crate) avail_u: bool,
    pub(crate) avail_l: bool,
    pub(crate) avail_u_chroma: bool,
    pub(crate) avail_l_chroma: bool,
    pub(crate) skip: bool,
    pub(crate) segment_id: usize,
    pub(crate) lossless: bool,
    pub(crate) y_mode: u8,
    pub(crate) uv_mode: u8,
    pub(crate) angle_delta_y: i32,
    pub(crate) angle_delta_uv: i32,
    pub(crate) cfl_alpha_u: i32,
    pub(crate) cfl_alpha_v: i32,
    pub(crate) use_filter_intra: bool,
    pub(crate) filter_intra_mode: u8,
    pub(crate) palette_size_y: usize,
    pub(crate) palette_size_uv: usize,
    pub(crate) palette_colors: [[u16; PALETTE_COLORS]; 3],
    pub(crate) color_map_y: Vec<u8>,
    pub(crate) color_map_uv: Vec<u8>,
    pub(crate) tx_size: usize,
    pub(crate) max_luma_w: usize,
    pub(crate) max_luma_h: usize,
}

impl Block {
    /** Whether either plane of the block carries a palette. */
    pub(crate) fn palette_used(&self) -> bool {
        self.palette_size_y > 0 || self.palette_size_uv > 0
    }

    /** Block width in samples. */
    pub(crate) fn width(&self) -> usize {
        self.bw4 * MI_SIZE
    }

    /** Block height in samples. */
    pub(crate) fn height(&self) -> usize {
        self.bh4 * MI_SIZE
    }
}

impl<'a> TileDecoder<'a> {
    /** decode_block(r, c, subSize): read one block's syntax and record it. */
    pub(super) fn decode_block(&mut self, r: usize, c: usize, size: usize) -> Result<()> {
        self.blk = self.block_geometry(r, c, size);
        self.intra_frame_mode_info()?;
        self.palette_tokens()?;
        self.read_block_tx_size()?;
        if self.blk.skip {
            self.reset_block_context();
        }
        self.residual()?;
        self.store_block();
        Ok(())
    }

    /** The position, size, chroma presence and neighbour availability of a block about to be read. */
    fn block_geometry(&self, r: usize, c: usize, size: usize) -> Block {
        let (sub_x, sub_y) = plane_subsampling(self.seq, 1);
        let bw4 = NUM_4X4_BLOCKS_WIDE[size] as usize;
        let bh4 = super::tables::NUM_4X4_BLOCKS_HIGH[size] as usize;
        let planes = self.seq.color.num_planes();
        let has_chroma =
            if (bh4 == 1 && sub_y == 1 && r & 1 == 0) || (bw4 == 1 && sub_x == 1 && c & 1 == 0) {
                false
            } else {
                planes > 1
            };
        let (ri, ci) = (r as isize, c as isize);
        let avail_u = self.is_inside(ri - 1, ci);
        let avail_l = self.is_inside(ri, ci - 1);
        let mut block = Block {
            mi_row: r,
            mi_col: c,
            mi_size: size,
            bw4,
            bh4,
            has_chroma,
            avail_u,
            avail_l,
            avail_u_chroma: avail_u,
            avail_l_chroma: avail_l,
            ..Block::default()
        };
        if has_chroma {
            if sub_y == 1 && bh4 == 1 {
                block.avail_u_chroma = self.is_inside(ri - 2, ci);
            }
            if sub_x == 1 && bw4 == 1 {
                block.avail_l_chroma = self.is_inside(ri, ci - 2);
            }
        } else {
            block.avail_u_chroma = false;
            block.avail_l_chroma = false;
        }
        block
    }

    /** reset_block_context(bw4, bh4): a skipped block clears the coefficient contexts it covers, for each plane it has. */
    fn reset_block_context(&mut self) {
        let planes = if self.blk.has_chroma { 3 } else { 1 };
        for plane in 0..planes {
            let (sub_x, sub_y) = plane_subsampling(self.seq, plane);
            let (c0, c1) = (
                self.blk.mi_col >> sub_x,
                (self.blk.mi_col + self.blk.bw4) >> sub_x,
            );
            let (r0, r1) = (
                self.blk.mi_row >> sub_y,
                (self.blk.mi_row + self.blk.bh4) >> sub_y,
            );
            self.above.planes[plane].level[c0..c1].fill(0);
            self.above.planes[plane].dc[c0..c1].fill(0);
            self.left.planes[plane].level[r0..r1].fill(0);
            self.left.planes[plane].dc[r0..r1].fill(0);
        }
    }

    /** Write the block's results into every 4x4 unit it covers, after its residual has been read. The palette, when there is one, is recorded in the frame's palette list. */
    fn store_block(&mut self) {
        let palette_index = self.push_palette();
        let blk = &self.blk;
        let delta_lf = self.dq.delta_lf.map(|v| v as i8);
        let has_chroma = blk.has_chroma;
        let info = (
            blk.mi_size as u8,
            blk.y_mode,
            blk.uv_mode,
            blk.skip,
            blk.tx_size as u8,
            blk.segment_id as u8,
            [blk.palette_size_y as u8, blk.palette_size_uv as u8],
        );
        self.fs.mi.fill(
            blk.mi_row,
            blk.mi_col,
            blk.bh4,
            blk.bw4,
            |m: &mut MiInfo| {
                m.mi_size = info.0;
                m.y_mode = info.1;
                if has_chroma {
                    m.uv_mode = info.2;
                }
                m.skip = info.3;
                m.is_inter = false;
                m.tx_size = info.4;
                m.segment_id = info.5;
                m.palette_size = info.6;
                m.palette_index = palette_index;
                m.delta_lf = delta_lf;
            },
        );
    }

    /** Record the block's palette, if it has one, and return its index plus one (zero for none). The colour-index maps move into the record; prediction reads them later. */
    fn push_palette(&mut self) -> u32 {
        if !self.blk.palette_used() {
            return 0;
        }
        let info = PaletteInfo {
            colors: self.blk.palette_colors,
            size: [self.blk.palette_size_y, self.blk.palette_size_uv],
            map_y: std::mem::take(&mut self.blk.color_map_y),
            map_uv: std::mem::take(&mut self.blk.color_map_uv),
        };
        self.fs.palettes.push(info);
        self.fs.palettes.len() as u32
    }

    /** Whether a block is large enough to carry a palette: at least 8x8, and no larger than 64 samples in either direction. */
    pub(super) fn palette_allowed(&self) -> bool {
        self.blk.mi_size >= BLOCK_8X8
            && self.blk.width() <= PALETTE_MAX_BLOCK_DIM
            && self.blk.height() <= PALETTE_MAX_BLOCK_DIM
    }
}
