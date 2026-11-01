/** Sequence header OBU (spec 5.5): stream-wide coding tools, frame size limits, operating points and the colour config. Timing and decoder-model fields are read so the bits line up, then kept only as far as frame headers need them. */
use crate::av1::{
    CP_BT_709, CP_UNSPECIFIED, CSP_UNKNOWN, DELTA_FRAME_ID_LENGTH_OFFSET, MC_IDENTITY,
    MC_UNSPECIFIED, OPERATING_POINT_IDC_BITS, SELECT_INTEGER_MV, SELECT_SCREEN_CONTENT_TOOLS,
    SEQ_LEVEL_BITS, SEQ_TIER_LEVEL_THRESHOLD, TC_SRGB, TC_UNSPECIFIED,
};
use crate::bits::BitReader;
use crate::error::{Error, Result};

/** Bits in the buffer-delay and presentation-time length fields of decoder_model_info. */
const DECODER_MODEL_LENGTH_BITS: u32 = 5;
/** Bits in the operating_point_idc-less fields: the display delay and its presence flag. */
const INITIAL_DISPLAY_DELAY_BITS: u32 = 4;
/** Bits of the frame width and height bit-count fields. */
const FRAME_DIMENSION_BITS_FIELD: u32 = 4;
/** Bits of additional_frame_id_length_minus_1. */
const FRAME_ID_FIELD_BITS: u32 = 3;
/** Bits of delta_frame_id_length_minus_2. */
const DELTA_FRAME_ID_BITS: u32 = 4;
/** Bits of order_hint_bits_minus_1. */
const ORDER_HINT_BITS_FIELD: u32 = 3;
/** Bits of seq_profile and the still-picture flags, and the chroma subsampling bits at profile 2. */
const PROFILE_BITS: u32 = 3;
/** Bits of the 12-bit subsampling fields. */
const SUBSAMPLING_FIELD_BITS: u32 = 1;
/** Bits of chroma_sample_position. */
const CHROMA_POSITION_BITS: u32 = 2;
/** Bits of a colour description code. */
const COLOR_CODE_BITS: u32 = 8;
/** Profile 2 is the only one that can code 12-bit depth and non-4:2:0 chroma subsampling choices. */
const PROFILE_2: u8 = 2;
/** Profile 1 is 4:4:4 only and has no mono_chrome flag. */
const PROFILE_1: u8 = 1;
/** High bit depth steps: 8 bits, 10 bits, 12 bits. */
const BITS_8: u8 = 8;
const BITS_10: u8 = 10;
const BITS_12: u8 = 12;
/** Initial display delay and decoder model field widths are fixed by the spec. */
const TICK_BITS: u32 = 32;
/** Operating point index bit width. */
const OPERATING_POINTS_COUNT_BITS: u32 = 5;

/** The colour config of a sequence: bit depth, planes, CICP codes, and chroma layout. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ColorConfig {
    pub(crate) bit_depth: u8,
    pub(crate) mono_chrome: bool,
    pub(crate) color_primaries: u8,
    pub(crate) transfer_characteristics: u8,
    pub(crate) matrix_coefficients: u8,
    pub(crate) color_range: bool,
    pub(crate) subsampling_x: bool,
    pub(crate) subsampling_y: bool,
    pub(crate) chroma_sample_position: u8,
    pub(crate) separate_uv_delta_q: bool,
}

impl ColorConfig {
    /** Planes in the picture: one for monochrome, three otherwise. */
    pub(crate) fn num_planes(&self) -> usize {
        if self.mono_chrome { 1 } else { 3 }
    }
}

