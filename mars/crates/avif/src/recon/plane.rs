/** One plane of reconstructed samples (the spec's CurrFrame[ plane ]): a row-major grid of u16 values whose size is padded to the superblock grid, so that a block overhanging the frame edge can be written without a bounds check per sample. */
/** A sample grid of `width` by `height` samples, stored row-major with a stride equal to the width. */
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Plane {
    width: usize,
    height: usize,
    data: Vec<u16>,
}

impl Plane {
    /** A plane of zeroed samples. */
    pub(crate) fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![0; width * height],
        }
    }

    /** The width of the grid in samples. */
    #[cfg(test)]
    pub(crate) fn width(&self) -> usize {
        self.width
    }

    /** The height of the grid in samples. */
    #[cfg(test)]
    pub(crate) fn height(&self) -> usize {
        self.height
    }

    /** The sample at (x, y), widened to i32 for the predictors. Callers stay inside the grid, which the superblock padding guarantees. */
    pub(crate) fn get(&self, x: usize, y: usize) -> i32 {
        i32::from(self.data[y * self.width + x])
    }

    /** Store a sample at (x, y). */
    pub(crate) fn set(&mut self, x: usize, y: usize, value: u16) {
        self.data[y * self.width + x] = value;
    }

    /** One full row of samples. */
    #[cfg(test)]
    pub(crate) fn row(&self, y: usize) -> &[u16] {
        &self.data[y * self.width..(y + 1) * self.width]
    }

    /** One full row of samples, mutably. */
    pub(crate) fn row_mut(&mut self, y: usize) -> &mut [u16] {
        &mut self.data[y * self.width..(y + 1) * self.width]
    }
}
