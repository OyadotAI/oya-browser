/** The symbol decoder (spec 8.2): the range decoder that turns tile bytes into symbols, with CDF adaptation. It is initialised over one tile's bytes and read by the block syntax. */
use crate::bits::BitReader;
use crate::error::{Error, Result};

/** Precision bits the range coder works in. */
const WINDOW_BITS: u32 = 15;
/** Top of the range: 2^15, which is also the CDF scale. */
const CDF_ONE: u32 = 1 << WINDOW_BITS;
/** Shift that trims CDF precision before the range multiply. */
const EC_PROB_SHIFT: u32 = 6;
/** Minimum probability mass given to each symbol. */
const EC_MIN_PROB: u32 = 4;
/** Shift of the range that the multiply leaves behind. */
const RANGE_SHIFT: u32 = 8;
/** Adaptation rate: base rate, plus one at each count threshold, capped by the symbol count's log. */
const RATE_BASE: u32 = 3;
const COUNT_LOW: u16 = 15;
const COUNT_HIGH: u16 = 31;
const COUNT_MAX: u16 = 32;
/** Largest symbol-count log that adds to the adaptation rate. */
const RATE_LOG_CAP: u32 = 2;
/** Probability of each boolean symbol in read_bool: one half. */
const BOOL_HALF: u16 = 1 << 14;
/** A conformance floor on how far the decoder may run past the tile's bits. */
const MIN_MAX_BITS: i64 = -14;

#[cfg(test)]
#[path = "symbol_tests.rs"]
mod tests;

/** Floor of log2 of a non-zero value. */
fn floor_log2(x: u32) -> u32 {
    31 - x.leading_zeros()
}

/** The state of the symbol decoder over one tile: the bit reader, the window value and range, and the bits left. */
pub(crate) struct SymbolDecoder<'a> {
    reader: BitReader<'a>,
    value: u32,
    range: u32,
    max_bits: i64,
    disable_cdf_update: bool,
}

impl<'a> SymbolDecoder<'a> {
    /** init_symbol(sz): start decoding over `data`, which is one tile's bytes. Empty tiles are refused. */
    pub(crate) fn new(data: &'a [u8], disable_cdf_update: bool) -> Result<Self> {
        if data.is_empty() {
            return Err(Error::Malformed("empty tile"));
        }
        let sz_bits = data.len() * 8;
        let num_bits = sz_bits.min(WINDOW_BITS as usize) as u32;
        let mut reader = BitReader::new(data);
        let buf = reader.f(num_bits)?;
        let padded = buf << (WINDOW_BITS - num_bits);
        Ok(Self {
            reader,
            value: (CDF_ONE - 1) ^ padded as u32,
            range: CDF_ONE,
            max_bits: sz_bits as i64 - i64::from(WINDOW_BITS),
            disable_cdf_update,
        })
    }

    /** read_symbol(cdf): decode one symbol from an adaptive CDF of N+1 entries (N symbols plus the counter), then adapt the CDF unless updates are disabled. */
    pub(crate) fn read_symbol(&mut self, cdf: &mut [u16]) -> Result<usize> {
        let adapt = !self.disable_cdf_update;
        self.decode(cdf, adapt)
    }

    /** read_bool(): one equiprobable bit. The CDF it uses is built fresh, so no adaptation is kept. */
    pub(crate) fn read_bool(&mut self) -> Result<bool> {
        let mut cdf = [BOOL_HALF, CDF_ONE as u16, 0];
        Ok(self.decode(&mut cdf, false)? == 1)
    }

    /** read_literal(n): n equiprobable bits, most significant first. */
    pub(crate) fn read_literal(&mut self, n: u32) -> Result<u32> {
        let mut x = 0u32;
        for _ in 0..n {
            x = (x << 1) | u32::from(self.read_bool()?);
        }
        Ok(x)
    }