/** The fields of a sequence header that frame headers and the decoder read. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Sequence {
    pub(crate) profile: u8,
    pub(crate) still_picture: bool,
    pub(crate) reduced_still_picture_header: bool,
    pub(crate) equal_picture_interval: bool,
    pub(crate) decoder_model_info_present: bool,
    pub(crate) buffer_removal_time_length: u32,
    pub(crate) frame_presentation_time_length: u32,
    pub(crate) operating_point_idc: Vec<u32>,
    pub(crate) decoder_model_present_for_op: Vec<bool>,
    pub(crate) frame_width_bits: u32,
    pub(crate) frame_height_bits: u32,
    pub(crate) max_frame_width: u32,
    pub(crate) max_frame_height: u32,
    pub(crate) frame_id_numbers_present: bool,
    pub(crate) delta_frame_id_length: u32,
    pub(crate) additional_frame_id_length: u32,
    pub(crate) use_128x128_superblock: bool,
    pub(crate) enable_order_hint: bool,
    pub(crate) order_hint_bits: u32,
    pub(crate) seq_force_screen_content_tools: u8,
    pub(crate) seq_force_integer_mv: u8,
    pub(crate) enable_superres: bool,
    pub(crate) enable_cdef: bool,
    pub(crate) enable_restoration: bool,
    pub(crate) enable_filter_intra: bool,
    pub(crate) enable_intra_edge_filter: bool,
    pub(crate) film_grain_params_present: bool,
    pub(crate) color: ColorConfig,
}

/** Parse the payload of a sequence header OBU. Operating point 0 is the one a decoder uses. */
pub(crate) fn parse(payload: &[u8]) -> Result<Sequence> {
    let mut r = BitReader::new(payload);
    let profile = r.f(PROFILE_BITS)? as u8;
    if profile > PROFILE_2 {
        return Err(Error::Malformed("seq_profile"));
    }
    let still_picture = r.flag()?;
    let reduced = r.flag()?;
    let mut seq = Sequence::blank(profile, still_picture, reduced);
    if reduced {
        r.f(SEQ_LEVEL_BITS)?; // seq_level_idx[0] has no effect on intra decoding.
    } else {
        read_operating_points(&mut r, &mut seq)?;
    }
    read_frame_limits(&mut r, &mut seq)?;
    read_tools(&mut r, &mut seq)?;
    seq.color = color_config(&mut r, profile)?;
    seq.film_grain_params_present = r.flag()?;
    Ok(seq)
}

impl Sequence {
    /** A sequence with every optional field at its spec default, before the header's bits fill it in. */
    fn blank(profile: u8, still_picture: bool, reduced: bool) -> Self {
        Self {
            profile,
            still_picture,
            reduced_still_picture_header: reduced,
            equal_picture_interval: false,
            decoder_model_info_present: false,
            buffer_removal_time_length: 0,
            frame_presentation_time_length: 0,
            operating_point_idc: vec![0],
            decoder_model_present_for_op: vec![false],
            frame_width_bits: 0,
            frame_height_bits: 0,
            max_frame_width: 0,
            max_frame_height: 0,
            frame_id_numbers_present: false,
            delta_frame_id_length: 0,
            additional_frame_id_length: 0,
            use_128x128_superblock: false,
            enable_order_hint: false,
            order_hint_bits: 0,
            seq_force_screen_content_tools: SELECT_SCREEN_CONTENT_TOOLS,
            seq_force_integer_mv: SELECT_INTEGER_MV,
            enable_superres: false,
            enable_cdef: false,
            enable_restoration: false,
            enable_filter_intra: false,
            enable_intra_edge_filter: false,
            film_grain_params_present: false,
            color: ColorConfig {
                bit_depth: BITS_8,
                mono_chrome: false,
                color_primaries: CP_UNSPECIFIED,
                transfer_characteristics: TC_UNSPECIFIED,
                matrix_coefficients: MC_UNSPECIFIED,
                color_range: false,
                subsampling_x: true,
                subsampling_y: true,
                chroma_sample_position: CSP_UNKNOWN,
                separate_uv_delta_q: false,
            },
        }
    }
}

