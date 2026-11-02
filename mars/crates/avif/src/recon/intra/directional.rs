/** Directional intra prediction (spec 7.11.2.4): an angle from the mode and its angle delta, edge conditioning when the edge filter is on, and the projection of each sample onto the above row, the left column, or both. */
use super::Request;
use super::edges::Edges;
use super::filter::{corner, filter_edge, strength, upsample, use_upsample};
use crate::recon::math::round2;
use crate::recon::tables::{DR_INTRA_DERIVATIVE, MODE_TO_ANGLE};

/** Blocks whose two sides sum to at least this many samples use the corner filter. */
const CORNER_FILTER_MIN_SUM: usize = 24;
/** Degrees per angle-delta step. */
const ANGLE_STEP: i32 = 3;
/** Fractional bits of the projection step, and the mask of the interpolation weight. */
const FRAC_BITS: u32 = 6;
const WEIGHT_MASK: i32 = 0x1F;
/** Interpolation weights sum to this, so the blend rounds with five bits. */
const BLEND_BITS: u32 = 5;
/** Angles that need no edge filtering: straight down and straight across. */
const VERTICAL: i32 = 90;
const HORIZONTAL: i32 = 180;

/** Edge conditioning of one directional block: the corner filter, the edge filters on each side that is available, and the upsampling of each edge. It returns whether the above and left edges were upsampled. */
fn condition_edges(e: &mut Edges, req: &Request, w: usize, h: usize, p_angle: i32) -> (bool, bool) {
    if !req.edge_filter {
        return (false, false);
    }
    let smooth = req.smooth_neighbour;
    if p_angle != VERTICAL && p_angle != HORIZONTAL {
        if p_angle > 90 && p_angle < 180 && w + h >= CORNER_FILTER_MIN_SUM {
            let c = corner(e);
            e.set_above(-1, c);
            e.set_left(-1, c);
        }
        if req.have_above {
            let s = strength(w, h, smooth, p_angle - 90);
            let reach = w.min(req.max_x + 1 - req.x);
            let count = reach + if p_angle < 90 { h } else { 0 } + 1;
            filter_edge(e.above_mut(), count, s);
        }
        if req.have_left {
            let s = strength(w, h, smooth, p_angle - 180);
            let reach = h.min(req.max_y + 1 - req.y);
            let count = reach + if p_angle > 180 { w } else { 0 } + 1;
            filter_edge(e.left_mut(), count, s);
        }
    }
    let max = (1i32 << req.bit_depth) - 1;
    let up_above = use_upsample(w, h, smooth, p_angle - 90);
    if up_above {
        upsample(e.above_mut(), w + if p_angle < 90 { h } else { 0 }, max);
    }
    let up_left = use_upsample(w, h, smooth, p_angle - 180);
    if up_left {
        upsample(e.left_mut(), h + if p_angle > 180 { w } else { 0 }, max);
    }
    (up_above, up_left)
}

/** Predict a directional block: the angle is the mode's nominal angle moved by the angle delta, and every sample is taken from the edges at the projected position. Returns the w by h prediction, row-major. */
pub(super) fn predict(e: &mut Edges, req: &Request, w: usize, h: usize) -> Vec<i32> {
    let p_angle = MODE_TO_ANGLE[usize::from(req.mode)] + req.angle_delta * ANGLE_STEP;
    let (up_a, up_l) = condition_edges(e, req, w, h, p_angle);
    let (ua, ul) = (u32::from(up_a), u32::from(up_l));
    let mut pred = vec![0i32; w * h];
    if p_angle < VERTICAL {
        project_above(e, &mut pred, w, h, p_angle, ua);
    } else if p_angle > VERTICAL && p_angle < HORIZONTAL {
        project_mixed(e, &mut pred, w, h, p_angle, ua, ul);
    } else if p_angle > HORIZONTAL {
        project_left(e, &mut pred, w, h, p_angle, ul);
    } else if p_angle == VERTICAL {
        fill_rows(e, &mut pred, w, h);
    } else {
        fill_cols(e, &mut pred, w, h);
    }
    pred
}

