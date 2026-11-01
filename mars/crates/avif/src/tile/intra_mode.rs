/** Intra frame mode info, the part before the prediction modes (spec intra_frame_mode_info, intra_segment_id, read_segment_id, read_skip, read_cdef, read_delta_qindex and read_delta_lf): segment ids, the skip flag, the CDEF index and the delta-q and delta-lf values of a superblock. */
use super::constants::{
    BLOCK_64X64, BLOCK_128X128, CDEF_UNIT_MI, DELTA_ESCAPE_BITS, DELTA_LF_SMALL, DELTA_Q_SMALL,
    FRAME_LF_COUNT, MAX_LOOP_FILTER, SEG_LVL_SKIP,
};
use super::decoder::TileDecoder;
use crate::av1::MAX_QINDEX;
use crate::error::{Error, Result};

/** Sentinel for an unavailable neighbour in the segment-id prediction. */
const NO_SEGMENT: i32 = -1;
/** The lowest and highest quantiser index a delta may move the running index to. */
const QINDEX_MIN: i32 = 1;

impl<'a> TileDecoder<'a> {
    /** intra_frame_mode_info(): everything read ahead of the prediction modes. An intra block copy block is refused, since its motion syntax is not read yet. */
    pub(super) fn intra_frame_mode_info(&mut self) -> Result<()> {
        self.blk.skip = false;
        if self.hdr.seg.seg_id_pre_skip {
            self.intra_segment_id()?;
        }
        self.read_skip()?;
        if !self.hdr.seg.seg_id_pre_skip {
            self.intra_segment_id()?;
        }
        self.read_cdef()?;
        self.read_delta_qindex()?;
        self.read_delta_lf()?;
        self.dq.read_deltas = false;
        if self.hdr.allow_intrabc && self.sym.read_symbol(&mut self.cdf.intrabc)? == 1 {
            return Err(Error::Unsupported("intra block copy"));
        }
        self.intra_modes()
    }

    /** intra_segment_id(): the segment of the block, read when segmentation is on, and the block's lossless flag from it. */
    fn intra_segment_id(&mut self) -> Result<()> {
        self.blk.segment_id = if self.hdr.seg.enabled {
            self.read_segment_id()?
        } else {
            0
        };
        // The id is clipped to the active segments when it is read, so this lookup only fails if that clip is missing.
        self.blk.lossless = *self
            .hdr
            .lossless_array
            .get(self.blk.segment_id)
            .ok_or(Error::Malformed("segment id"))?;
        Ok(())
    }

    /** read_segment_id(): the segment is coded as a difference from a prediction taken from the neighbours; a skipped block takes the prediction itself. */
    fn read_segment_id(&mut self) -> Result<usize> {
        let (prev_ul, prev_u, prev_l) = self.neighbour_segments();
        let pred = segment_prediction(prev_ul, prev_u, prev_l);
        if self.blk.skip {
            return Ok(pred as usize);
        }
        let ctx = segment_ctx(prev_ul, prev_u, prev_l);
        let coded = self.sym.read_symbol(&mut self.cdf.segment_id[ctx])? as i32;
        Ok(coded_segment(coded, pred, self.hdr.seg.last_active_seg_id))
    }

    /** The segment ids of the above-left, above and left neighbours, or NO_SEGMENT for one outside the tile. */
    fn neighbour_segments(&self) -> (i32, i32, i32) {
        let (r, c) = (self.blk.mi_row, self.blk.mi_col);
        let seg = |row: usize, col: usize| i32::from(self.fs.mi.at(row, col).segment_id);
        let ul = if self.blk.avail_u && self.blk.avail_l {
            seg(r - 1, c - 1)
        } else {
            NO_SEGMENT
        };
        let u = if self.blk.avail_u {
            seg(r - 1, c)
        } else {
            NO_SEGMENT
        };
        let l = if self.blk.avail_l {
            seg(r, c - 1)
        } else {
            NO_SEGMENT
        };
        (ul, u, l)
    }

