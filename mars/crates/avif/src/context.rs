/** Per-tile CDF context (spec init_non_coeff_cdfs and init_coeff_cdfs): the adaptive copies of every CDF an intra frame's tile reads. Each tile starts from the defaults; the context saved at context_update_tile_id is a clone taken at tile end. */
use crate::cdf_tables::*;
use crate::tile::{MV_COMPONENTS, MV_CONTEXTS, MV_OFFSET_BITS};

/** Upper bounds of base_q_idx for the first three coefficient contexts; anything above falls in the last one. */
const Q_CTX_LIMITS: [u8; 3] = [20, 60, 120];

/** The coefficient-table index for a base quantizer index: 0 up to 20, 1 up to 60, 2 up to 120, else 3. */
pub(crate) fn coeff_q_context(base_q_idx: u8) -> usize {
    Q_CTX_LIMITS
        .iter()
        .position(|&limit| base_q_idx <= limit)
        .unwrap_or(Q_CTX_LIMITS.len())
}

/** Padded palette colour CDFs for one plane, one entry per palette size from 2 to 8. */
type PaletteColorDefaults = [[[u16; 9]; 5]; 7];

/** Default colour-index CDFs for luma palettes, padded to nine entries per row. */
static PALETTE_Y_COLOR_DEFAULTS: PaletteColorDefaults = [
    pad_row(&DEFAULT_PALETTE_SIZE_2_Y_COLOR_CDF),
    pad_row(&DEFAULT_PALETTE_SIZE_3_Y_COLOR_CDF),
    pad_row(&DEFAULT_PALETTE_SIZE_4_Y_COLOR_CDF),
    pad_row(&DEFAULT_PALETTE_SIZE_5_Y_COLOR_CDF),
    pad_row(&DEFAULT_PALETTE_SIZE_6_Y_COLOR_CDF),
    pad_row(&DEFAULT_PALETTE_SIZE_7_Y_COLOR_CDF),
    pad_row(&DEFAULT_PALETTE_SIZE_8_Y_COLOR_CDF),
];

/** Default colour-index CDFs for chroma palettes, padded to nine entries per row. */
static PALETTE_UV_COLOR_DEFAULTS: PaletteColorDefaults = [
    pad_row(&DEFAULT_PALETTE_SIZE_2_UV_COLOR_CDF),
    pad_row(&DEFAULT_PALETTE_SIZE_3_UV_COLOR_CDF),
    pad_row(&DEFAULT_PALETTE_SIZE_4_UV_COLOR_CDF),
    pad_row(&DEFAULT_PALETTE_SIZE_5_UV_COLOR_CDF),
    pad_row(&DEFAULT_PALETTE_SIZE_6_UV_COLOR_CDF),
    pad_row(&DEFAULT_PALETTE_SIZE_7_UV_COLOR_CDF),
    pad_row(&DEFAULT_PALETTE_SIZE_8_UV_COLOR_CDF),
];

/** Copy one size's colour CDFs into the padded layout; the unused tail stays zero and is never read. */
const fn pad_row<const N: usize>(src: &[[u16; N]; 5]) -> [[u16; 9]; 5] {
    let mut out = [[0u16; 9]; 5];
    let mut ctx = 0;
    while ctx < 5 {
        let mut i = 0;
        while i < N {
            out[ctx][i] = src[ctx][i];
            i += 1;
        }
        ctx += 1;
    }
    out
}

/** The adaptive CDFs of one motion vector context: the joint, and for each component its sign, class, class-0 bit and offset bits. */
#[derive(Clone)]
pub(crate) struct MvCdfs {
    pub(crate) joint: [u16; 5],
    pub(crate) sign: [[u16; 3]; MV_COMPONENTS],
    pub(crate) class: [[u16; 12]; MV_COMPONENTS],
    pub(crate) class0_bit: [[u16; 3]; MV_COMPONENTS],
    pub(crate) bits: [[[u16; 3]; MV_OFFSET_BITS]; MV_COMPONENTS],
}

impl MvCdfs {
    /** The defaults every motion vector context starts from (spec init_non_coeff_cdfs). */
    fn new() -> Self {
        Self {
            joint: DEFAULT_MV_JOINT_CDF,
            sign: [DEFAULT_MV_SIGN_CDF; MV_COMPONENTS],
            class: DEFAULT_MV_CLASS_CDF,
            class0_bit: [DEFAULT_MV_CLASS0_BIT_CDF; MV_COMPONENTS],
            bits: [DEFAULT_MV_BIT_CDF; MV_COMPONENTS],
        }
    }
}

