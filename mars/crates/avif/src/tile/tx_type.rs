/** Transform type syntax for intra blocks (spec transform_type 5.11.47, compute_tx_type 5.11.40 and get_tx_set 5.11.48): the type a luma transform block codes, the type a chroma block takes from its mode, and the transform set that limits both. */
use super::constants::{
    DCT_DCT, TX_16X16, TX_32X32, TX_SET_DCTONLY, TX_SET_INTER_1, TX_SET_INTER_2, TX_SET_INTER_3,
    TX_SET_INTRA_1, TX_SET_INTRA_2,
};
use super::decoder::TileDecoder;
use super::lr::plane_subsampling;
use super::tables::{
    FILTER_INTRA_MODE_TO_INTRA_DIR, MODE_TO_TXFM, TX_SIZE_SQR, TX_SIZE_SQR_UP,
    TX_TYPE_IN_SET_INTER, TX_TYPE_IN_SET_INTRA, TX_TYPE_INTER_INV_SET1, TX_TYPE_INTER_INV_SET2,
    TX_TYPE_INTER_INV_SET3, TX_TYPE_INTRA_INV_SET1, TX_TYPE_INTRA_INV_SET2,
};
use crate::error::Result;

impl<'a> TileDecoder<'a> {
    /** transform_type(x4, y4, txSz): read the luma transform type when the set has choices and the quantiser is non-zero, then record it for the 4x4 units of the transform. */
    pub(super) fn transform_type(&mut self, x4: usize, y4: usize, tx: usize) -> Result<()> {
        let set = self.tx_set(tx);
        let tx_type = if set > TX_SET_DCTONLY && self.qindex(true, self.blk.segment_id) > 0 {
            self.read_tx_type(tx, set)?
        } else {
            DCT_DCT
        };
        let (w4, h4) = (tx_w4(tx), tx_h4(tx));
        self.fs.mi.fill(y4, x4, h4, w4, |m| m.tx_type = tx_type);
        Ok(())
    }

    /** The coded transform type: the inter symbol for an inter block, the intra one otherwise. */
    fn read_tx_type(&mut self, tx: usize, set: usize) -> Result<u8> {
        if self.blk.is_inter {
            self.read_inter_tx_type(tx, set)
        } else {
            self.read_intra_tx_type(tx, set)
        }
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

    /** inter_tx_type: the coded index of an inter block's set, mapped through that set's inverse table. Set 2 has one CDF; sets 1 and 3 are chosen by the transform's square size. */
    fn read_inter_tx_type(&mut self, tx: usize, set: usize) -> Result<u8> {
        let sqr = TX_SIZE_SQR[tx] as usize;
        match set {
            TX_SET_INTER_1 => {
                let index = self
                    .sym
                    .read_symbol(&mut self.cdf.inter_tx_type_set1[sqr])?;
                Ok(TX_TYPE_INTER_INV_SET1[index])
            }
            TX_SET_INTER_2 => {
                let index = self.sym.read_symbol(&mut self.cdf.inter_tx_type_set2)?;
                Ok(TX_TYPE_INTER_INV_SET2[index])
            }
            _ => {
                let index = self
                    .sym
                    .read_symbol(&mut self.cdf.inter_tx_type_set3[sqr])?;
                Ok(TX_TYPE_INTER_INV_SET3[index])
            }
        }
    }

    /** get_tx_set(txSz): the transform set of a block's transform. Intra and inter blocks choose among different sets, and transforms wider than 32 samples have DCT only. */
    pub(super) fn tx_set(&self, tx: usize) -> usize {
        let sqr = TX_SIZE_SQR[tx] as usize;
        let sqr_up = TX_SIZE_SQR_UP[tx] as usize;
        if sqr_up > TX_32X32 {
            TX_SET_DCTONLY
        } else if self.blk.is_inter {
            if self.hdr.reduced_tx_set || sqr_up == TX_32X32 {
                TX_SET_INTER_3
            } else if sqr == TX_16X16 {
                TX_SET_INTER_2
            } else {
                TX_SET_INTER_1
            }
        } else if sqr_up == TX_32X32 {
            TX_SET_DCTONLY
        } else if self.hdr.reduced_tx_set || sqr == TX_16X16 {
            TX_SET_INTRA_2
        } else {
            TX_SET_INTRA_1
        }
    }

    /** compute_tx_type(plane, txSz, blockX, blockY): the type a transform block uses. Luma reads what transform_type recorded. Chroma of an inter block takes the type of the luma at the same place, and chroma of an intra block takes its mode's type; either is DCT when the set does not allow it. */
    pub(super) fn compute_tx_type(&self, plane: usize, tx: usize, x4: usize, y4: usize) -> u8 {
        if self.blk.lossless || TX_SIZE_SQR_UP[tx] as usize > TX_32X32 {
            return DCT_DCT;
        }
        if plane == 0 {
            return self.fs.mi.at(y4, x4).tx_type;
        }
        let set = self.tx_set(tx);
        let tx_type = if self.blk.is_inter {
            self.inter_chroma_tx_type(plane, x4, y4)
        } else {
            MODE_TO_TXFM[usize::from(self.blk.uv_mode)]
        };
        if self.is_tx_type_in_set(set, tx_type) {
            tx_type
        } else {
            DCT_DCT
        }
    }

    /** The type an inter block's chroma takes: the luma type at the matching position, kept inside the block. */
    fn inter_chroma_tx_type(&self, plane: usize, x4: usize, y4: usize) -> u8 {
        let (sub_x, sub_y) = plane_subsampling(self.seq, plane);
        let luma_x4 = self.blk.mi_col.max(x4 << sub_x);
        let luma_y4 = self.blk.mi_row.max(y4 << sub_y);
        self.fs.mi.at(luma_y4, luma_x4).tx_type
    }

    /** is_tx_type_in_set: whether a type belongs to a set, in the table of the block's kind. */
    fn is_tx_type_in_set(&self, set: usize, tx_type: u8) -> bool {
        if self.blk.is_inter {
            TX_TYPE_IN_SET_INTER[set][usize::from(tx_type)] != 0
        } else {
            TX_TYPE_IN_SET_INTRA[set][usize::from(tx_type)] != 0
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
