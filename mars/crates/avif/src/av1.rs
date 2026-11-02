/** AV1 spec constants and enumerations used by the OBU and header parsers, named as the spec names them so each one can be checked against 03.symbols and section 5. */
/** OBU type: sequence header. */
pub(crate) const OBU_SEQUENCE_HEADER: u8 = 1;
/** OBU type: frame header. */
pub(crate) const OBU_FRAME_HEADER: u8 = 3;
/** OBU type: frame, a frame header followed by its tile group. */
pub(crate) const OBU_FRAME: u8 = 6;
pub(crate) const OBU_TILE_GROUP: u8 = 4;
/** Most OBUs in one temporal unit this crate will index before refusing the input. */
pub(crate) const MAX_OBUS: usize = 4096;
/** Longest leb128 value in bytes: the spec limits leb128 to eight bytes. */
pub(crate) const LEB128_MAX_BYTES: usize = 8;
/** Payload bits in one leb128 byte. */
pub(crate) const LEB128_BITS: u32 = 7;
/** Continuation bit in a leb128 byte. */
pub(crate) const LEB128_MORE: u8 = 0x80;
/** Bits in a leb128 value that the spec allows (values below 2^32). */
pub(crate) const LEB128_VALUE_BITS: u32 = 32;
/** The OBU header's extension flag bit, the has-size bit, and the forbidden bit. */
pub(crate) const OBU_FORBIDDEN_BIT: u8 = 0x80;
pub(crate) const OBU_EXTENSION_BIT: u8 = 0x04;
pub(crate) const OBU_HAS_SIZE_BIT: u8 = 0x02;
/** The OBU type occupies bits 3 to 6 of the header byte. */
pub(crate) const OBU_TYPE_SHIFT: u32 = 3;
pub(crate) const OBU_TYPE_MASK: u8 = 0x0f;
/** The extension byte carries temporal_id in its top three bits and spatial_id in the next two. */
pub(crate) const OBU_TEMPORAL_SHIFT: u32 = 5;
pub(crate) const OBU_SPATIAL_SHIFT: u32 = 3;
pub(crate) const OBU_SPATIAL_MASK: u8 = 0x03;
/** Bytes in an OBU header and in its extension. */
pub(crate) const OBU_HEADER_LEN: usize = 1;
pub(crate) const OBU_EXTENSION_LEN: usize = 1;

/** Frame type: key frame. */
pub(crate) const KEY_FRAME: u8 = 0;
/** Frame type: inter frame. */
pub(crate) const INTER_FRAME: u8 = 1;
/** Frame type: intra-only frame. */
pub(crate) const INTRA_ONLY_FRAME: u8 = 2;
/** Reference slots in the decoder's reference store. */
pub(crate) const NUM_REF_FRAMES: u32 = 8;
/** Value of seq_force_screen_content_tools and seq_force_integer_mv meaning the frame header codes it. */
pub(crate) const SELECT_SCREEN_CONTENT_TOOLS: u8 = 2;
pub(crate) const SELECT_INTEGER_MV: u8 = 2;
/** Segmentation: number of segments, features per segment, and the reference-frame feature index. */
pub(crate) const MAX_SEGMENTS: usize = 8;
pub(crate) const SEG_LVL_MAX: usize = 8;
pub(crate) const SEG_LVL_ALT_Q: usize = 0;
/** The first loop-filter segment feature: Y vertical, Y horizontal, U and V follow in order. */
pub(crate) const SEG_LVL_ALT_LF_Y_V: usize = 1;
pub(crate) const SEG_LVL_REF_FRAME: usize = 5;
/** Bits, sign and clip range of each segmentation feature, by feature index. */
pub(crate) const SEGMENTATION_FEATURE_BITS: [u32; SEG_LVL_MAX] = [8, 6, 6, 6, 6, 3, 0, 0];
pub(crate) const SEGMENTATION_FEATURE_SIGNED: [bool; SEG_LVL_MAX] =
    [true, true, true, true, true, false, false, false];
