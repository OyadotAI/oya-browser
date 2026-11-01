/** Per-4x4 bookkeeping of a frame's tiles (the spec's YModes, Skips, MiSizes, SegmentIds, PaletteSizes, TxSizes and friends), and the above and left context arrays that the coefficient and partition contexts read. Writes past the frame edge are clipped, because nothing reads them. */
use super::constants::{CONTEXT_MARGIN, FRAME_LF_COUNT};

/** Everything the spec stores per 4x4 luma unit that a later block, a later context or the loop filter may read. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MiInfo {
    pub(crate) mi_size: u8,
    pub(crate) y_mode: u8,
    pub(crate) uv_mode: u8,
    pub(crate) skip: bool,
    pub(crate) is_inter: bool,
    pub(crate) tx_size: u8,
    pub(crate) inter_tx_size: u8,
    pub(crate) segment_id: u8,
    pub(crate) palette_size: [u8; 2],
    /** Index into the frame's palette list plus one, or zero when the block has no palette. */
    pub(crate) palette_index: u32,
    pub(crate) delta_lf: [i8; FRAME_LF_COUNT],
    pub(crate) tx_type: u8,
}

/** A frame-sized grid of MiInfo, one cell per 4x4 luma unit, stored row-major. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MiGrid {
    rows: usize,
    cols: usize,
    cells: Vec<MiInfo>,
}

impl MiGrid {
    /** An all-default grid of `rows` by `cols` units. */
    pub(crate) fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            cells: vec![MiInfo::default(); rows * cols],
        }
    }

    /** The cell at (row, col). Callers pass positions inside the frame, which the tile bounds guarantee for every read. */
    pub(crate) fn at(&self, row: usize, col: usize) -> &MiInfo {
        &self.cells[row * self.cols + col]
    }

    /** Apply `update` to every cell of a `h` by `w` block at (row, col), clipped to the frame so overhanging blocks do not write past it. */
    pub(crate) fn fill(
        &mut self,
        row: usize,
        col: usize,
        h: usize,
        w: usize,
        update: impl Fn(&mut MiInfo),
    ) {
        for r in row..(row + h).min(self.rows) {
            for c in col..(col + w).min(self.cols) {
                update(&mut self.cells[r * self.cols + c]);
            }
        }
    }
}

/** One plane's above or left context: the cumulative level and the dc category of each 4x4 column (or row) of that plane. */
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PlaneEdge {
    pub(crate) level: Vec<u8>,
    pub(crate) dc: Vec<u8>,
}

/** The above or left context for all planes, indexed by absolute plane 4x4 position. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EdgeContexts {
    pub(crate) planes: [PlaneEdge; 3],
}

impl EdgeContexts {
    /** Zeroed contexts long enough for `len` 4x4 units plus the overhang margin. */
    pub(crate) fn new(len: usize) -> Self {
        let edge = PlaneEdge {
            level: vec![0; len + CONTEXT_MARGIN],
            dc: vec![0; len + CONTEXT_MARGIN],
        };
        Self {
            planes: [edge.clone(), edge.clone(), edge],
        }
    }

    /** Zero the entries in `[start, end)` of every plane, the spec's clear_above_context and clear_left_context. */
    pub(crate) fn clear_range(&mut self, start: usize, end: usize) {
        for plane in &mut self.planes {
            let end = end.min(plane.level.len());
            plane.level[start.min(end)..end].fill(0);
            plane.dc[start.min(end)..end].fill(0);
        }
    }
}

/** The transform size of each 4x4 unit of one plane (the spec's LoopfilterTxSizes), kept for the loop filter. Writes past the grid are clipped. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TxSizeGrid {
    rows: usize,
    cols: usize,
    cells: Vec<u8>,
}

impl TxSizeGrid {
    /** A grid of `rows` by `cols` units, every entry zero. */
    pub(crate) fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            cells: vec![0; rows * cols],
        }
    }

    /** The transform size recorded for the unit at (row, col). */
    pub(crate) fn at(&self, row: usize, col: usize) -> u8 {
        self.cells[row * self.cols + col]
    }

    /** Record a transform size over a `h` by `w` run of units starting at (row, col), clipped to the grid. */
    pub(crate) fn fill(&mut self, row: usize, col: usize, h: usize, w: usize, tx_size: u8) {
        for r in row..(row + h).min(self.rows) {
            for c in col..(col + w).min(self.cols) {
                self.cells[r * self.cols + c] = tx_size;
            }
        }
    }
}

/** The CDEF index of each 64x64 luma unit: -1 until a block with coefficients codes one, then the strength index. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CdefGrid {
    cols: usize,
    cells: Vec<i8>,
}

impl CdefGrid {
    /** A grid covering `mi_rows` by `mi_cols` 4x4 units in 64x64 steps, every entry unset. */
    pub(crate) fn new(mi_rows: usize, mi_cols: usize) -> Self {
        let cols = mi_cols.div_ceil(super::constants::CDEF_UNIT_MI);
        let rows = mi_rows.div_ceil(super::constants::CDEF_UNIT_MI);
        Self {
            cols,
            cells: vec![-1; rows * cols],
        }
    }

    /** The index stored for the 64x64 unit that holds 4x4 position (mi_row, mi_col). */
    pub(crate) fn get(&self, mi_row: usize, mi_col: usize) -> i8 {
        self.cells
            .get(self.slot(mi_row, mi_col))
            .copied()
            .unwrap_or(-1)
    }

    /** Store an index for the 64x64 unit holding (mi_row, mi_col); positions past the grid are ignored. */
    pub(crate) fn set(&mut self, mi_row: usize, mi_col: usize, value: i8) {
        let slot = self.slot(mi_row, mi_col);
        if let Some(cell) = self.cells.get_mut(slot) {
            *cell = value;
        }
    }

    /** Index of the 64x64 unit that holds a 4x4 position. */
    fn slot(&self, mi_row: usize, mi_col: usize) -> usize {
        let unit = super::constants::CDEF_UNIT_MI;
        (mi_row / unit) * self.cols + mi_col / unit
    }
}
