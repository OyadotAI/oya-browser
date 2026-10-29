/** Numeric tokens are consumed completely before validating literal grammar. */
use super::super::value::literal;
use super::*;
/** Literal scanning validates whole numeric tokens rather than accepting a numeric prefix. */
pub(super) fn number(source: &mut Source<'_>) -> EngineResult<Kind> {
    let start = source.location();
    let rest = source.rest();
    let prefixed = ["0x", "0X", "0o", "0O", "0b", "0B"]
        .iter()
        .any(|prefix| rest.starts_with(prefix));
    if prefixed {
        source.next();
        source.next();
        while source
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            source.next();
        }
    } else {
        while source
            .peek()
            .is_some_and(|c| c.is_ascii_digit() || c == '_')
        {
            source.next();
        }
        let integer = &rest[..source.location().byte_offset - start.byte_offset];
        let legacy_octal = integer.len() > 1
            && integer.starts_with('0')
            && integer.bytes().all(|byte| (b'0'..=b'7').contains(&byte));
        if source.peek() == Some('.') && !legacy_octal {
            source.next();
            while source
                .peek()
                .is_some_and(|c| c.is_ascii_digit() || c == '_')
            {
                source.next();
            }
        }
        if !legacy_octal && source.peek().is_some_and(|c| c == 'e' || c == 'E') {
            source.next();
            if source.peek().is_some_and(|c| c == '+' || c == '-') {
                source.next();
            }
            while source
                .peek()
                .is_some_and(|c| c.is_ascii_digit() || c == '_')
            {
                source.next();
            }
        }
        if source.peek() == Some('n') {
            source.next();
        }
    }
    let length = source.location().byte_offset - start.byte_offset;
    let value = &rest[..length];
    if let Some(reason) = separator_error(value.trim_end_matches('n'), prefixed) {
        return Err(errors::syntax(start, reason));
    }
    if value.ends_with('n') && (prefixed || !value.contains(['.', 'e', 'E'])) {
        let digits: String = value[..value.len() - 1]
            .chars()
            .filter(|c| *c != '_')
            .collect();
        literal(&value[..value.len() - 1])
            .map_err(|_| errors::syntax(start, "invalid BigInt literal"))?;
        if source
            .peek()
            .is_some_and(|c| identifier_start(c) || c == '\\' || c.is_ascii_digit())
        {
            return Err(errors::syntax(start, "invalid numeric suffix"));
        }
        return Ok(match crate::v9::vm::bigint::parse_bigint(&digits) {
            Some(big) => Kind::BigInt(big),
            None => Kind::BigIntWide(digits),
        });
    }
    if source
        .peek()
        .is_some_and(|c| identifier_start(c) || c == '\\' || c.is_ascii_digit())
    {
        return Err(errors::syntax(start, "invalid numeric suffix"));
    }
    if !prefixed
        && value.len() > 1
        && value.starts_with('0')
        && value.as_bytes()[1].is_ascii_digit()
    {
        return legacy(value, start).map(Kind::Number);
    }
    Ok(Kind::Number(literal(value).map_err(|_| {
        errors::syntax(start, "invalid numeric literal")
    })?))
}
/** V8's reason for `1__0`. */
const CONTINUOUS_SEPARATOR: &str = "Only one underscore is allowed as numeric separator";
/** V8's reason for `1_` and `1_.5`. */
const TRAILING_SEPARATOR: &str =
    "Numeric separators are not allowed at the end of numeric literals";
/** V8's reason for `0_1`. */
const LEADING_ZERO_SEPARATOR: &str = "Numeric separator can not be used after leading 0.";
/** The separator misuse V8 names in a scanned literal, or None when the literal has no
separator fault V8 names (its other faults fall to the literal checks below). A leading-zero
legacy literal is left to its own check, as V8 reads it without separators. */
fn separator_error(digits: &str, prefixed: bool) -> Option<&'static str> {
    let bytes = digits.as_bytes();
    let legacy =
        !prefixed && bytes.first() == Some(&b'0') && bytes.get(1).is_some_and(u8::is_ascii_digit);
    if legacy || !digits.contains('_') {
        return None;
    }
    if !prefixed && digits.starts_with("0_") {
        return Some(LEADING_ZERO_SEPARATOR);
    }
    if digits.contains("__") {
        return Some(CONTINUOUS_SEPARATOR);
    }
    let radix = if prefixed { 16 } else { 10 };
    let trailing = digits.char_indices().any(|(index, c)| {
        c == '_'
            && digits[..index]
                .chars()
                .last()
                .is_some_and(|before| before.is_digit(radix))
            && !digits[index + 1..].starts_with(|after: char| after.is_digit(radix))
    });
    trailing.then_some(TRAILING_SEPARATOR)
}
/** Sloppy-mode `010` is octal; a leading-zero literal with an 8 or 9 is decimal instead. */
fn legacy(value: &str, start: crate::protocol::SourceLocation) -> EngineResult<f64> {
    let integer_end = value.find(['.', 'e', 'E']).unwrap_or(value.len());
    let integer = &value[..integer_end];
    if integer.contains('_') {
        return Err(errors::syntax(start, "invalid legacy numeric literal"));
    }
    if integer.bytes().all(|byte| (b'0'..=b'7').contains(&byte)) {
        if integer_end != value.len() {
            return Err(errors::syntax(start, "invalid legacy numeric literal"));
        }
        Ok(integer
            .bytes()
            .fold(0.0, |total, byte| total * 8.0 + f64::from(byte - b'0')))
    } else {
        // Annex B's decimal form permits separators in its fraction and exponent, never in its leading-zero integer.
        literal(value.trim_start_matches('0'))
    }
}
