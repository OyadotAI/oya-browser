/** Bounds-checked big-endian cursor over untrusted bytes: every read reports truncation instead of panicking. */
use crate::constants::{FOURCC_LEN, FourCc, MAX_UINT_WIDTH};
use crate::error::{Error, Result};

/** A forward-only cursor; each read consumes bytes and fails with Truncated when they run out. */
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /** Start reading at the first byte. */
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    /** Offset of the cursor from the start of the input. */
    pub(crate) fn position(&self) -> usize {
        self.pos
    }

    /** Bytes not yet consumed. */
    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }

    /** Consume the next `len` bytes, or fail if fewer remain. */
    pub(crate) fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        if self.remaining() < len {
            return Err(Error::Truncated);
        }
        let out = &self.bytes[self.pos..self.pos + len];
        self.pos += len;
        Ok(out)
    }

    /** The unread bytes, consuming nothing. */
    pub(crate) fn rest(&self) -> &'a [u8] {
        &self.bytes[self.pos..]
    }

    /** One unsigned byte. */
    pub(crate) fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    /** Unsigned big-endian integer of `width` bytes, where 0 reads as zero. */
    pub(crate) fn uint(&mut self, width: usize) -> Result<u64> {
        if width > MAX_UINT_WIDTH {
            return Err(Error::Malformed("integer width"));
        }
        Ok(self
            .take(width)?
            .iter()
            .fold(0u64, |acc, b| (acc << 8) | u64::from(*b)))
    }

    /** Unsigned 16-bit big-endian integer. */
    pub(crate) fn u16(&mut self) -> Result<u16> {
        Ok(self.uint(2)? as u16)
    }

    /** Unsigned 32-bit big-endian integer. */
    pub(crate) fn u32(&mut self) -> Result<u32> {
        Ok(self.uint(4)? as u32)
    }

    /** Unsigned 64-bit big-endian integer. */
    pub(crate) fn u64(&mut self) -> Result<u64> {
        self.uint(8)
    }

    /** A four-character code. */
    pub(crate) fn fourcc(&mut self) -> Result<FourCc> {
        let b = self.take(FOURCC_LEN)?;
        Ok([b[0], b[1], b[2], b[3]])
    }
}
