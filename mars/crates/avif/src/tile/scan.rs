/** Coefficient scan selection (spec get_scan, get_default_scan, get_mrow_scan and get_mcol_scan): which of the generated scan orders a transform block reads its coefficients in, by transform size and type. */
use super::constants::{
    H_ADST, H_DCT, H_FLIPADST, IDTX, TX_16X64, TX_32X32, TX_64X16, TX_SIZES_ALL, V_ADST, V_DCT,
    V_FLIPADST,
};
use super::scan_tables::*;
use super::tables::TX_SIZE_SQR_UP;

/** The default scan of each transform size, indexed by size. Sizes with a 64-sample side use their 32-sample scan. */
static DEFAULT_SCANS: [&[u16]; TX_SIZES_ALL] = [
    &DEFAULT_SCAN_4X4,
    &DEFAULT_SCAN_8X8,
    &DEFAULT_SCAN_16X16,
    &DEFAULT_SCAN_32X32,
    &DEFAULT_SCAN_32X32,
    &DEFAULT_SCAN_4X8,
    &DEFAULT_SCAN_8X4,
    &DEFAULT_SCAN_8X16,
    &DEFAULT_SCAN_16X8,
    &DEFAULT_SCAN_16X32,
    &DEFAULT_SCAN_32X16,
    &DEFAULT_SCAN_32X32,
    &DEFAULT_SCAN_32X32,
    &DEFAULT_SCAN_4X16,
    &DEFAULT_SCAN_16X4,
    &DEFAULT_SCAN_8X32,
    &DEFAULT_SCAN_32X8,
    &DEFAULT_SCAN_16X32,
    &DEFAULT_SCAN_32X16,
];

/** The row-scan (for vertical-only transforms) of each size. Sizes without one use the 16x4 scan, as the spec's fallback does. */
static MROW_SCANS: [&[u16]; TX_SIZES_ALL] = [
    &MROW_SCAN_4X4,
    &MROW_SCAN_8X8,
    &MROW_SCAN_16X16,
    &MROW_SCAN_16X4,
    &MROW_SCAN_16X4,
    &MROW_SCAN_4X8,
    &MROW_SCAN_8X4,
    &MROW_SCAN_8X16,
    &MROW_SCAN_16X8,
    &MROW_SCAN_16X4,
    &MROW_SCAN_16X4,
    &MROW_SCAN_16X4,
    &MROW_SCAN_16X4,
    &MROW_SCAN_4X16,
    &MROW_SCAN_16X4,
    &MROW_SCAN_16X4,
    &MROW_SCAN_16X4,
    &MROW_SCAN_16X4,
    &MROW_SCAN_16X4,
];

/** The column-scan (for horizontal-only transforms) of each size, with the same fallback. */
static MCOL_SCANS: [&[u16]; TX_SIZES_ALL] = [
    &MCOL_SCAN_4X4,
    &MCOL_SCAN_8X8,
    &MCOL_SCAN_16X16,
    &MCOL_SCAN_16X4,
    &MCOL_SCAN_16X4,
    &MCOL_SCAN_4X8,
    &MCOL_SCAN_8X4,
    &MCOL_SCAN_8X16,
    &MCOL_SCAN_16X8,
    &MCOL_SCAN_16X4,
    &MCOL_SCAN_16X4,
    &MCOL_SCAN_16X4,
    &MCOL_SCAN_16X4,
    &MCOL_SCAN_4X16,
    &MCOL_SCAN_16X4,
    &MCOL_SCAN_16X4,
    &MCOL_SCAN_16X4,
    &MCOL_SCAN_16X4,
    &MCOL_SCAN_16X4,
];

/** The coefficient scan a transform block reads its levels in: a 16x64 or 64x16 block uses its 32-sample scan, any 64-sample block the 32x32 one, IDTX the default, and the 1-D types the row or column scan. */
pub(crate) fn get_scan(tx: usize, tx_type: u8) -> &'static [u16] {
    if tx == TX_16X64 {
        return &DEFAULT_SCAN_16X32;
    }
    if tx == TX_64X16 {
        return &DEFAULT_SCAN_32X16;
    }
    if TX_SIZE_SQR_UP[tx] as usize > TX_32X32 {
        return &DEFAULT_SCAN_32X32;
    }
    if tx_type == IDTX {
        return DEFAULT_SCANS[tx];
    }
    if matches!(tx_type, V_DCT | V_ADST | V_FLIPADST) {
        MROW_SCANS[tx]
    } else if matches!(tx_type, H_DCT | H_ADST | H_FLIPADST) {
        MCOL_SCANS[tx]
    } else {
        DEFAULT_SCANS[tx]
    }
}
