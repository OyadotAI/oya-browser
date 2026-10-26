/** Exact binary summation accumulates finite IEEE-754 inputs as integers before one rounding. */

/** IEEE-754 double precision's fraction width. */
const FRACTION: usize = 52;
/** Finite exponent fields plus the maximum safe iterable count bound the exact integer. */
const LIMBS: usize = (2046 + FRACTION + 53).div_ceil(u64::BITS as usize);
/** Mask excluding the implicit leading bit. */
const FRACTION_MASK: u64 = (1 << FRACTION) - 1;
/** The all-ones exponent field encodes infinities and NaNs. */
const INFINITE_EXPONENT: u64 = 0x7ff;

/** Positive and negative magnitudes avoid any rounding or cancellation during accumulation. */
pub(super) struct ExactSum {
    positive: [u64; LIMBS],
    negative: [u64; LIMBS],
    nan: bool,
    positive_infinity: bool,
    negative_infinity: bool,
    negative_zero_only: bool,
}

impl ExactSum {
    /** An empty sum is negative zero, as are sums containing negative zero alone. */
    pub fn new() -> Self {
        Self {
            positive: [0; LIMBS],
            negative: [0; LIMBS],
            nan: false,
            positive_infinity: false,
            negative_infinity: false,
            negative_zero_only: true,
        }
    }

    /** Every finite number is an integer multiple of the smallest positive subnormal. */
    pub fn add(&mut self, number: f64) {
        self.negative_zero_only &= number.to_bits() == (-0.0f64).to_bits();
        let bits = number.to_bits();
        let exponent = (bits >> FRACTION) & INFINITE_EXPONENT;
        let fraction = bits & FRACTION_MASK;
        if exponent == INFINITE_EXPONENT {
            self.special(number);
            return;
        }
        let mantissa = if exponent == 0 {
            fraction
        } else {
            fraction | (1 << FRACTION)
        };
        let magnitude = if number.is_sign_negative() {
            &mut self.negative
        } else {
            &mut self.positive
        };
        add_shifted(magnitude, mantissa, exponent.saturating_sub(1) as usize);
    }

    /** Infinities and NaNs still require the owning algorithm to exhaust and validate input. */
    fn special(&mut self, number: f64) {
        self.nan |= number.is_nan();
        self.positive_infinity |= number == f64::INFINITY;
        self.negative_infinity |= number == f64::NEG_INFINITY;
    }

    /** Cancel magnitudes exactly, then round to nearest with ties to even once. */
    pub fn finish(self) -> f64 {
        if self.nan || self.positive_infinity && self.negative_infinity {
            return f64::NAN;
        }
        if self.positive_infinity {
            return f64::INFINITY;
        }
        if self.negative_infinity {
            return f64::NEG_INFINITY;
        }
        let negative = self
            .negative
            .iter()
            .rev()
            .cmp(self.positive.iter().rev())
            .is_gt();
        let magnitude = if negative {
            subtract(self.negative, &self.positive)
        } else {
            subtract(self.positive, &self.negative)
        };
        rounded(&magnitude, negative, self.negative_zero_only)
    }
}

/** Add up to two shifted words and propagate every carry, never through floating arithmetic. */
fn add_shifted(words: &mut [u64; LIMBS], mantissa: u64, shift: usize) {
    let index = shift / u64::BITS as usize;
    let shifted = u128::from(mantissa) << (shift % u64::BITS as usize);
    let mut carry = shifted;
    for word in &mut words[index..] {
        let sum = u128::from(*word) + u128::from(carry as u64);
        *word = sum as u64;
        carry = (carry >> u64::BITS) + (sum >> u64::BITS);
        if carry == 0 {
            break;
        }
    }
}

/** Subtract the smaller unsigned magnitude, propagating borrows across exact words. */
fn subtract(mut larger: [u64; LIMBS], smaller: &[u64; LIMBS]) -> [u64; LIMBS] {
    let mut borrow = false;
    for (word, other) in larger.iter_mut().zip(smaller) {
        let (value, first) = word.overflowing_sub(*other);
        let (value, second) = value.overflowing_sub(u64::from(borrow));
        *word = value;
        borrow = first || second;
    }
    larger
}

/** Read a bit without assuming it lies in the most significant word. */
fn bit(words: &[u64; LIMBS], index: usize) -> bool {
    words[index / u64::BITS as usize] & (1 << (index % u64::BITS as usize)) != 0
}

/** Form the leading significand and apply guard/sticky/even rounding. */
fn significand(words: &[u64; LIMBS], high: usize) -> u64 {
    let shift = high.saturating_sub(FRACTION);
    let mut value = 0;
    for index in (shift..=high).rev() {
        value = (value << 1) | u64::from(bit(words, index));
    }
    if shift > 0
        && bit(words, shift - 1)
        && (value & 1 != 0 || (0..shift - 1).any(|index| bit(words, index)))
    {
        value += 1;
    }
    value
}

/** The exact integer's binary position determines subnormal, normal, or overflow encoding. */
fn rounded(words: &[u64; LIMBS], negative: bool, negative_zero_only: bool) -> f64 {
    let Some(last) = words.iter().rposition(|word| *word != 0) else {
        return if negative_zero_only { -0.0 } else { 0.0 };
    };
    let mut high =
        last * u64::BITS as usize + (u64::BITS - 1 - words[last].leading_zeros()) as usize;
    let mut value = significand(words, high);
    if value == 1 << (FRACTION + 1) {
        value >>= 1;
        high += 1;
    }
    let exponent = high.saturating_sub(FRACTION - 1) as u64;
    let bits = if exponent >= INFINITE_EXPONENT {
        INFINITE_EXPONENT << FRACTION
    } else {
        (exponent << FRACTION) | (value & FRACTION_MASK)
    };
    f64::from_bits(bits | (u64::from(negative) << (u64::BITS - 1)))
}
