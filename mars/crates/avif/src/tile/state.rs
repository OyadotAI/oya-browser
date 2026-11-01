/** Frame-wide state that the tiles of one frame fill in and the later stages read: the per-4x4 grid, CDEF indices, loop-restoration units, the palette list, and the transform blocks with their dequantised coefficients. */
use super::constants::{FRAME_LF_COUNT, MI_SIZE, PALETTE_COLORS, SB_SAMPLES};
use super::grid::{CdefGrid, MiGrid, TxSizeGrid};
use super::lr::LrUnit;
use crate::frame::FrameHeader;
use crate::recon::Plane;
use crate::sequence::Sequence;

/** A palette used by one or more blocks: the colours of Y, U and V, the sizes, and the colour-index maps that predict the block's samples. */
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PaletteInfo {
    pub(crate) colors: [[u16; PALETTE_COLORS]; 3],
    pub(crate) size: [usize; 2],
    pub(crate) map_y: Vec<u8>,
    pub(crate) map_uv: Vec<u8>,
}

/** The loop-restoration units of one plane, row-major. Units are empty when the plane's restoration type is none. */
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LrPlane {
    pub(crate) unit_rows: usize,
    pub(crate) unit_cols: usize,
    pub(crate) units: Vec<LrUnit>,
}

/** Everything a frame's tiles produce. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FrameState {
    pub(crate) mi: MiGrid,
    pub(crate) cdef: CdefGrid,
    pub(crate) lr: [LrPlane; 3],
    pub(crate) palettes: Vec<PaletteInfo>,
    /** The reconstructed samples of each plane, padded to whole superblocks. Planes beyond the picture's colour count are empty. */
    pub(crate) planes: [Plane; 3],
    /** The transform size of each 4x4 unit of each plane, for the loop filter. */
    pub(crate) lf_tx: [TxSizeGrid; 3],
}

impl FrameState {
    /** Empty state sized for the frame: the grids cover its mode-info units, and each restored plane gets its unit array. */
    pub(crate) fn new(seq: &Sequence, hdr: &FrameHeader) -> Self {
        let rows = hdr.size.mi_rows as usize;
        let cols = hdr.size.mi_cols as usize;
        let mut lr: [LrPlane; 3] = Default::default();
        for (plane, slot) in lr.iter_mut().enumerate().take(seq.color.num_planes()) {
            *slot = super::lr::plane_units(hdr, seq, plane);
        }
        let _ = FRAME_LF_COUNT;
        Self {
            mi: MiGrid::new(rows, cols),
            cdef: CdefGrid::new(rows, cols),
            lr,
            palettes: Vec::new(),
            planes: plane_buffers(seq, hdr),
            lf_tx: [
                TxSizeGrid::new(rows, cols),
                TxSizeGrid::new(rows, cols),
                TxSizeGrid::new(rows, cols),
            ],
        }
    }
}

/** The sample planes of a frame: each is the frame's mode-info area rounded up to whole 128-sample superblocks, so a transform block that overhangs the edge is written in place. */
fn plane_buffers(seq: &Sequence, hdr: &FrameHeader) -> [Plane; 3] {
    let width = (hdr.size.mi_cols as usize * MI_SIZE).next_multiple_of(SB_SAMPLES);
    let height = (hdr.size.mi_rows as usize * MI_SIZE).next_multiple_of(SB_SAMPLES);
    let mut planes: [Plane; 3] = Default::default();
    for (plane, slot) in planes.iter_mut().enumerate().take(seq.color.num_planes()) {
        let (sub_x, sub_y) = super::lr::plane_subsampling(seq, plane);
        *slot = Plane::new(width >> sub_x, height >> sub_y);
    }
    planes
}