    /** read_ns(n) on symbols: a non-symmetric value in 0..n, coded with read_literal bits as the ns(n) header descriptor is. */
    pub(crate) fn read_ns(&mut self, n: u32) -> Result<u32> {
        if n == 0 {
            return Err(Error::Malformed("NS range"));
        }
        let w = floor_log2(n) + 1;
        let m = (1u32 << w) - n;
        let v = self.read_literal(w - 1)?;
        if v < m {
            return Ok(v);
        }
        let extra = self.read_literal(1)?;
        Ok((v << 1) - m + extra)
    }

    /** exit_symbol() (spec 8.2.4): the tile's trailing bit must be one, and every padding bit after it zero. A tile that stops early or late, or is padded wrongly, fails here, so a pass proves the symbol stream stayed in step. */
    pub(crate) fn exit(&self) -> Result<()> {
        if self.max_bits < MIN_MAX_BITS {
            return Err(Error::Malformed("symbol decoder overran the tile"));
        }
        let pos = self.reader.bit_pos() as i64;
        let trailing = pos - i64::from(WINDOW_BITS).min(self.max_bits + i64::from(WINDOW_BITS));
        let padding_end = pos + self.max_bits.max(0);
        if trailing < 0 || !self.reader.bit_at(trailing as usize)? {
            return Err(Error::Malformed("tile trailing bit missing"));
        }
        for bit in trailing + 1..padding_end {
            if self.reader.bit_at(bit as usize)? {
                return Err(Error::Malformed("tile padding is not zero"));
            }
        }
        Ok(())
    }

    /** The shared decoding core: find the symbol whose interval holds the value, renormalise, and adapt if asked. */
    fn decode(&mut self, cdf: &mut [u16], adapt: bool) -> Result<usize> {
        let n = cdf.len() - 1;
        let symbol = self.find_symbol(cdf, n)?;
        if adapt {
            adapt_cdf(cdf, n, symbol);
        }
        Ok(symbol)
    }

    /** The interval search of the symbol decoding process: scan symbols until the value falls in one, then narrow the range. */
    fn find_symbol(&mut self, cdf: &[u16], n: usize) -> Result<usize> {
        let mut cur = self.range;
        let mut symbol = 0usize;
        let mut prev;
        loop {
            if symbol >= n {
                return Err(Error::Malformed("CDF has no terminal entry"));
            }
            prev = cur;
            let f = CDF_ONE - u32::from(cdf[symbol]);
            cur = ((self.range >> RANGE_SHIFT) * (f >> EC_PROB_SHIFT)) >> (7 - EC_PROB_SHIFT);
            cur += EC_MIN_PROB * (n - symbol - 1) as u32;
            if self.value >= cur {
                break;
            }
            symbol += 1;
        }
        self.range = prev - cur;
        self.value -= cur;
        self.renormalise()?;
        Ok(symbol)
    }

    /** Renormalisation steps 1 to 7: shift the range back up and pull fresh bits into the value. */
    fn renormalise(&mut self) -> Result<()> {
        let bits = WINDOW_BITS - floor_log2(self.range);
        self.range <<= bits;
        let num_bits = i64::from(bits).min(self.max_bits.max(0)) as u32;
        let new_data = self.reader.f(num_bits)?;
        let padded = new_data << (bits - num_bits);
        self.value = padded as u32 ^ (((self.value + 1) << bits) - 1);
        self.max_bits -= i64::from(bits);
        Ok(())
    }
}

/** The CDF adaptation step: move every entry before the decoded symbol toward the top and the rest toward zero, faster early on. */
fn adapt_cdf(cdf: &mut [u16], n: usize, symbol: usize) {
    let count = cdf[n];
    let rate = RATE_BASE
        + u32::from(count > COUNT_LOW)
        + u32::from(count > COUNT_HIGH)
        + floor_log2(n as u32).min(RATE_LOG_CAP);
    let mut tmp = 0u32;
    for (i, entry) in cdf.iter_mut().enumerate().take(n - 1) {
        if i == symbol {
            tmp = CDF_ONE;
        }
        let v = u32::from(*entry);
        *entry = if tmp < v {
            (v - ((v - tmp) >> rate)) as u16
        } else {
            (v + ((tmp - v) >> rate)) as u16
        };
    }
    if count < COUNT_MAX {
        cdf[n] = count + 1;
    }
}
