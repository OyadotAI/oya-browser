use super::coeff_ctx::{coeff_base_ctx, coeff_base_eob_ctx, coeff_br_ctx, tx_class};
/** Coefficient syntax of one transform block (spec coeffs 5.11.39 with its eob and level reads): the all-zero flag, the end-of-block position, the levels from the last coefficient back to the first, the signs, and the Golomb tails. The quantised levels are kept in the decoder's buffer, and the non-zero dequantised ones go to the frame state. */
use super::constants::{
    BR_CDF_SIZE, BR_ROUNDS, COEFF_BASE_RANGE, COEFF_LEVEL_MASK, CUL_LEVEL_MAX, DC_CATEGORY_NEG,
    DC_CATEGORY_POS, DCT_DCT, EOB_SIZE_CAP, EOB_SIZE_OFFSET, GOLOMB_MAX_LENGTH, MAX_SCAN_LEN,
    MI_UNIT_LOG2, NUM_BASE_LEVELS, SEG_EOB_LARGE, TX_16X64, TX_64X16, TX_CLASS_2D,
};
use super::decoder::TileDecoder;
use super::samples::Coded;
use super::scan::get_scan;
use super::tables::{
    TX_HEIGHT, TX_HEIGHT_LOG2, TX_SIZE_SQR, TX_SIZE_SQR_UP, TX_WIDTH, TX_WIDTH_LOG2,
};
use crate::error::{Error, Result};

/** The cumulative level and dc category a transform block leaves for its neighbours. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Cumulative {
    level: u8,
    dc: u8,
}

/** Where a transform block sits: its plane, its 4x4 position in that plane, and its size in 4x4 units. */
#[derive(Clone, Copy, Debug)]
struct Place {
    plane: usize,
    x4: usize,
    y4: usize,
    w4: usize,
    h4: usize,
}

impl<'a> TileDecoder<'a> {
    /** coeffs(plane, startX, startY, txSz): read one transform block's coefficients and update the contexts that neighbours read. A block with coefficients returns its type and dequantised values, for reconstruction; an all-zero block returns None. */
    pub(super) fn coeffs(
        &mut self,
        plane: usize,
        start_x: usize,
        start_y: usize,
        tx: usize,
    ) -> Result<Option<Coded>> {
        let place = Place {
            plane,
            x4: start_x >> MI_UNIT_LOG2,
            y4: start_y >> MI_UNIT_LOG2,
            w4: TX_WIDTH[tx] as usize >> MI_UNIT_LOG2,
            h4: TX_HEIGHT[tx] as usize >> MI_UNIT_LOG2,
        };
        let tx_ctx = (TX_SIZE_SQR[tx] as usize + TX_SIZE_SQR_UP[tx] as usize + 1) >> 1;
        let seg_eob = seg_eob(tx);
        self.quant[..seg_eob].fill(0);
        let ctx = self.all_zero_ctx(plane, tx, place.x4, place.y4, place.w4, place.h4);
        let all_zero = self.sym.read_symbol(&mut self.cdf.txb_skip[tx_ctx][ctx])? == 1;
        let (eob, edge, tx_type) = if all_zero {
            self.clear_tx_types(place);
            (0, Cumulative::default(), DCT_DCT)
        } else {
            self.read_nonzero_block(place, tx, tx_ctx)?
        };
        self.set_contexts(place, edge);
        if eob == 0 {
            return Ok(None);
        }
        let coeffs = self.dequantise(plane, tx, tx_type)?;
        Ok(Some(Coded { tx_type, coeffs }))
    }

    /** A transform block that has coefficients: its type, its scan, the end position, the levels, and the signs and tails. */
    fn read_nonzero_block(
        &mut self,
        place: Place,
        tx: usize,
        tx_ctx: usize,
    ) -> Result<(usize, Cumulative, u8)> {
        let ptype = usize::from(place.plane > 0);
        if place.plane == 0 {
            self.transform_type(place.x4, place.y4, tx)?;
        }
        let tx_type = self.compute_tx_type(place.plane, tx, place.x4, place.y4);
        let scan = get_scan(tx, tx_type);
        let eob = self.read_eob(tx, tx_ctx, ptype, tx_type)?;
        self.read_levels(tx, tx_ctx, ptype, tx_type, scan, eob)?;
        let edge = self.read_signs(place, scan, eob, ptype)?;
        Ok((eob, edge, tx_type))
    }

