/** Normalized base-2^32 long division computes quotient and remainder with bounded double-width intermediates. */
use super::*;

impl BigInt {
    /** Quotients truncate toward zero and remainders retain the dividend's sign. */
    pub(in crate::v9) fn divide(&self, other: &Self) -> Option<(Self, Self)> {
        if other.is_zero() {
            return None;
        }
        match magnitude_cmp(&self.words, &other.words) {
            Ordering::Less => return Some((Self::zero(), self.clone())),
            Ordering::Equal => {
                return Some((
                    Self::from_i128(if self.negative != other.negative {
                        -1
                    } else {
                        1
                    }),
                    Self::zero(),
                ));
            }
            Ordering::Greater => {}
        }
        let (quotient, remainder) = if let Some(bit) = other.power_of_two() {
            let quotient = bits::shift_right_words(&self.words, bit);
            let count = bit.div_ceil(WORD_BITS);
            let mut remainder = self.words[..count.min(self.words.len())].to_vec();
            if bit % WORD_BITS != 0 {
                if let Some(last) = remainder.last_mut() {
                    *last &= (1u32 << (bit % WORD_BITS)) - 1;
                }
            }
            (quotient, remainder)
        } else if other.words.len() == 1 {
            let mut quotient = self.words.to_vec();
            let remainder = divide_word(&mut quotient, other.words[0]);
            (quotient, vec![remainder])
        } else {
            divide_words(&self.words, &other.words)
        };
        Some((
            Self::new(self.negative != other.negative, quotient)?,
            Self::new(self.negative, remainder)?,
        ))
    }
}

/** Single-limb divisors are common in radix conversion and need one linear pass. */
pub(super) fn divide_word(words: &mut Vec<u32>, divisor: u32) -> u32 {
    let mut remainder = 0u64;
    for word in words.iter_mut().rev() {
        let value = (remainder << WORD_BITS) | u64::from(*word);
        *word = (value / u64::from(divisor)) as u32;
        remainder = value % u64::from(divisor);
    }
    trim(words);
    remainder as u32
}
/** Knuth's normalized quotient estimate uses two high dividend words and corrects with the next divisor word. */
fn divide_words(dividend: &[u32], divisor: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let shift = divisor.last().expect("divisor").leading_zeros() as usize;
    let v = bits::shift_left_words(divisor, shift);
    let mut u = bits::shift_left_words(dividend, shift);
    u.resize(dividend.len() + 1, 0);
    let n = v.len();
    let m = dividend.len() - n;
    let mut quotient = vec![0; m + 1];
    let base = 1u64 << WORD_BITS;
    let high = u64::from(v[n - 1]);
    for j in (0..=m).rev() {
        let numerator = (u64::from(u[j + n]) << WORD_BITS) | u64::from(u[j + n - 1]);
        let mut estimate = numerator / high;
        let mut remainder = numerator % high;
        while estimate >= base
            || u128::from(estimate) * u128::from(v[n - 2])
                > (u128::from(remainder) << WORD_BITS) + u128::from(u[j + n - 2])
        {
            estimate -= 1;
            remainder += high;
            if remainder >= base {
                break;
            }
        }
        if subtract_product(&mut u[j..=j + n], &v, estimate) {
            estimate -= 1;
            add_back(&mut u[j..=j + n], &v);
        }
        quotient[j] = estimate as u32;
    }
    (quotient, bits::shift_right_words(&u[..n], shift))
}
/** Subtract one estimated digit's product and report when the estimate was one too large. */
fn subtract_product(target: &mut [u32], divisor: &[u32], digit: u64) -> bool {
    let mut carry = 0u64;
    let mut borrow = 0i64;
    for (i, &word) in divisor.iter().enumerate() {
        let product = digit * u64::from(word) + carry;
        carry = product >> WORD_BITS;
        let difference = i64::from(target[i]) - i64::from(product as u32) - borrow;
        target[i] = difference as u32;
        borrow = i64::from(difference < 0);
    }
    let top = i64::from(target[divisor.len()]) - carry as i64 - borrow;
    target[divisor.len()] = top as u32;
    top < 0
}
/** The corrected quotient digit adds the divisor back to the negative temporary remainder. */
fn add_back(target: &mut [u32], divisor: &[u32]) {
    let mut carry = 0u64;
    for (i, &word) in divisor.iter().enumerate() {
        let value = u64::from(target[i]) + u64::from(word) + carry;
        target[i] = value as u32;
        carry = value >> WORD_BITS;
    }
    let last = divisor.len();
    target[last] = target[last].wrapping_add(carry as u32);
}
