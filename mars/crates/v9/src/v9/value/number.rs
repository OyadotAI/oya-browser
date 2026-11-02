/** ECMAScript numeric grammar and one-rounding power-of-two integer conversion. */
use super::super::source::whitespace;
use crate::protocol::{EngineError, EngineResult};

mod radix;
/** Literal separators are accepted only between two digits of the same radix. */
pub(in crate::v9) fn literal(source: &str) -> EngineResult<f64> {
    let radix = radix(source);
    let digits = if radix.is_some() {
        &source[2..]
    } else {
        source
    };
    for (index, _) in digits.bytes().enumerate().filter(|(_, b)| *b == b'_') {
        if index == 0
            || !digit(digits.as_bytes()[index - 1], radix.unwrap_or(10))
            || !digits
                .as_bytes()
                .get(index + 1)
                .is_some_and(|b| digit(*b, radix.unwrap_or(10)))
        {
            return Err(invalid());
        }
    }
    parse(&source.replace('_', ""), false).ok_or_else(invalid)
}
/** StringToNumber accepts whitespace, signed decimals/Infinity and unsigned radix integers. */
pub(in crate::v9) fn string_number(source: &str) -> f64 {
    let source = source.trim_matches(whitespace);
    if source.is_empty() {
        return 0.0;
    }
    parse(source, true).unwrap_or(f64::NAN)
}
/** Rust's decimal parser is used only after the supported language grammar validates. */
fn parse(source: &str, string: bool) -> Option<f64> {
    if string {
        match source {
            "Infinity" | "+Infinity" => return Some(f64::INFINITY),
            "-Infinity" => return Some(f64::NEG_INFINITY),
            _ => {}
        }
    }
    if let Some(radix) = radix(source) {
        return radix_number(&source[2..], radix);
    }
    let unsigned = source.strip_prefix(['+', '-']).unwrap_or(source);
    if !string
        && unsigned.len() > 1
        && unsigned.starts_with('0')
        && unsigned.as_bytes()[1].is_ascii_digit()
    {
        return None;
    }
    if !decimal(unsigned) {
        return None;
    }
    source.parse::<f64>().ok()
}
/** Prefixed forms never accept a sign or a separator at the StringToNumber boundary. */
fn radix(source: &str) -> Option<u32> {
    if source.starts_with("0x") || source.starts_with("0X") {
        Some(16)
    } else if source.starts_with("0o") || source.starts_with("0O") {
        Some(8)
    } else if source.starts_with("0b") || source.starts_with("0B") {
        Some(2)
    } else {
        None
    }
}
/** Decimal validation requires mantissa digits and a complete optional exponent. */
fn decimal(source: &str) -> bool {
    let (mantissa, exponent) = source
        .find(['e', 'E'])
        .map(|i| (&source[..i], Some(&source[i + 1..])))
        .unwrap_or((source, None));
    if let Some(exponent) = exponent {
        let exponent = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
        if exponent.is_empty() || !exponent.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    let mut dots = 0;
    let mut digits = 0;
    for byte in mantissa.bytes() {
        if byte == b'.' {
            dots += 1;
        } else if byte.is_ascii_digit() {
            digits += 1;
        } else {
            return false;
        }
    }
    dots <= 1 && digits > 0
}
/** The entire significant integer is rounded once with guard/sticky bits and ties-to-even. */
fn radix_number(source: &str, radix: u32) -> Option<f64> {
    if source.is_empty() || !source.bytes().all(|b| digit(b, radix)) {
        return None;
    }
    let bits_per_digit = radix.trailing_zeros();
    let mut bits = Vec::new();
    for character in source.chars() {
        let digit = character.to_digit(radix)?;
        for bit in (0..bits_per_digit).rev() {
            if !bits.is_empty() || digit >> bit & 1 != 0 {
                bits.push(digit >> bit & 1 != 0);
            }
        }
    }
    if bits.is_empty() {
        return Some(0.0);
    }
    if bits.len() > 1024 {
        return Some(f64::INFINITY);
    }
    let retained = bits.len().min(f64::MANTISSA_DIGITS as usize);
    let mut significand = bits[..retained]
        .iter()
        .fold(0_u64, |value, bit| (value << 1) | u64::from(*bit));
    if bits.len() > retained
        && bits[retained]
        && (bits[retained + 1..].iter().any(|bit| *bit) || significand & 1 != 0)
    {
        significand += 1;
    }
    Some(significand as f64 * 2_f64.powi((bits.len() - retained) as i32))
}
/** Radix digits are ASCII; Unicode digit classes are not JavaScript numeric syntax. */
fn digit(byte: u8, radix: u32) -> bool {
    (byte as char).is_digit(radix)
}
/** Caller attaches the original token location without echoing the numeric source. */
fn invalid() -> EngineError {
    EngineError::new(
        "script_syntax_error",
        "Invalid JavaScript numeric literal.",
        "Use a supported complete numeric literal; no script changes were applied.",
    )
}

/** ToUint32 truncates before modulo; nonfinite numbers and signed zero produce zero. */
pub(in crate::v9) fn to_uint32(value: f64) -> u32 {
    if !value.is_finite() || value == 0.0 {
        return 0;
    }
    value
        .trunc()
        .rem_euclid(crate::constants::JS_UINT32_MODULUS) as u32
}

/** ToInt32: ToUint32 reinterpreted as signed. */
pub(in crate::v9) fn to_int32(value: f64) -> i32 {
    to_uint32(value) as i32
}

/** Number::toString(10), using the pinned ECMAScript-compatible shortest formatting. */
pub(in crate::v9) fn number_to_string(value: f64) -> String {
    ryu_js::Buffer::new().format(value).to_string()
}

/** Number::toString(radix) uses the pinned conversion algorithm under the binary64 precision bound. */
pub(in crate::v9) fn radix_string(value: f64, radix: u32) -> String {
    radix::format(value, radix)
}
