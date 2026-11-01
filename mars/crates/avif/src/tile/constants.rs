/** Block size index of a 4x4 luma block. */
pub(crate) const BLOCK_4X4: usize = 0;
/** Samples in one 4x4 mode-info unit, and the log2 of that count. */
pub(crate) const MI_SIZE: usize = 4;
pub(crate) const MI_UNIT_LOG2: usize = 2;
/** The largest transform dimension in samples, which 64-sample chroma transforms are capped from. */
pub(crate) const MAX_TX_DIM: usize = 64;
/** Block size index of an 8x8 luma block, the smallest that can carry a partition. */
pub(crate) const BLOCK_8X8: usize = 3;
/** Block size index of a 64x64 block, the default superblock. */
pub(crate) const BLOCK_64X64: usize = 12;
/** Block size index of a 128x128 block, the large superblock. */
pub(crate) const BLOCK_128X128: usize = 15;

/** Partition types, in the spec's order. */
pub(crate) const PARTITION_NONE: usize = 0;
pub(crate) const PARTITION_HORZ: usize = 1;
pub(crate) const PARTITION_VERT: usize = 2;
pub(crate) const PARTITION_SPLIT: usize = 3;
pub(crate) const PARTITION_HORZ_A: usize = 4;
pub(crate) const PARTITION_HORZ_B: usize = 5;
pub(crate) const PARTITION_VERT_A: usize = 6;
pub(crate) const PARTITION_VERT_B: usize = 7;
pub(crate) const PARTITION_HORZ_4: usize = 8;
pub(crate) const PARTITION_VERT_4: usize = 9;

/** Transform size indices of the square sizes and the few rectangular ones the decoder names. */
pub(crate) const TX_4X4: usize = 0;
pub(crate) const TX_16X16: usize = 2;
pub(crate) const TX_32X32: usize = 3;
pub(crate) const TX_32X64: usize = 11;
pub(crate) const TX_64X32: usize = 12;
pub(crate) const TX_64X64: usize = 4;
pub(crate) const TX_16X32: usize = 9;
pub(crate) const TX_32X16: usize = 10;
pub(crate) const TX_16X64: usize = 17;
pub(crate) const TX_64X16: usize = 18;
/** Count of all transform sizes, and of the square ones. */
pub(crate) const TX_SIZES_ALL: usize = 19;
/** The largest number of tx_depth splits a block may code. */
pub(crate) const MAX_TX_DEPTH: usize = 2;

/** Intra prediction modes: the luma set, the first directional mode, the last directional mode and the chroma CFL mode. */
pub(crate) const DC_PRED: u8 = 0;
pub(crate) const V_PRED: u8 = 1;
pub(crate) const D67_PRED: u8 = 8;
pub(crate) const UV_CFL_PRED: u8 = 13;
/** Largest magnitude of an angle delta, and the symbol count of angle_delta (two signs times that plus zero). */
pub(crate) const MAX_ANGLE_DELTA: i32 = 3;

/** Delta-quantiser and delta-loop-filter symbol cut-offs, the loop filter strength count and its maximum. */
pub(crate) const DELTA_Q_SMALL: u32 = 3;
pub(crate) const DELTA_LF_SMALL: u32 = 3;
pub(crate) const FRAME_LF_COUNT: usize = 4;
pub(crate) const MAX_LOOP_FILTER: i32 = 63;
/** Bits read after the delta_q_abs and delta_lf_abs escape symbol. */
pub(crate) const DELTA_ESCAPE_BITS: u32 = 3;

/** Chroma-from-luma sign codes and the alpha alphabet. */
pub(crate) const CFL_SIGN_ZERO: usize = 0;
pub(crate) const CFL_SIGN_NEG: usize = 1;
pub(crate) const CFL_SIGN_COUNT: usize = 3;

