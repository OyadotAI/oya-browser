/**
 * Binary64 radix formatting, adapted from V8 DoubleToRadixStringView at the pinned revision.
 * Copyright 2011 the V8 project authors. All rights reserved. BSD terms: mars/THIRD_PARTY_NOTICES.md.
 * The next-double interval bounds precision; fractional digits may extend into subnormals.
 */
use super::number_to_string;

/** Every supported digit, indexed by its numeric value. */
const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
/** Integers beyond this magnitude omit low digits the input cannot represent. */
const UNREPRESENTED_INTEGER: f64 = 9_007_199_254_740_992.0;
/** A midpoint divides two neighboring representable binary64 values equally. */
const HALF: f64 = 0.5;

/** Format a valid radix without truncating long subnormal fractions or inventing low digits. */
pub(super) fn format(value: f64, radix: u32) -> String {
    if !value.is_finite() || value == 0.0 || radix == 10 {
        return number_to_string(value);
    }
    let magnitude = value.abs();
    let mut integer = magnitude.floor();
    let fraction = fraction(magnitude, radix, &mut integer);
    let mut output = integer_digits(integer, radix);
    output.extend(fraction);
    if value < 0.0 {
        output.insert(0, b'-');
    }
    String::from_utf8(output).expect("radix digits are ASCII")
}

/** Emit only fractional digits needed to distinguish the input from its next double. */
fn fraction(value: f64, radix: u32, integer: &mut f64) -> Vec<u8> {
    let mut rest = value - *integer;
    let mut delta = (HALF * (value.next_up() - value)).max(f64::from_bits(1));
    let mut digits = Vec::new();
    while rest >= delta {
        rest *= f64::from(radix);
        delta *= f64::from(radix);
        let digit = rest as u8;
        digits.push(digit);
        rest -= f64::from(digit);
        if should_round(rest, delta, digit) {
            carry(&mut digits, radix, integer);
            break;
        }
    }
    encode_fraction(digits)
}

/** Halfway remainders round to an even radix digit, with a carry when the interval crosses one. */
fn should_round(rest: f64, delta: f64, digit: u8) -> bool {
    (rest > HALF || (rest == HALF && digit & 1 != 0)) && rest + delta > 1.0
}

/** Carry backwards, dropping maximal trailing digits; an empty fraction carries to the integer. */
fn carry(digits: &mut Vec<u8>, radix: u32, integer: &mut f64) {
    while let Some(digit) = digits.pop() {
        if u32::from(digit) + 1 < radix {
            digits.push(digit + 1);
            return;
        }
    }
    *integer += 1.0;
}

/** The decimal point is absent when rounding removes the entire fraction. */
fn encode_fraction(digits: Vec<u8>) -> Vec<u8> {
    if digits.is_empty() {
        return Vec::new();
    }
    std::iter::once(b'.')
        .chain(digits.into_iter().map(|digit| DIGITS[digit as usize]))
        .collect()
}

/** Fill low unrepresented integer places with zero before converting the significant part. */
fn integer_digits(mut integer: f64, radix: u32) -> Vec<u8> {
    let radix = f64::from(radix);
    let mut digits = Vec::new();
    while integer / radix >= UNREPRESENTED_INTEGER {
        integer /= radix;
        digits.push(b'0');
    }
    loop {
        let remainder = integer % radix;
        digits.push(DIGITS[remainder as usize]);
        integer = (integer - remainder) / radix;
        if integer <= 0.0 {
            break;
        }
    }
    digits.reverse();
    digits
}
