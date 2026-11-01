/** Frame header parameter groups (spec 5.9.12 to 5.9.20 and 5.9.30): quantizer, segmentation, loop filter, CDEF, loop restoration, transform mode and film grain. Intra frames always have no primary reference, so the inter-only branches are not read. */
use crate::av1::{
    DEFAULT_LOOP_FILTER_REF_DELTAS, DELTA_Q_BITS, LOOP_FILTER_MODE_DELTAS, MAX_SEGMENTS, ONLY_4X4,
    REMAP_LR_TYPE, RESTORATION_TILESIZE_MAX, RESTORE_NONE, SEG_LVL_MAX, SEG_LVL_REF_FRAME,
    SEGMENTATION_FEATURE_BITS, SEGMENTATION_FEATURE_MAX, SEGMENTATION_FEATURE_SIGNED,
    TOTAL_REFS_PER_FRAME, TX_MODE_LARGEST, TX_MODE_SELECT,
};
use crate::bits::BitReader;
use crate::error::Result;
use crate::sequence::ColorConfig;

/** Bits of the quantizer, the quantizer matrix levels, and the loop filter and CDEF fields. */
const BASE_Q_IDX_BITS: u32 = 8;
const QM_LEVEL_BITS: u32 = 4;
const LOOP_FILTER_LEVEL_BITS: u32 = 6;
const LOOP_FILTER_SHARPNESS_BITS: u32 = 3;
const LOOP_FILTER_DELTA_BITS: u32 = 7;
const DELTA_RES_BITS: u32 = 2;
const CDEF_DAMPING_BITS: u32 = 2;
const CDEF_BITS_FIELD: u32 = 2;
const CDEF_STRENGTH_BITS: u32 = 4;
const CDEF_SEC_BITS: u32 = 2;
const CDEF_SEC_THREE: u8 = 3;
const CDEF_DAMPING_BASE: u8 = 3;
const LR_TYPE_BITS: u32 = 2;
const LR_SHIFT_BITS: u32 = 1;
const GRAIN_POINT_BITS: u32 = 8;
const GRAIN_COUNT_BITS: u32 = 4;
const GRAIN_COEFF_BITS: u32 = 8;
const GRAIN_OFFSET_BITS: u32 = 9;
const GRAIN_SEED_BITS: u32 = 16;
const GRAIN_SMALL_BITS: u32 = 2;
/** Number of CDEF strength entries a frame may code (1 << cdef_bits, with cdef_bits at most 3). */
const CDEF_MAX_STRENGTHS: usize = 8;
/** Loop filter level slots: luma vertical, luma horizontal, U and V. */
const LOOP_FILTER_LEVELS: usize = 4;
/** Planes that carry a separate loop restoration type. */
const PLANES: usize = 3;
/** The lr_unit_shift and lr_uv_shift arithmetic. */
const LR_UNIT_BASE_SHIFT: u32 = 2;
/** Bits of the transform-mode flag and reduced_tx_set. */
const FLAG_BITS: u32 = 1;

/** Quantizer parameters: the base index, per-plane DC/AC deltas, and quantizer matrix levels. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Quant {
    pub(crate) base_q_idx: u8,
    pub(crate) delta_q_y_dc: i64,
    pub(crate) delta_q_u_dc: i64,
    pub(crate) delta_q_u_ac: i64,
    pub(crate) delta_q_v_dc: i64,
    pub(crate) delta_q_v_ac: i64,
    pub(crate) using_qmatrix: bool,
    pub(crate) qm_y: u8,
    pub(crate) qm_u: u8,
    pub(crate) qm_v: u8,
}

/** quantization_params(): read the base index, then each plane's deltas, then the optional matrix levels. */
pub(crate) fn quantization(r: &mut BitReader<'_>, color: &ColorConfig) -> Result<Quant> {
    let mut q = Quant {
        base_q_idx: r.f(BASE_Q_IDX_BITS)? as u8,
        ..Quant::default()
    };
    q.delta_q_y_dc = read_delta_q(r)?;
    if color.num_planes() > 1 {
        let diff_uv_delta = color.separate_uv_delta_q && r.flag()?;
        q.delta_q_u_dc = read_delta_q(r)?;
        q.delta_q_u_ac = read_delta_q(r)?;
        if diff_uv_delta {
            q.delta_q_v_dc = read_delta_q(r)?;
            q.delta_q_v_ac = read_delta_q(r)?;
        } else {
            q.delta_q_v_dc = q.delta_q_u_dc;
            q.delta_q_v_ac = q.delta_q_u_ac;
        }
    }
    q.using_qmatrix = r.flag()?;
    if q.using_qmatrix {
        q.qm_y = r.f(QM_LEVEL_BITS)? as u8;
        q.qm_u = r.f(QM_LEVEL_BITS)? as u8;
        q.qm_v = if color.separate_uv_delta_q {
            r.f(QM_LEVEL_BITS)? as u8
        } else {
            q.qm_u
        };
    }
    Ok(q)
}

