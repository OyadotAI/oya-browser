/** Validity of an intra block copy vector (spec is_mv_valid, the conformance rule for use_intrabc): the vector is whole samples, the source it names lies inside the tile, and the source's superblock is far enough behind the block's that a pipelined decoder has already produced it. A stream that breaks these is malformed, so the decoder never copies from samples it has not decoded. */
use super::constants::{INTRABC_DELAY_SB64, MI_SIZE, MV_FRAC_BITS, MV_LIMIT, MV_UNIT};
use super::decoder::{TileBounds, TileDecoder};

/** Superblock sizes in samples, the shift of a 64-sample superblock column, and the shift of a 4x4 count to 64-sample columns. */
const SB64_SAMPLES: i64 = 64;
const SB128_SAMPLES: i64 = 128;
const SB64_SHIFT: u32 = 6;
const SB64_MI_SHIFT: u32 = 4;
/** Chroma of a block narrower or shorter than this many samples reads this many samples further up or left. */
const SMALL_CHROMA_DIM: usize = 8;
const CHROMA_REACH: i64 = 4;

/** What the validity rule reads of one intra block copy block: its place and size, its chroma subsampling when it carries chroma, its tile, its superblock size, and its vector in 1/8 sample units. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DvBlock {
    pub(crate) mi_row: usize,
    pub(crate) mi_col: usize,
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) chroma_sub: Option<(usize, usize)>,
    pub(crate) tile: TileBounds,
    pub(crate) sb128: bool,
    pub(crate) mv: [i32; 2],
}

/** Whether a vector meets every rule of is_mv_valid for the block. A component at or past the magnitude limit fails first, then the whole-sample rule, the tile rule and the delay rule. */
pub(crate) fn dv_is_valid(block: &DvBlock) -> bool {
    if block.mv.iter().any(|&v| v.abs() >= MV_LIMIT) {
        return false;
    }
    let whole = block.mv.iter().all(|&v| v % MV_UNIT == 0);
    whole && source_in_tile(block) && source_is_behind(block)
}

/** The source rectangle as (top, left, bottom, right) in samples. A small chroma block also reads the samples four to its upper left. */
fn source_edges(block: &DvBlock) -> (i64, i64, i64, i64) {
    let mi = MI_SIZE as i64;
    let mut top = block.mi_row as i64 * mi + i64::from(block.mv[0] >> MV_FRAC_BITS);
    let mut left = block.mi_col as i64 * mi + i64::from(block.mv[1] >> MV_FRAC_BITS);
    let bottom = top + block.height as i64;
    let right = left + block.width as i64;
    if let Some((sub_x, sub_y)) = block.chroma_sub {
        if block.width < SMALL_CHROMA_DIM && sub_x == 1 {
            left -= CHROMA_REACH;
        }
        if block.height < SMALL_CHROMA_DIM && sub_y == 1 {
            top -= CHROMA_REACH;
        }
    }
    (top, left, bottom, right)
}

/** The source rectangle lies inside the tile. */
fn source_in_tile(block: &DvBlock) -> bool {
    let (top, left, bottom, right) = source_edges(block);
    let t = &block.tile;
    let mi = MI_SIZE as i64;
    top >= t.mi_row_start as i64 * mi
        && left >= t.mi_col_start as i64 * mi
        && bottom <= t.mi_row_end as i64 * mi
        && right <= t.mi_col_end as i64 * mi
}

/** The source is far enough behind the block: its superblock is at least the fixed delay before the block's superblock in raster order, and it lies within the wavefront that delay allows. */
fn source_is_behind(block: &DvBlock) -> bool {
    let sb_px = if block.sb128 {
        SB128_SAMPLES
    } else {
        SB64_SAMPLES
    };
    let (_, _, bottom, right) = source_edges(block);
    let mi = MI_SIZE as i64;
    let active_row = block.mi_row as i64 * mi / sb_px;
    let active_col = (block.mi_col as i64 * mi) >> SB64_SHIFT;
    let src_row = (bottom - 1) / sb_px;
    let src_col = (right - 1) >> SB64_SHIFT;
    let t = &block.tile;
    let per_row = ((((t.mi_col_end - t.mi_col_start) as i64) - 1) >> SB64_MI_SHIFT) + 1;
    let active = active_row * per_row + active_col;
    let src = src_row * per_row + src_col;
    let delay = i64::from(INTRABC_DELAY_SB64);
    if src >= active - delay {
        return false;
    }
    let gradient = 1 + delay + i64::from(block.sb128);
    let wavefront = gradient * (active_row - src_row);
    !(src_row > active_row || src_col >= active_col - delay + wavefront)
}

impl<'a> TileDecoder<'a> {
    /** The validity check for a vector of the block being decoded. */
    pub(super) fn intrabc_mv_is_valid(&self, mv: [i32; 2]) -> bool {
        let chroma_sub = if self.blk.has_chroma {
            Some(super::lr::plane_subsampling(self.seq, 1))
        } else {
            None
        };
        dv_is_valid(&DvBlock {
            mi_row: self.blk.mi_row,
            mi_col: self.blk.mi_col,
            width: self.blk.width(),
            height: self.blk.height(),
            chroma_sub,
            tile: self.tile,
            sb128: self.seq.use_128x128_superblock,
            mv,
        })
    }
}