/** timing_info, decoder_model_info and the operating point loop. Only the fields frame headers use are kept. */
fn read_operating_points(r: &mut BitReader<'_>, seq: &mut Sequence) -> Result<()> {
    let timing_info_present = r.flag()?;
    let mut decoder_model_info = None;
    if timing_info_present {
        r.f(TICK_BITS)?; // num_units_in_display_tick
        r.f(TICK_BITS)?; // time_scale
        seq.equal_picture_interval = r.flag()?;
        if seq.equal_picture_interval {
            r.uvlc()?; // num_ticks_per_picture_minus_1
        }
        if r.flag()? {
            decoder_model_info = Some(read_decoder_model_info(r)?);
        }
    }
    seq.decoder_model_info_present = decoder_model_info.is_some();
    let initial_display_delay_present = r.flag()?;
    let count = r.f(OPERATING_POINTS_COUNT_BITS)? as usize + 1;
    seq.operating_point_idc.clear();
    seq.decoder_model_present_for_op.clear();
    for _ in 0..count {
        seq.operating_point_idc
            .push(r.f(OPERATING_POINT_IDC_BITS)? as u32);
        let level = r.f(SEQ_LEVEL_BITS)?;
        if level > SEQ_TIER_LEVEL_THRESHOLD {
            r.flag()?; // seq_tier
        }
        let mut model_present = false;
        if let Some((buffer_delay_length, _, _)) = decoder_model_info {
            model_present = r.flag()?;
            if model_present {
                skip_operating_parameters(r, buffer_delay_length)?;
            }
        }
        seq.decoder_model_present_for_op.push(model_present);
        if initial_display_delay_present && r.flag()? {
            r.f(INITIAL_DISPLAY_DELAY_BITS)?;
        }
    }
    if let Some(length) = decoder_model_info {
        seq.buffer_removal_time_length = length.1;
        seq.frame_presentation_time_length = length.2;
    }
    Ok(())
}

/** decoder_model_info(): returns (buffer_delay_length, buffer_removal_time_length, frame_presentation_time_length). */
fn read_decoder_model_info(r: &mut BitReader<'_>) -> Result<(u32, u32, u32)> {
    let buffer_delay_length = r.f(DECODER_MODEL_LENGTH_BITS)? as u32 + 1;
    r.f(TICK_BITS)?; // num_units_in_decoding_tick
    let removal = r.f(DECODER_MODEL_LENGTH_BITS)? as u32 + 1;
    let presentation = r.f(DECODER_MODEL_LENGTH_BITS)? as u32 + 1;
    Ok((buffer_delay_length, removal, presentation))
}

/** operating_parameters_info(): two buffer delays of the given width and a low-delay flag. */
fn skip_operating_parameters(r: &mut BitReader<'_>, buffer_delay_length: u32) -> Result<()> {
    r.f(buffer_delay_length)?;
    r.f(buffer_delay_length)?;
    r.flag()?;
    Ok(())
}

/** frame_width_bits, frame_height_bits, the maximum frame size and the frame-ID fields. */
fn read_frame_limits(r: &mut BitReader<'_>, seq: &mut Sequence) -> Result<()> {
    seq.frame_width_bits = r.f(FRAME_DIMENSION_BITS_FIELD)? as u32 + 1;
    seq.frame_height_bits = r.f(FRAME_DIMENSION_BITS_FIELD)? as u32 + 1;
    seq.max_frame_width = r.f(seq.frame_width_bits)? as u32 + 1;
    seq.max_frame_height = r.f(seq.frame_height_bits)? as u32 + 1;
    if !seq.reduced_still_picture_header {
        seq.frame_id_numbers_present = r.flag()?;
    }
    if seq.frame_id_numbers_present {
        seq.delta_frame_id_length = r.f(DELTA_FRAME_ID_BITS)? as u32 + DELTA_FRAME_ID_LENGTH_OFFSET;
        seq.additional_frame_id_length = r.f(FRAME_ID_FIELD_BITS)? as u32 + 1;
    }
    Ok(())
}

/** The coding-tool flags. A reduced still-picture header skips the inter tools and fixes the screen-content selectors. */
fn read_tools(r: &mut BitReader<'_>, seq: &mut Sequence) -> Result<()> {
    seq.use_128x128_superblock = r.flag()?;
    seq.enable_filter_intra = r.flag()?;
    seq.enable_intra_edge_filter = r.flag()?;
    if !seq.reduced_still_picture_header {
        read_inter_tools(r, seq)?;
    }
    seq.enable_superres = r.flag()?;
    seq.enable_cdef = r.flag()?;
    seq.enable_restoration = r.flag()?;
    Ok(())
}