    /** seg_feature_active(feature) for the block's segment. */
    pub(super) fn seg_feature_active(&self, feature: usize) -> bool {
        self.hdr.seg.enabled
            && self
                .hdr
                .seg
                .feature_enabled
                .get(self.blk.segment_id)
                .is_some_and(|features| features[feature])
    }

    /** read_skip(): the skip flag, forced on when the segment's skip feature is active before the segment id is read. */
    fn read_skip(&mut self) -> Result<()> {
        if self.hdr.seg.seg_id_pre_skip && self.seg_feature_active(SEG_LVL_SKIP) {
            self.blk.skip = true;
            return Ok(());
        }
        let (r, c) = (self.blk.mi_row, self.blk.mi_col);
        let mut ctx = 0;
        if self.blk.avail_u {
            ctx += usize::from(self.fs.mi.at(r - 1, c).skip);
        }
        if self.blk.avail_l {
            ctx += usize::from(self.fs.mi.at(r, c - 1).skip);
        }
        self.blk.skip = self.sym.read_symbol(&mut self.cdf.skip[ctx])? == 1;
        Ok(())
    }

    /** read_cdef(): the 64x64 unit's CDEF strength index, read with the first block of the unit that has coefficients. */
    fn read_cdef(&mut self) -> Result<()> {
        if self.blk.skip
            || self.hdr.coded_lossless
            || !self.seq.enable_cdef
            || self.hdr.allow_intrabc
        {
            return Ok(());
        }
        let r = self.blk.mi_row & !(CDEF_UNIT_MI - 1);
        let c = self.blk.mi_col & !(CDEF_UNIT_MI - 1);
        if self.fs.cdef.get(r, c) != -1 {
            return Ok(());
        }
        let index = self.sym.read_literal(u32::from(self.hdr.cdef.bits))? as i8;
        for i in (r..r + self.blk.bh4).step_by(CDEF_UNIT_MI) {
            for j in (c..c + self.blk.bw4).step_by(CDEF_UNIT_MI) {
                self.fs.cdef.set(i, j, index);
            }
        }
        Ok(())
    }

    /** Whether a block is a whole superblock with no coefficients, the case where the delta reads are skipped. */
    fn skips_delta_reads(&self) -> bool {
        let sb_size = if self.seq.use_128x128_superblock {
            BLOCK_128X128
        } else {
            BLOCK_64X64
        };
        self.blk.mi_size == sb_size && self.blk.skip
    }

    /** read_delta_qindex(): on the first block of a superblock, an optional change to the running quantiser index. */
    pub(super) fn read_delta_qindex(&mut self) -> Result<()> {
        if self.skips_delta_reads() || !self.dq.read_deltas {
            return Ok(());
        }
        let abs = self.read_delta_magnitude(DeltaKind::Q)?;
        if abs != 0 {
            let reduced = self.read_sign(abs)?;
            let shift = self.hdr.delta_q_res;
            let next = self.dq.current_qindex + (reduced << shift);
            self.dq.current_qindex = next.clamp(QINDEX_MIN, MAX_QINDEX);
        }
        Ok(())
    }

    /** read_delta_lf(): optional changes to the running loop-filter levels, one per strength when delta_lf_multi is set. */
    pub(super) fn read_delta_lf(&mut self) -> Result<()> {
        if self.skips_delta_reads() || !self.dq.read_deltas || !self.hdr.delta_lf_present {
            return Ok(());
        }
        let count = if !self.hdr.delta_lf_multi {
            1
        } else if self.seq.color.num_planes() > 1 {
            FRAME_LF_COUNT
        } else {
            FRAME_LF_COUNT - 2
        };
        for i in 0..count {
            let abs = self.read_delta_magnitude(DeltaKind::Lf(i))?;
            if abs != 0 {
                let reduced = self.read_sign(abs)?;
                let shift = self.hdr.delta_lf_res;
                let next = self.dq.delta_lf[i] + (reduced << shift);
                self.dq.delta_lf[i] = next.clamp(-MAX_LOOP_FILTER, MAX_LOOP_FILTER);
            }
        }
        Ok(())
    }