pub(crate) const SEGMENTATION_FEATURE_MAX: [i32; SEG_LVL_MAX] = [255, 63, 63, 63, 63, 7, 0, 0];
/** Loop filter reference-delta slots (TOTAL_REFS_PER_FRAME) and mode-delta slots. */
pub(crate) const TOTAL_REFS_PER_FRAME: usize = 8;
pub(crate) const LOOP_FILTER_MODE_DELTAS: usize = 2;
/** Loop filter reference-delta defaults for intra frames, in the spec's slot order (INTRA, LAST..ALTREF). */
pub(crate) const DEFAULT_LOOP_FILTER_REF_DELTAS: [i32; TOTAL_REFS_PER_FRAME] =
    [1, 0, 0, 0, -1, 0, -1, -1];
/** Superres: numerator, smallest denominator, and bits that code the denominator. */
pub(crate) const SUPERRES_NUM: u32 = 8;
pub(crate) const SUPERRES_DENOM_MIN: u32 = 9;
pub(crate) const SUPERRES_DENOM_BITS: u32 = 3;
/** Tile limits from the level definitions. */
pub(crate) const MAX_TILE_WIDTH: u32 = 4096;
pub(crate) const MAX_TILE_AREA: u32 = 4096 * 2304;
pub(crate) const MAX_TILE_ROWS: u32 = 64;
pub(crate) const MAX_TILE_COLS: u32 = 64;
/** Superblock sizes in mode-info units: 64x64 uses shift 4, 128x128 uses shift 5. */
pub(crate) const SB64_SHIFT: u32 = 4;
pub(crate) const SB128_SHIFT: u32 = 5;
/** Mode-info unit (4x4 luma) shift, and the tile-size field width. */
pub(crate) const MI_SIZE_LOG2: u32 = 2;
pub(crate) const TILE_SIZE_BYTES_FIELD_BITS: u32 = 2;
/** Colour description codes that trigger the sRGB identity shortcut. */
pub(crate) const CP_BT_709: u8 = 1;
pub(crate) const TC_SRGB: u8 = 13;
pub(crate) const MC_IDENTITY: u8 = 0;
/** Colour description values for unspecified fields. */
pub(crate) const CP_UNSPECIFIED: u8 = 2;
pub(crate) const TC_UNSPECIFIED: u8 = 2;
pub(crate) const MC_UNSPECIFIED: u8 = 2;
/** Loop restoration types, and the table that maps lr_type codes onto them. */
pub(crate) const RESTORE_NONE: u8 = 0;
pub(crate) const RESTORE_WIENER: u8 = 1;
pub(crate) const RESTORE_SGRPROJ: u8 = 2;
pub(crate) const RESTORE_SWITCHABLE: u8 = 3;
pub(crate) const REMAP_LR_TYPE: [u8; 4] = [
    RESTORE_NONE,
    RESTORE_SWITCHABLE,
    RESTORE_WIENER,
    RESTORE_SGRPROJ,
];
/** Largest loop restoration unit size, in luma samples. */
pub(crate) const RESTORATION_TILESIZE_MAX: u32 = 256;
/** Transform modes. */
pub(crate) const ONLY_4X4: u8 = 0;
pub(crate) const TX_MODE_LARGEST: u8 = 1;
pub(crate) const TX_MODE_SELECT: u8 = 2;
/** Chroma sample position value meaning unknown. */
pub(crate) const CSP_UNKNOWN: u8 = 0;
pub(crate) const DELTA_FRAME_ID_LENGTH_OFFSET: u32 = 2;
/** Bits in the operating point idc field and its tier and level fields. */
pub(crate) const OPERATING_POINT_IDC_BITS: u32 = 12;
pub(crate) const SEQ_LEVEL_BITS: u32 = 5;
/** seq_level_idx above this value carries a tier bit. */
pub(crate) const SEQ_TIER_LEVEL_THRESHOLD: u64 = 7;
/** Delta-q and delta-lf coded width (su(1+6)). */
pub(crate) const DELTA_Q_BITS: u32 = 7;
/** Quantizer index range. */
pub(crate) const MAX_QINDEX: i32 = 255;
