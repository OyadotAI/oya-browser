/** Transform type syntax for intra blocks (spec transform_type 5.11.47, compute_tx_type 5.11.40 and get_tx_set 5.11.48): the type a luma transform block codes, the type a chroma block takes from its mode, and the transform set that limits both. */
use super::constants::{
    DCT_DCT, TX_16X16, TX_32X32, TX_SET_DCTONLY, TX_SET_INTRA_1, TX_SET_INTRA_2,
};
use super::decoder::TileDecoder;
use super::tables::{
    FILTER_INTRA_MODE_TO_INTRA_DIR, MODE_TO_TXFM, TX_SIZE_SQR, TX_SIZE_SQR_UP,
    TX_TYPE_IN_SET_INTRA, TX_TYPE_INTRA_INV_SET1, TX_TYPE_INTRA_INV_SET2,
};
use crate::error::Result;

impl<'a> TileDecoder<'a> {
    /** transform_type(x4, y4, txSz): read the luma transform type when the set has choices and the quantiser is non-zero, then record it for the 4x4 units of the transform. */
    pub(super) fn transform_type(&mut self, x4: usize, y4: usize, tx: usize) -> Result<()> {
        let set = self.intra_tx_set(tx);
        let tx_type = if set > TX_SET_DCTONLY && self.qindex(true, self.blk.segment_id) > 0 {
            self.read_intra_tx_type(tx, set)?
        } else {
            DCT_DCT
        };
        let (w4, h4) = (tx_w4(tx), tx_h4(tx));
        self.fs.mi.fill(y4, x4, h4, w4, |m| m.tx_type = tx_type);
        Ok(())
    }

    /** intra_tx_type: the coded index, mapped through the inverse table of the set. The CDF depends on the transform size and the intra direction, which is the filter-intra mode's when one is used. */
    fn read_intra_tx_type(&mut self, tx: usize, set: usize) -> Result<u8> {
        let dir = if self.blk.use_filter_intra {
            FILTER_INTRA_MODE_TO_INTRA_DIR[usize::from(self.blk.filter_intra_mode)] as usize
        } else {
            usize::from(self.blk.y_mode)
        };
        let sqr = TX_SIZE_SQR[tx] as usize;
        if set == TX_SET_INTRA_1 {
            let index = self
                .sym
                .read_symbol(&mut self.cdf.intra_tx_type_set1[sqr][dir])?;
            Ok(TX_TYPE_INTRA_INV_SET1[index])
        } else {
            let index = self
                .sym
                .read_symbol(&mut self.cdf.intra_tx_type_set2[sqr][dir])?;
            Ok(TX_TYPE_INTRA_INV_SET2[index])
        }
    }

    /** get_tx_set for an intra block: the DCT-only set for large or 32-square transforms, set 2 for reduced sets and 16-square transforms, and set 1 otherwise. */
    pub(super) fn intra_tx_set(&self, tx: usize) -> usize {
        let sqr = TX_SIZE_SQR[tx] as usize;
        let sqr_up = TX_SIZE_SQR_UP[tx] as usize;
        if sqr_up >= TX_32X32 {
            TX_SET_DCTONLY
        } else if self.hdr.reduced_tx_set || sqr == TX_16X16 {
            TX_SET_INTRA_2
        } else {
            TX_SET_INTRA_1
        }
    }

    /** compute_tx_type(plane, txSz, blockX, blockY): the type a transform block uses. Luma reads what transform_type recorded; chroma takes its mode's type when the set allows it. */
    pub(super) fn compute_tx_type(&self, plane: usize, tx: usize, x4: usize, y4: usize) -> u8 {
        if self.blk.lossless || TX_SIZE_SQR_UP[tx] as usize > TX_32X32 {
            return DCT_DCT;
        }
        if plane == 0 {
            return self.fs.mi.at(y4, x4).tx_type;
        }
        let set = self.intra_tx_set(tx);
        let tx_type = MODE_TO_TXFM[usize::from(self.blk.uv_mode)];
        if TX_TYPE_IN_SET_INTRA[set][usize::from(tx_type)] == 0 {
            DCT_DCT
        } else {
            tx_type
        }
    }
}

/** Width of a transform in 4x4 units. */
pub(crate) fn tx_w4(tx: usize) -> usize {
    super::tables::TX_WIDTH[tx] as usize >> super::constants::MI_UNIT_LOG2
}

/** Height of a transform in 4x4 units. */
pub(crate) fn tx_h4(tx: usize) -> usize {
    super::tables::TX_HEIGHT[tx] as usize >> super::constants::MI_UNIT_LOG2
}
