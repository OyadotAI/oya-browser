/** MSB-first bit reader for AV1 headers: the f(n), su(n), ns(n) and uvlc() descriptors of spec section 4.10, bounded to the bytes given. */
use crate::error::{Error, Result};

/** Bits in one byte. */
const BITS_PER_BYTE: usize = 8;
/** Widest f(n) the spec uses in a header. */
const MAX_F_BITS: u32 = 32;
/** The uvlc prefix length at which the value saturates to 2^32 - 1. */
const UVLC_MAX_LEADING_ZEROS: u32 = 32;

/** A cursor over header bytes, counted in bits. Every read fails with Truncated past the end. */
pub(crate) struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BitReader<'a> {
    /** Start reading at the first bit of `data`. */
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /** The number of bits consumed so far. */
    pub(crate) fn bit_pos(&self) -> usize {
        self.pos
    }

    /** One bit, most significant first within each byte. */
    fn bit(&mut self) -> Result<u64> {
        let byte = *self
            .data
            .get(self.pos / BITS_PER_BYTE)
            .ok_or(Error::Truncated)?;
        let shift = BITS_PER_BYTE - 1 - self.pos % BITS_PER_BYTE;
        self.pos += 1;
        Ok(u64::from((byte >> shift) & 1))
    }

    /** f(n): an unsigned n-bit value, n at most 32. */
    pub(crate) fn f(&mut self, n: u32) -> Result<u64> {
        if n > MAX_F_BITS {
            return Err(Error::Malformed("header field width"));
        }
        let mut value = 0u64;
        for _ in 0..n {
            value = (value << 1) | self.bit()?;
        }
        Ok(value)
    }

    /** A one-bit flag. */
    pub(crate) fn flag(&mut self) -> Result<bool> {
        Ok(self.bit()? == 1)
    }

    /** su(n): an n-bit two's complement value. */
    pub(crate) fn su(&mut self, n: u32) -> Result<i64> {
        if n == 0 {
            return Err(Error::Malformed("signed field width"));
        }
        let value = self.f(n)? as i64;
        let sign = 1i64 << (n - 1);
        Ok(if value & sign != 0 {
            value - 2 * sign
        } else {
            value
        })
    }

    /** ns(n): a non-symmetric unsigned value in 0..n, using floor(log2 n) + 1 bits minus one for the short codes. */
    pub(crate) fn ns(&mut self, n: u32) -> Result<u32> {
        if n == 0 {
            return Err(Error::Malformed("ns range"));
        }
        let w = 32 - n.leading_zeros();
        let m = (1u64 << w) - u64::from(n);
        let v = self.f(w - 1)?;
        if v < m {
            return Ok(v as u32);
        }
        let extra = self.f(1)?;
        Ok(((v << 1) - m + extra) as u32)
    }

    /** uvlc(): a variable-length unsigned value, saturating at 2^32 - 1. */
    pub(crate) fn uvlc(&mut self) -> Result<u32> {
        let mut leading_zeros = 0u32;
        while !self.flag()? {
            leading_zeros = leading_zeros.saturating_add(1);
        }
        if leading_zeros >= UVLC_MAX_LEADING_ZEROS {
            return Ok(u32::MAX);
        }
        let value = self.f(leading_zeros)?;
        Ok((value + (1u64 << leading_zeros) - 1) as u32)
    }

    /** The bit at absolute position `pos`, without moving the cursor. Positions past the data are Truncated. */
    pub(crate) fn bit_at(&self, pos: usize) -> Result<bool> {
        let byte = *self.data.get(pos / BITS_PER_BYTE).ok_or(Error::Truncated)?;
        Ok((byte >> (BITS_PER_BYTE - 1 - pos % BITS_PER_BYTE)) & 1 == 1)
    }

    /** Skip to the next byte boundary. */
    pub(crate) fn byte_align(&mut self) {
        self.pos = self.pos.div_ceil(BITS_PER_BYTE) * BITS_PER_BYTE;
    }
}
