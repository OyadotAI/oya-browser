/** Preserve Annex B lexical restrictions without confusing decoded values with raw spelling. */
use super::Kind;

/** V8's strict-mode message for a legacy octal literal such as `010`. */
pub(super) const OCTAL_LITERAL: &str = "Octal literals are not allowed in strict mode.";
/** V8's strict-mode message for a leading-zero decimal such as `08`. */
pub(super) const LEADING_ZERO_DECIMAL: &str =
    "Decimals with leading zeros are not allowed in strict mode.";
/** V8's strict-mode message for the `\8` and `\9` string escapes. */
pub(super) const DECIMAL_ESCAPES: &str = "\\8 and \\9 are not allowed in strict mode.";
/** V8's strict-mode message for an octal string escape such as `\1` or `\12`. */
pub(super) const OCTAL_ESCAPE: &str = "Octal escape sequences are not allowed in strict mode.";

/** A legacy spelling strict code refuses, kept in a byte on every token instead of a message
pointer twice its size. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::v9) enum Legacy {
    OctalLiteral,
    LeadingZeroDecimal,
    DecimalEscapes,
    OctalEscape,
}

impl Legacy {
    /** V8's strict-mode message for the spelling. */
    pub fn message(self) -> &'static str {
        match self {
            Self::OctalLiteral => OCTAL_LITERAL,
            Self::LeadingZeroDecimal => LEADING_ZERO_DECIMAL,
            Self::DecimalEscapes => DECIMAL_ESCAPES,
            Self::OctalEscape => OCTAL_ESCAPE,
        }
    }
}

/** The legacy spelling of a literal or escape, or None when the token is strict-safe. */
pub(super) fn literal(text: &str, kind: &Kind) -> Option<Legacy> {
    match kind {
        Kind::Number(_) => number(text),
        Kind::String(_) | Kind::Opaque(_) if text.starts_with(['\'', '"']) => escapes(text),
        _ => None,
    }
}

/** Leading-zero numbers: all-octal digits are an octal literal, anything else a decimal. */
fn number(text: &str) -> Option<Legacy> {
    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'0') || !bytes.get(1).is_some_and(u8::is_ascii_digit) {
        return None;
    }
    let octal = bytes.iter().all(|byte| (b'0'..=b'7').contains(byte));
    Some(if octal {
        Legacy::OctalLiteral
    } else {
        Legacy::LeadingZeroDecimal
    })
}

/** Skip complete escape pairs, so an escaped backslash never makes the next digit an escape. */
fn escapes(text: &str) -> Option<Legacy> {
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\\'
            && let Some(escaped) = chars.next()
            && let Some(message) = escape_message(escaped, chars.peek().copied())
        {
            return Some(message);
        }
    }
    None
}

/** The message for one escape: `\8` or `\9`, an octal digit, or `\0` followed by a digit. */
fn escape_message(escaped: char, next: Option<char>) -> Option<Legacy> {
    match escaped {
        '8' | '9' => Some(Legacy::DecimalEscapes),
        '1'..='7' => Some(Legacy::OctalEscape),
        '0' if next.is_some_and(|digit| digit.is_ascii_digit()) => Some(Legacy::OctalEscape),
        _ => None,
    }
}
