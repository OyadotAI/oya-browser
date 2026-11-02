/** Uncompressed frame header (spec 5.9) for key and intra-only frames: frame type and visibility, frame size, tiles, and the parameter groups in spec order. Inter and switch frames are refused with Unsupported because their reference and motion syntax is not read yet. */
use crate::av1::{
    INTER_FRAME, INTRA_ONLY_FRAME, KEY_FRAME, MAX_QINDEX, MAX_SEGMENTS, MAX_TILE_AREA,
    MAX_TILE_COLS, MAX_TILE_ROWS, MAX_TILE_WIDTH, MI_SIZE_LOG2, NUM_REF_FRAMES, SB64_SHIFT,
    SB128_SHIFT, SEG_LVL_ALT_Q, SELECT_INTEGER_MV, SELECT_SCREEN_CONTENT_TOOLS,
    SUPERRES_DENOM_BITS, SUPERRES_DENOM_MIN, SUPERRES_NUM, TILE_SIZE_BYTES_FIELD_BITS,
};
use crate::bits::BitReader;
use crate::error::{Error, Result};
use crate::params::{self, Cdef, LoopFilter, LoopRestoration, Quant, Segmentation};
use crate::sequence::{ColorConfig, Sequence};

/** Bits of the frame type, the show-existing index, and the refresh mask. */
const FRAME_TYPE_BITS: u32 = 2;
const EXISTING_INDEX_BITS: u32 = 3;
const REFRESH_BITS: u32 = 8;
/** Bits of a one-bit flag, read as an unsigned field. */
const FLAG_BITS: u32 = 1;
/** Bits of each render dimension minus one. */
const RENDER_DIM_BITS: u32 = 16;
/** Bits of the superres coded denominator are named in the spec (SUPERRES_DENOM_BITS). */
/** All reference slots: the refresh mask of a key frame that is shown. */
const ALL_FRAMES: u8 = 0xff;
/** Rounding divisor used when the superres width is computed. */
const SUPERRES_ROUND_DIV: u32 = 2;
/** Mode-info grid: MiCols is twice the 8-pixel block count, so the width is rounded up to eight. */
const MI_PIXEL_SHIFT: u32 = 3;
const MI_ALIGN_PIXELS: u32 = 7;
const MI_PAIR: u32 = 2;
/** The largest tile log2 a 32-bit shift can express; bounds tile_log2 so a shift never overflows. */
const MAX_TILE_LOG2: u32 = 31;
/** Operating point bit tests: a point's idc has one temporal bit and one spatial bit per layer. */
const SPATIAL_IDC_SHIFT: u32 = 8;

/** Frame dimensions: the coded size, the superres-upscaled size, the render size, and the mode-info grid. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FrameSize {
    pub(crate) frame_width: u32,
    pub(crate) frame_height: u32,
    pub(crate) upscaled_width: u32,
    pub(crate) render_width: u32,
    pub(crate) render_height: u32,
    pub(crate) superres_denom: u32,
    pub(crate) mi_cols: u32,
    pub(crate) mi_rows: u32,
}

/** Tile layout: counts, the mode-info start of each tile row and column, and the tile-size field width. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Tiles {
    pub(crate) cols: u32,
    pub(crate) rows: u32,
    pub(crate) cols_log2: u32,
    pub(crate) rows_log2: u32,
    pub(crate) mi_col_starts: Vec<u32>,
    pub(crate) mi_row_starts: Vec<u32>,
    pub(crate) context_update_tile_id: u32,
    pub(crate) tile_size_bytes: u32,
}

/** A key or intra-only frame header with every field the tile decoder needs. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FrameHeader {
    pub(crate) frame_type: u8,
    pub(crate) show_frame: bool,
    pub(crate) showable_frame: bool,
    pub(crate) error_resilient_mode: bool,
    pub(crate) disable_cdf_update: bool,
    pub(crate) allow_screen_content_tools: bool,
    pub(crate) order_hint: u32,
    pub(crate) refresh_frame_flags: u8,
    pub(crate) size: FrameSize,
    pub(crate) allow_intrabc: bool,
    pub(crate) disable_frame_end_update_cdf: bool,
    pub(crate) tiles: Tiles,
    pub(crate) quant: Quant,
    pub(crate) seg: Segmentation,
    pub(crate) delta_q_present: bool,
    pub(crate) delta_q_res: u32,
    pub(crate) delta_lf_present: bool,
    pub(crate) delta_lf_res: u32,
    pub(crate) delta_lf_multi: bool,
    pub(crate) coded_lossless: bool,
    pub(crate) all_lossless: bool,
    pub(crate) lossless_array: [bool; MAX_SEGMENTS],
    pub(crate) loop_filter: LoopFilter,
    pub(crate) cdef: Cdef,
    pub(crate) lr: LoopRestoration,
    pub(crate) tx_mode: u8,
    pub(crate) reduced_tx_set: bool,
    pub(crate) film_grain_apply: bool,
    /** Bytes the header occupies, including the byte alignment that follows it in a frame OBU. */
    pub(crate) header_bytes: usize,
}

