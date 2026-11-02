/** The edge samples a block is predicted from (spec 7.11.2, the AboveRow and LeftCol arrays): the reconstructed row above and column to the left of a transform block, with the substitutions the spec makes at the tile, frame and decode-order edges. Entries are addressed from -2 so that the upsampling and filters can write just before the start. */
use super::Request;
use crate::recon::Plane;

/** Index offset of entry 0 in the edge arrays, leaving room for the -1 and -2 entries the filters and upsampling use. */
pub(super) const EDGE_OFFSET: usize = 16;
/** Length of an edge array: the largest edge is 2 * (64 + 64) samples, which upsampling and the directional reads can reach, with the offset on top. */
pub(super) const EDGE_LEN: usize = 2 * 128 + 2 * EDGE_OFFSET;

/** The above row and the left column of one block, each addressed from index -2 through the largest sample index a predictor reads. */
#[derive(Clone, Debug)]
pub(super) struct Edges {
    above: Vec<i32>,
    left: Vec<i32>,
}

impl Edges {
    /** Edges with every entry zero, to be filled by build or by the edge filters. */
    pub(super) fn zeroed() -> Self {
        Self {
            above: vec![0; EDGE_LEN],
            left: vec![0; EDGE_LEN],
        }
    }

    /** AboveRow[i] for i from -2 up. */
    pub(super) fn above(&self, i: isize) -> i32 {
        self.above[slot(i)]
    }

    /** LeftCol[i] for i from -2 up. */
    pub(super) fn left(&self, i: isize) -> i32 {
        self.left[slot(i)]
    }

    /** Set AboveRow[i]. */
    pub(super) fn set_above(&mut self, i: isize, value: i32) {
        self.above[slot(i)] = value;
    }

    /** Set LeftCol[i]. */
    pub(super) fn set_left(&mut self, i: isize, value: i32) {
        self.left[slot(i)] = value;
    }

    /** The whole above array, for the edge filter, which reads and rewrites a window of it. */
    pub(super) fn above_mut(&mut self) -> &mut [i32] {
        &mut self.above
    }

    /** The whole left array, mutably. */
    pub(super) fn left_mut(&mut self) -> &mut [i32] {
        &mut self.left
    }
}

/** Position of entry i in an edge array. */
fn slot(i: isize) -> usize {
    (i + EDGE_OFFSET as isize) as usize
}

/** Fill AboveRow and LeftCol for a block from the plane (the first part of the intra prediction process): the substitutions when a side is unavailable, the clamp to the last decoded column or row, and the corner sample. */
pub(super) fn build(plane: &Plane, req: &Request) -> Edges {
    let mut e = Edges::zeroed();
    let (x, y) = (req.x, req.y);
    let count = (1usize << req.log2w) + (1usize << req.log2h);
    let mid = 1i32 << (req.bit_depth - 1);
    fill_above(plane, req, &mut e, count, mid);
    fill_left(plane, req, &mut e, count, mid);
    let corner = match (req.have_above, req.have_left) {
        (true, true) => plane.get(x - 1, y - 1),
        (true, false) => plane.get(x, y - 1),
        (false, true) => plane.get(x - 1, y),
        (false, false) => mid,
    };
    e.set_above(-1, corner);
    e.set_left(-1, corner);
    e
}

/** AboveRow[0..w+h]: the row above, read up to the last decoded column, or the substitute when there is no row above. */
fn fill_above(plane: &Plane, req: &Request, e: &mut Edges, count: usize, mid: i32) {
    let (x, y) = (req.x, req.y);
    let w = 1usize << req.log2w;
    for i in 0..count {
        let value = if !req.have_above && req.have_left {
            plane.get(x - 1, y)
        } else if !req.have_above {
            mid - 1
        } else {
            let reach = if req.have_above_right { 2 * w } else { w };
            let limit = req.max_x.min(x + reach - 1);
            plane.get(limit.min(x + i), y - 1)
        };
        e.set_above(i as isize, value);
    }
}

/** LeftCol[0..w+h]: the column to the left, read down to the last decoded row, or the substitute when there is no column to the left. */
fn fill_left(plane: &Plane, req: &Request, e: &mut Edges, count: usize, mid: i32) {
    let (x, y) = (req.x, req.y);
    let h = 1usize << req.log2h;
    for i in 0..count {
        let value = if !req.have_left && req.have_above {
            plane.get(x, y - 1)
        } else if !req.have_left {
            mid + 1
        } else {
            let reach = if req.have_below_left { 2 * h } else { h };
            let limit = req.max_y.min(y + reach - 1);
            plane.get(x - 1, limit.min(y + i))
        };
        e.set_left(i as isize, value);
    }
}
