/** Literal pattern escaping preserves UTF-16 and checks expansion before allocating output. */
use super::*;
use crate::constants::{
    JS_STRING_UNITS, UTF16_HIGH_SURROGATE_END, UTF16_HIGH_SURROGATE_START, UTF16_LOW_SURROGATE_END,
    UTF16_LOW_SURROGATE_START,
};
use crate::v9::source::whitespace;

/** Only these punctuation characters admit a grammar-independent backslash escape. */
const SYNTAX: &str = "^$\\.*+?()[]{}|/";
/** Other punctuation needs hexadecimal escaping to remain valid in Unicode patterns. */
const OTHER_PUNCTUATORS: &str = ",-=<>#&!%:;@~'`\"";
/** Hexadecimal escapes use canonical lowercase digits and fixed widths. */
const HEX: &[u8; 16] = b"0123456789abcdef";
const HEX_DIGITS: usize = 2;
const UNICODE_DIGITS: usize = 4;
const ESCAPE_PREFIX: usize = 2;
const NIBBLE_BITS: usize = 4;
const NIBBLE_MASK: u16 = 0xf;
const BYTE_MAX: u16 = 0xff;

/** Escape modes are determined entirely from primitive string data, without author callbacks. */
#[derive(Clone, Copy)]
enum Encoding {
    Raw,
    Backslash(u16),
    Hex,
    Unicode,
}

/** Reject boxed strings and other values before looking up any conversion hooks. */
pub(super) fn escape(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let JsValue::String(input) = call.arg(0) else {
        return Err(vm.type_error("RegExp.escape requires a string"));
    };
    let length = output_length(vm, &input)?;
    let mut output = Vec::with_capacity(length);
    for (index, (units, length)) in points(&input).enumerate() {
        let point = &units[..length];
        vm.charge(point.len())?;
        append(&mut output, point, encoding(point, index == 0));
    }
    Ok(JsValue::String(JsString::from_units(output)?))
}

/** Both the exact output size and all scanning work stay within the existing engine budgets. */
fn output_length(vm: &mut Vm, input: &JsString) -> Res<usize> {
    let mut length = 0usize;
    for (index, (units, point_length)) in points(input).enumerate() {
        let point = &units[..point_length];
        vm.charge(point.len())?;
        let size = encoded_length(point, encoding(point, index == 0));
        length = length
            .checked_add(size)
            .filter(|n| *n <= JS_STRING_UNITS)
            .ok_or_else(|| EngineError::limit("JavaScript string UTF-16 units", JS_STRING_UNITS))?;
    }
    Ok(length)
}

/** Read code points across rope boundaries without forcing a contiguous source allocation. */
fn points(input: &JsString) -> impl Iterator<Item = ([u16; 2], usize)> + '_ {
    let mut offset = 0;
    std::iter::from_fn(move || {
        let first = input.unit_at(offset)?;
        let second = input.unit_at(offset + 1);
        let paired = (UTF16_HIGH_SURROGATE_START..=UTF16_HIGH_SURROGATE_END).contains(&first)
            && second.is_some_and(|unit| {
                (UTF16_LOW_SURROGATE_START..=UTF16_LOW_SURROGATE_END).contains(&unit)
            });
        let length = if paired { 2 } else { 1 };
        offset += length;
        Some(([first, second.unwrap_or_default()], length))
    })
}

/** Leading ASCII alphanumerics cannot merge with an adjoining decimal or control escape. */
fn encoding(point: &[u16], first: bool) -> Encoding {
    if point.len() != 1 {
        return Encoding::Raw;
    }
    let Some(character) = char::from_u32(u32::from(point[0])) else {
        return Encoding::Unicode;
    };
    if first && character.is_ascii_alphanumeric() {
        return Encoding::Hex;
    }
    encode_character(character)
}

/** Syntax, control escapes, punctuation, and ECMAScript whitespace have distinct encodings. */
fn encode_character(character: char) -> Encoding {
    if SYNTAX.contains(character) {
        return Encoding::Backslash(character as u16);
    }
    if let Some(control) = control_escape(character) {
        return Encoding::Backslash(control);
    }
    if OTHER_PUNCTUATORS.contains(character) || whitespace(character) {
        return if character as u32 <= u32::from(BYTE_MAX) {
            Encoding::Hex
        } else {
            Encoding::Unicode
        };
    }
    Encoding::Raw
}

/** The specification's control table deliberately excludes NUL and backspace. */
fn control_escape(character: char) -> Option<u16> {
    match character {
        '\t' => Some(b't' as u16),
        '\n' => Some(b'n' as u16),
        '\u{b}' => Some(b'v' as u16),
        '\u{c}' => Some(b'f' as u16),
        '\r' => Some(b'r' as u16),
        _ => None,
    }
}

/** Exact sizes allow a single bounded allocation after the primitive-only preflight. */
fn encoded_length(point: &[u16], encoding: Encoding) -> usize {
    match encoding {
        Encoding::Raw => point.len(),
        Encoding::Backslash(_) => ESCAPE_PREFIX,
        Encoding::Hex => ESCAPE_PREFIX + HEX_DIGITS,
        Encoding::Unicode => ESCAPE_PREFIX + UNICODE_DIGITS,
    }
}

/** Valid surrogate pairs remain raw; unmatched code units already have Unicode mode. */
fn append(output: &mut Vec<u16>, point: &[u16], encoding: Encoding) {
    match encoding {
        Encoding::Raw => output.extend_from_slice(point),
        Encoding::Backslash(character) => output.extend([b'\\' as u16, character]),
        Encoding::Hex => append_hex(output, point[0], b'x', HEX_DIGITS),
        Encoding::Unicode => append_hex(output, point[0], b'u', UNICODE_DIGITS),
    }
}

/** Writing digits directly avoids temporary strings and preserves leading zeroes. */
fn append_hex(output: &mut Vec<u16>, unit: u16, prefix: u8, digits: usize) {
    output.extend([b'\\' as u16, prefix as u16]);
    for index in (0..digits).rev() {
        let digit = (unit >> (index * NIBBLE_BITS)) & NIBBLE_MASK;
        output.push(HEX[usize::from(digit)] as u16);
    }
}
