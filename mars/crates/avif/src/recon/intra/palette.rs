/** Palette prediction (spec 7.11.4): each sample of a palette block is the palette colour its colour index names. The colour-index map is coded once per block and is read here at the transform block's offset. */
use crate::recon::Plane;

/** The palette of one plane, the colour-index map that covers the block with its row stride, and the transform block's place inside the block in 4x4 units. */
#[derive(Clone, Copy, Debug)]
pub(crate) struct PaletteBlock<'a> {
    pub(crate) colors: &'a [u16],
    pub(crate) map: &'a [u8],
    pub(crate) stride: usize,
    pub(crate) x4: usize,
    pub(crate) y4: usize,
}

/** Write the palette prediction of a `w` by `h` transform block at (start_x, start_y). */
pub(crate) fn predict(
    plane: &mut Plane,
    start_x: usize,
    start_y: usize,
    w: usize,
    h: usize,
    p: &PaletteBlock<'_>,
) {
    for i in 0..h {
        let row = (p.y4 * 4 + i) * p.stride + p.x4 * 4;
        let out = plane.row_mut(start_y + i);
        for j in 0..w {
            out[start_x + j] = p.colors[usize::from(p.map[row + j])];
        }
    }
}