/** Palette limits: colours per palette, the smallest and largest sizes coded, the neighbour count of the colour order, and the block dimension palettes are allowed for. */
pub(crate) const PALETTE_COLORS: usize = 8;
pub(crate) const PALETTE_MIN_SIZE: usize = 2;
pub(crate) const PALETTE_NUM_NEIGHBORS: usize = 3;
pub(crate) const PALETTE_MAX_BLOCK_DIM: usize = 64;
/** Offset from the block's log2 size sum to the palette size context, and the bits of the extra-bit field. */
pub(crate) const PALETTE_BSIZE_CTX_OFFSET: usize = 2;
pub(crate) const PALETTE_EXTRA_BITS: u32 = 2;
/** Minimum bits of a luma or U palette delta, and of a V palette delta, below the bit depth. */
pub(crate) const PALETTE_DELTA_MIN_BITS: u32 = 3;
pub(crate) const PALETTE_DELTA_MIN_BITS_V: u32 = 4;
/** Chroma palette maps are padded to four samples when the subsampled block is narrower. */
pub(crate) const PALETTE_CHROMA_MIN_DIM: usize = 4;
pub(crate) const PALETTE_CHROMA_PAD: usize = 2;
/** Stride of a colour-index map: the largest palette block dimension. */
pub(crate) const COLOR_MAP_STRIDE: usize = PALETTE_MAX_BLOCK_DIM;

/** Transform sets, and the transform types the decoder names. */
pub(crate) const TX_SET_DCTONLY: usize = 0;
pub(crate) const TX_SET_INTRA_1: usize = 1;
pub(crate) const TX_SET_INTRA_2: usize = 2;
pub(crate) const DCT_DCT: u8 = 0;
pub(crate) const IDTX: u8 = 9;
pub(crate) const V_DCT: u8 = 10;
pub(crate) const H_DCT: u8 = 11;
pub(crate) const V_ADST: u8 = 12;
pub(crate) const H_ADST: u8 = 13;
pub(crate) const V_FLIPADST: u8 = 14;
pub(crate) const H_FLIPADST: u8 = 15;
/** Transform classes: two-dimensional, horizontal-only and vertical-only. */
pub(crate) const TX_CLASS_2D: usize = 0;
pub(crate) const TX_CLASS_HORIZ: usize = 1;
pub(crate) const TX_CLASS_VERT: usize = 2;

/** Coefficient coding: base and range levels, the context counts of coeff_base and coeff_br, and the Golomb escape bound. */
pub(crate) const NUM_BASE_LEVELS: u32 = 2;
pub(crate) const COEFF_BASE_RANGE: u32 = 12;
pub(crate) const BR_CDF_SIZE: u32 = 4;
pub(crate) const BR_ROUNDS: u32 = COEFF_BASE_RANGE / (BR_CDF_SIZE - 1);
pub(crate) const SIG_COEF_CONTEXTS_2D: usize = 26;
/** Neighbour offsets read for coeff_base, and the three positional context offsets added past the 2D set. */
pub(crate) const SIG_REF_DIFF_OFFSET_NUM: usize = 5;
pub(crate) const COEFF_BASE_POS_CTX_OFFSET: [usize; 3] = [
    SIG_COEF_CONTEXTS_2D,
    SIG_COEF_CONTEXTS_2D + 5,
    SIG_COEF_CONTEXTS_2D + 10,
];
pub(crate) const COEFF_BR_MAG_CAP: u32 = COEFF_BASE_RANGE + NUM_BASE_LEVELS + 1;
pub(crate) const COEFF_BR_MAG_LIMIT: u32 = 6;
pub(crate) const COEFF_BR_NEAR_OFFSET: usize = 7;
pub(crate) const COEFF_BR_FAR_OFFSET: usize = 14;
/** Coefficient sizes: a scan's largest length, the length of the 16x64 and 64x16 scans, the eob-size classes and the largest class. */
pub(crate) const MAX_SCAN_LEN: usize = 1024;
pub(crate) const SEG_EOB_LARGE: usize = 512;
pub(crate) const EOB_SIZE_CAP: usize = 5;
pub(crate) const EOB_SIZE_OFFSET: usize = 4;
/** Largest level a coefficient may carry before its Golomb tail, and the cap on Golomb prefix length (conforming streams stay below 20). */
pub(crate) const GOLOMB_MAX_LENGTH: u32 = 20;
pub(crate) const COEFF_LEVEL_MASK: u32 = 0xF_FFFF;
/** Cumulative-level cap and the dc_sign categories. */
pub(crate) const CUL_LEVEL_MAX: u32 = 63;
pub(crate) const DC_CATEGORY_NEG: u8 = 1;
pub(crate) const DC_CATEGORY_POS: u8 = 2;

