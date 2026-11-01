/** The reconstruction of one transform block (spec 7.12.3): the sparse dequantised coefficients are laid out densely, inverse transformed, and the residual is added to the prediction already in the plane, with the flips applied to where each sample lands. */
use super::inverse::{Geometry, inverse_2d};
use super::plane::Plane;
use super::txkind::tx_kinds;

/** Where a transform block sits, what transform it uses and how its samples are coded. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResidualSpec {
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) geom: Geometry,
    pub(crate) tx_type: u8,
    pub(crate) lossless: bool,
    pub(crate) bit_depth: u32,
}

/** The dequantised coefficients of a block as sparse (raster position, value) pairs. Each position is row-major over the block's coded size, which is at most 32 samples in each direction. */
pub(crate) type Dequant<'a> = &'a [(u16, i32)];

/** Number of coded coefficients of a block of the given log2 side, capped at 32 samples per side. */
fn coded_len(geom: Geometry) -> usize {
    let tw = (1usize << geom.log2w).min(32);
    let th = (1usize << geom.log2h).min(32);
    tw * th
}

/** Lay the sparse coefficients out densely in the coded size, the spec's Dequant array, zero everywhere else. */
fn densify(coeffs: Dequant<'_>, geom: Geometry) -> Vec<i32> {
    let mut dense = vec![0i32; coded_len(geom)];
    for &(pos, value) in coeffs {
        dense[usize::from(pos)] = value;
    }
    dense
}

/** Add the residual of a block into the plane at the block's position. A vertical flip reverses the row a residual row lands on, and a horizontal flip the column. Samples are clipped to the bit depth. */
pub(super) fn add_residual(plane: &mut Plane, spec: &ResidualSpec, residual: &[i32]) {
    let w = 1usize << spec.geom.log2w;
    let h = 1usize << spec.geom.log2h;
    let kinds = tx_kinds(spec.tx_type);
    let max = (1i32 << spec.bit_depth) - 1;
    for i in 0..h {
        let yy = if kinds.flip_ud { h - i - 1 } else { i };
        let row = plane.row_mut(spec.y + yy);
        for j in 0..w {
            let xx = if kinds.flip_lr { w - j - 1 } else { j };
            let sample = &mut row[spec.x + xx];
            *sample = (i32::from(*sample) + residual[i * w + j]).clamp(0, max) as u16;
        }
    }
}

/** Reconstruct one transform block: the coefficients are inverse transformed and added to the samples already predicted at the block's position in `plane`. */
pub(crate) fn reconstruct(plane: &mut Plane, spec: &ResidualSpec, coeffs: Dequant<'_>) {
    let dense = densify(coeffs, spec.geom);
    let kinds = tx_kinds(spec.tx_type);
    let residual = inverse_2d(&dense, spec.geom, kinds, spec.lossless, spec.bit_depth);
    add_residual(plane, spec, &residual);
}
