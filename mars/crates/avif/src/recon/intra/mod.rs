/** Intra prediction of one transform block (spec 7.11.2): the request a transform block makes, the edge samples it is predicted from, and the dispatch to the predictor its mode names. The prediction is written into the plane, to be completed by the residual. */
mod basic;
mod cfl;
mod directional;
mod edges;
mod filter;
mod palette;
mod recursive;
mod smooth;
#[cfg(test)]
mod tests;

pub(crate) use cfl::{CflBlock, predict as predict_cfl};
pub(crate) use palette::{PaletteBlock, predict as predict_palette};

use crate::recon::Plane;

/** Intra mode numbers that have their own predictors (the spec's mode enumeration). */
const DC_PRED: u8 = 0;
const V_PRED: u8 = 1;
const D67_PRED: u8 = 8;
const SMOOTH_PRED: u8 = 9;
const SMOOTH_V_PRED: u8 = 10;
const SMOOTH_H_PRED: u8 = 11;
const PAETH_PRED: u8 = 12;

/** What a transform block asks of intra prediction: where it sits, which neighbours exist, the mode and size, and the tool settings the mode reads. Positions are in the plane's samples. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Request {
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) have_left: bool,
    pub(crate) have_above: bool,
    pub(crate) have_above_right: bool,
    pub(crate) have_below_left: bool,
    pub(crate) mode: u8,
    pub(crate) log2w: u32,
    pub(crate) log2h: u32,
    /** The angle delta of this plane, in steps of three degrees. */
    pub(crate) angle_delta: i32,
    /** The filter-intra mode when the block uses recursive filtering, which is luma only. */
    pub(crate) filter_intra: Option<u8>,
    /** Whether the sequence enables the intra edge filter. */
    pub(crate) edge_filter: bool,
    /** Whether the block above or to the left is smooth predicted, which picks the stronger edge filter. */
    pub(crate) smooth_neighbour: bool,
    /** The last sample column and row the decoded area covers in this plane. */
    pub(crate) max_x: usize,
    pub(crate) max_y: usize,
    pub(crate) bit_depth: u32,
}

/** Whether a mode is one of the directional modes, the ones that carry an angle delta. */
fn is_directional(mode: u8) -> bool {
    (V_PRED..=D67_PRED).contains(&mode)
}

/** Predict a transform block into the plane: build its edges, pick the predictor the request names, and write the w by h result at (x, y). */
pub(crate) fn predict(plane: &mut Plane, req: &Request) {
    let (w, h) = (1usize << req.log2w, 1usize << req.log2h);
    let mut e = edges::build(plane, req);
    let clip = (1i32 << req.bit_depth) - 1;
    let pred = if let Some(mode) = req.filter_intra {
        recursive::predict(&e, w, h, mode, clip)
    } else if is_directional(req.mode) {
        directional::predict(&mut e, req, w, h)
    } else {
        simple(&e, req, w, h)
    };
    for i in 0..h {
        let row = plane.row_mut(req.y + i);
        for j in 0..w {
            row[req.x + j] = pred[i * w + j] as u16;
        }
    }
}

/** The non-directional, non-recursive predictors: DC, Paeth and the three smooth modes. */
fn simple(e: &edges::Edges, req: &Request, w: usize, h: usize) -> Vec<i32> {
    match req.mode {
        DC_PRED => basic::dc(e, req),
        PAETH_PRED => basic::paeth(e, w, h),
        SMOOTH_PRED | SMOOTH_V_PRED | SMOOTH_H_PRED => smooth::predict(e, req, req.mode, w, h),
        // Only the non-directional modes reach here; the mode table guarantees it.
        _ => basic::paeth(e, w, h),
    }
}