/** What a frame header OBU carries: a frame to show from the reference store, or a new frame. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Parsed {
    ShowExisting(u8),
    Frame(Box<FrameHeader>),
}

/** Parse one uncompressed frame header. `temporal_id` and `spatial_id` come from the OBU extension and select which buffer-removal times are coded. */
pub(crate) fn parse(
    seq: &Sequence,
    r: &mut BitReader<'_>,
    temporal_id: u8,
    spatial_id: u8,
) -> Result<Parsed> {
    let id_len = seq.additional_frame_id_length + seq.delta_frame_id_length;
    let visibility = if seq.reduced_still_picture_header {
        Visibility {
            frame_type: KEY_FRAME,
            show_frame: true,
            showable: false,
            error_resilient: true,
        }
    } else {
        if r.flag()? {
            let idx = r.f(EXISTING_INDEX_BITS)? as u8;
            skip_existing_extras(r, seq, id_len)?;
            return Ok(Parsed::ShowExisting(idx));
        }
        read_visibility(r, seq)?
    };
    if visibility.frame_type != KEY_FRAME && visibility.frame_type != INTRA_ONLY_FRAME {
        return Err(Error::Unsupported(
            if visibility.frame_type == INTER_FRAME {
                "inter frame"
            } else {
                "switch frame"
            },
        ));
    }
    let disable_cdf_update = r.flag()?;
    let allow_sct = if seq.seq_force_screen_content_tools == SELECT_SCREEN_CONTENT_TOOLS {
        r.f(FLAG_BITS)? == 1
    } else {
        seq.seq_force_screen_content_tools == 1
    };
    read_integer_mv(r, seq, allow_sct)?;
    if seq.frame_id_numbers_present {
        r.f(id_len)?; // current_frame_id
    }
    let frame_size_override = !seq.reduced_still_picture_header && r.flag()?;
    let order_hint = r.f(seq.order_hint_bits)? as u32;
    read_buffer_removal_times(r, seq, temporal_id, spatial_id)?;
    let frame_type = visibility.frame_type;
    let refresh_frame_flags = if visibility.show_frame && frame_type == KEY_FRAME {
        ALL_FRAMES
    } else {
        r.f(REFRESH_BITS)? as u8
    };
    if refresh_frame_flags != ALL_FRAMES && seq.enable_order_hint && visibility.error_resilient {
        for _ in 0..NUM_REF_FRAMES {
            r.f(seq.order_hint_bits)?; // ref_order_hint; reference tracking is not kept yet.
        }
    }
    let size = read_frame_size(r, seq, frame_size_override)?;
    let allow_intrabc = allow_sct && size.upscaled_width == size.frame_width && r.flag()?;
    let disable_frame_end_update_cdf = if seq.reduced_still_picture_header || disable_cdf_update {
        true
    } else {
        r.flag()?
    };
    let head = Head {
        visibility,
        disable_cdf_update,
        allow_sct,
        order_hint,
        refresh_frame_flags,
        size,
        allow_intrabc,
        disable_frame_end_update_cdf,
    };
    finish(r, seq, head)
}