    /** The luma type of an all-zero block is recorded as DCT, so later chroma and neighbours see a defined type. */
    fn clear_tx_types(&mut self, place: Place) {
        if place.plane == 0 {
            self.fs
                .mi
                .fill(place.y4, place.x4, place.h4, place.w4, |m| {
                    m.tx_type = DCT_DCT
                });
        }
    }

    /** The end-of-block position: a class symbol, then the extra bits that refine it. */
    fn read_eob(&mut self, tx: usize, tx_ctx: usize, ptype: usize, tx_type: u8) -> Result<usize> {
        let multisize = (TX_WIDTH_LOG2[tx] as usize).min(EOB_SIZE_CAP)
            + (TX_HEIGHT_LOG2[tx] as usize).min(EOB_SIZE_CAP)
            - EOB_SIZE_OFFSET;
        let ctx = if tx_class(tx_type) == TX_CLASS_2D {
            0
        } else {
            1
        };
        let class = self.read_eob_class(multisize, ptype, ctx)?;
        let eob_pt = class + 1;
        let mut eob = if eob_pt < 2 {
            eob_pt
        } else {
            (1 << (eob_pt - 2)) + 1
        };
        if eob_pt >= 3 {
            if self
                .sym
                .read_symbol(&mut self.cdf.eob_extra[tx_ctx][ptype][eob_pt - 3])?
                == 1
            {
                eob += 1 << (eob_pt - 3);
            }
            for i in 1..eob_pt.saturating_sub(2) {
                let shift = eob_pt.saturating_sub(2) - 1 - i;
                if self.sym.read_literal(1)? == 1 {
                    eob += 1 << shift;
                }
            }
        }
        Ok(eob)
    }

    /** eob_pt_16 to eob_pt_1024: the symbol for the block's eob size class, with its CDF selected by size. */
    fn read_eob_class(&mut self, multisize: usize, ptype: usize, ctx: usize) -> Result<usize> {
        match multisize {
            0 => self.sym.read_symbol(&mut self.cdf.eob_pt_16[ptype][ctx]),
            1 => self.sym.read_symbol(&mut self.cdf.eob_pt_32[ptype][ctx]),
            2 => self.sym.read_symbol(&mut self.cdf.eob_pt_64[ptype][ctx]),
            3 => self.sym.read_symbol(&mut self.cdf.eob_pt_128[ptype][ctx]),
            4 => self.sym.read_symbol(&mut self.cdf.eob_pt_256[ptype][ctx]),
            5 => self.sym.read_symbol(&mut self.cdf.eob_pt_512[ptype]),
            _ => self.sym.read_symbol(&mut self.cdf.eob_pt_1024[ptype]),
        }
    }

    /** The levels of the coefficients from the last one back to the first. Each level is a base symbol, extended by range symbols while it stays large. */
    fn read_levels(
        &mut self,
        tx: usize,
        tx_ctx: usize,
        ptype: usize,
        tx_type: u8,
        scan: &[u16],
        eob: usize,
    ) -> Result<()> {
        let class = tx_class(tx_type);
        let br_ctx_cdf = tx_ctx.min(BR_TX_CTX_CAP);
        for c in (0..eob).rev() {
            let pos = scan[c] as usize;
            let mut level = if c + 1 == eob {
                let ctx = coeff_base_eob_ctx(c, tx);
                self.sym
                    .read_symbol(&mut self.cdf.coeff_base_eob[tx_ctx][ptype][ctx])?
                    as u32
                    + 1
            } else {
                let ctx = coeff_base_ctx(&self.quant, tx, class, pos);
                self.sym
                    .read_symbol(&mut self.cdf.coeff_base[tx_ctx][ptype][ctx])?
                    as u32
            };
            if level > NUM_BASE_LEVELS {
                let ctx = coeff_br_ctx(&self.quant, tx, class, pos);
                for _ in 0..BR_ROUNDS {
                    let range = self
                        .sym
                        .read_symbol(&mut self.cdf.coeff_br[br_ctx_cdf][ptype][ctx])?
                        as u32;
                    level += range;
                    if range < BR_CDF_SIZE - 1 {
                        break;
                    }
                }
            }
            self.quant[pos] = level as i32;
        }
        Ok(())
    }