/** The inter-only tool flags and the screen-content and order-hint selectors. */
fn read_inter_tools(r: &mut BitReader<'_>, seq: &mut Sequence) -> Result<()> {
    r.flag()?; // enable_interintra_compound
    r.flag()?; // enable_masked_compound
    r.flag()?; // enable_warped_motion
    r.flag()?; // enable_dual_filter
    seq.enable_order_hint = r.flag()?;
    if seq.enable_order_hint {
        r.flag()?; // enable_jnt_comp
        r.flag()?; // enable_ref_frame_mvs
    }
    let choose_sct = r.flag()?;
    seq.seq_force_screen_content_tools = if choose_sct {
        SELECT_SCREEN_CONTENT_TOOLS
    } else {
        r.f(1)? as u8
    };
    seq.seq_force_integer_mv = if seq.seq_force_screen_content_tools > 0 {
        if r.flag()? {
            SELECT_INTEGER_MV
        } else {
            r.f(1)? as u8
        }
    } else {
        SELECT_INTEGER_MV
    };
    if seq.enable_order_hint {
        seq.order_hint_bits = r.f(ORDER_HINT_BITS_FIELD)? as u32 + 1;
    }
    Ok(())
}

/** color_config(): bit depth, monochrome, CICP codes, colour range, and chroma subsampling. */
fn color_config(r: &mut BitReader<'_>, profile: u8) -> Result<ColorConfig> {
    let high_bitdepth = r.flag()?;
    let bit_depth = if profile == PROFILE_2 && high_bitdepth {
        if r.flag()? { BITS_12 } else { BITS_10 }
    } else if high_bitdepth {
        BITS_10
    } else {
        BITS_8
    };
    let mono_chrome = if profile == PROFILE_1 {
        false
    } else {
        r.flag()?
    };
    let (primaries, transfer, matrix) = if r.flag()? {
        (
            r.f(COLOR_CODE_BITS)? as u8,
            r.f(COLOR_CODE_BITS)? as u8,
            r.f(COLOR_CODE_BITS)? as u8,
        )
    } else {
        (CP_UNSPECIFIED, TC_UNSPECIFIED, MC_UNSPECIFIED)
    };
    let mut cfg = ColorConfig {
        bit_depth,
        mono_chrome,
        color_primaries: primaries,
        transfer_characteristics: transfer,
        matrix_coefficients: matrix,
        color_range: false,
        subsampling_x: true,
        subsampling_y: true,
        chroma_sample_position: CSP_UNKNOWN,
        separate_uv_delta_q: false,
    };
    if mono_chrome {
        cfg.color_range = r.flag()?;
        return Ok(cfg);
    }
    if primaries == CP_BT_709 && transfer == TC_SRGB && matrix == MC_IDENTITY {
        cfg.color_range = true;
        cfg.subsampling_x = false;
        cfg.subsampling_y = false;
    } else {
        cfg.color_range = r.flag()?;
        read_subsampling(r, profile, bit_depth, &mut cfg)?;
    }
    cfg.separate_uv_delta_q = r.flag()?;
    Ok(cfg)
}

/** Chroma subsampling from the profile, with the 12-bit profile-2 case coded explicitly. */
fn read_subsampling(
    r: &mut BitReader<'_>,
    profile: u8,
    bit_depth: u8,
    cfg: &mut ColorConfig,
) -> Result<()> {
    if profile == 0 {
        return read_position(r, cfg);
    }
    if profile == PROFILE_1 {
        cfg.subsampling_x = false;
        cfg.subsampling_y = false;
        return Ok(());
    }
    if bit_depth == BITS_12 {
        cfg.subsampling_x = r.f(SUBSAMPLING_FIELD_BITS)? == 1;
        cfg.subsampling_y = if cfg.subsampling_x {
            r.f(SUBSAMPLING_FIELD_BITS)? == 1
        } else {
            false
        };
    } else {
        cfg.subsampling_x = true;
        cfg.subsampling_y = false;
    }
    if cfg.subsampling_x && cfg.subsampling_y {
        cfg.chroma_sample_position = r.f(CHROMA_POSITION_BITS)? as u8;
    }
    Ok(())
}

/** Profile 0 is always 4:2:0; read the chroma sample position for it. */
fn read_position(r: &mut BitReader<'_>, cfg: &mut ColorConfig) -> Result<()> {
    cfg.subsampling_x = true;
    cfg.subsampling_y = true;
    cfg.chroma_sample_position = r.f(CHROMA_POSITION_BITS)? as u8;
    Ok(())
}
