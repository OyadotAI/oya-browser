/** One edge of the deblocking pass (spec 7.14.2 and 7.14.3): which 4x4 units lie on either side of a boundary, whether the boundary is a transform edge that is filtered, the largest filter it allows, and the sample filter run along its four samples. */
use super::filters::{Cfg, Sample, filter_sample};
use super::level::{Strength, strength};
use crate::frame::FrameHeader;
use crate::sequence::Sequence;
use crate::tile::{
    FrameState, MI_SIZE, MiInfo, NUM_4X4_BLOCKS_HIGH, NUM_4X4_BLOCKS_WIDE, SUBSAMPLED_SIZE,
    TX_HEIGHT, TX_WIDTH,
};

/** The largest filter the luma plane may use, and the chroma plane's. */
const LUMA_MAX_FILTER: usize = 16;
const CHROMA_MAX_FILTER: usize = 8;
/** Each 4x4 block is four samples wide. */
const UNIT: usize = 4;

/** The fixed inputs of one plane and direction: the plane, the pass (0 for vertical edges, 1 for horizontal), its subsampling and the bit depth. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PassCtx {
    pub(super) plane: usize,
    pub(super) pass: usize,
    pub(super) sub_x: usize,
    pub(super) sub_y: usize,
    pub(super) bit_depth: u32,
}

impl PassCtx {
    /** The context of one plane and pass of a sequence. */
    fn new(seq: &Sequence, plane: usize, pass: usize) -> Self {
        let (sub_x, sub_y) = if plane == 0 {
            (0, 0)
        } else {
            (
                usize::from(seq.color.subsampling_x),
                usize::from(seq.color.subsampling_y),
            )
        };
        Self {
            plane,
            pass,
            sub_x,
            sub_y,
            bit_depth: u32::from(seq.color.bit_depth),
        }
    }

    /** The step between samples across the edge, as (dx, dy): horizontal for vertical edges. */
    fn across(&self) -> (usize, usize) {
        if self.pass == 0 { (1, 0) } else { (0, 1) }
    }
}

/** The 4x4 units an edge separates, in luma units after the subsampling adjustment, with the edge's location in its plane. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Edge {
    row: usize,
    col: usize,
    prev_row: usize,
    prev_col: usize,
    xp: usize,
    yp: usize,
}

/** Visit every edge of one plane and direction, row by row, in 4x4 luma steps (spec 7.14.1). */
pub(super) fn filter_pass(
    seq: &Sequence,
    hdr: &FrameHeader,
    fs: &mut FrameState,
    plane: usize,
    pass: usize,
) {
    let ctx = PassCtx::new(seq, plane, pass);
    let (rows, cols) = (hdr.size.mi_rows as usize, hdr.size.mi_cols as usize);
    for row in (0..rows).step_by(1 << ctx.sub_y) {
        for col in (0..cols).step_by(1 << ctx.sub_x) {
            filter_edge(&ctx, hdr, fs, row, col);
        }
    }
}

/** The edge loop filter process (spec 7.14.2) for the edge at luma unit (row, col): skip it when it is off screen or not a filtered transform edge, otherwise filter its four samples with the strength of this block, or of the block across it when this block's level is zero. */
fn filter_edge(ctx: &PassCtx, hdr: &FrameHeader, fs: &mut FrameState, row: usize, col: usize) {
    let Some(edge) = locate(ctx, hdr, row, col) else {
        return;
    };
    let plane = ctx.plane;
    let here = *fs.mi.at(edge.row, edge.col);
    let tx = fs.lf_tx[plane].at(edge.row >> ctx.sub_y, edge.col >> ctx.sub_x);
    let prev_tx = fs.lf_tx[plane].at(edge.prev_row >> ctx.sub_y, edge.prev_col >> ctx.sub_x);
    if !applies(ctx, &here, tx, &edge) {
        return;
    }
    let size = filter_size(ctx, tx, prev_tx);
    let mut s = strength(hdr, &here, plane, ctx.pass);
    if s.lvl == 0 {
        let prev = *fs.mi.at(edge.prev_row, edge.prev_col);
        s = strength(hdr, &prev, plane, ctx.pass);
    }
    if s.lvl == 0 {
        return;
    }
    run_samples(ctx, fs, &edge, size, &s);
}

