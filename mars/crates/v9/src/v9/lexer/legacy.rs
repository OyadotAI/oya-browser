/** Preserve Annex B lexical restrictions without confusing decoded values with raw spelling. */
use super::Kind;

/** Leading-zero decimals and legacy decimal/octal string escapes are forbidden in strict code. */
pub(super) fn literal(text: &str, kind: &Kind) -> bool {
    match kind {
        Kind::Number(_) => {
            text.as_bytes().first() == Some(&b'0')
                && text.as_bytes().get(1).is_some_and(u8::is_ascii_digit)
        }
        Kind::String(_) | Kind::Opaque(_) if text.starts_with(['\'', '"']) => escapes(text),
        _ => false,
    }
}

/** Skip complete escape pairs, so an escaped backslash never makes the next digit an escape. */
fn escapes(text: &str) -> bool {
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\\'
            && let Some(escaped) = chars.next()
            && (matches!(escaped, '1'..='9')
                || escaped == '0' && chars.peek().is_some_and(char::is_ascii_digit))
        {
            return true;
        }
    }
    false
}
