/** Parsing and radix formatting preserve arbitrary integer digits without a floating-point round trip. */
use super::*;

/** Syntax failures and valid values exceeding the existing memory bound have different JS error classes. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::v9) enum ParseError {
    Syntax,
    Limit,
}
/** Decimal chunks amortize multiplication without exceeding a double-width arithmetic intermediate. */
const DECIMAL_CHUNK: usize = 9;
/** One full decimal chunk fits in one unsigned limb. */
const DECIMAL_RADIX: u32 = 1_000_000_000;

impl BigInt {
    /** Decimal chunk multiplication is quadratic; radix-prefixed input and invalid syntax need only a scan. */
    pub(in crate::v9) fn parse_work(text: &str) -> usize {
        let digits = text
            .trim_matches(crate::v9::source::whitespace)
            .trim_start_matches(['+', '-'])
            .trim_start_matches('0');
        let scan = text.len().max(1);
        if digits.bytes().all(|byte| byte.is_ascii_digit()) {
            let chunks = digits.len().div_ceil(DECIMAL_CHUNK);
            scan.saturating_add(chunks.saturating_mul(chunks))
        } else {
            scan
        }
    }
    /** Formatting charges its actual algorithm class before entering a potentially long division loop. */
    pub(in crate::v9) fn format_work(&self, radix: u32) -> usize {
        if radix.is_power_of_two() {
            return self.bit_len().max(1);
        }
        let width = (u32::BITS - 1 - radix.leading_zeros()) as usize;
        let mut base = radix;
        let mut chunk = 1usize;
        while let Some(next) = base.checked_mul(radix) {
            base = next;
            chunk += 1;
        }
        self.words
            .len()
            .saturating_mul(self.bit_len().div_ceil(width * chunk))
            .max(1)
    }
    /** Diagnostic previews stay bounded without changing language-level String conversion. */
    pub(in crate::v9) fn diagnostic(&self) -> String {
        const PREVIEW_BITS: usize = 256;
        if self.bit_len() <= PREVIEW_BITS {
            return self.to_string();
        }
        let magnitude = if self.negative {
            self.negated()
        } else {
            self.clone()
        };
        let prefix = magnitude
            .shift_right(self.bit_len() - PREVIEW_BITS)
            .to_radix(16);
        format!("{}0x{prefix}…", if self.negative { "-" } else { "" })
    }
    /** StringToBigInt admits whitespace, signed decimals and unsigned radix-prefixed integers. */
    pub(in crate::v9) fn parse(text: &str) -> Result<Self, ParseError> {
        let text = text.trim_matches(crate::v9::source::whitespace);
        if text.is_empty() {
            return Ok(Self::zero());
        }
        let signed = text.starts_with(['+', '-']);
        let negative = text.starts_with('-');
        let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
        let radix = match unsigned.get(..2) {
            Some("0x" | "0X") => 16,
            Some("0o" | "0O") => 8,
            Some("0b" | "0B") => 2,
            _ => 10,
        };
        if signed && radix != 10 {
            return Err(ParseError::Syntax);
        }
        let digits = if radix == 10 {
            unsigned
        } else {
            &unsigned[2..]
        };
        if digits.is_empty()
            || !digits
                .bytes()
                .all(|byte| (byte as char).to_digit(radix).is_some())
        {
            return Err(ParseError::Syntax);
        }
        let digits = digits.trim_start_matches('0');
        if digits.is_empty() {
            return Ok(Self::zero());
        }
        let words = if radix == 10 {
            parse_decimal(digits)?
        } else {
            parse_power(digits, radix)?
        };
        Self::new(negative, words).ok_or(ParseError::Limit)
    }
    /** Radices that are powers of two extract bits directly; others divide by the largest limb-sized radix power. */
    pub(in crate::v9) fn to_radix(&self, radix: u32) -> String {
        if self.is_zero() {
            return "0".to_string();
        }
        let mut digits = if radix.is_power_of_two() {
            format_power(self, radix)
        } else {
            format_chunks(self, radix)
        };
        if self.negative {
            digits.insert(0, '-');
        }
        digits
    }
    /** Number conversion rounds once with guard/sticky bits and ties to even. */
    pub(in crate::v9) fn to_f64(&self) -> f64 {
        let length = self.bit_len();
        if length == 0 {
            return 0.0;
        }
        let dropped = length.saturating_sub(f64::MANTISSA_DIGITS as usize);
        let mut top = 0u64;
        for bit in (dropped..length).rev() {
            top = (top << 1) | u64::from(self.bit(bit));
        }
        if dropped != 0 && self.bit(dropped - 1) && (self.any_low(dropped - 1) || top & 1 != 0) {
            top += 1;
        }
        let number = top as f64 * 2f64.powi(dropped as i32);
        if self.negative { -number } else { number }
    }
    /** Every finite integral Number has an exact binary integer representation, including values above i128. */
    pub(in crate::v9) fn from_f64(number: f64) -> Option<Self> {
        if !number.is_finite() || number.fract() != 0.0 {
            return None;
        }
        if number == 0.0 {
            return Some(Self::zero());
        }
        let bits = number.abs().to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023;
        let significand = (bits & ((1u64 << 52) - 1)) | (1u64 << 52);
        let value = Self::from_i128(i128::from(significand));
        let mut value = if exponent >= 52 {
            value.shift_left((exponent - 52) as usize)?
        } else {
            value.shift_right((52 - exponent) as usize)
        };
        value.negative = number < 0.0;
        Some(value)
    }
    /** Individual bit reads are bounded by the canonical magnitude. */
    fn bit(&self, bit: usize) -> bool {
        self.words
            .get(bit / WORD_BITS)
            .is_some_and(|word| word & (1u32 << (bit % WORD_BITS)) != 0)
    }
    /** A sticky bit summarizes all discarded words without allocating a shifted copy. */
    fn any_low(&self, bits: usize) -> bool {
        let whole = bits / WORD_BITS;
        self.words[..whole.min(self.words.len())]
            .iter()
            .any(|word| *word != 0)
            || bits % WORD_BITS != 0
                && self
                    .words
                    .get(whole)
                    .is_some_and(|word| word & ((1u32 << (bits % WORD_BITS)) - 1) != 0)
    }
}