/** Dequantiser: the mask of the magnitude bits kept, the denominators of the large transforms, and the bit depth the lookups start from. */
pub(crate) const DQ_MAGNITUDE_MASK: i64 = 0xFF_FFFF;
/** Bits of headroom a dequantised coefficient is clipped to beyond the bit depth: a signed value of 7 + bit depth bits. */
pub(crate) const DEQUANT_CLIP_BASE_BITS: u32 = 7;
pub(crate) const DQ_DENOM_HALF: i64 = 2;
pub(crate) const DQ_DENOM_QUARTER: i64 = 4;
pub(crate) const BITDEPTH_MIN: u32 = 8;
pub(crate) const QLOOKUP_MAX_INDEX: i32 = 255;

/** Loop restoration: Wiener taps, self-guided parameter sets and their sub-exponential code parameters. */
pub(crate) const WIENER_COEFFS: usize = 3;
pub(crate) const SGRPROJ_PARAMS_BITS: u32 = 4;
pub(crate) const SGRPROJ_PRJ_SUBEXP_K: u32 = 4;
pub(crate) const SGRPROJ_PRJ_BITS: u32 = 7;
pub(crate) const SGR_PASSES: usize = 2;

/** The segmentation feature for a skip block, and the segments addressable by a block's segment id. */
pub(crate) const SEG_LVL_SKIP: usize = 6;

/** Largest superblock in samples: planes are padded to a multiple of it. */
pub(crate) const SB_SAMPLES: usize = 128;

/** Superblock size in 4x4 units for the 64 and 128 superblock choices, and the 64-sample CDEF unit in 4x4 units. */
pub(crate) const SB64_MI: usize = 16;
pub(crate) const SB128_MI: usize = 32;
pub(crate) const CDEF_UNIT_MI: usize = 16;
/** Extra mode-info columns and rows beyond the frame, so a block that overhangs the edge can write its contexts. */
pub(crate) const CONTEXT_MARGIN: usize = 32;

