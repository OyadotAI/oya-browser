/** Motion vector difference syntax (spec read_mv and read_mv_component, 5.11.32): the joint that says which components are coded, then each component's sign, class and offset. Intra frames force whole-sample vectors, so the fraction and high-precision bits are fixed rather than read. */
use super::constants::{
    CLASS_OFFSET_BASE, CLASS0_SIZE, INTEGER_MV_FR, INTEGER_MV_HP, MV_INTRABC_CONTEXT,
    MV_JOINT_HNZVNZ, MV_JOINT_HNZVZ, MV_JOINT_HZVNZ,
};
use super::decoder::TileDecoder;
use crate::error::Result;

/** Bits below the offset in a magnitude: the two-bit fraction and the one-bit high-precision field. */
const FRACTION_FIELD_BITS: u32 = 3;

impl<'a> TileDecoder<'a> {
    /** read_mv: the difference between a block's vector and its prediction, as (row, column) in 1/8 sample units. An intra block copy block reads with its own motion vector context. */
    pub(super) fn read_intrabc_mv_diff(&mut self) -> Result<[i32; 2]> {
        let joint = self
            .sym
            .read_symbol(&mut self.cdf.mv[MV_INTRABC_CONTEXT].joint)?;
        let mut diff = [0, 0];
        if joint == MV_JOINT_HZVNZ || joint == MV_JOINT_HNZVNZ {
            diff[0] = self.read_mv_component(0)?;
        }
        if joint == MV_JOINT_HNZVZ || joint == MV_JOINT_HNZVNZ {
            diff[1] = self.read_mv_component(1)?;
        }
        Ok(diff)
    }

    /** read_mv_component(comp): one signed component. Its magnitude is a class and, within the class, an offset built from the coded bits and the fixed whole-sample fraction. */
    fn read_mv_component(&mut self, comp: usize) -> Result<i32> {
        let ctx = MV_INTRABC_CONTEXT;
        let negative = self.sym.read_symbol(&mut self.cdf.mv[ctx].sign[comp])? == 1;
        let class = self.sym.read_symbol(&mut self.cdf.mv[ctx].class[comp])?;
        let magnitude = if class == 0 {
            let bit = self
                .sym
                .read_symbol(&mut self.cdf.mv[ctx].class0_bit[comp])? as i32;
            Self::fraction_offset(bit)
        } else {
            let offset = self.read_class_offset(comp, class)?;
            (CLASS0_SIZE << (class + CLASS_OFFSET_BASE)) + Self::fraction_offset(offset)
        };
        Ok(if negative { -magnitude } else { magnitude })
    }

    /** The integer offset bits of a class above zero: one bit per class step, low bit first. */
    fn read_class_offset(&mut self, comp: usize, class: usize) -> Result<i32> {
        let mut offset = 0;
        for i in 0..class {
            let bit = self
                .sym
                .read_symbol(&mut self.cdf.mv[MV_INTRABC_CONTEXT].bits[comp][i])?
                as i32;
            offset |= bit << i;
        }
        Ok(offset)
    }

    /** The magnitude a whole-sample offset adds to a class base: the offset shifted past the fraction bits, with the fixed fraction and precision bits, plus one. */
    fn fraction_offset(offset: i32) -> i32 {
        let fixed = (INTEGER_MV_FR << 1) | INTEGER_MV_HP;
        ((offset << FRACTION_FIELD_BITS) | fixed) + 1
    }
}