/** show_frame, showable_frame, error_resilient_mode and the frame type. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Visibility {
    frame_type: u8,
    show_frame: bool,
    showable: bool,
    error_resilient: bool,
}

/** The fields read before the tile and parameter groups. */
struct Head {
    visibility: Visibility,
    disable_cdf_update: bool,
    allow_sct: bool,
    order_hint: u32,
    refresh_frame_flags: u8,
    size: FrameSize,
    allow_intrabc: bool,
    disable_frame_end_update_cdf: bool,
}

/** Read the tile layout and the parameter groups in spec order, then the reduced transform-set flag and film grain. */
fn finish(r: &mut BitReader<'_>, seq: &Sequence, h: Head) -> Result<Parsed> {
    let tiles = tile_info(r, seq, h.size.mi_cols, h.size.mi_rows)?;
    let color: &ColorConfig = &seq.color;
    let quant = params::quantization(r, color)?;
    let seg = params::segmentation(r)?;
    let (delta_q_present, delta_q_res) = params::delta_q(r, quant.base_q_idx)?;
    let (delta_lf_present, delta_lf_res, delta_lf_multi) =
        params::delta_lf(r, delta_q_present, h.allow_intrabc)?;
    let lossless_array = lossless(&quant, &seg);
    let coded_lossless = lossless_array.iter().all(|&l| l);
    let all_lossless = coded_lossless && h.size.frame_width == h.size.upscaled_width;
    let intrabc_or_lossless = coded_lossless || h.allow_intrabc;
    let loop_filter = params::loop_filter(r, color, intrabc_or_lossless)?;
    let cdef = params::cdef(r, color, intrabc_or_lossless || !seq.enable_cdef)?;
    let lr = params::loop_restoration(
        r,
        color,
        seq.use_128x128_superblock,
        all_lossless || h.allow_intrabc || !seq.enable_restoration,
    )?;
    let tx_mode = params::tx_mode(r, coded_lossless)?;
    let reduced_tx_set = r.flag()?;
    let film_grain_apply = params::film_grain(
        r,
        color,
        seq.film_grain_params_present,
        h.visibility.show_frame || h.visibility.showable,
    )?;
    r.byte_align();
    Ok(Parsed::Frame(Box::new(FrameHeader {
        frame_type: h.visibility.frame_type,
        show_frame: h.visibility.show_frame,
        showable_frame: h.visibility.showable,
        error_resilient_mode: h.visibility.error_resilient,
        disable_cdf_update: h.disable_cdf_update,
        allow_screen_content_tools: h.allow_sct,
        order_hint: h.order_hint,
        refresh_frame_flags: h.refresh_frame_flags,
        size: h.size,
        allow_intrabc: h.allow_intrabc,
        disable_frame_end_update_cdf: h.disable_frame_end_update_cdf,
        tiles,
        quant,
        seg,
        delta_q_present,
        delta_q_res,
        delta_lf_present,
        delta_lf_res,
        delta_lf_multi,
        coded_lossless,
        all_lossless,
        lossless_array,
        loop_filter,
        cdef,
        lr,
        tx_mode,
        reduced_tx_set,
        film_grain_apply,
        header_bytes: r.bit_pos() / 8,
    })))
}

/** show_frame, showable_frame and error_resilient_mode for a non-reduced header. A shown key frame is always error resilient; every other case codes the flag. */
fn read_visibility(r: &mut BitReader<'_>, seq: &Sequence) -> Result<Visibility> {
    let frame_type = r.f(FRAME_TYPE_BITS)? as u8;
    let show_frame = r.flag()?;
    if show_frame && seq.decoder_model_info_present && !seq.equal_picture_interval {
        r.f(seq.frame_presentation_time_length)?;
    }
    let showable = if show_frame {
        frame_type != KEY_FRAME
    } else {
        r.flag()?
    };
    let error_resilient = if frame_type == KEY_FRAME && show_frame {
        true
    } else {
        r.flag()?
    };
    Ok(Visibility {
        frame_type,
        show_frame,
        showable,
        error_resilient,
    })
}