/** The adaptive CDFs an intra frame tile reads. Coefficient tables are stored already selected by the frame's quantizer context. */
#[derive(Clone)]
pub(crate) struct TileCdfs {
    pub(crate) intra_frame_y_mode: [[[u16; 14]; 5]; 5],
    pub(crate) y_mode: [[u16; 14]; 4],
    pub(crate) uv_mode_cfl_not_allowed: [[u16; 14]; 13],
    pub(crate) uv_mode_cfl_allowed: [[u16; 15]; 13],
    pub(crate) angle_delta: [[u16; 8]; 8],
    pub(crate) intrabc: [u16; 3],
    pub(crate) partition_w8: [[u16; 5]; 4],
    pub(crate) partition_w16: [[u16; 11]; 4],
    pub(crate) partition_w32: [[u16; 11]; 4],
    pub(crate) partition_w64: [[u16; 11]; 4],
    pub(crate) partition_w128: [[u16; 9]; 4],
    pub(crate) skip: [[u16; 3]; 3],
    pub(crate) segment_id: [[u16; 9]; 3],
    pub(crate) tx_8x8: [[u16; 3]; 3],
    pub(crate) tx_16x16: [[u16; 4]; 3],
    pub(crate) tx_32x32: [[u16; 4]; 3],
    pub(crate) tx_64x64: [[u16; 4]; 3],
    pub(crate) txfm_split: [[u16; 3]; 21],
    /** Motion vector CDFs by motion vector context (ordinary vectors and intra block copy). */
    pub(crate) mv: [MvCdfs; MV_CONTEXTS],
    /** Inter transform-type CDFs: set 1 and set 3 by square size, set 2 once. */
    pub(crate) inter_tx_type_set1: [[u16; 17]; 2],
    pub(crate) inter_tx_type_set2: [u16; 13],
    pub(crate) inter_tx_type_set3: [[u16; 3]; 4],
    pub(crate) filter_intra_mode: [u16; 6],
    pub(crate) filter_intra: [[u16; 3]; 22],
    pub(crate) palette_uv_mode: [[u16; 3]; 2],
    pub(crate) palette_y_mode: [[[u16; 3]; 3]; 7],
    pub(crate) palette_y_size: [[u16; 8]; 7],
    pub(crate) palette_uv_size: [[u16; 8]; 7],
    /** Palette colour-index CDFs by palette size minus two, then colour context. Each row is padded to nine entries; a size-N read uses the first N + 1. */
    pub(crate) palette_y_color: [[[u16; 9]; 5]; 7],
    pub(crate) palette_uv_color: [[[u16; 9]; 5]; 7],
    pub(crate) delta_q: [u16; 5],
    pub(crate) delta_lf: [u16; 5],
    pub(crate) use_wiener: [u16; 3],
    pub(crate) use_sgrproj: [u16; 3],
    pub(crate) restoration_type: [u16; 4],
    /** One delta_lf CDF per loop filter strength, used when delta_lf_multi is set. */
    pub(crate) delta_lf_multi: [[u16; 5]; 4],
    pub(crate) intra_tx_type_set1: [[[u16; 8]; 13]; 2],
    pub(crate) intra_tx_type_set2: [[[u16; 6]; 13]; 3],
    pub(crate) cfl_sign: [u16; 9],
    pub(crate) cfl_alpha: [[u16; 17]; 6],
    pub(crate) txb_skip: [[[u16; 3]; 13]; 5],
    pub(crate) eob_pt_16: [[[u16; 6]; 2]; 2],
    pub(crate) eob_pt_32: [[[u16; 7]; 2]; 2],
    pub(crate) eob_pt_64: [[[u16; 8]; 2]; 2],
    pub(crate) eob_pt_128: [[[u16; 9]; 2]; 2],
    pub(crate) eob_pt_256: [[[u16; 10]; 2]; 2],
    pub(crate) eob_pt_512: [[u16; 11]; 2],
    pub(crate) eob_pt_1024: [[u16; 12]; 2],
    pub(crate) eob_extra: [[[[u16; 3]; 9]; 2]; 5],
    pub(crate) dc_sign: [[[u16; 3]; 3]; 2],
    pub(crate) coeff_base_eob: [[[[u16; 4]; 4]; 2]; 5],
    pub(crate) coeff_base: [[[[u16; 5]; 42]; 2]; 5],
    pub(crate) coeff_br: [[[[u16; 5]; 21]; 2]; 5],
}