/** Transform sets of inter blocks, after the intra ones (spec TX_SET_INTER_1..3). */
pub(crate) const TX_SET_INTER_1: usize = 1;
pub(crate) const TX_SET_INTER_2: usize = 2;
pub(crate) const TX_SET_INTER_3: usize = 3;
/** Count of square transform sizes (TX_SIZES in the spec), and the largest variable transform tree depth. */
pub(crate) const TX_SIZES: usize = 5;
pub(crate) const MAX_VARTX_DEPTH: usize = 2;
/** Reference frame of an intra block, and the none marker of its second reference. */
pub(crate) const INTRA_FRAME: i8 = 0;
pub(crate) const NONE_FRAME: i8 = -1;
/** Motion vector joints: which components are nonzero (spec MV_JOINT_*). */
pub(crate) const MV_JOINT_HNZVZ: usize = 1;
pub(crate) const MV_JOINT_HZVNZ: usize = 2;
pub(crate) const MV_JOINT_HNZVNZ: usize = 3;
/** Motion vector contexts (intra block copy has its own), the component count, the class-0 size and the offset bits of a class. */
pub(crate) const MV_CONTEXTS: usize = 2;
pub(crate) const MV_INTRABC_CONTEXT: usize = 1;
pub(crate) const MV_COMPONENTS: usize = 2;
pub(crate) const CLASS0_SIZE: i32 = 2;
pub(crate) const MV_OFFSET_BITS: usize = 10;
/** The fraction and high-precision bits an integer-only vector codes as: fixed, so nothing is read for them. */
pub(crate) const INTEGER_MV_FR: i32 = 3;
pub(crate) const INTEGER_MV_HP: i32 = 1;
/** Motion vectors are in 1/8 sample units, with three fractional bits, so one sample is MV_UNIT. A class of magnitude c starts at CLASS0_SIZE shifted by c plus CLASS_OFFSET_BASE. */
pub(crate) const MV_FRAC_BITS: u32 = 3;
pub(crate) const MV_UNIT: i32 = 1 << MV_FRAC_BITS;
pub(crate) const CLASS_OFFSET_BASE: usize = 2;
/** Chroma and luma positions in 1/16 sample units are twice the 1/8 vector, before the chroma subsampling shift. */
pub(crate) const HALF_UNIT_SCALE: i32 = 2;
/** The largest magnitude a motion vector component may take (spec is_mv_valid). */
pub(crate) const MV_LIMIT: i32 = 1 << 14;
/** Border beyond the frame edge that a candidate motion vector is clamped to, in 1/8 sample units. */
pub(crate) const MV_BORDER: i32 = 128;
/** Weight a neighbour found in the immediate row or column gets over the rest, and the stack's capacity. */
pub(crate) const REF_CAT_LEVEL: u32 = 640;
pub(crate) const MAX_REF_MV_STACK_SIZE: usize = 8;
/** Candidate rows and columns the stack search looks at, from the block's edge (spec scan row and column deltas). */
pub(crate) const MV_SCAN_NEAR: isize = -1;
pub(crate) const MV_SCAN_FAR_ROW: isize = -3;
pub(crate) const MV_SCAN_FARTHER_ROW: isize = -5;
pub(crate) const MV_SCAN_FAR_COL: isize = -3;
pub(crate) const MV_SCAN_FARTHER_COL: isize = -5;
/** Weight of a candidate found at a single point, and the block size (in 4x4 units) above which the top-right point is skipped. */
pub(crate) const POINT_WEIGHT: u32 = 4;
pub(crate) const POINT_SKIP_MI: usize = 16;
/** Intra block copy: the delay, in pixels and in 64-sample superblocks, a source must keep behind the block it predicts (spec INTRABC_DELAY_*). */
pub(crate) const INTRABC_DELAY_PIXELS: i32 = 256;
pub(crate) const INTRABC_DELAY_SB64: i32 = 4;
/** Bilinear filter taps: the total weight of a filter, the taps per sub-sample position, and the fractional bits of a position. */
pub(crate) const FILTER_TOTAL: i32 = 128;
pub(crate) const SUBPEL_BITS: u32 = 4;
pub(crate) const SUBPEL_MASK: i32 = (1 << SUBPEL_BITS) - 1;
/** Rounding shifts of the two bilinear passes at 8, 10 and 12 bits, and the reduction 12-bit streams take on each. */
pub(crate) const INTER_ROUND0: u32 = 3;
pub(crate) const INTER_ROUND1: u32 = 11;
pub(crate) const HIGH_BITDEPTH_ROUND: u32 = 2;
/** Taps of the separable filter: the tap that sits at the sample itself and the one after it, and the rows of context above it. */
pub(crate) const FILTER_TAP_SELF: usize = 3;
pub(crate) const FILTER_TAP_NEXT: usize = 4;
pub(crate) const FILTER_ROWS_ABOVE: usize = 3;
pub(crate) const FILTER_ROWS_EXTRA: usize = 7;
/** Bit depth at which the two bilinear passes round by the larger shifts. */
pub(crate) const BITDEPTH_HIGH: u32 = 12;
/** The rounding bias of a whole-sample candidate: half a sample less one, in 1/8 sample units. */
pub(crate) const INTEGER_ROUND_BIAS: i32 = 3;