    /** One delta magnitude: a symbol that is small or the escape value, and for the escape a literal length and offset. */
    fn read_delta_magnitude(&mut self, kind: DeltaKind) -> Result<i32> {
        let small = match kind {
            DeltaKind::Q => DELTA_Q_SMALL,
            DeltaKind::Lf(_) => DELTA_LF_SMALL,
        };
        let symbol = match kind {
            DeltaKind::Q => self.sym.read_symbol(&mut self.cdf.delta_q)?,
            DeltaKind::Lf(i) if self.hdr.delta_lf_multi => {
                self.sym.read_symbol(&mut self.cdf.delta_lf_multi[i])?
            }
            DeltaKind::Lf(_) => self.sym.read_symbol(&mut self.cdf.delta_lf)?,
        } as u32;
        if symbol != small {
            return Ok(symbol as i32);
        }
        let rem_bits = self.sym.read_literal(DELTA_ESCAPE_BITS)? + 1;
        let abs_bits = self.sym.read_literal(rem_bits)?;
        Ok((abs_bits + (1 << rem_bits) + 1) as i32)
    }

    /** The sign of a non-zero delta, applied to its magnitude. */
    fn read_sign(&mut self, abs: i32) -> Result<i32> {
        let negative = self.sym.read_literal(1)? == 1;
        Ok(if negative { -abs } else { abs })
    }
}

/** Which running delta a magnitude belongs to: the quantiser index, or one loop-filter strength. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeltaKind {
    Q,
    Lf(usize),
}

/** The prediction of a block's segment id from its neighbours: the one they share, or the left, or the above. */
pub(super) fn segment_prediction(prev_ul: i32, prev_u: i32, prev_l: i32) -> i32 {
    if prev_u == NO_SEGMENT {
        if prev_l == NO_SEGMENT { 0 } else { prev_l }
    } else if prev_l == NO_SEGMENT || prev_ul == prev_u {
        prev_u
    } else {
        prev_l
    }
}

/** The context of a segment id symbol: how many of the three neighbours agree. */
fn segment_ctx(prev_ul: i32, prev_u: i32, prev_l: i32) -> usize {
    if prev_ul < 0 {
        0
    } else if prev_ul == prev_u && prev_ul == prev_l {
        2
    } else if prev_ul == prev_u || prev_ul == prev_l || prev_u == prev_l {
        1
    } else {
        0
    }
}

/** The segment a coded difference names: neg_deinterleave around the prediction, then clipped to the last active segment (the spec's Clip3). Untrusted bits can name any id, so the clip is what keeps every later table lookup inside the eight segments. */
pub(super) fn coded_segment(coded: i32, pred: i32, last_active: u8) -> usize {
    let max = i32::from(last_active) + 1;
    neg_deinterleave(coded, pred, max).clamp(0, i32::from(last_active)) as usize
}

/** neg_deinterleave(diff, ref, max): maps the coded difference back to a segment id around the prediction. */
pub(super) fn neg_deinterleave(diff: i32, reference: i32, max: i32) -> i32 {
    if reference == 0 {
        return diff;
    }
    if reference >= max - 1 {
        return max - diff - 1;
    }
    if 2 * reference < max {
        if diff <= 2 * reference {
            return interleaved(diff, reference);
        }
        diff
    } else {
        if diff <= 2 * (max - reference - 1) {
            return interleaved(diff, reference);
        }
        max - (diff + 1)
    }
}

/** The odd and even differences around a reference: odd ones step up, even ones step down. */
fn interleaved(diff: i32, reference: i32) -> i32 {
    if diff & 1 == 1 {
        reference + ((diff + 1) >> 1)
    } else {
        reference - (diff >> 1)
    }
}
