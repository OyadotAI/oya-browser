/** JSON syntax diagnostics retain the failed grammar production and count original UTF16 source coordinates. */
use super::*;

/** V8 includes ten source units on either side of an unexpected token in a longer input. */
const CONTEXT_UNITS: usize = 10;
/** A shorter input fits in the complete-source diagnostic instead. */
const CONTEXT_MINIMUM: usize = CONTEXT_UNITS * 2 + 1;

/** Each grammar failure identifies its own diagnostic before the reader unwinds. */
#[derive(Clone, Copy)]
pub(super) enum Reason {
    Unexpected,
    UnterminatedString,
    ControlCharacter,
    EscapedCharacter,
    UnicodeEscape,
    PropertyNameOrBrace,
    PropertyName,
    Colon,
    ArraySeparator,
    ObjectSeparator,
    NumberAfterMinus,
    Fraction,
    Exponent,
    UnexpectedNumber,
    TrailingCharacter,
}

/** Formatting only reads parser-owned units and never repeats the input's authored ToString operation. */
pub(super) fn error(
    vm: &mut Vm,
    units: Input<'_>,
    position: usize,
    reason: Reason,
) -> crate::v9::vm::Thrown {
    let message = match reason {
        Reason::Unexpected => unexpected(units, position),
        _ => located(units, position, reason),
    };
    vm.syntax_error(&message)
}

/** Specific grammar failures carry their original source position even at end of input. */
fn located(units: Input<'_>, position: usize, reason: Reason) -> String {
    let description = match reason {
        Reason::UnterminatedString => "Unterminated string in JSON",
        Reason::ControlCharacter => "Bad control character in string literal in JSON",
        Reason::EscapedCharacter => "Bad escaped character in JSON",
        Reason::UnicodeEscape => "Bad Unicode escape in JSON",
        Reason::PropertyNameOrBrace => "Expected property name or '}' in JSON",
        Reason::PropertyName => "Expected double-quoted property name in JSON",
        Reason::Colon => "Expected ':' after property name in JSON",
        Reason::ArraySeparator => "Expected ',' or ']' after array element in JSON",
        Reason::ObjectSeparator => "Expected ',' or '}' after property value in JSON",
        Reason::NumberAfterMinus => "No number after minus sign in JSON",
        Reason::Fraction => "Unterminated fractional number in JSON",
        Reason::Exponent => "Exponent part is missing a number in JSON",
        Reason::UnexpectedNumber => "Unexpected number in JSON",
        Reason::TrailingCharacter => "Unexpected non-whitespace character after JSON",
        Reason::Unexpected => unreachable!("unexpected token uses its source context"),
    };
    let (line, column) = coordinates(units, position);
    format!("{description} at position {position} (line {line} column {column})")
}

/** Only CR and LF delimit JSON diagnostic lines; a CRLF pair contributes one break. */
fn coordinates(units: Input<'_>, position: usize) -> (usize, usize) {
    let end = position.min(units.len());
    let (mut cursor, mut line, mut line_start) = (0, 1, 0);
    while cursor < end {
        if matches!(units.get(cursor), Some(0x0a | 0x0d)) {
            if units.get(cursor) == Some(0x0d)
                && cursor + 1 < end
                && units.get(cursor + 1) == Some(0x0a)
            {
                cursor += 1;
            }
            line += 1;
            line_start = cursor + 1;
        }
        cursor += 1;
    }
    (line, position - line_start + 1)
}

/** Generic token failures include bounded original text; special common coercion results avoid repeating the token. */
fn unexpected(units: Input<'_>, position: usize) -> String {
    let Some(token) = units.get(position) else {
        return "Unexpected end of JSON input".into();
    };
    let token = String::from_utf16_lossy(&[token]);
    if ["undefined", "NaN", "Infinity", "[object Object]"]
        .iter()
        .any(|text| {
            (0..units.len())
                .filter_map(|index| units.get(index))
                .eq(text.encode_utf16())
        })
    {
        return format!(
            "\"{}\" is not valid JSON",
            String::from_utf16_lossy(&units.range(0, units.len()))
        );
    }
    let (start, end) = context_range(units.len(), position);
    let prefix = if start > 0 { "..." } else { "" };
    let suffix = if end < units.len() { "..." } else { "" };
    let context = String::from_utf16_lossy(&units.range(start, end));
    format!("Unexpected token '{token}', {prefix}\"{context}\"{suffix} is not valid JSON")
}

/** Source context follows the actual error position, independently of line boundaries or byte width. */
fn context_range(length: usize, position: usize) -> (usize, usize) {
    if length < CONTEXT_MINIMUM {
        (0, length)
    } else if position < CONTEXT_UNITS {
        (0, position + CONTEXT_UNITS)
    } else if position < length - CONTEXT_UNITS {
        (position - CONTEXT_UNITS, position + CONTEXT_UNITS)
    } else {
        (position - CONTEXT_UNITS, length)
    }
}
