/** Infinite two's-complement bitwise operations and signed shifts preserve exact mathematical values. */
use super::*;

impl BigInt {
    /** A power-of-two magnitude can be shifted and multiplied without processing its zero prefix. */
    pub(in crate::v9) fn power_of_two(&self) -> Option<usize> {
        let mut bit = None;
        for (i, &word) in self.words.iter().enumerate() {
            if word == 0 {
                continue;
            }
            if bit.is_some() || !word.is_power_of_two() {
                return None;
            }
            bit = Some(i * WORD_BITS + word.trailing_zeros() as usize);
        }
        bit
    }
    /** Left shift checks the resulting bit length before allocating its bounded zero prefix. */
    pub(in crate::v9) fn shift_left(&self, count: usize) -> Option<Self> {
        if self.is_zero() {
            return Some(self.clone());
        }
        if self.bit_len().checked_add(count)? > MAX_BITS {
            return None;
        }
        Self::new(self.negative, shift_left_words(&self.words, count))
    }
    /** Arithmetic right shift rounds negative values toward negative infinity, including huge counts. */
    pub(in crate::v9) fn shift_right(&self, count: usize) -> Self {
        if count >= self.bit_len() {
            return Self::from_i128(if self.negative { -1 } else { 0 });
        }
        let mut result =
            Self::new(false, shift_right_words(&self.words, count)).expect("right shift fits");
        if self.negative && lost_bits(&self.words, count) {
            result = result
                .add(&Self::from_i128(1))
                .expect("right-shift rounding fits");
        }
        result.negative = self.negative && !result.is_zero();
        result
    }
    /** A finite extra sign word models infinitely extended operands before result normalization. */
    pub(in crate::v9) fn bitwise(
        &self,
        other: &Self,
        operation: impl Fn(u32, u32) -> u32,
    ) -> Option<Self> {
        let width = self.words.len().max(other.words.len()) + 1;
        let left = self.twos_words(width);
        let right = other.twos_words(width);
        let mut words: Vec<_> = left
            .into_iter()
            .zip(right)
            .map(|(a, b)| operation(a, b))
            .collect();
        let negative = words.last().is_some_and(|word| word & 0x8000_0000 != 0);
        if negative {
            negate_words(&mut words);
        }
        Self::new(negative, words)
    }
    /** Complement follows ~x = -x - 1, including values beyond the inline representation. */
    pub(in crate::v9) fn complement(&self) -> Option<Self> {
        self.negated().subtract(&Self::from_i128(1))
    }
    /** BigInt.asUintN uses low two's-complement bits rather than clamping or saturating. */
    pub(in crate::v9) fn unsigned_n(&self, bits: u64) -> Option<Self> {
        if bits == 0 {
            return Some(Self::zero());
        }
        if !self.negative && bits >= self.bit_len() as u64 {
            return Some(self.clone());
        }
        let bits = usize::try_from(bits)
            .ok()
            .filter(|bits| *bits <= MAX_BITS)?;
        let mut words = self.twos_words(bits.div_ceil(WORD_BITS));
        mask_last(&mut words, bits);
        Self::new(false, words)
    }
    /** BigInt.asIntN interprets exactly its low bits with the selected signed width. */
    pub(in crate::v9) fn signed_n(&self, bits: u64) -> Self {
        if bits == 0 {
            return Self::zero();
        }
        if bits > self.bit_len() as u64 {
            return self.clone();
        }
        let bits = bits as usize;
        let mut words = self.twos_words(bits.div_ceil(WORD_BITS));
        mask_last(&mut words, bits);
        let negative = words[(bits - 1) / WORD_BITS] & (1u32 << ((bits - 1) % WORD_BITS)) != 0;
        if negative {
            negate_words(&mut words);
            mask_last(&mut words, bits);
        }
        Self::new(negative, words).expect("truncation fits")
    }
    /** Each negative operand is sign-extended within the bounded temporary width. */
    fn twos_words(&self, width: usize) -> Vec<u32> {
        let mut words = vec![0; width];
        let copied = width.min(self.words.len());
        words[..copied].copy_from_slice(&self.words[..copied]);
        if self.negative {
            negate_words(&mut words);
        }
        words
    }
}

/** Unsigned left shifts are shared with quotient normalization. */
pub(super) fn shift_left_words(words: &[u32], count: usize) -> Vec<u32> {
    if words.is_empty() {
        return Vec::new();
    }
    let whole = count / WORD_BITS;
    let bits = count % WORD_BITS;
    let mut result = vec![0; whole];
    let mut carry = 0u64;
    for &word in words {
        let value = (u64::from(word) << bits) | carry;
        result.push(value as u32);
        carry = value >> WORD_BITS;
    }
    if carry != 0 {
        result.push(carry as u32);
    }
    result
}
/** Unsigned right shifts never round; signed callers apply their own lost-bit correction. */
pub(super) fn shift_right_words(words: &[u32], count: usize) -> Vec<u32> {
    let whole = count / WORD_BITS;
    let bits = count % WORD_BITS;
    if whole >= words.len() {
        return Vec::new();
    }
    let mut result = Vec::with_capacity(words.len() - whole);
    for i in whole..words.len() {
        let mut value = words[i] >> bits;
        if bits != 0 && i + 1 < words.len() {
            value |= words[i + 1] << (WORD_BITS - bits);
        }
        result.push(value);
    }
    trim(&mut result);
    result
}
/** Dropped low bits distinguish exact division by a power of two from negative floor rounding. */
fn lost_bits(words: &[u32], count: usize) -> bool {
    let whole = count / WORD_BITS;
    let bits = count % WORD_BITS;
    words[..whole.min(words.len())]
        .iter()
        .any(|word| *word != 0)
        || bits != 0
            && words
                .get(whole)
                .is_some_and(|word| *word & ((1u32 << bits) - 1) != 0)
}
/** Complement plus carry implements bounded two's-complement negation without signed overflow. */
fn negate_words(words: &mut [u32]) {
    let mut carry = 1u64;
    for word in words {
        let value = u64::from(!*word) + carry;
        *word = value as u32;
        carry = value >> WORD_BITS;
    }
}
/** A partial top word contains only the requested low bits. */
fn mask_last(words: &mut [u32], bits: usize) {
    if bits % WORD_BITS != 0 {
        if let Some(last) = words.last_mut() {
            *last &= (1u32 << (bits % WORD_BITS)) - 1;
        }
    }
}
