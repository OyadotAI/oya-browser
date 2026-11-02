/** Context selection for coefficient syntax (spec 8.3.2 for all_zero, coeff_base, coeff_base_eob, coeff_br and dc_sign): the neighbourhood sums that choose which adaptive CDF a coefficient level or sign is read with. */
use super::constants::{
    COEFF_BASE_POS_CTX_OFFSET, COEFF_BR_FAR_OFFSET, COEFF_BR_MAG_CAP, COEFF_BR_MAG_LIMIT,
    COEFF_BR_NEAR_OFFSET, SIG_REF_DIFF_OFFSET_NUM, TX_CLASS_2D, TX_CLASS_HORIZ, TX_CLASS_VERT,
};
use super::decoder::TileDecoder;
use super::lr::plane_subsampling;
use super::tables::{
    ADJUSTED_TX_SIZE, COEFF_BASE_CTX_OFFSET, MAG_REF_OFFSET_WITH_TX_CLASS, NUM_4X4_BLOCKS_HIGH,
    NUM_4X4_BLOCKS_WIDE, SIG_REF_DIFF_OFFSET, TX_HEIGHT, TX_WIDTH, TX_WIDTH_LOG2,
};

/** Largest neighbour magnitude counted into a coeff_base context. */
const COEFF_BASE_MAG_CAP: i32 = 3;
/** Largest context a coeff_base neighbourhood sum contributes. */
const COEFF_BASE_SUM_CAP: i32 = 4;
/** Coefficient-area fractions that split the coeff_base_eob contexts: one eighth and one quarter. */
const EOB_AREA_EIGHTH: usize = 8;
const EOB_AREA_QUARTER: usize = 4;
/** Row and column bounds of the coeff_base position table, and the index bound of the positional offsets. */
const POS_TABLE_CAP: usize = 4;
const POS_OFFSET_CAP: usize = 2;
/** coeff_br: the rows and columns counted as near the origin for the two-dimensional class. */
const BR_NEAR_ROWS: usize = 2;

/** The transform class of a transform type: vertical-only, horizontal-only, or two-dimensional. */
pub(crate) fn tx_class(tx_type: u8) -> usize {
    match tx_type {
        super::constants::V_DCT | super::constants::V_ADST | super::constants::V_FLIPADST => {
            TX_CLASS_VERT
        }
        super::constants::H_DCT | super::constants::H_ADST | super::constants::H_FLIPADST => {
            TX_CLASS_HORIZ
        }
        _ => TX_CLASS_2D,
    }
}

/** Log2 width, width and height of a transform as coefficients are stored for it, where 64-sample sides are coded as 32. */
fn adjusted_dims(tx: usize) -> (usize, usize, usize) {
    let adj = ADJUSTED_TX_SIZE[tx] as usize;
    let bwl = TX_WIDTH_LOG2[adj] as usize;
    (bwl, 1 << bwl, TX_HEIGHT[adj] as usize)
}

/** The context of coeff_base_eob: where the last coefficient falls in the scan, by fractions of the coded area. */
pub(crate) fn coeff_base_eob_ctx(c: usize, tx: usize) -> usize {
    let (bwl, _, height) = adjusted_dims(tx);
    let area = height << bwl;
    if c == 0 {
        0
    } else if c <= area / EOB_AREA_EIGHTH {
        1
    } else if c <= area / EOB_AREA_QUARTER {
        2
    } else {
        3
    }
}

/** The context of coeff_base: the capped magnitudes of the neighbours that the transform class reads, plus a position offset. */
pub(crate) fn coeff_base_ctx(quant: &[i32], tx: usize, class: usize, pos: usize) -> usize {
    let (bwl, width, height) = adjusted_dims(tx);
    let (row, col) = (pos >> bwl, pos & (width - 1));
    let mut mag = 0i32;
    for offset in SIG_REF_DIFF_OFFSET[class]
        .iter()
        .take(SIG_REF_DIFF_OFFSET_NUM)
    {
        let rr = row as isize + offset[0] as isize;
        let cc = col as isize + offset[1] as isize;
        if rr >= 0 && cc >= 0 && (rr as usize) < height && (cc as usize) < width {
            let value = quant[((rr as usize) << bwl) + cc as usize].abs();
            mag += value.min(COEFF_BASE_MAG_CAP);
        }
    }
    let ctx = ((mag + 1) >> 1).min(COEFF_BASE_SUM_CAP) as usize;
    if class == TX_CLASS_2D {
        if row == 0 && col == 0 {
            return 0;
        }
        return ctx
            + COEFF_BASE_CTX_OFFSET[tx][row.min(POS_TABLE_CAP)][col.min(POS_TABLE_CAP)] as usize;
    }
    let idx = if class == TX_CLASS_VERT { row } else { col };
    ctx + COEFF_BASE_POS_CTX_OFFSET[idx.min(POS_OFFSET_CAP)]
}