/** The extra fields a show-existing header carries: presentation time and display frame ID. */
fn skip_existing_extras(r: &mut BitReader<'_>, seq: &Sequence, id_len: u32) -> Result<()> {
    if seq.decoder_model_info_present && !seq.equal_picture_interval {
        r.f(seq.frame_presentation_time_length)?;
    }
    if seq.frame_id_numbers_present {
        r.f(id_len)?;
    }
    Ok(())
}

/** force_integer_mv is coded only with screen content tools on and the sequence leaving it to the frame. Intra frames force it on regardless, so the value is not kept. */
fn read_integer_mv(r: &mut BitReader<'_>, seq: &Sequence, allow_sct: bool) -> Result<()> {
    if allow_sct && seq.seq_force_integer_mv == SELECT_INTEGER_MV {
        r.f(FLAG_BITS)?;
    }
    Ok(())
}

/** The per-decoder-model buffer removal times for the operating points that apply to this OBU's layers. */
fn read_buffer_removal_times(
    r: &mut BitReader<'_>,
    seq: &Sequence,
    temporal_id: u8,
    spatial_id: u8,
) -> Result<()> {
    if !seq.decoder_model_info_present || !r.flag()? {
        return Ok(());
    }
    for (op, &idc) in seq.operating_point_idc.iter().enumerate() {
        if !seq.decoder_model_present_for_op[op] {
            continue;
        }
        let in_temporal = (idc >> temporal_id) & 1 == 1;
        let in_spatial = (idc >> (u32::from(spatial_id) + SPATIAL_IDC_SHIFT)) & 1 == 1;
        if idc == 0 || (in_temporal && in_spatial) {
            r.f(seq.buffer_removal_time_length)?;
        }
    }
    Ok(())
}

/** frame_size(), superres_params() and render_size(), in spec order. The frame size is either the sequence maximum or an explicit override. */
fn read_frame_size(
    r: &mut BitReader<'_>,
    seq: &Sequence,
    override_flag: bool,
) -> Result<FrameSize> {
    let (mut width, mut height) = (seq.max_frame_width, seq.max_frame_height);
    if override_flag {
        width = r.f(seq.frame_width_bits)? as u32 + 1;
        height = r.f(seq.frame_height_bits)? as u32 + 1;
    }
    let (coded_width, denom) = superres(r, seq, width)?;
    let (mi_cols, mi_rows) = mode_info_grid(coded_width, height);
    let different = r.flag()?;
    let (render_width, render_height) = if different {
        (
            r.f(RENDER_DIM_BITS)? as u32 + 1,
            r.f(RENDER_DIM_BITS)? as u32 + 1,
        )
    } else {
        (width, height)
    };
    Ok(FrameSize {
        frame_width: coded_width,
        frame_height: height,
        upscaled_width: width,
        render_width,
        render_height,
        superres_denom: denom,
        mi_cols,
        mi_rows,
    })
}

/** superres_params(): returns the downscaled coded width and the denominator. */
fn superres(r: &mut BitReader<'_>, seq: &Sequence, upscaled: u32) -> Result<(u32, u32)> {
    let use_superres = seq.enable_superres && r.flag()?;
    let denom = if use_superres {
        r.f(SUPERRES_DENOM_BITS)? as u32 + SUPERRES_DENOM_MIN
    } else {
        SUPERRES_NUM
    };
    let coded = (upscaled * SUPERRES_NUM + denom / SUPERRES_ROUND_DIV) / denom;
    Ok((coded, denom))
}

/** compute_image_size(): the mode-info grid, rounded up to whole 8x8 blocks and kept even. */
fn mode_info_grid(width: u32, height: u32) -> (u32, u32) {
    let cols = MI_PAIR * ((width + MI_ALIGN_PIXELS) >> MI_PIXEL_SHIFT);
    let rows = MI_PAIR * ((height + MI_ALIGN_PIXELS) >> MI_PIXEL_SHIFT);
    (cols, rows)
}

