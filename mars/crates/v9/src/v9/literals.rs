/** V8 ScanString/ScanEscape-guided UTF-16 decoding; see THIRD_PARTY_NOTICES.md. */
use super::{
    errors,
    source::{Source, line_terminator},
    value::JsString,
};
use crate::{
    constants::JS_STRING_UNITS,
    protocol::{EngineResult, SourceLocation},
};

/** String literals preserve surrogate units until an explicit document boundary. A literal over
JS_STRING_UNITS still scans to its closing quote and returns None, so the script parses and only
code that evaluates the literal fails. */
pub(super) fn string(
    source: &mut Source<'_>,
    quote: char,
    start: SourceLocation,
) -> EngineResult<Option<JsString>> {
    let mut units = Vec::new();
    let mut fits = true;
    while let Some(character) = source.next() {
        if character == quote {
            return if fits {
                JsString::from_units(units).map(Some)
            } else {
                Ok(None)
            };
        }
        if matches!(character, '\n' | '\r') {
            return Err(errors::syntax(start, "unescaped line break in string"));
        }
        if character == '\\' {
            escape(source, &mut units).map_err(|reason| errors::syntax(start, reason))?;
        } else {
            push(character as u32, &mut units);
        }
        if units.len() > JS_STRING_UNITS {
            fits = false;
            units.clear();
        }
    }
    Err(errors::syntax(start, "unterminated string literal"))
}
/** Template chunks end at a backtick (tail) or at `${`; None cooks an overlong or invalid chunk.
The reason for the chunk's first invalid escape comes back with it, worded as V8 words it. */
pub(super) fn template(
    source: &mut Source<'_>,
    start: SourceLocation,
) -> EngineResult<(Option<JsString>, bool, Option<&'static str>)> {
    let mut units = Vec::new();
    let mut cooked = true;
    let mut invalid = None;
    loop {
        let Some(character) = source.next() else {
            return Err(errors::syntax(start, "unterminated template literal"));
        };
        match character {
            '`' => break Ok((cook(units, cooked && invalid.is_none())?, true, invalid)),
            '$' if source.peek() == Some('{') => {
                source.next();
                break Ok((cook(units, cooked && invalid.is_none())?, false, invalid));
            }
            '\\' => {
                let decimal = template_decimal_escape(source.rest());
                let escaped = escape(source, &mut units);
                if invalid.is_none() {
                    invalid = decimal.or(escaped.err());
                }
            }
            '\r' => {
                if source.peek() == Some('\n') {
                    source.next();
                }
                push(10, &mut units);
            }
            other => push(other as u32, &mut units),
        }
        if units.len() > JS_STRING_UNITS {
            cooked = false;
            units.clear();
        }
    }
}
/** V8's reason for a `\8` or `\9` escape in a template chunk. */
const TEMPLATE_DECIMAL_ESCAPE: &str = "\\8 and \\9 are not allowed in template strings.";
/** V8's reason for a legacy octal escape in a template chunk. */
const TEMPLATE_OCTAL_ESCAPE: &str = "Octal escape sequences are not allowed in template strings.";
/** Templates forbid legacy decimal/octal escapes while preserving the standalone null escape;
the reason names which kind V8 saw. */
fn template_decimal_escape(text: &str) -> Option<&'static str> {
    let mut chars = text.chars();
    match chars.next() {
        Some('8' | '9') => Some(TEMPLATE_DECIMAL_ESCAPE),
        Some('1'..='7') => Some(TEMPLATE_OCTAL_ESCAPE),
        Some('0') if chars.next().is_some_and(|c| c.is_ascii_digit()) => {
            Some(TEMPLATE_OCTAL_ESCAPE)
        }
        _ => None,
    }
}
/** Only a fully decoded, bounded chunk becomes a string value. */
fn cook(units: Vec<u16>, cooked: bool) -> EngineResult<Option<JsString>> {
    if cooked {
        JsString::from_units(units).map(Some)
    } else {
        Ok(None)
    }
}
/** V8's reason for a `\x` escape short of two hex digits. */
const INVALID_HEX_ESCAPE: &str = "Invalid hexadecimal escape sequence";
/** V8's reason for a `\u` escape that is not four hex digits or a braced sequence of them. */
const INVALID_UNICODE_ESCAPE: &str = "Invalid Unicode escape sequence";
/** V8's reason for a braced escape past U+10FFFF. */
const UNDEFINED_CODE_POINT: &str = "Undefined Unicode code-point";
/** The highest Unicode code point. */
const MAX_CODE_POINT: u32 = 0x10ffff;
/** Non-escape characters are identity escapes; line continuations contribute no code units.
A failure is the V8 reason, which the caller locates. */
fn escape(source: &mut Source<'_>, units: &mut Vec<u16>) -> Result<(), &'static str> {
    let character = source.next().ok_or("unterminated escape")?;
    if line_terminator(character) {
        if character == '\r' && source.peek() == Some('\n') {
            source.next();
        }
        return Ok(());
    }
    let value = match character {
        'b' => 8,
        'f' => 12,
        'n' => 10,
        'r' => 13,
        't' => 9,
        'v' => 11,
        'x' => hex(source, 2, INVALID_HEX_ESCAPE)?,
        'u' => unicode(source)?,
        '0'..='7' => legacy_octal(source, character),
        other => other as u32,
    };
    push(value, units);
    Ok(())
}
/** Sloppy-mode octal escapes take up to three digits and never exceed \\377. */
fn legacy_octal(source: &mut Source<'_>, first: char) -> u32 {
    let mut value = first as u32 - '0' as u32;
    let digits = if first <= '3' { 2 } else { 1 };
    for _ in 0..digits {
        match source.peek().and_then(|c| c.to_digit(8)) {
            Some(digit) => {
                source.next();
                value = value * 8 + digit;
            }
            None => break,
        }
    }
    value
}
/** Fixed-width escapes require every promised hexadecimal digit; `reason` is the V8 text for
the escape kind that fell short. */
fn hex(source: &mut Source<'_>, count: usize, reason: &'static str) -> Result<u32, &'static str> {
    let mut value = 0;
    for _ in 0..count {
        let digit = source.peek().and_then(|c| c.to_digit(16)).ok_or(reason)?;
        source.next();
        value = value * 16 + digit;
    }
    Ok(value)
}
/** Braced code points use a checked bound, including surrogate code points. A failure is the
V8 reason, which the caller locates. */
pub(super) fn unicode(source: &mut Source<'_>) -> Result<u32, &'static str> {
    if source.peek() != Some('{') {
        return hex(source, 4, INVALID_UNICODE_ESCAPE);
    }
    source.next();
    let mut value = 0_u32;
    let mut digits = 0;
    while source.peek() != Some('}') {
        let digit = source
            .peek()
            .and_then(|c| c.to_digit(16))
            .ok_or(INVALID_UNICODE_ESCAPE)?;
        source.next();
        value = value
            .checked_mul(16)
            .and_then(|n| n.checked_add(digit))
            .filter(|n| *n <= MAX_CODE_POINT)
            .ok_or(UNDEFINED_CODE_POINT)?;
        digits += 1;
    }
    source.next();
    if digits == 0 {
        return Err(INVALID_UNICODE_ESCAPE);
    }
    Ok(value)
}
/** Direct code-unit insertion retains lone surrogates instead of using Rust char conversion. */
fn push(value: u32, units: &mut Vec<u16>) {
    if value <= 0xffff {
        units.push(value as u16);
    } else {
        let value = value - 0x10000;
        units.push(0xd800 + (value >> 10) as u16);
        units.push(0xdc00 + (value & 0x3ff) as u16);
    }
}
