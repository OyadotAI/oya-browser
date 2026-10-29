/*! Exact fixed decimal rounding: Rust supplies all digits, while binary ties round upward. */

/** Fraction bits in the binary64 significand, excluding its implicit leading bit. */
const FRACTION_BITS: u32 = f64::MANTISSA_DIGITS - 1;
/** All exponent bits set distinguishes finite binary64 exponents from infinities and NaNs. */
const EXPONENT_MASK: u64 = 0x7ff;
/** Stored exponent bias in binary64. */
const EXPONENT_BIAS: i32 = 1023;
/** Subnormal values have no implicit leading bit and share this binary exponent. */
const SUBNORMAL_EXPONENT: i32 = -1074;

/** Fixed decimal digits round an exact halfway magnitude up, including when it is negative. */
pub(super) fn fixed(value: f64, fraction: usize) -> String {
    let result = format!("{value:.fraction$}");
    if half_rounded_down(value, fraction) {
        return increment(result);
    }
    result
}

/** Detect a true binary midpoint without multiplying by ten and introducing another rounding. */
fn half_rounded_down(value: f64, fraction: usize) -> bool {
    if !value.is_finite() || value == 0.0 {
        return false;
    }
    let bits = value.abs().to_bits();
    let encoded = (bits >> FRACTION_BITS) & EXPONENT_MASK;
    let significand = significand(bits, encoded);
    let exponent = exponent(encoded);
    let zeros = significand.trailing_zeros();
    // 10^fraction contributes fraction powers of two. Its odd factor, 5^fraction,
    // is 1 modulo 4, so the remaining significand also identifies the even floor.
    exponent + zeros as i32 + fraction as i32 == -1 && ((significand >> zeros) & 2) == 0
}

/** Restore the implicit leading bit only for normal numbers. */
fn significand(bits: u64, encoded: u64) -> u64 {
    let fraction = bits & ((1u64 << FRACTION_BITS) - 1);
    fraction
        | if encoded == 0 {
            0
        } else {
            1u64 << FRACTION_BITS
        }
}

/** Binary exponent of the integer significand. */
fn exponent(encoded: u64) -> i32 {
    if encoded == 0 {
        SUBNORMAL_EXPONENT
    } else {
        encoded as i32 - EXPONENT_BIAS - FRACTION_BITS as i32
    }
}

/** Add one unit in the last printed place while retaining the sign and decimal point. */
fn increment(result: String) -> String {
    let mut bytes = result.into_bytes();
    for byte in bytes.iter_mut().rev().filter(|byte| byte.is_ascii_digit()) {
        if *byte < b'9' {
            *byte += 1;
            return String::from_utf8(bytes).expect("formatted numbers are ASCII");
        }
        *byte = b'0';
    }
    bytes.insert(usize::from(bytes.first() == Some(&b'-')), b'1');
    String::from_utf8(bytes).expect("formatted numbers are ASCII")
}

/** Significant digits that hold any binary64 value's exact decimal expansion (at most 767). */
const EXACT_DIGITS: usize = 800;

/** Exponential notation with `fraction` digits after the point, as `toExponential` and
`toPrecision` print it. Rust's `{:e}` rounds exact ties to even (`1.25` to `1.2e0`) and keeps the
sign of zero; the spec picks the larger magnitude on a tie and prints `-0` as `0`. So this rounds
the exact expansion half-up itself. */
pub(super) fn exponential(value: f64, fraction: usize) -> String {
    let exact = format!("{:.EXACT_DIGITS$e}", value.abs());
    let (mantissa, exponent) = exact.split_once('e').expect("exponent form");
    let digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).collect();
    let mut exponent: i32 = exponent.parse().expect("integer exponent");
    let mut kept = digits[..=fraction].to_vec();
    if digits[fraction + 1] >= b'5' && round_up(&mut kept) {
        exponent += 1;
    }
    let sign = if value < 0.0 { "-" } else { "" };
    format!("{sign}{}", exponent_form(&kept, exponent))
}

/** Adds one in the last kept digit; true when it carried out of the first (9.99 to 10.0). */
fn round_up(digits: &mut [u8]) -> bool {
    for digit in digits.iter_mut().rev() {
        if *digit < b'9' {
            *digit += 1;
            return false;
        }
        *digit = b'0';
    }
    digits[0] = b'1';
    true
}

/** `d.ddde+x` from significant digits and a decimal exponent. */
fn exponent_form(digits: &[u8], exponent: i32) -> String {
    let (first, rest) = digits.split_at(1);
    let point = if rest.is_empty() { "" } else { "." };
    let sign = if exponent < 0 { "-" } else { "+" };
    let text = |part: &[u8]| String::from_utf8(part.to_vec()).expect("ASCII digits");
    format!(
        "{}{point}{}e{sign}{}",
        text(first),
        text(rest),
        exponent.abs()
    )
}