/** The blend of two neighbouring edge samples at a fractional position, with 1/32 weights. */
fn blend(a: i32, b: i32, shift: i32) -> i32 {
    round2(
        i64::from(a) * i64::from(32 - shift) + i64::from(b) * i64::from(shift),
        BLEND_BITS,
    ) as i32
}

/** Derivative of the angle in the direction the projection runs, read from the table at an angle. */
fn derivative(angle: i32) -> i32 {
    DR_INTRA_DERIVATIVE[angle as usize]
}

/** Angles below 90 degrees: each sample reads the above row at a position projected to the right, and clamps to its last entry past the row's end. */
fn project_above(e: &Edges, pred: &mut [i32], w: usize, h: usize, p_angle: i32, ua: u32) {
    let dx = derivative(p_angle);
    let max_base = ((w + h - 1) << ua) as i32;
    for i in 0..h {
        let idx = (i as i32 + 1) * dx;
        let shift = ((idx << ua) >> 1) & WEIGHT_MASK;
        for j in 0..w {
            let base = (idx >> (index_shift(ua))) + ((j as i32) << ua);
            pred[i * w + j] = if base < max_base {
                blend(e.above(base as isize), e.above(base as isize + 1), shift)
            } else {
                e.above(max_base as isize)
            };
        }
    }
}

/** The shift that turns a projected position into an edge index: the fractional bits left once the edge is upsampled, which doubles its resolution. */
fn index_shift(upsampled: u32) -> u32 {
    FRAC_BITS - upsampled
}

/** Angles between 90 and 180 degrees: a sample projects onto the above row when the projection lands on it, and onto the left column otherwise. */
fn project_mixed(e: &Edges, pred: &mut [i32], w: usize, h: usize, p_angle: i32, ua: u32, ul: u32) {
    let dx = derivative(HORIZONTAL - p_angle);
    let dy = derivative(p_angle - VERTICAL);
    for i in 0..h {
        for j in 0..w {
            let idx = ((j as i32) << FRAC_BITS) - (i as i32 + 1) * dx;
            let base = idx >> index_shift(ua);
            pred[i * w + j] = if base >= -(1 << ua) {
                let shift = ((idx << ua) >> 1) & WEIGHT_MASK;
                blend(e.above(base as isize), e.above(base as isize + 1), shift)
            } else {
                let idx = ((i as i32) << FRAC_BITS) - (j as i32 + 1) * dy;
                let base = idx >> index_shift(ul);
                let shift = ((idx << ul) >> 1) & WEIGHT_MASK;
                blend(e.left(base as isize), e.left(base as isize + 1), shift)
            };
        }
    }
}

/** Angles above 180 degrees: each sample reads the left column at a position projected downwards. */
fn project_left(e: &Edges, pred: &mut [i32], w: usize, h: usize, p_angle: i32, ul: u32) {
    let dy = derivative(270 - p_angle);
    for j in 0..w {
        let idx = (j as i32 + 1) * dy;
        let shift = ((idx << ul) >> 1) & WEIGHT_MASK;
        for i in 0..h {
            let base = (idx >> index_shift(ul)) + ((i as i32) << ul);
            pred[i * w + j] = blend(e.left(base as isize), e.left(base as isize + 1), shift);
        }
    }
}

/** Straight down: every row is a copy of the above row. */
fn fill_rows(e: &Edges, pred: &mut [i32], w: usize, h: usize) {
    for i in 0..h {
        for j in 0..w {
            pred[i * w + j] = e.above(j as isize);
        }
    }
}

/** Straight across: every column is a copy of the left column. */
fn fill_cols(e: &Edges, pred: &mut [i32], w: usize, h: usize) {
    for i in 0..h {
        for j in 0..w {
            pred[i * w + j] = e.left(i as isize);
        }
    }
}