/** read_delta_q(): a presence flag, then a signed 7-bit delta when present. */
fn read_delta_q(r: &mut BitReader<'_>) -> Result<i64> {
    if r.flag()? { r.su(DELTA_Q_BITS) } else { Ok(0) }
}

/** Segmentation parameters. Intra frames always start with update_map and update_data set. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Segmentation {
    pub(crate) enabled: bool,
    pub(crate) feature_enabled: [[bool; SEG_LVL_MAX]; MAX_SEGMENTS],
    pub(crate) feature_data: [[i32; SEG_LVL_MAX]; MAX_SEGMENTS],
    pub(crate) seg_id_pre_skip: bool,
    pub(crate) last_active_seg_id: u8,
}

impl Default for Segmentation {
    /** Segmentation off, with every feature cleared. */
    fn default() -> Self {
        Self {
            enabled: false,
            feature_enabled: [[false; SEG_LVL_MAX]; MAX_SEGMENTS],
            feature_data: [[0; SEG_LVL_MAX]; MAX_SEGMENTS],
            seg_id_pre_skip: false,
            last_active_seg_id: 0,
        }
    }
}

/** segmentation_params(): the enabled flag and, when data updates, each segment's feature values. */
pub(crate) fn segmentation(r: &mut BitReader<'_>) -> Result<Segmentation> {
    let mut seg = Segmentation {
        enabled: r.flag()?,
        ..Segmentation::default()
    };
    if seg.enabled {
        read_features(r, &mut seg)?;
    }
    for i in 0..MAX_SEGMENTS {
        for j in 0..SEG_LVL_MAX {
            if seg.feature_enabled[i][j] {
                seg.last_active_seg_id = i as u8;
                seg.seg_id_pre_skip |= j >= SEG_LVL_REF_FRAME;
            }
        }
    }
    Ok(seg)
}

/** Read every segment feature's enable flag and clipped value. */
fn read_features(r: &mut BitReader<'_>, seg: &mut Segmentation) -> Result<()> {
    for i in 0..MAX_SEGMENTS {
        for j in 0..SEG_LVL_MAX {
            let enabled = r.flag()?;
            seg.feature_enabled[i][j] = enabled;
            seg.feature_data[i][j] = if enabled { read_feature(r, j)? } else { 0 };
        }
    }
    Ok(())
}

/** One feature value, read as signed or unsigned and clipped to the feature's range. */
fn read_feature(r: &mut BitReader<'_>, j: usize) -> Result<i32> {
    let bits = SEGMENTATION_FEATURE_BITS[j];
    let limit = SEGMENTATION_FEATURE_MAX[j];
    if SEGMENTATION_FEATURE_SIGNED[j] {
        let value = r.su(1 + bits)? as i32;
        Ok(value.clamp(-limit, limit))
    } else {
        let value = r.f(bits)? as i32;
        Ok(value.clamp(0, limit))
    }
}

/** delta_q_params(): the delta-q presence flag and resolution, read only when the base index is above zero. */
pub(crate) fn delta_q(r: &mut BitReader<'_>, base_q_idx: u8) -> Result<(bool, u32)> {
    if base_q_idx == 0 || !r.flag()? {
        return Ok((false, 0));
    }
    Ok((true, r.f(DELTA_RES_BITS)? as u32))
}

/** delta_lf_params(): the delta-lf presence, resolution and multi flag, read only when delta-q is present and intra block copy is off. */
pub(crate) fn delta_lf(
    r: &mut BitReader<'_>,
    delta_q_present: bool,
    allow_intrabc: bool,
) -> Result<(bool, u32, bool)> {
    if !delta_q_present || allow_intrabc || !r.flag()? {
        return Ok((false, 0, false));
    }
    let res = r.f(DELTA_RES_BITS)? as u32;
    let multi = r.flag()?;
    Ok((true, res, multi))
}

