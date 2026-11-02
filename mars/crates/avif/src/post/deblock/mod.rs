/** The deblocking loop filter (spec 7.14): every vertical edge of a plane is smoothed, then every horizontal edge, for each plane the frame filters. Edges are visited in 4x4 units, and the order inside a pass does not matter, which is what lets the frame be filtered in place. */
mod edge;
mod filters;
mod level;

use crate::frame::FrameHeader;
use crate::sequence::Sequence;
use crate::tile::FrameState;

/** Filter the frame in place when its loop filter levels ask for it (spec decode_frame_wrapup step 1). Planes whose own level is zero are skipped, except luma, which always runs when the frame filters at all. */
pub(crate) fn filter_frame(seq: &Sequence, hdr: &FrameHeader, fs: &mut FrameState) {
    let levels = hdr.loop_filter.level;
    if levels[0] == 0 && levels[1] == 0 {
        return;
    }
    for plane in 0..seq.color.num_planes() {
        if plane == 0 || levels[1 + plane] != 0 {
            for pass in 0..2 {
                edge::filter_pass(seq, hdr, fs, plane, pass);
            }
        }
    }
}

#[cfg(test)]
mod tests;
