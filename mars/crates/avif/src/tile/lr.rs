/** Loop-restoration syntax of a tile (spec 5.11.57, 5.11.58): which restoration unit each superblock covers, the type and coefficients coded for each unit, and the sub-exponential codes the coefficients use. */
use super::constants::{
    SGR_PASSES, SGRPROJ_PARAMS_BITS, SGRPROJ_PRJ_BITS, SGRPROJ_PRJ_SUBEXP_K, WIENER_COEFFS,
};
use super::decoder::TileDecoder;
use super::state::LrPlane;
use super::tables::{
    SGR_PARAMS, SGRPROJ_XQD_MAX, SGRPROJ_XQD_MID, SGRPROJ_XQD_MIN, WIENER_TAPS_K, WIENER_TAPS_MAX,
    WIENER_TAPS_MID, WIENER_TAPS_MIN,
};
use crate::av1::{RESTORE_NONE, RESTORE_SGRPROJ, RESTORE_WIENER};
use crate::error::{Error, Result};
use crate::frame::FrameHeader;
use crate::sequence::Sequence;

/** Superres numerator of the spec's column arithmetic when superres is off, and the 4x4 unit in samples. */
const SUPERRES_NUM: usize = 8;
const MI_SAMPLES: usize = 4;
/** Longest prefix a sub-exponential code may take before the value is malformed. */
const SUBEXP_MAX_STEPS: u32 = 16;

/** One restoration unit: its type, the Wiener taps of each pass, the self-guided set and its two projection weights. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LrUnit {
    pub(crate) kind: u8,
    pub(crate) wiener: [[i32; WIENER_COEFFS]; SGR_PASSES],
    pub(crate) sgr_set: u8,
    pub(crate) sgr_xqd: [i32; SGR_PASSES],
}

/** The reference values each plane's next coded coefficient is coded against, updated as units are read. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LrRefs {
    wiener: [[[i32; WIENER_COEFFS]; SGR_PASSES]; 3],
    sgr_xqd: [[i32; SGR_PASSES]; 3],
}

impl LrRefs {
    /** The references a tile starts from: the spec's Wiener and self-guided midpoints for every plane. */
    pub(crate) fn new() -> Self {
        let wiener_mid = WIENER_TAPS_MID.map(i32::from);
        let sgr_mid = SGRPROJ_XQD_MID.map(i32::from);
        Self {
            wiener: [[wiener_mid; SGR_PASSES]; 3],
            sgr_xqd: [sgr_mid; 3],
        }
    }
}

/** Units of a plane of the frame: the count is the spec's count_units_in_frame on the plane's size, and a plane without restoration has none. */
pub(crate) fn plane_units(hdr: &FrameHeader, seq: &Sequence, plane: usize) -> LrPlane {
    if hdr.lr.types[plane] == RESTORE_NONE {
        return LrPlane::default();
    }
    let (sub_x, sub_y) = plane_subsampling(seq, plane);
    let size = hdr.lr.unit_size[plane] as usize;
    let rows = count_units(size, round2(hdr.size.frame_height as usize, sub_y));
    let cols = count_units(size, round2(hdr.size.upscaled_width as usize, sub_x));
    LrPlane {
        unit_rows: rows,
        unit_cols: cols,
        units: vec![LrUnit::default(); rows * cols],
    }
}

/** count_units_in_frame(): how many restoration units fit, rounding the last partial unit to the one before it, and at least one. */
fn count_units(unit_size: usize, frame_size: usize) -> usize {
    ((frame_size + (unit_size >> 1)) / unit_size).max(1)
}

/** Round2(x, n): x divided by 2^n with rounding to nearest, for n of zero or more. */
fn round2(x: usize, n: usize) -> usize {
    if n == 0 { x } else { (x + (1 << (n - 1))) >> n }
}

/** Subsampling shifts of a plane: none for luma, the sequence's for chroma. */
pub(crate) fn plane_subsampling(seq: &Sequence, plane: usize) -> (usize, usize) {
    if plane == 0 {
        (0, 0)
    } else {
        (
            usize::from(seq.color.subsampling_x),
            usize::from(seq.color.subsampling_y),
        )
    }
}

impl<'a> TileDecoder<'a> {
    /** read_lr(r, c, bSize): read the units whose top-left corner falls inside the superblock at (r, c), for every plane that uses restoration. */
    pub(super) fn read_lr(&mut self, r: usize, c: usize, bsize_mi: usize) -> Result<()> {
        if self.hdr.allow_intrabc {
            return Ok(());
        }
        for plane in 0..self.seq.color.num_planes() {
            if self.hdr.lr.types[plane] != RESTORE_NONE {
                self.read_plane_lr(plane, r, c, bsize_mi)?;
            }
        }
        Ok(())
    }

    /** The units of one plane that a superblock covers, read in raster order. */
    fn read_plane_lr(&mut self, plane: usize, r: usize, c: usize, sb_mi: usize) -> Result<()> {
        let (sub_x, sub_y) = plane_subsampling(self.seq, plane);
        let size = self.hdr.lr.unit_size[plane] as usize;
        let units = &self.fs.lr[plane];
        let (rows, cols) = (units.unit_rows, units.unit_cols);
        let row_start = (r * (MI_SAMPLES >> sub_y)).div_ceil(size);
        let row_end = rows.min(((r + sb_mi) * (MI_SAMPLES >> sub_y)).div_ceil(size));
        let denom_scale = self.hdr.size.superres_denom as usize;
        let numerator = (MI_SAMPLES >> sub_x) * denom_scale;
        let denominator = size * SUPERRES_NUM;
        let col_start = (c * numerator).div_ceil(denominator);
        let col_end = cols.min(((c + sb_mi) * numerator).div_ceil(denominator));
        for unit_row in row_start..row_end {
            for unit_col in col_start..col_end {
                let unit = self.read_lr_unit(plane, unit_row, unit_col)?;
                self.fs.lr[plane].units[unit_row * cols + unit_col] = unit;
            }
        }
        Ok(())
    }