/** Loop filter levels, sharpness and reference deltas. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LoopFilter {
    pub(crate) level: [u8; LOOP_FILTER_LEVELS],
    pub(crate) sharpness: u8,
    pub(crate) delta_enabled: bool,
    pub(crate) ref_deltas: [i32; TOTAL_REFS_PER_FRAME],
    pub(crate) mode_deltas: [i32; LOOP_FILTER_MODE_DELTAS],
}

/** loop_filter_params(): zeroed for lossless or intra-block-copy frames, otherwise read with optional delta updates. */
pub(crate) fn loop_filter(
    r: &mut BitReader<'_>,
    color: &ColorConfig,
    disabled: bool,
) -> Result<LoopFilter> {
    let mut lf = LoopFilter {
        level: [0; LOOP_FILTER_LEVELS],
        sharpness: 0,
        delta_enabled: false,
        ref_deltas: DEFAULT_LOOP_FILTER_REF_DELTAS,
        mode_deltas: [0; LOOP_FILTER_MODE_DELTAS],
    };
    if disabled {
        return Ok(lf);
    }
    lf.level[0] = r.f(LOOP_FILTER_LEVEL_BITS)? as u8;
    lf.level[1] = r.f(LOOP_FILTER_LEVEL_BITS)? as u8;
    if color.num_planes() > 1 && (lf.level[0] != 0 || lf.level[1] != 0) {
        lf.level[2] = r.f(LOOP_FILTER_LEVEL_BITS)? as u8;
        lf.level[3] = r.f(LOOP_FILTER_LEVEL_BITS)? as u8;
    }
    lf.sharpness = r.f(LOOP_FILTER_SHARPNESS_BITS)? as u8;
    lf.delta_enabled = r.flag()?;
    if lf.delta_enabled && r.flag()? {
        read_loop_filter_deltas(r, &mut lf)?;
    }
    Ok(lf)
}

/** The optional reference and mode delta updates of loop_filter_params. */
fn read_loop_filter_deltas(r: &mut BitReader<'_>, lf: &mut LoopFilter) -> Result<()> {
    for slot in lf.ref_deltas.iter_mut() {
        if r.flag()? {
            *slot = r.su(1 + LOOP_FILTER_DELTA_BITS)? as i32;
        }
    }
    for slot in lf.mode_deltas.iter_mut() {
        if r.flag()? {
            *slot = r.su(1 + LOOP_FILTER_DELTA_BITS)? as i32;
        }
    }
    Ok(())
}

/** CDEF damping, bit count and per-strength primary and secondary values for luma and chroma. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Cdef {
    pub(crate) damping: u8,
    pub(crate) bits: u8,
    pub(crate) y_pri: [u8; CDEF_MAX_STRENGTHS],
    pub(crate) y_sec: [u8; CDEF_MAX_STRENGTHS],
    pub(crate) uv_pri: [u8; CDEF_MAX_STRENGTHS],
    pub(crate) uv_sec: [u8; CDEF_MAX_STRENGTHS],
}

/** cdef_params(): all zero (and damping 3) when CDEF cannot run, otherwise the coded strengths. */
pub(crate) fn cdef(r: &mut BitReader<'_>, color: &ColorConfig, disabled: bool) -> Result<Cdef> {
    let mut c = Cdef {
        damping: CDEF_DAMPING_BASE,
        bits: 0,
        y_pri: [0; CDEF_MAX_STRENGTHS],
        y_sec: [0; CDEF_MAX_STRENGTHS],
        uv_pri: [0; CDEF_MAX_STRENGTHS],
        uv_sec: [0; CDEF_MAX_STRENGTHS],
    };
    if disabled {
        return Ok(c);
    }
    c.damping = r.f(CDEF_DAMPING_BITS)? as u8 + CDEF_DAMPING_BASE;
    c.bits = r.f(CDEF_BITS_FIELD)? as u8;
    for i in 0..(1usize << c.bits) {
        c.y_pri[i] = r.f(CDEF_STRENGTH_BITS)? as u8;
        c.y_sec[i] = read_secondary(r)?;
        if color.num_planes() > 1 {
            c.uv_pri[i] = r.f(CDEF_STRENGTH_BITS)? as u8;
            c.uv_sec[i] = read_secondary(r)?;
        }
    }
    Ok(c)
}

/** A CDEF secondary strength; the coded value 3 stands for 4. */
fn read_secondary(r: &mut BitReader<'_>) -> Result<u8> {
    let v = r.f(CDEF_SEC_BITS)? as u8;
    Ok(if v == CDEF_SEC_THREE { v + 1 } else { v })
}

