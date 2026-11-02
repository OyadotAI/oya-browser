/** The 2-D inverse transform of one transform block (spec 7.13.3): row transforms, a clamp, column transforms, and the final rounding. Lossless blocks use the Walsh-Hadamard transform in both passes. */
use super::adst;
use super::dct;
use super::identity;
use super::math::{clip3, round2};
use super::tables::TRANSFORM_ROW_SHIFT;
use super::txkind::{Kind, TxKinds};

/** The row shift of a transform size (spec Transform_Row_Shift), which keeps the row outputs within the column clamping range. */
pub(crate) fn row_shift(tx_size: usize) -> u32 {
    u32::from(TRANSFORM_ROW_SHIFT[tx_size])
}

/** The geometry of one transform block: its log2 width and height, and the row shift the size takes from Transform_Row_Shift. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Geometry {
    pub(crate) log2w: u32,
    pub(crate) log2h: u32,
    pub(crate) row_shift: u32,
}

/** The largest transform side that carries coefficients; sizes above it are zero beyond row and column 32. */
const COEFF_SIDE: usize = 32;
/** Shift applied to the column outputs of a lossy block. */
const COL_SHIFT: u32 = 4;
/** Pre-shift the lossless row pass applies to its inputs. */
const WHT_ROW_SHIFT: u32 = 2;

/** A 1-D transform applied in place to the first 2^n entries of an array with clamping range r. */
type OneD = fn(&mut [i32], u32, u32);

/** The 1-D transforms indexed by Kind: DCT, ADST and identity. */
const ONE_D: [OneD; 3] = [dct_1d, adst_1d, identity_1d];

/** The DCT as a 1-D transform in the shape the table expects. */
fn dct_1d(t: &mut [i32], n: u32, r: u32) {
    dct::inverse(t, n, r);
}

/** The ADST as a 1-D transform. A size the type does not allow (ADST above 16 samples) is read as a DCT, which the transform-set rules make unreachable for a conforming stream. */
fn adst_1d(t: &mut [i32], n: u32, r: u32) {
    if n <= 4 {
        adst::inverse_any(t, n, r);
    } else {
        dct::inverse(t, n, r);
    }
}

/** The identity transform as a 1-D transform; it takes no clamping range. */
fn identity_1d(t: &mut [i32], n: u32, _r: u32) {
    identity::inverse(t, n);
}

/** Apply the 1-D transform a kind names to the first 2^n entries of t. */
fn apply(kind: Kind, t: &mut [i32], n: u32, r: u32) {
    ONE_D[kind as usize](t, n, r);
}

/** Ranges of the clamps: rows clamp to bit depth plus eight bits, columns to the larger of bit depth plus six and sixteen. */
fn row_range(bit_depth: u32) -> u32 {
    bit_depth + 8
}

/** Column clamping range: max(BitDepth + 6, 16). */
fn col_range(bit_depth: u32) -> u32 {
    (bit_depth + 6).max(16)
}

/** Whether the block is 2:1 rectangular, which scales its row inputs by 1/sqrt(2) before the row pass. */
fn is_two_to_one(geom: Geometry) -> bool {
    geom.log2w.abs_diff(geom.log2h) == 1
}

/** The row pass over every row of the block: each row's coefficients are scaled and transformed, shifted by the row shift, and clamped to the column range. */
fn row_pass(
    coef: &[i32],
    tw: usize,
    geom: Geometry,
    kinds: TxKinds,
    lossless: bool,
    bit_depth: u32,
) -> Vec<i32> {
    let (w, h) = (1usize << geom.log2w, 1usize << geom.log2h);
    let clamp = col_range(bit_depth);
    let (low, high) = (-(1i64 << (clamp - 1)), (1i64 << (clamp - 1)) - 1);
    let mut out = vec![0i32; w * h];
    for i in 0..h {
        let mut t = [0i32; 64];
        if i < COEFF_SIDE {
            for j in 0..w.min(COEFF_SIDE) {
                t[j] = coef[i * tw + j];
            }
        }
        if is_two_to_one(geom) {
            for v in &mut t[..w] {
                *v = round2(i64::from(*v) * 2896, 12) as i32;
            }
        }
        if lossless {
            super::identity::wht(&mut t, WHT_ROW_SHIFT);
        } else {
            apply(kinds.row, &mut t, geom.log2w, row_range(bit_depth));
        }
        for j in 0..w {
            let v = round2(i64::from(t[j]), geom.row_shift);
            out[i * w + j] = clip3(low, high, v) as i32;
        }
    }
    out
}

/** The column pass over every column of the residual, in place: each column is transformed and rounded by the column shift. */
fn column_pass(res: &mut [i32], geom: Geometry, kinds: TxKinds, lossless: bool, bit_depth: u32) {
    let (w, h) = (1usize << geom.log2w, 1usize << geom.log2h);
    let col_shift = if lossless { 0 } else { COL_SHIFT };
    for j in 0..w {
        let mut t = [0i32; 64];
        for i in 0..h {
            t[i] = res[i * w + j];
        }
        if lossless {
            super::identity::wht(&mut t, 0);
        } else {
            apply(kinds.col, &mut t, geom.log2h, col_range(bit_depth));
        }
        for i in 0..h {
            res[i * w + j] = round2(i64::from(t[i]), col_shift) as i32;
        }
    }
}

/** The residual of one transform block: `coef` holds the dequantised coefficients row-major with a stride of min(32, width), and the result is the block's full width by height residual, row-major. The lossless flag selects the Walsh-Hadamard transform and zero shifts. */
pub(crate) fn inverse_2d(
    coef: &[i32],
    geom: Geometry,
    kinds: TxKinds,
    lossless: bool,
    bit_depth: u32,
) -> Vec<i32> {
    let tw = (1usize << geom.log2w).min(COEFF_SIDE);
    let geom = if lossless {
        Geometry {
            row_shift: 0,
            ..geom
        }
    } else {
        geom
    };
    let mut res = row_pass(coef, tw, geom, kinds, lossless, bit_depth);
    column_pass(&mut res, geom, kinds, lossless, bit_depth);
    res
}