/** Decimal parsing works on nine-digit chunks and checks growth before appending another limb. */
fn parse_decimal(digits: &str) -> Result<Vec<u32>, ParseError> {
    if digits.len() > MAX_BITS {
        return Err(ParseError::Limit);
    }
    let mut words = Vec::new();
    let first = match digits.len() % DECIMAL_CHUNK {
        0 => DECIMAL_CHUNK,
        length => length,
    };
    let mut offset = 0;
    while offset < digits.len() {
        let length = if offset == 0 { first } else { DECIMAL_CHUNK };
        let chunk = digits[offset..offset + length]
            .parse::<u32>()
            .map_err(|_| ParseError::Syntax)?;
        let factor = if length == DECIMAL_CHUNK {
            DECIMAL_RADIX
        } else {
            10u32.pow(length as u32)
        };
        multiply_add(&mut words, factor, chunk)?;
        offset += length;
    }
    Ok(words)
}
/** One limb-sized multiply-add cannot overflow its u64 intermediate. */
fn multiply_add(words: &mut Vec<u32>, factor: u32, add: u32) -> Result<(), ParseError> {
    let mut carry = u64::from(add);
    for word in words.iter_mut() {
        let value = u64::from(*word) * u64::from(factor) + carry;
        *word = value as u32;
        carry = value >> WORD_BITS;
    }
    if carry != 0 {
        if words.len() >= MAX_WORDS {
            return Err(ParseError::Limit);
        }
        words.push(carry as u32);
    }
    Ok(())
}
/** Radix prefixes provide a fixed number of bits per ASCII digit. */
fn parse_power(digits: &str, radix: u32) -> Result<Vec<u32>, ParseError> {
    let width = radix.trailing_zeros() as usize;
    let first = digits.as_bytes()[0] as char;
    let first = first.to_digit(radix).ok_or(ParseError::Syntax)?;
    let length = (digits.len() - 1)
        .checked_mul(width)
        .and_then(|bits| bits.checked_add(WORD_BITS - first.leading_zeros() as usize))
        .ok_or(ParseError::Limit)?;
    if length > MAX_BITS {
        return Err(ParseError::Limit);
    }
    let mut words = vec![0; length.div_ceil(WORD_BITS)];
    for (index, digit) in digits.bytes().rev().enumerate() {
        let value = (digit as char).to_digit(radix).ok_or(ParseError::Syntax)?;
        let offset = index * width;
        let word = offset / WORD_BITS;
        let shift = offset % WORD_BITS;
        words[word] |= value << shift;
        if shift + width > WORD_BITS && word + 1 < words.len() {
            words[word + 1] |= value >> (WORD_BITS - shift);
        }
    }
    Ok(words)
}
/** Direct radix extraction is linear even for million-bit powers of two. */
fn format_power(value: &BigInt, radix: u32) -> String {
    let width = radix.trailing_zeros() as usize;
    let count = value.bit_len().div_ceil(width);
    let mut result = String::with_capacity(count);
    for index in (0..count).rev() {
        let offset = index * width;
        let word = offset / WORD_BITS;
        let shift = offset % WORD_BITS;
        let pair = u64::from(value.words[word])
            | (u64::from(value.words.get(word + 1).copied().unwrap_or(0)) << WORD_BITS);
        let digit = ((pair >> shift) & u64::from(radix - 1)) as u32;
        result.push(char::from_digit(digit, radix).expect("radix digit"));
    }
    result
}
/** Chunk division avoids repeated per-character whole-magnitude division. */
fn format_chunks(value: &BigInt, radix: u32) -> String {
    let mut base = radix;
    let mut width = 1;
    while let Some(next) = base.checked_mul(radix) {
        base = next;
        width += 1;
    }
    let mut words = value.words.to_vec();
    let mut chunks = Vec::new();
    while !words.is_empty() {
        chunks.push(division::divide_word(&mut words, base));
    }
    let mut result = String::new();
    for (index, chunk) in chunks.into_iter().rev().enumerate() {
        let mut digits = small_digits(chunk, radix);
        if index != 0 {
            for _ in digits.len()..width {
                result.push('0');
            }
        }
        result.push_str(&digits);
        digits.clear();
    }
    result
}
/** One formatting chunk is a machine word and never requires arbitrary-precision recursion. */
fn small_digits(mut value: u32, radix: u32) -> String {
    let mut digits = Vec::new();
    loop {
        digits.push(char::from_digit(value % radix, radix).expect("radix digit"));
        value /= radix;
        if value == 0 {
            break;
        }
    }
    digits.into_iter().rev().collect()
}
