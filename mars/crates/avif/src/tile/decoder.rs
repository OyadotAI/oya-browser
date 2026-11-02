/** The tile decoder: one tile's symbol decoder, its adaptive CDFs and above and left contexts, the delta-q and delta-lf state, and the frame state it writes into. decode() runs decode_tile (spec 5.11.2) and checks the tile ends in step with the bitstream. */
use super::block::Block;
use super::constants::{
    BLOCK_64X64, BLOCK_128X128, CONTEXT_MARGIN, FRAME_LF_COUNT, MAX_SCAN_LEN, SB64_MI, SB128_MI,
};
use super::decoded::BlockDecoded;
use super::grid::EdgeContexts;
use super::lr::{LrRefs, plane_subsampling};
use super::state::FrameState;
use crate::context::TileCdfs;
use crate::error::Result;
use crate::frame::FrameHeader;
use crate::sequence::Sequence;
use crate::symbol::SymbolDecoder;

/** The mode-info rectangle a tile covers, in 4x4 luma units; the end values are exclusive. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TileBounds {
    pub(crate) mi_row_start: usize,
    pub(crate) mi_row_end: usize,
    pub(crate) mi_col_start: usize,
    pub(crate) mi_col_end: usize,
}

/** The state that changes as a tile's superblocks are read: the current quantiser index, the running loop-filter deltas, and whether this superblock may carry a delta. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DeltaState {
    pub(crate) current_qindex: i32,
    pub(crate) delta_lf: [i32; FRAME_LF_COUNT],
    pub(crate) read_deltas: bool,
}

/** One tile's decoder. It borrows the frame's headers and state, and owns the symbol decoder and the tile's CDFs. */
pub(crate) struct TileDecoder<'a> {
    pub(super) hdr: &'a FrameHeader,
    pub(super) seq: &'a Sequence,
    pub(super) fs: &'a mut FrameState,
    pub(super) sym: SymbolDecoder<'a>,
    pub(super) cdf: TileCdfs,
    pub(super) tile: TileBounds,
    pub(super) above: EdgeContexts,
    pub(super) left: EdgeContexts,
    pub(super) dq: DeltaState,
    pub(super) lr_ref: LrRefs,
    pub(super) decoded: BlockDecoded,
    pub(super) quant: Vec<i32>,
    pub(super) blk: Block,
}

impl<'a> TileDecoder<'a> {
    /** A decoder for one tile whose bytes are `data`. The CDFs start from the frame's defaults and the quantiser index from the frame's base index. */
    pub(crate) fn new(
        seq: &'a Sequence,
        hdr: &'a FrameHeader,
        fs: &'a mut FrameState,
        tile: TileBounds,
        data: &'a [u8],
    ) -> Result<Self> {
        let sym = SymbolDecoder::new(data, hdr.disable_cdf_update)?;
        let base = i32::from(hdr.quant.base_q_idx);
        Ok(Self {
            hdr,
            seq,
            fs,
            sym,
            cdf: TileCdfs::new(hdr.quant.base_q_idx),
            tile,
            above: EdgeContexts::new(hdr.size.mi_cols as usize + CONTEXT_MARGIN),
            left: EdgeContexts::new(hdr.size.mi_rows as usize + CONTEXT_MARGIN),
            dq: DeltaState {
                current_qindex: base,
                delta_lf: [0; FRAME_LF_COUNT],
                read_deltas: false,
            },
            lr_ref: LrRefs::new(),
            decoded: BlockDecoded::new(),
            quant: vec![0; MAX_SCAN_LEN],
            blk: Block::default(),
        })
    }

    /** Decode every superblock of the tile, then check the symbol decoder ended exactly at the tile's padding. */
    pub(crate) fn decode(mut self) -> Result<()> {
        self.decode_tile()?;
        self.sym.exit()
    }

    /** decode_tile(): reset the above context and the tile's references, then walk the superblocks in raster order. */
    fn decode_tile(&mut self) -> Result<()> {
        let all = self.above.planes[0].level.len();
        self.above.clear_range(0, all);
        self.dq.delta_lf = [0; FRAME_LF_COUNT];
        let sb = self.sb_mi();
        let size = if sb == SB128_MI {
            BLOCK_128X128
        } else {
            BLOCK_64X64
        };
        for r in (self.tile.mi_row_start..self.tile.mi_row_end).step_by(sb) {
            self.left.clear_range(r, r + sb);
            for c in (self.tile.mi_col_start..self.tile.mi_col_end).step_by(sb) {
                self.dq.read_deltas = self.hdr.delta_q_present;
                self.clear_cdef(r, c, sb);
                self.clear_block_decoded(r, c, sb);
                self.read_lr(r, c, sb)?;
                self.decode_partition(r, c, size)?;
            }
        }
        Ok(())
    }

    /** clear_cdef(r, c): the 64x64 units of a superblock start with no CDEF index. */
    fn clear_cdef(&mut self, r: usize, c: usize, sb: usize) {
        let unit = SB64_MI;
        for i in (r..r + sb).step_by(unit) {
            for j in (c..c + sb).step_by(unit) {
                self.fs.cdef.set(i, j, -1);
            }
        }
    }

    /** clear_block_decoded_flags(r, c, sbSize4): mark each plane's edge units of the superblock as decoded where the tile reaches. */
    fn clear_block_decoded(&mut self, r: usize, c: usize, sb: usize) {
        for plane in 0..self.seq.color.num_planes() {
            let (sub_x, sub_y) = plane_subsampling(self.seq, plane);
            let width4 = ((self.tile.mi_col_end - c) >> sub_x) as isize;
            let height4 = ((self.tile.mi_row_end - r) >> sub_y) as isize;
            let sb4_y = (sb >> sub_y) as isize;
            let sb4_x = (sb >> sub_x) as isize;
            self.decoded
                .reset_plane(plane, sb4_y, sb4_x, width4, height4);
        }
    }

    /** Superblock size in 4x4 units. */
    pub(super) fn sb_mi(&self) -> usize {
        if self.seq.use_128x128_superblock {
            SB128_MI
        } else {
            SB64_MI
        }
    }

    /** is_inside(r, c): whether a candidate position lies in this tile. Negative candidates are outside. */
    pub(super) fn is_inside(&self, r: isize, c: isize) -> bool {
        c >= self.tile.mi_col_start as isize
            && c < self.tile.mi_col_end as isize
            && r >= self.tile.mi_row_start as isize
            && r < self.tile.mi_row_end as isize
    }

    /** Mode-info rows of the frame. */
    pub(super) fn mi_rows(&self) -> usize {
        self.hdr.size.mi_rows as usize
    }

    /** Mode-info columns of the frame. */
    pub(super) fn mi_cols(&self) -> usize {
        self.hdr.size.mi_cols as usize
    }
}