impl TileCdfs {
    /** The default context for a frame whose base quantizer index is `base_q_idx`. Every tile of a frame starts from this. */
    pub(crate) fn new(base_q_idx: u8) -> Self {
        let q = coeff_q_context(base_q_idx);
        Self {
            intra_frame_y_mode: DEFAULT_INTRA_FRAME_Y_MODE_CDF,
            y_mode: DEFAULT_Y_MODE_CDF,
            uv_mode_cfl_not_allowed: DEFAULT_UV_MODE_CFL_NOT_ALLOWED_CDF,
            uv_mode_cfl_allowed: DEFAULT_UV_MODE_CFL_ALLOWED_CDF,
            angle_delta: DEFAULT_ANGLE_DELTA_CDF,
            intrabc: DEFAULT_INTRABC_CDF,
            partition_w8: DEFAULT_PARTITION_W8_CDF,
            partition_w16: DEFAULT_PARTITION_W16_CDF,
            partition_w32: DEFAULT_PARTITION_W32_CDF,
            partition_w64: DEFAULT_PARTITION_W64_CDF,
            partition_w128: DEFAULT_PARTITION_W128_CDF,
            skip: DEFAULT_SKIP_CDF,
            segment_id: DEFAULT_SEGMENT_ID_CDF,
            tx_8x8: DEFAULT_TX_8X8_CDF,
            tx_16x16: DEFAULT_TX_16X16_CDF,
            tx_32x32: DEFAULT_TX_32X32_CDF,
            tx_64x64: DEFAULT_TX_64X64_CDF,
            txfm_split: DEFAULT_TXFM_SPLIT_CDF,
            mv: [MvCdfs::new(), MvCdfs::new()],
            inter_tx_type_set1: DEFAULT_INTER_TX_TYPE_SET1_CDF,
            inter_tx_type_set2: DEFAULT_INTER_TX_TYPE_SET2_CDF,
            inter_tx_type_set3: DEFAULT_INTER_TX_TYPE_SET3_CDF,
            filter_intra_mode: DEFAULT_FILTER_INTRA_MODE_CDF,
            filter_intra: DEFAULT_FILTER_INTRA_CDF,
            palette_uv_mode: DEFAULT_PALETTE_UV_MODE_CDF,
            palette_y_mode: DEFAULT_PALETTE_Y_MODE_CDF,
            palette_y_size: DEFAULT_PALETTE_Y_SIZE_CDF,
            palette_uv_size: DEFAULT_PALETTE_UV_SIZE_CDF,
            palette_y_color: PALETTE_Y_COLOR_DEFAULTS,
            palette_uv_color: PALETTE_UV_COLOR_DEFAULTS,
            delta_q: DEFAULT_DELTA_Q_CDF,
            delta_lf: DEFAULT_DELTA_LF_CDF,
            delta_lf_multi: [DEFAULT_DELTA_LF_CDF; 4],
            use_wiener: DEFAULT_USE_WIENER_CDF,
            use_sgrproj: DEFAULT_USE_SGRPROJ_CDF,
            restoration_type: DEFAULT_RESTORATION_TYPE_CDF,
            intra_tx_type_set1: DEFAULT_INTRA_TX_TYPE_SET1_CDF,
            intra_tx_type_set2: DEFAULT_INTRA_TX_TYPE_SET2_CDF,
            cfl_sign: DEFAULT_CFL_SIGN_CDF,
            cfl_alpha: DEFAULT_CFL_ALPHA_CDF,
            txb_skip: DEFAULT_TXB_SKIP_CDF[q],
            eob_pt_16: DEFAULT_EOB_PT_16_CDF[q],
            eob_pt_32: DEFAULT_EOB_PT_32_CDF[q],
            eob_pt_64: DEFAULT_EOB_PT_64_CDF[q],
            eob_pt_128: DEFAULT_EOB_PT_128_CDF[q],
            eob_pt_256: DEFAULT_EOB_PT_256_CDF[q],
            eob_pt_512: DEFAULT_EOB_PT_512_CDF[q],
            eob_pt_1024: DEFAULT_EOB_PT_1024_CDF[q],
            eob_extra: DEFAULT_EOB_EXTRA_CDF[q],
            dc_sign: DEFAULT_DC_SIGN_CDF[q],
            coeff_base_eob: DEFAULT_COEFF_BASE_EOB_CDF[q],
            coeff_base: DEFAULT_COEFF_BASE_CDF[q],
            coeff_br: DEFAULT_COEFF_BR_CDF[q],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Each quantizer boundary maps to the spec's coefficient context index. */
    #[test]
    fn the_q_context_boundaries_follow_init_coeff_cdfs() {
        assert_eq!(coeff_q_context(0), 0);
        assert_eq!(coeff_q_context(20), 0);
        assert_eq!(coeff_q_context(21), 1);
        assert_eq!(coeff_q_context(60), 1);
        assert_eq!(coeff_q_context(61), 2);
        assert_eq!(coeff_q_context(120), 2);
        assert_eq!(coeff_q_context(121), 3);
        assert_eq!(coeff_q_context(255), 3);
    }

    /** A frame's tiles start from the default coefficient tables for its quantizer context. */
    #[test]
    fn a_context_takes_the_coefficient_defaults_for_its_quantizer() {
        assert_eq!(TileCdfs::new(0).txb_skip, DEFAULT_TXB_SKIP_CDF[0]);
        assert_eq!(TileCdfs::new(200).coeff_base, DEFAULT_COEFF_BASE_CDF[3]);
    }

    /** The saved context is a copy: adapting the tile's CDFs afterwards leaves it unchanged. */
    #[test]
    fn a_saved_context_is_independent_of_later_adaptation() {
        let mut tile = TileCdfs::new(0);
        let saved = tile.clone();
        tile.skip[0][0] = 1;
        assert_ne!(tile.skip, saved.skip);
        assert_eq!(saved.skip, DEFAULT_SKIP_CDF);
    }
}
