/** The source samples of loop restoration (spec 7.17.6): inside the current 64-row stripe they come from the CDEF output, and up to two rows beyond it from the deblocked picture, with coordinates clamped to the plane. A block fetches its neighbourhood once into a window, so the filters index plain memory. */
use crate::recon::Plane;

/** How far past a block the filters read: three rows or columns for the widest filter, plus the one-sample border of the box sums. */
pub(super) const MARGIN: isize = 4;
/** How far past the stripe the deblocked picture is read: the spec fetches two rows and repeats the second for further rows. */
const STRIPE_REACH: isize = 2;

/** The two pictures restoration reads, and the plane's extent and the stripe the current block is in. */
#[derive(Clone, Copy)]
pub(super) struct Source<'a> {
    pub(super) cur: &'a Plane,
    pub(super) cdef: &'a Plane,
    pub(super) end_x: isize,
    pub(super) end_y: isize,
    pub(super) stripe_start: isize,
    pub(super) stripe_end: isize,
}

impl Source<'_> {
    /** get_source_sample(x, y): the sample the filters see at a plane position, with clamping to the plane and the stripe rules. */
    fn sample(&self, x: isize, y: isize) -> i32 {
        let x = x.clamp(0, self.end_x) as usize;
        let y = y.clamp(0, self.end_y);
        if y < self.stripe_start {
            let y = y.max(self.stripe_start - STRIPE_REACH) as usize;
            self.cur.get(x, y)
        } else if y > self.stripe_end {
            let y = y.min(self.stripe_end + STRIPE_REACH) as usize;
            self.cur.get(x, y)
        } else {
            self.cdef.get(x, y as usize)
        }
    }
}

/** The source samples of one block and its margin, row-major with a stride that covers the margin on both sides. */
pub(super) struct Window {
    x0: isize,
    y0: isize,
    stride: usize,
    data: Vec<i32>,
}

impl Window {
    /** Fetch the `w` by `h` block at (x0, y0) and its margin from the source. */
    pub(super) fn fetch(src: &Source, x0: isize, y0: isize, w: usize, h: usize) -> Self {
        let stride = w + 2 * MARGIN as usize;
        let rows = h + 2 * MARGIN as usize;
        let (wx, wy) = (x0 - MARGIN, y0 - MARGIN);
        let mut data = Vec::with_capacity(stride * rows);
        for j in 0..rows as isize {
            for i in 0..stride as isize {
                data.push(src.sample(wx + i, wy + j));
            }
        }
        Self {
            x0: wx,
            y0: wy,
            stride,
            data,
        }
    }

    /** The sample at a plane position within the window (the block and its margin). */
    pub(super) fn at(&self, x: isize, y: isize) -> i32 {
        let i = (y - self.y0) as usize * self.stride + (x - self.x0) as usize;
        self.data[i]
    }
}