/** Per-segment lossless flags: a segment is lossless when its quantizer index is zero and every delta is zero. */
fn lossless(q: &Quant, seg: &Segmentation) -> [bool; MAX_SEGMENTS] {
    let mut out = [false; MAX_SEGMENTS];
    let deltas_zero = q.delta_q_y_dc == 0
        && q.delta_q_u_ac == 0
        && q.delta_q_u_dc == 0
        && q.delta_q_v_ac == 0
        && q.delta_q_v_dc == 0;
    for (segment, slot) in out.iter_mut().enumerate() {
        *slot = segment_qindex(q, seg, segment) == 0 && deltas_zero;
    }
    out
}

/** get_qindex(1, segment): the base index, shifted by the segment's quantizer feature when that feature is active. */
fn segment_qindex(q: &Quant, seg: &Segmentation, segment: usize) -> i32 {
    let base = i32::from(q.base_q_idx);
    if seg.enabled && seg.feature_enabled[segment][SEG_LVL_ALT_Q] {
        (base + seg.feature_data[segment][SEG_LVL_ALT_Q]).clamp(0, MAX_QINDEX)
    } else {
        base
    }
}

/** tile_info(): the uniform or explicit tile grid, checked against the tile limits. */
fn tile_info(r: &mut BitReader<'_>, seq: &Sequence, mi_cols: u32, mi_rows: u32) -> Result<Tiles> {
    let geo = Geometry::new(seq, mi_cols, mi_rows);
    let uniform = r.flag()?;
    let mut tiles = if uniform {
        uniform_tiles(r, &geo)?
    } else {
        explicit_tiles(r, &geo)?
    };
    if tiles.cols_log2 > 0 || tiles.rows_log2 > 0 {
        tiles.context_update_tile_id = r.f(tiles.cols_log2 + tiles.rows_log2)? as u32;
        tiles.tile_size_bytes = r.f(TILE_SIZE_BYTES_FIELD_BITS)? as u32 + 1;
    }
    Ok(tiles)
}

/** Superblock grid and tile limits for one frame. */
struct Geometry {
    mi_cols: u32,
    mi_rows: u32,
    sb_cols: u32,
    sb_rows: u32,
    sb_shift: u32,
    min_log2_tiles: u32,
    min_log2_cols: u32,
    max_log2_cols: u32,
    max_log2_rows: u32,
    max_tile_width_sb: u32,
}

impl Geometry {
    /** Derive the superblock grid and the tile limits that tile_info() needs. */
    fn new(seq: &Sequence, mi_cols: u32, mi_rows: u32) -> Self {
        let sb_shift = if seq.use_128x128_superblock {
            SB128_SHIFT
        } else {
            SB64_SHIFT
        };
        let add = (1u32 << sb_shift) - 1;
        let sb_cols = (mi_cols + add) >> sb_shift;
        let sb_rows = (mi_rows + add) >> sb_shift;
        let sb_size = sb_shift + MI_SIZE_LOG2;
        let max_tile_width_sb = MAX_TILE_WIDTH >> sb_size;
        let max_tile_area_sb = MAX_TILE_AREA >> (2 * sb_size);
        let min_log2_cols = tile_log2(max_tile_width_sb, sb_cols);
        Self {
            mi_cols,
            mi_rows,
            sb_cols,
            sb_rows,
            sb_shift,
            min_log2_tiles: min_log2_cols.max(tile_log2(max_tile_area_sb, sb_rows * sb_cols)),
            min_log2_cols,
            max_log2_cols: tile_log2(1, sb_cols.min(MAX_TILE_COLS)),
            max_log2_rows: tile_log2(1, sb_rows.min(MAX_TILE_ROWS)),
            max_tile_width_sb,
        }
    }
}

/** tile_log2(blkSize, target): the smallest k with blkSize << k at least target. */
fn tile_log2(blk_size: u32, target: u32) -> u32 {
    let mut k = 0u32;
    while k < MAX_TILE_LOG2 && (blk_size << k) < target {
        k += 1;
    }
    k
}

