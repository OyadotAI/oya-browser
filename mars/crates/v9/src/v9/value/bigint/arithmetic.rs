/** Signed addition and multiplication operate on bounded canonical magnitudes. */
use super::*;

impl BigInt {
    /** Equal signs add magnitudes; opposite signs subtract the smaller magnitude from the larger. */
    pub(in crate::v9) fn add(&self, other: &Self) -> Option<Self> {
        if self.negative == other.negative {
            return Self::new(self.negative, add(&self.words, &other.words)?);
        }
        let order = magnitude_cmp(&self.words, &other.words);
        if order.is_lt() {
            Self::new(other.negative, subtract(&other.words, &self.words))
        } else {
            Self::new(self.negative, subtract(&self.words, &other.words))
        }
    }
    /** Subtraction shares the immutable operand storage when changing its sign. */
    pub(in crate::v9) fn subtract(&self, other: &Self) -> Option<Self> {
        self.add(&other.negated())
    }
    /** Zero prefixes are skipped, making shifted sparse values ordinary linear-space multiplication. */
    pub(in crate::v9) fn multiply(&self, other: &Self) -> Option<Self> {
        if self.is_zero() || other.is_zero() {
            return Some(Self::zero());
        }
        let length = self.words.len().checked_add(other.words.len())?;
        if length > MAX_WORDS + 1 {
            return None;
        }
        let mut result = vec![0u32; length];
        let first = other.words.iter().position(|word| *word != 0).unwrap_or(0);
        for (i, &left) in self
            .words
            .iter()
            .enumerate()
            .filter(|(_, word)| **word != 0)
        {
            let mut carry = 0u64;
            for (j, &right) in other.words.iter().enumerate().skip(first) {
                let value = u64::from(left) * u64::from(right) + u64::from(result[i + j]) + carry;
                result[i + j] = value as u32;
                carry = value >> WORD_BITS;
            }
            result[i + other.words.len()] = carry as u32;
        }
        Self::new(self.negative != other.negative, result)
    }
    /** Exponentiation by squaring is bounded before shifts or limb allocations. */
    pub(in crate::v9) fn power(&self, exponent: &Self) -> Option<Self> {
        if exponent.negative {
            return None;
        }
        if exponent.is_zero() {
            return Some(Self::from_i128(1));
        }
        if self.is_zero() {
            return Some(Self::zero());
        }
        if self.words.as_ref() == [1] {
            return Some(if self.negative && exponent.low_u64() & 1 != 0 {
                Self::from_i128(-1)
            } else {
                Self::from_i128(1)
            });
        }
        let mut exponent = exponent.to_usize()?;
        if let Some(bit) = self.power_of_two() {
            let shift = bit.checked_mul(exponent)?;
            return Self::from_i128(if self.negative && exponent & 1 != 0 {
                -1
            } else {
                1
            })
            .shift_left(shift);
        }
        if (self.bit_len() - 1).checked_mul(exponent)? >= MAX_BITS {
            return None;
        }
        let mut base = self.clone();
        let mut result = Self::from_i128(1);
        while exponent != 0 {
            if exponent & 1 != 0 {
                result = result.multiply(&base)?;
            }
            exponent >>= 1;
            if exponent != 0 {
                base = base.multiply(&base)?;
            }
        }
        Some(result)
    }
}

/** Unsigned addition has only one carry word beyond the larger input. */
fn add(left: &[u32], right: &[u32]) -> Option<Vec<u32>> {
    let length = left.len().max(right.len());
    if length > MAX_WORDS {
        return None;
    }
    let mut result = Vec::with_capacity(length + 1);
    let mut carry = 0u64;
    for i in 0..length {
        let value = u64::from(left.get(i).copied().unwrap_or(0))
            + u64::from(right.get(i).copied().unwrap_or(0))
            + carry;
        result.push(value as u32);
        carry = value >> WORD_BITS;
    }
    if carry != 0 {
        result.push(carry as u32);
    }
    Some(result)
}
/** The caller orders the inputs, so the final borrow must be zero. */
fn subtract(left: &[u32], right: &[u32]) -> Vec<u32> {
    let mut result = Vec::with_capacity(left.len());
    let mut borrow = 0i64;
    for (i, &word) in left.iter().enumerate() {
        let value = i64::from(word) - i64::from(right.get(i).copied().unwrap_or(0)) - borrow;
        result.push(value as u32);
        borrow = i64::from(value < 0);
    }
    trim(&mut result);
    result
}