/** Run the sample filter on the four samples along the edge. */
fn run_samples(ctx: &PassCtx, fs: &mut FrameState, edge: &Edge, size: usize, s: &Strength) {
    let (dx, dy) = ctx.across();
    let cfg = Cfg {
        size,
        chroma: ctx.plane > 0,
        bit_depth: ctx.bit_depth,
    };
    let buf = &mut fs.planes[ctx.plane];
    for i in 0..MI_SIZE {
        let site = Sample {
            x: edge.xp + dy * i,
            y: edge.yp + dx * i,
            dx,
            dy,
        };
        filter_sample(buf, site, cfg, s);
    }
}

/** The position and neighbours of an edge, or None when the edge lies outside the visible picture or on its left or top border (spec 7.14.2, onScreen). The unit is adjusted to the subsampled plane's sampling position. */
fn locate(ctx: &PassCtx, hdr: &FrameHeader, row: usize, col: usize) -> Option<Edge> {
    let (x, y) = (col * MI_SIZE, row * MI_SIZE);
    let (dx, dy) = ctx.across();
    let off_screen = x >= hdr.size.frame_width as usize
        || y >= hdr.size.frame_height as usize
        || (ctx.pass == 0 && x == 0)
        || (ctx.pass == 1 && y == 0);
    if off_screen {
        return None;
    }
    let (row, col) = (row | ctx.sub_y, col | ctx.sub_x);
    Some(Edge {
        row,
        col,
        prev_row: row - (dy << ctx.sub_y),
        prev_col: col - (dx << ctx.sub_x),
        xp: x >> ctx.sub_x,
        yp: y >> ctx.sub_y,
    })
}

/** Whether the edge is filtered (spec 7.14.2, applyFilter): it must be a transform edge, and either a prediction block edge, or a non-skipped block. Every intra block is an intra reference, so the last term holds for all blocks of these frames. */
fn applies(ctx: &PassCtx, mi: &MiInfo, tx: u8, edge: &Edge) -> bool {
    let (block_edge, tx_edge) = (is_block_edge(ctx, mi, edge), is_tx_edge(ctx, tx, edge));
    let is_intra = !mi.is_inter;
    tx_edge && (block_edge || !mi.skip || is_intra)
}

/** Whether the edge falls on a prediction block boundary of the block's plane size. */
fn is_block_edge(ctx: &PassCtx, mi: &MiInfo, edge: &Edge) -> bool {
    let plane_size = SUBSAMPLED_SIZE[usize::from(mi.mi_size)][ctx.sub_x][ctx.sub_y] as usize;
    let (pos, len) = if ctx.pass == 0 {
        (edge.xp, NUM_4X4_BLOCKS_WIDE[plane_size] as usize * UNIT)
    } else {
        (edge.yp, NUM_4X4_BLOCKS_HIGH[plane_size] as usize * UNIT)
    };
    pos % len == 0
}

/** Whether the edge falls on a transform boundary of the transform size on this side. */
fn is_tx_edge(ctx: &PassCtx, tx: u8, edge: &Edge) -> bool {
    let tx = usize::from(tx);
    let (pos, len) = if ctx.pass == 0 {
        (edge.xp, TX_WIDTH[tx] as usize)
    } else {
        (edge.yp, TX_HEIGHT[tx] as usize)
    };
    pos % len == 0
}

/** The filter size process (spec 7.14.3): the smaller of the two transform sizes across the edge, capped at 16 for luma and 8 for chroma. */
fn filter_size(ctx: &PassCtx, tx: u8, prev_tx: u8) -> usize {
    let (a, b) = (usize::from(tx), usize::from(prev_tx));
    let base = if ctx.pass == 0 {
        TX_WIDTH[a].min(TX_WIDTH[b]) as usize
    } else {
        TX_HEIGHT[a].min(TX_HEIGHT[b]) as usize
    };
    let cap = if ctx.plane == 0 {
        LUMA_MAX_FILTER
    } else {
        CHROMA_MAX_FILTER
    };
    base.min(cap)
}