/** The context of coeff_br: the capped magnitudes of the neighbours that the class reads, then whether the position is at the origin, near it, or along the class's edge. */
pub(crate) fn coeff_br_ctx(quant: &[i32], tx: usize, class: usize, pos: usize) -> usize {
    let (bwl, width, height) = adjusted_dims(tx);
    let (row, col) = (pos >> bwl, pos & (width - 1));
    let mut mag = 0u32;
    for offset in MAG_REF_OFFSET_WITH_TX_CLASS[class].iter() {
        let rr = row + offset[0] as usize;
        let cc = col + offset[1] as usize;
        if rr < height && cc < width {
            mag += (quant[rr * width + cc].max(0) as u32).min(COEFF_BR_MAG_CAP);
        }
    }
    let mag = ((mag + 1) >> 1).min(COEFF_BR_MAG_LIMIT) as usize;
    if pos == 0 {
        return mag;
    }
    let near = match class {
        TX_CLASS_2D => row < BR_NEAR_ROWS && col < BR_NEAR_ROWS,
        TX_CLASS_HORIZ => col == 0,
        _ => row == 0,
    };
    mag + if near {
        COEFF_BR_NEAR_OFFSET
    } else {
        COEFF_BR_FAR_OFFSET
    }
}

impl<'a> TileDecoder<'a> {
    /** The all_zero context of a transform block: the largest level above and to the left, or the coded dc categories for chroma. */
    pub(super) fn all_zero_ctx(
        &self,
        plane: usize,
        tx: usize,
        x4: usize,
        y4: usize,
        w4: usize,
        h4: usize,
    ) -> usize {
        let (sub_x, sub_y) = plane_subsampling(self.seq, plane);
        let max_x4 = self.mi_cols() >> sub_x;
        let max_y4 = self.mi_rows() >> sub_y;
        let (w, h) = (TX_WIDTH[tx] as usize, TX_HEIGHT[tx] as usize);
        let bsize = self.plane_residual_size(self.blk.mi_size, plane);
        let bw = NUM_4X4_BLOCKS_WIDE[bsize] as usize * super::constants::MI_SIZE;
        let bh = NUM_4X4_BLOCKS_HIGH[bsize] as usize * super::constants::MI_SIZE;
        let above = &self.above.planes[plane];
        let left = &self.left.planes[plane];
        if plane == 0 {
            let top = (0..w4)
                .filter(|k| x4 + k < max_x4)
                .map(|k| above.level[x4 + k])
                .max()
                .unwrap_or(0);
            let lft = (0..h4)
                .filter(|k| y4 + k < max_y4)
                .map(|k| left.level[y4 + k])
                .max()
                .unwrap_or(0);
            return luma_all_zero_ctx(top, lft, bw == w && bh == h);
        }
        let any_above = (0..w4)
            .filter(|i| x4 + i < max_x4)
            .any(|i| above.level[x4 + i] != 0 || above.dc[x4 + i] != 0);
        let any_left = (0..h4)
            .filter(|i| y4 + i < max_y4)
            .any(|i| left.level[y4 + i] != 0 || left.dc[y4 + i] != 0);
        let mut ctx = usize::from(any_above) + usize::from(any_left) + 7;
        if bw * bh > w * h {
            ctx += 3;
        }
        ctx
    }

    /** The dc_sign context: whether the coded dc categories above and to the left lean negative, positive, or balance. */
    pub(super) fn dc_sign_ctx(
        &self,
        plane: usize,
        x4: usize,
        y4: usize,
        w4: usize,
        h4: usize,
    ) -> usize {
        let (sub_x, sub_y) = plane_subsampling(self.seq, plane);
        let max_x4 = self.mi_cols() >> sub_x;
        let max_y4 = self.mi_rows() >> sub_y;
        let above = &self.above.planes[plane].dc;
        let left = &self.left.planes[plane].dc;
        let mut sum = 0i32;
        for k in (0..w4).filter(|k| x4 + k < max_x4) {
            sum += dc_sign_value(above[x4 + k]);
        }
        for k in (0..h4).filter(|k| y4 + k < max_y4) {
            sum += dc_sign_value(left[y4 + k]);
        }
        match sum {
            s if s < 0 => 1,
            s if s > 0 => 2,
            _ => 0,
        }
    }
}

/** The luma all_zero context from the largest neighbouring levels and whether the block is exactly the transform. */
fn luma_all_zero_ctx(top: u8, left: u8, exact: bool) -> usize {
    let (top, left) = (usize::from(top), usize::from(left));
    if exact {
        0
    } else if top == 0 && left == 0 {
        1
    } else if top == 0 || left == 0 {
        2 + usize::from(top.max(left) > 3)
    } else if top.max(left) <= 3 {
        4
    } else if top.min(left) <= 3 {
        5
    } else {
        6
    }
}

/** A neighbouring dc category as a signed vote: one counts against, two for. */
fn dc_sign_value(category: u8) -> i32 {
    match category {
        super::constants::DC_CATEGORY_NEG => -1,
        super::constants::DC_CATEGORY_POS => 1,
        _ => 0,
    }
}
