/** Post-filters that run on a frame's reconstructed samples after every tile is decoded (spec 7.14 to 7.17, in the order decode_frame_wrapup gives): the deblocking loop filter, then CDEF, then loop restoration. Each stage reads the previous stage's output; the restoration stage needs both the deblocked and the CDEF planes. */
mod cdef;
mod deblock;
mod restore;

use crate::frame::FrameHeader;
use crate::recon::Plane;
use crate::sequence::Sequence;
use crate::tile::FrameState;

pub(crate) use cdef::cdef_frame;
pub(crate) use deblock::filter_frame as deblock_frame;
pub(crate) use restore::restore_frame;

/** Run every post-filter on a decoded frame and return its final planes. The deblocked planes stay in `fs`, because the restoration stage reads the rows above and below each stripe from them. */
pub(crate) fn filter(seq: &Sequence, hdr: &FrameHeader, fs: &mut FrameState) -> [Plane; 3] {
    deblock_frame(seq, hdr, fs);
    let cdef = cdef_frame(seq, hdr, fs);
    restore_frame(seq, hdr, fs, &fs.planes, &cdef)
}
