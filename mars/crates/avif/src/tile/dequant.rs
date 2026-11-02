/** Dequantisation of one transform block (spec get_qindex, get_dc_quant, get_ac_quant and the dequantisation step of the reconstruct process, 7.12.3): the quantiser index a block uses, the step sizes of each plane, and the scaled coefficients that are stored for reconstruction. */
use super::constants::{
    BITDEPTH_MIN, DEQUANT_CLIP_BASE_BITS, DQ_DENOM_HALF, DQ_DENOM_QUARTER, DQ_MAGNITUDE_MASK, IDTX,
    QLOOKUP_MAX_INDEX, TX_16X32, TX_16X64, TX_32X16, TX_32X32, TX_32X64, TX_64X16, TX_64X32,
    TX_64X64,
};
use super::decoder::TileDecoder;
use super::tables::{AC_QLOOKUP, DC_QLOOKUP, TX_HEIGHT, TX_WIDTH};
use crate::av1::SEG_LVL_ALT_Q;
use crate::error::{Error, Result};

/** The quantiser-matrix level at which the spec applies no matrix. */
const QM_LEVEL_NONE: u8 = 15;

impl<'a> TileDecoder<'a> {
    /** get_qindex(ignoreDeltaQ, segmentId): the quantiser index of a segment, moved by the segment's quantiser feature and, unless ignored, by the running delta. */
    pub(super) fn qindex(&self, ignore_delta: bool, segment: usize) -> i32 {
        let delta_active = !ignore_delta && self.hdr.delta_q_present;
        let base = if delta_active {
            self.dq.current_qindex
        } else {
            i32::from(self.hdr.quant.base_q_idx)
        };
        if self.hdr.seg.enabled && self.hdr.seg.feature_enabled[segment][SEG_LVL_ALT_Q] {
            let data = self.hdr.seg.feature_data[segment][SEG_LVL_ALT_Q];
            return (base + data).clamp(0, QLOOKUP_MAX_INDEX);
        }
        base
    }

    /** The dequantised, non-zero coefficients of the transform block just read, as (raster position, value) pairs over its coded size (spec 7.12.3 step 1). */
    pub(super) fn dequantise(
        &self,
        plane: usize,
        tx: usize,
        tx_type: u8,
    ) -> Result<Vec<(u16, i32)>> {
        self.refuse_quantizer_matrix(plane, tx_type)?;
        let (dc_q, ac_q) = self.quantisers(plane);
        let bd = u32::from(self.seq.color.bit_depth);
        let limit = 1i64 << (DEQUANT_CLIP_BASE_BITS + bd);
        let denom = dequant_denominator(tx);
        let (tw, th) = (
            (TX_WIDTH[tx] as usize).min(32),
            (TX_HEIGHT[tx] as usize).min(32),
        );
        let mut coeffs = Vec::new();
        for pos in 0..tw * th {
            let level = i64::from(self.quant[pos]);
            if level == 0 {
                continue;
            }
            let step = if pos == 0 { dc_q } else { ac_q };
            let product = level * step;
            let magnitude = (product.abs() & DQ_MAGNITUDE_MASK) / denom;
            let value = if product < 0 { -magnitude } else { magnitude };
            coeffs.push((pos as u16, value.clamp(-limit, limit - 1) as i32));
        }
        Ok(coeffs)
    }

    /** Quantiser matrices change the step of each coefficient, and their tables are not carried yet, so a block that would use one is refused rather than decoded wrongly. */
    fn refuse_quantizer_matrix(&self, plane: usize, tx_type: u8) -> Result<()> {
        let q = &self.hdr.quant;
        if !q.using_qmatrix || self.blk.lossless || tx_type >= IDTX {
            return Ok(());
        }
        let level = match plane {
            0 => q.qm_y,
            1 => q.qm_u,
            _ => q.qm_v,
        };
        if level < QM_LEVEL_NONE {
            return Err(Error::Unsupported("quantizer matrix"));
        }
        Ok(())
    }

    /** The DC and AC step sizes for a plane at the block's quantiser index, with the plane's coded deltas applied. */
    fn quantisers(&self, plane: usize) -> (i64, i64) {
        let q = &self.hdr.quant;
        let index = self.qindex(false, self.blk.segment_id);
        let (dc_delta, ac_delta) = match plane {
            0 => (q.delta_q_y_dc, 0),
            1 => (q.delta_q_u_dc, q.delta_q_u_ac),
            _ => (q.delta_q_v_dc, q.delta_q_v_ac),
        };
        let depth = (usize::from(self.seq.color.bit_depth) - BITDEPTH_MIN as usize) >> 1;
        let dc = DC_QLOOKUP[depth][clamp_index(index as i64 + dc_delta)];
        let ac = AC_QLOOKUP[depth][clamp_index(index as i64 + ac_delta)];
        (i64::from(dc), i64::from(ac))
    }
}

/** The quantiser table index for a value, clamped to the table's range. */
fn clamp_index(value: i64) -> usize {
    value.clamp(0, i64::from(QLOOKUP_MAX_INDEX)) as usize
}

/** dqDenom: the divisor a transform size applies to its dequantised values, two for the 32-sample rectangles and four for the 64-sample ones. */
fn dequant_denominator(tx: usize) -> i64 {
    match tx {
        TX_32X32 | TX_16X32 | TX_32X16 | TX_16X64 | TX_64X16 => DQ_DENOM_HALF,
        TX_64X64 | TX_64X32 | TX_32X64 => DQ_DENOM_QUARTER,
        _ => 1,
    }
}