    /** The signs and Golomb tails, in scan order: the first coefficient's sign is coded with its context, the rest as literal bits. Returns the cumulative level and the dc category. */
    fn read_signs(
        &mut self,
        place: Place,
        scan: &[u16],
        eob: usize,
        ptype: usize,
    ) -> Result<Cumulative> {
        let dc_ctx = self.dc_sign_ctx(place.plane, place.x4, place.y4, place.w4, place.h4);
        let mut edge = Cumulative::default();
        let mut cul_level = 0u32;
        for (c, &scan_pos) in scan.iter().enumerate().take(eob) {
            let pos = scan_pos as usize;
            let sign = if self.quant[pos] == 0 {
                false
            } else if c == 0 {
                self.sym.read_symbol(&mut self.cdf.dc_sign[ptype][dc_ctx])? == 1
            } else {
                self.sym.read_literal(1)? == 1
            };
            let big_limit = (NUM_BASE_LEVELS + COEFF_BASE_RANGE) as i32;
            if self.quant[pos] > big_limit {
                let tail = self.read_golomb()?;
                self.quant[pos] = tail as i32 + big_limit;
            }
            if pos == 0 && self.quant[pos] > 0 {
                edge.dc = if sign {
                    DC_CATEGORY_NEG
                } else {
                    DC_CATEGORY_POS
                };
            }
            self.quant[pos] &= COEFF_LEVEL_MASK as i32;
            cul_level = cul_level.saturating_add(self.quant[pos] as u32);
            if sign {
                self.quant[pos] = -self.quant[pos];
            }
        }
        edge.level = cul_level.min(CUL_LEVEL_MAX) as u8;
        Ok(edge)
    }

    /** One Exp-Golomb value: a run of zero bits ended by a one, then that many bits below the leading one. */
    fn read_golomb(&mut self) -> Result<u32> {
        let mut length = 0u32;
        loop {
            length += 1;
            if self.sym.read_literal(1)? == 1 {
                break;
            }
            if length >= GOLOMB_MAX_LENGTH {
                return Err(Error::Malformed("coefficient golomb length"));
            }
        }
        let mut x = 1u32;
        for _ in 0..length - 1 {
            x = (x << 1) | self.sym.read_literal(1)?;
        }
        Ok(x)
    }

    /** Store the block's cumulative level and dc category for every 4x4 unit column and row it spans, for the contexts that follow. */
    fn set_contexts(&mut self, place: Place, edge: Cumulative) {
        let plane = place.plane;
        for i in 0..place.w4 {
            self.above.planes[plane].level[place.x4 + i] = edge.level;
            self.above.planes[plane].dc[place.x4 + i] = edge.dc;
        }
        for i in 0..place.h4 {
            self.left.planes[plane].level[place.y4 + i] = edge.level;
            self.left.planes[plane].dc[place.y4 + i] = edge.dc;
        }
    }
}

/** The number of coefficient positions a transform block has: 512 for the 16x64 and 64x16 sizes, otherwise its area capped at 1024. */
fn seg_eob(tx: usize) -> usize {
    if tx == TX_16X64 || tx == TX_64X16 {
        SEG_EOB_LARGE
    } else {
        let area = TX_WIDTH[tx] as usize * TX_HEIGHT[tx] as usize;
        area.min(MAX_SCAN_LEN)
    }
}

/** The coefficient CDFs for the three largest sizes share one entry, so the range read for a larger size uses this index. */
const BR_TX_CTX_CAP: usize = 3;