/** The uniform tile grid: log2 counts are coded as increments, and tile starts follow from the size. */
fn uniform_tiles(r: &mut BitReader<'_>, g: &Geometry) -> Result<Tiles> {
    let cols_log2 = increments(r, g.min_log2_cols, g.max_log2_cols)?;
    let tile_w = (g.sb_cols + (1 << cols_log2) - 1) >> cols_log2;
    let mi_col_starts = starts(g.sb_cols, tile_w, g.sb_shift, g.mi_cols);
    let min_log2_rows = g.min_log2_tiles.saturating_sub(cols_log2);
    let rows_log2 = increments(r, min_log2_rows, g.max_log2_rows)?;
    let tile_h = (g.sb_rows + (1 << rows_log2) - 1) >> rows_log2;
    let mi_row_starts = starts(g.sb_rows, tile_h, g.sb_shift, g.mi_rows);
    checked_tiles(mi_col_starts, mi_row_starts, cols_log2, rows_log2)
}

/** Coded tile log2 increments: start at the minimum and add one per set bit, up to the maximum. */
fn increments(r: &mut BitReader<'_>, min: u32, max: u32) -> Result<u32> {
    let mut log2 = min;
    while log2 < max && r.flag()? {
        log2 += 1;
    }
    Ok(log2)
}

/** Mode-info start of each tile along one axis, closed by the frame's mode-info count. */
fn starts(sb_count: u32, tile_sb: u32, sb_shift: u32, mi_total: u32) -> Vec<u32> {
    let mut out: Vec<u32> = (0..sb_count)
        .step_by(tile_sb as usize)
        .map(|sb| sb << sb_shift)
        .collect();
    out.push(mi_total);
    out
}

/** An explicit tile grid: each tile's superblock width (and then height) is coded with ns(). */
fn explicit_tiles(r: &mut BitReader<'_>, g: &Geometry) -> Result<Tiles> {
    let mut widest = 0u32;
    let mut mi_col_starts = Vec::new();
    let mut start = 0u32;
    while start < g.sb_cols {
        mi_col_starts.push(start << g.sb_shift);
        let max_width = (g.sb_cols - start).min(g.max_tile_width_sb);
        let size = r.ns(max_width)? + 1;
        widest = widest.max(size);
        start += size;
    }
    mi_col_starts.push(g.mi_cols);
    let cols_log2 = tile_log2(1, (mi_col_starts.len() - 1) as u32);
    let area = if g.min_log2_tiles > 0 {
        (g.sb_rows * g.sb_cols) >> (g.min_log2_tiles + 1)
    } else {
        g.sb_rows * g.sb_cols
    };
    let max_height = (area / widest.max(1)).max(1);
    let mut mi_row_starts = Vec::new();
    let mut start = 0u32;
    while start < g.sb_rows {
        mi_row_starts.push(start << g.sb_shift);
        let max_h = (g.sb_rows - start).min(max_height);
        start += r.ns(max_h)? + 1;
    }
    mi_row_starts.push(g.mi_rows);
    let rows_log2 = tile_log2(1, (mi_row_starts.len() - 1) as u32);
    checked_tiles(mi_col_starts, mi_row_starts, cols_log2, rows_log2)
}

/** Refuse tile grids past the spec's column and row limits, then assemble the layout. */
fn checked_tiles(
    mi_col_starts: Vec<u32>,
    mi_row_starts: Vec<u32>,
    cols_log2: u32,
    rows_log2: u32,
) -> Result<Tiles> {
    let cols = (mi_col_starts.len() - 1) as u32;
    let rows = (mi_row_starts.len() - 1) as u32;
    if cols > MAX_TILE_COLS || rows > MAX_TILE_ROWS {
        return Err(Error::Malformed("tile count"));
    }
    Ok(Tiles {
        cols,
        rows,
        cols_log2,
        rows_log2,
        mi_col_starts,
        mi_row_starts,
        context_update_tile_id: 0,
        tile_size_bytes: 0,
    })
}