/** Loop restoration type and unit size per plane. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LoopRestoration {
    pub(crate) types: [u8; PLANES],
    pub(crate) uses_lr: bool,
    pub(crate) unit_size: [u32; PLANES],
}

/** lr_params(): restoration types per plane and the unit size, with chroma shifted when subsampled. */
pub(crate) fn loop_restoration(
    r: &mut BitReader<'_>,
    color: &ColorConfig,
    seq_128: bool,
    no_restoration: bool,
) -> Result<LoopRestoration> {
    let mut lr = LoopRestoration {
        types: [RESTORE_NONE; PLANES],
        uses_lr: false,
        unit_size: [0; PLANES],
    };
    if no_restoration {
        return Ok(lr);
    }
    let mut uses_chroma = false;
    for i in 0..color.num_planes() {
        lr.types[i] = REMAP_LR_TYPE[r.f(LR_TYPE_BITS)? as usize];
        if lr.types[i] != RESTORE_NONE {
            lr.uses_lr = true;
            uses_chroma |= i > 0;
        }
    }
    if lr.uses_lr {
        let shift = read_unit_shift(r, seq_128)?;
        lr.unit_size[0] = RESTORATION_TILESIZE_MAX >> (LR_UNIT_BASE_SHIFT - shift);
        let uv_shift = if color.subsampling_x && color.subsampling_y && uses_chroma {
            r.f(FLAG_BITS)? as u32
        } else {
            0
        };
        lr.unit_size[1] = lr.unit_size[0] >> uv_shift;
        lr.unit_size[2] = lr.unit_size[0] >> uv_shift;
    }
    Ok(lr)
}

/** lr_unit_shift, with the 128x128 superblock path always adding one. */
fn read_unit_shift(r: &mut BitReader<'_>, seq_128: bool) -> Result<u32> {
    if seq_128 {
        return Ok(r.f(LR_SHIFT_BITS)? as u32 + 1);
    }
    let mut shift = r.f(LR_SHIFT_BITS)? as u32;
    if shift == 1 {
        shift += r.f(LR_SHIFT_BITS)? as u32;
    }
    Ok(shift)
}

/** read_tx_mode(): only four-by-four transforms when the frame is coded lossless, otherwise one bit selects. */
pub(crate) fn tx_mode(r: &mut BitReader<'_>, coded_lossless: bool) -> Result<u8> {
    if coded_lossless {
        return Ok(ONLY_4X4);
    }
    Ok(if r.flag()? {
        TX_MODE_SELECT
    } else {
        TX_MODE_LARGEST
    })
}

/** film_grain_params(): read every field so the header ends in the right place. Returns whether grain is applied; Mars does not synthesise grain yet. */
pub(crate) fn film_grain(
    r: &mut BitReader<'_>,
    color: &ColorConfig,
    present: bool,
    shown_or_showable: bool,
) -> Result<bool> {
    if !present || !shown_or_showable {
        return Ok(false);
    }
    if !r.flag()? {
        return Ok(false);
    }
    r.f(GRAIN_SEED_BITS)?;
    let num_y = read_points(r)?;
    let csfl = if color.mono_chrome { false } else { r.flag()? };
    let (num_cb, num_cr) = if color.mono_chrome
        || csfl
        || (color.subsampling_x && color.subsampling_y && num_y == 0)
    {
        (0, 0)
    } else {
        (read_points(r)?, read_points(r)?)
    };
    r.f(GRAIN_SMALL_BITS)?; // grain_scaling_minus_8
    let lag = r.f(GRAIN_SMALL_BITS)? as u32;
    let pos_luma = 2 * lag * (lag + 1);
    let pos_chroma = if num_y > 0 { pos_luma + 1 } else { pos_luma };
    if num_y > 0 {
        read_coeffs(r, pos_luma)?;
    }
    if csfl || num_cb > 0 {
        read_coeffs(r, pos_chroma)?;
    }
    if csfl || num_cr > 0 {
        read_coeffs(r, pos_chroma)?;
    }
    r.f(GRAIN_SMALL_BITS)?; // ar_coeff_shift_minus_6
    r.f(GRAIN_SMALL_BITS)?; // grain_scale_shift
    if num_cb > 0 {
        read_chroma_mults(r)?;
    }
    if num_cr > 0 {
        read_chroma_mults(r)?;
    }
    r.f(FLAG_BITS)?; // overlap_flag
    r.f(FLAG_BITS)?; // clip_to_restricted_range
    Ok(true)
}

/** Read `count` auto-regressive coefficients one at a time, since each is 8 bits and a wide read would exceed the bit reader. */
fn read_coeffs(r: &mut BitReader<'_>, count: u32) -> Result<()> {
    for _ in 0..count {
        r.f(GRAIN_COEFF_BITS)?;
    }
    Ok(())
}

/** One plane's grain multiplier, luma multiplier and offset. */
fn read_chroma_mults(r: &mut BitReader<'_>) -> Result<()> {
    r.f(GRAIN_COEFF_BITS)?;
    r.f(GRAIN_COEFF_BITS)?;
    r.f(GRAIN_OFFSET_BITS)?;
    Ok(())
}

/** A point list of film grain: a count, then value and scaling pairs. Returns the count. */
fn read_points(r: &mut BitReader<'_>) -> Result<u32> {
    let count = r.f(GRAIN_COUNT_BITS)? as u32;
    for _ in 0..count {
        r.f(GRAIN_POINT_BITS)?;
        r.f(GRAIN_POINT_BITS)?;
    }
    Ok(count)
}
