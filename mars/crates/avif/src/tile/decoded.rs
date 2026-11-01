/** BlockDecoded (spec clear_block_decoded_flags): for each plane of a superblock, which 4x4 units of the block to its above, above-right, left and below-left have been decoded. Intra prediction reads these to decide which edge samples exist. Indices run from -1 to the superblock size, stored with an offset of one. */
use super::constants::SB128_MI;

/** Stored side of the flag grid: the superblock's 4x4 count plus the -1 border and the one-past-the-end entry. */
const DECODED_DIM: usize = SB128_MI + 2;

/** The decoded flags of the three planes of one superblock. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BlockDecoded {
    flags: Vec<bool>,
}

impl BlockDecoded {
    /** Every flag clear. */
    pub(crate) fn new() -> Self {
        Self {
            flags: vec![false; 3 * DECODED_DIM * DECODED_DIM],
        }
    }

    /** Whether a 4x4 unit of one plane at (y, x) is decoded; -1 names the border, which is decoded only on the row above and the column to the left. */
    pub(crate) fn get(&self, plane: usize, y: isize, x: isize) -> bool {
        self.flags[Self::slot(plane, y, x)]
    }

    /** Set the flag of a 4x4 unit at (y, x), where either index may be -1 for the border. */
    pub(crate) fn set(&mut self, plane: usize, y: isize, x: isize, value: bool) {
        let slot = Self::slot(plane, y, x);
        self.flags[slot] = value;
    }

    /** Set the flags of one plane for the superblock at a tile position: the row above is decoded up to the tile's right edge, the column to the left up to its bottom edge, and nothing inside. `sb4_y` and `sb4_x` are the superblock size in 4x4 units of that plane, `width4` and `height4` the tile's remaining size in the same units. */
    pub(crate) fn reset_plane(
        &mut self,
        plane: usize,
        sb4_y: isize,
        sb4_x: isize,
        width4: isize,
        height4: isize,
    ) {
        for y in -1..=sb4_y {
            for x in -1..=sb4_x {
                let value = (y < 0 && x < width4) || (x < 0 && y < height4);
                self.set(plane, y, x, value);
            }
        }
        self.set(plane, sb4_y, -1, false);
    }

    /** Flat index of a (y, x) position in one plane, offset so that -1 maps to zero. */
    fn slot(plane: usize, y: isize, x: isize) -> usize {
        let row = (y + 1) as usize;
        let col = (x + 1) as usize;
        (plane * DECODED_DIM + row) * DECODED_DIM + col
    }
}