    /** read_lr_unit(): the unit's type, then its coefficients for that type. */
    fn read_lr_unit(&mut self, plane: usize, _row: usize, _col: usize) -> Result<LrUnit> {
        let kind = match self.hdr.lr.types[plane] {
            RESTORE_WIENER => self.read_flag_type(RESTORE_WIENER, true)?,
            RESTORE_SGRPROJ => self.read_flag_type(RESTORE_SGRPROJ, false)?,
            _ => self.sym.read_symbol(&mut self.cdf.restoration_type)? as u8,
        };
        let mut unit = LrUnit {
            kind,
            ..LrUnit::default()
        };
        if unit.kind == RESTORE_WIENER {
            self.read_wiener(plane, &mut unit)?;
        } else if unit.kind == RESTORE_SGRPROJ {
            self.read_sgrproj(plane, &mut unit)?;
        }
        Ok(unit)
    }

    /** A plane whose type is fixed to one filter codes a yes-or-no flag for it. */
    fn read_flag_type(&mut self, filter: u8, wiener: bool) -> Result<u8> {
        let on = if wiener {
            self.sym.read_symbol(&mut self.cdf.use_wiener)?
        } else {
            self.sym.read_symbol(&mut self.cdf.use_sgrproj)?
        };
        Ok(if on == 1 { filter } else { RESTORE_NONE })
    }

    /** The Wiener taps of both passes. Chroma codes only taps one and two; the first is zero. */
    fn read_wiener(&mut self, plane: usize, unit: &mut LrUnit) -> Result<()> {
        for pass in 0..SGR_PASSES {
            let first = if plane > 0 { 1 } else { 0 };
            if first == 1 {
                unit.wiener[pass][0] = 0;
            }
            for j in first..WIENER_COEFFS {
                let low = i32::from(WIENER_TAPS_MIN[j]);
                let high = i32::from(WIENER_TAPS_MAX[j]) + 1;
                let reference = self.lr_ref.wiener[plane][pass][j];
                let v = self.decode_signed_subexp(low, high, WIENER_TAPS_K[j] as u32, reference)?;
                unit.wiener[pass][j] = v;
                self.lr_ref.wiener[plane][pass][j] = v;
            }
        }
        Ok(())
    }

    /** The self-guided set and its two projection weights. A zero radius codes no weight; the second weight then follows from the first. */
    fn read_sgrproj(&mut self, plane: usize, unit: &mut LrUnit) -> Result<()> {
        let set = self.sym.read_literal(SGRPROJ_PARAMS_BITS)? as usize;
        unit.sgr_set = set as u8;
        for i in 0..SGR_PASSES {
            let low = i32::from(SGRPROJ_XQD_MIN[i]);
            let high = i32::from(SGRPROJ_XQD_MAX[i]) + 1;
            let v = if SGR_PARAMS[set][i * 2] != 0 {
                let reference = self.lr_ref.sgr_xqd[plane][i];
                self.decode_signed_subexp(low, high, SGRPROJ_PRJ_SUBEXP_K, reference)?
            } else if i == 1 {
                let target = (1 << SGRPROJ_PRJ_BITS) - self.lr_ref.sgr_xqd[plane][0];
                target.clamp(low, high - 1)
            } else {
                0
            };
            unit.sgr_xqd[i] = v;
            self.lr_ref.sgr_xqd[plane][i] = v;
        }
        Ok(())
    }

    /** decode_signed_subexp_with_ref_bool(low, high, k, r): a value in [low, high) coded relative to the reference r. */
    fn decode_signed_subexp(&mut self, low: i32, high: i32, k: u32, reference: i32) -> Result<i32> {
        let x = self.decode_unsigned_subexp(high - low, k, reference - low)?;
        Ok(x + low)
    }

    /** decode_unsigned_subexp_with_ref_bool(mx, k, r): the value folded around the reference so small changes cost few bits. */
    fn decode_unsigned_subexp(&mut self, mx: i32, k: u32, r: i32) -> Result<i32> {
        let v = self.decode_subexp(mx, k)?;
        if (r << 1) <= mx {
            Ok(inverse_recenter(r, v))
        } else {
            Ok(mx - 1 - inverse_recenter(mx - 1 - r, v))
        }
    }

    /** decode_subexp_bool(numSyms, k): an Elias-style prefix of growing ranges, ending in a non-symmetric or literal tail. */
    fn decode_subexp(&mut self, num_syms: i32, k: u32) -> Result<i32> {
        let mut i = 0u32;
        let mut mk = 0i32;
        loop {
            if i > SUBEXP_MAX_STEPS {
                return Err(Error::Malformed("loop restoration coefficient"));
            }
            let b2 = if i > 0 { k + i - 1 } else { k };
            let a = 1i32 << b2;
            if num_syms <= mk + 3 * a {
                let v = self.sym.read_ns((num_syms - mk) as u32)? as i32;
                return Ok(v + mk);
            }
            if self.sym.read_literal(1)? == 0 {
                let v = self.sym.read_literal(b2)? as i32;
                return Ok(v + mk);
            }
            i += 1;
            mk += a;
        }
    }
}

/** inverse_recenter(r, v): the value v re-ordered so that small codes are the values nearest the reference r. */
pub(super) fn inverse_recenter(r: i32, v: i32) -> i32 {
    if v > 2 * r {
        v
    } else if v & 1 == 1 {
        r - ((v + 1) >> 1)
    } else {
        r + (v >> 1)
    }
}
