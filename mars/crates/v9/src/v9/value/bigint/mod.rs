/** Immutable signed limbs extend BigInts within the existing retained-value memory budget. */
use std::{cmp::Ordering, fmt, sync::Arc};
mod arithmetic;
mod bits;
mod division;
mod text;
pub(in crate::v9) use text::ParseError;
mod wire;

/** A limb occupies two of the UTF-16-sized retained units already charged by V9. */
pub(in crate::v9) const MAX_WORDS: usize = crate::constants::JS_STRING_UNITS / 2;
/** Bits per unsigned limb; arithmetic intermediates use a double-width word. */
const WORD_BITS: usize = u32::BITS as usize;
/** Allocation bounds derive from the existing per-value byte allowance, not a larger engine quota. */
pub(in crate::v9) const MAX_BITS: usize = MAX_WORDS * WORD_BITS;

/** Canonical magnitude storage is immutable and shared by copies and sign-only operations. */
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct BigInt {
    negative: bool,
    words: Arc<[u32]>,
}

impl BigInt {
    /** Every construction removes leading zero limbs and rejects an oversized allocation. */
    fn new(negative: bool, mut words: Vec<u32>) -> Option<Self> {
        trim(&mut words);
        if words.len() > MAX_WORDS {
            return None;
        }
        Some(Self {
            negative: negative && !words.is_empty(),
            words: words.into(),
        })
    }
    /** Native binary decoders share normalization after checking the same pre-allocation limb bound. */
    pub(in crate::v9) fn from_words(negative: bool, words: Vec<u32>) -> Option<Self> {
        if words.len() > MAX_WORDS {
            return None;
        }
        Self::new(negative, words)
    }
    /** Packet readers can reject a declared word count before allocating any magnitude data. */
    pub(in crate::v9) fn word_limit() -> usize {
        MAX_WORDS
    }
    /** Inline values enter the arbitrary-width core exactly, including i128::MIN. */
    pub(in crate::v9) fn from_i128(value: i128) -> Self {
        let mut magnitude = value.unsigned_abs();
        let mut words = Vec::new();
        while magnitude != 0 {
            words.push(magnitude as u32);
            magnitude >>= WORD_BITS;
        }
        Self::new(value < 0, words).expect("inline BigInt fits")
    }
    /** Canonical zero has no magnitude allocation. */
    pub(in crate::v9) fn zero() -> Self {
        Self::from_i128(0)
    }
    /** An immutable sign does not create a second copy of the limbs. */
    pub(in crate::v9) fn negated(&self) -> Self {
        Self {
            negative: !self.negative && !self.is_zero(),
            words: self.words.clone(),
        }
    }
    /** Zero never has a negative representation. */
    pub(in crate::v9) fn is_zero(&self) -> bool {
        self.words.is_empty()
    }
    /** Sign tests do not convert or allocate. */
    pub(in crate::v9) fn negative(&self) -> bool {
        self.negative
    }
    /** Retained limb units are charged through the same budget as strings and buffers. */
    pub(in crate::v9) fn units(&self) -> usize {
        self.words.len() * 2
    }
    /** The collector deduplicates immutable backing contents when recounting retained values. */
    pub(in crate::v9) fn words(&self) -> &[u32] {
        &self.words
    }
    /** Mathematical bit length excludes the sign and canonical leading zeros. */
    pub(in crate::v9) fn bit_len(&self) -> usize {
        self.words.last().map_or(0, |last| {
            (self.words.len() - 1) * WORD_BITS + WORD_BITS - last.leading_zeros() as usize
        })
    }
    /** Results fitting the original public inline representation are normalized back into it. */
    pub(in crate::v9) fn to_i128(&self) -> Option<i128> {
        if self.words.len() > 4 {
            return None;
        }
        let magnitude = self.words.iter().rev().fold(0u128, |value, word| {
            (value << WORD_BITS) | u128::from(*word)
        });
        if self.negative {
            if magnitude == 1u128 << 127 {
                Some(i128::MIN)
            } else {
                i128::try_from(magnitude).ok().map(|value| -value)
            }
        } else {
            i128::try_from(magnitude).ok()
        }
    }
    /** Small nonnegative counts are admitted without saturating a large positive magnitude. */
    pub(in crate::v9) fn to_usize(&self) -> Option<usize> {
        if self.negative || self.bit_len() > usize::BITS as usize {
            return None;
        }
        Some(
            self.words
                .iter()
                .rev()
                .fold(0u64, |value, word| (value << WORD_BITS) | u64::from(*word))
                as usize,
        )
    }
    /** TypedArray and Wasm i64 conversions use the mathematical value modulo 2^64. */
    pub(in crate::v9) fn low_u64(&self) -> u64 {
        let magnitude = u64::from(self.words.first().copied().unwrap_or(0))
            | (u64::from(self.words.get(1).copied().unwrap_or(0)) << WORD_BITS);
        if self.negative {
            magnitude.wrapping_neg()
        } else {
            magnitude
        }
    }
    /** Multiplication work estimates exclude zero prefixes and account for actual limb products. */
    pub(in crate::v9) fn product_work(&self, other: &Self) -> usize {
        let live = |words: &[u32]| {
            words.len().saturating_sub(
                words
                    .iter()
                    .position(|word| *word != 0)
                    .unwrap_or(words.len()),
            )
        };
        live(&self.words).saturating_mul(live(&other.words))
    }
}

impl Ord for BigInt {
    /** Signed comparison never rounds through a floating-point intermediary. */
    fn cmp(&self, other: &Self) -> Ordering {
        match self.negative.cmp(&other.negative) {
            Ordering::Equal => {
                if self.negative {
                    magnitude_cmp(&other.words, &self.words)
                } else {
                    magnitude_cmp(&self.words, &other.words)
                }
            }
            Ordering::Less => Ordering::Greater,
            Ordering::Greater => Ordering::Less,
        }
    }
}
impl PartialOrd for BigInt {
    /** Canonical BigInts have a total mathematical ordering. */
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl fmt::Debug for BigInt {
    /** Diagnostics cannot accidentally print millions of retained limbs. */
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BigInt")
            .field("negative", &self.negative)
            .field("bits", &self.bit_len())
            .finish()
    }
}
impl fmt::Display for BigInt {
    /** Decimal formatting shares the radix conversion used by BigInt.prototype.toString. */
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_radix(10))
    }
}
/** Canonical magnitudes never retain high zero words. */
fn trim(words: &mut Vec<u32>) {
    while words.last() == Some(&0) {
        words.pop();
    }
}
/** A longer canonical magnitude is larger, otherwise compare most-significant limbs first. */
fn magnitude_cmp(left: &[u32], right: &[u32]) -> Ordering {
    left.len()
        .cmp(&right.len())
        .then_with(|| left.iter().rev().cmp(right.iter().rev()))
}
