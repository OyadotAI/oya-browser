/** Loop-filter strength (spec 7.14.4 and 7.14.5): the filter level of an edge from the frame's base levels, the block's segment and its delta, and the limits the sample filter compares against. The frame is intra-only, so the reference is always INTRA_FRAME and no mode delta applies. */
use crate::av1::SEG_LVL_ALT_LF_Y_V;
use crate::frame::FrameHeader;
use crate::tile::{MAX_LOOP_FILTER, MiInfo};

/** The reference index of intra blocks in the loop-filter reference deltas. */
const INTRA_REF: usize = 0;
/** The filter level is shifted right by this before it scales the reference delta. */
const DELTA_SCALE_SHIFT: u32 = 5;
/** Sharpness above which the limit shifts by two, and above zero by one. */
const SHARP_HIGH: i32 = 4;
/** The sharpest limit ceiling is nine less the sharpness. */
const LIMIT_CEILING: i32 = 9;
/** The threshold is the level divided by sixteen. */
const THRESH_SHIFT: u32 = 4;
/** The blimit adds two to the level before doubling it. */
const BLIMIT_OFFSET: i32 = 2;

/** The strength of one edge: its filter level, and the limit, blimit and threshold the sample filter derives from it (spec 7.14.4). */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Strength {
    pub(super) lvl: i32,
    pub(super) limit: i32,
    pub(super) blimit: i32,
    pub(super) thresh: i32,
}

/** The strength of the edge on the block `mi` for one plane and direction. */
pub(super) fn strength(hdr: &FrameHeader, mi: &MiInfo, plane: usize, pass: usize) -> Strength {
    let lvl = filter_level(hdr, mi, plane, pass);
    let sharp = i32::from(hdr.loop_filter.sharpness);
    let shift = sharpness_shift(sharp);
    let scaled = lvl >> shift;
    let limit = if sharp > 0 {
        scaled.clamp(1, LIMIT_CEILING - sharp)
    } else {
        scaled.max(1)
    };
    Strength {
        lvl,
        limit,
        blimit: BLIMIT_OFFSET * (lvl + BLIMIT_OFFSET) + limit,
        thresh: lvl >> THRESH_SHIFT,
    }
}

/** The shift that scales the level down to a limit, from the sharpness. */
fn sharpness_shift(sharp: i32) -> i32 {
    if sharp > SHARP_HIGH {
        2
    } else if sharp > 0 {
        1
    } else {
        0
    }
}

/** Adaptive filter strength selection (spec 7.14.5): the base level plus the block's delta, the segment feature, then the reference delta, each clipped to the legal range. */
fn filter_level(hdr: &FrameHeader, mi: &MiInfo, plane: usize, pass: usize) -> i32 {
    let i = if plane == 0 { pass } else { plane + 1 };
    let delta = i32::from(if hdr.delta_lf_multi {
        mi.delta_lf[i]
    } else {
        mi.delta_lf[0]
    });
    let base = (delta + i32::from(hdr.loop_filter.level[i])).clamp(0, MAX_LOOP_FILTER);
    let seg = segment_level(hdr, mi.segment_id as usize, SEG_LVL_ALT_LF_Y_V + i, base);
    if !hdr.loop_filter.delta_enabled {
        return seg;
    }
    let n_shift = seg >> DELTA_SCALE_SHIFT;
    let ref_delta = hdr.loop_filter.ref_deltas[INTRA_REF] << n_shift;
    (seg + ref_delta).clamp(0, MAX_LOOP_FILTER)
}

/** The level after the segment's loop-filter feature, when the segmentation feature is on for this segment; otherwise the level is unchanged. */
fn segment_level(hdr: &FrameHeader, segment: usize, feature: usize, level: i32) -> i32 {
    let seg = &hdr.seg;
    if seg.enabled && seg.feature_enabled[segment][feature] {
        (level + seg.feature_data[segment][feature]).clamp(0, MAX_LOOP_FILTER)
    } else {
        level
    }
}
