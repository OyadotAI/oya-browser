/**
 * Byte-level helpers for XML: white space, name characters, character and entity references,
 * and the character validity rule. The scanner only moves across ASCII bytes and whole names, so
 * every position it keeps is a character boundary.
 */

/// The longest reference name an error message quotes, so a hostile name cannot grow it.
pub const MESSAGE_CHARS: usize = 64;

/** XML white space: space, tab, carriage return and line feed. */
pub fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}

/** A byte that may start a name: a letter, underscore, colon, or any non-ASCII byte, which
approximates the Unicode name ranges. */
pub fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || matches!(byte, b'_' | b':') || byte >= 0x80
}

/** A byte that may continue a name. */
pub fn is_name_char(byte: u8) -> bool {
    is_name_start(byte) || byte.is_ascii_digit() || matches!(byte, b'-' | b'.')
}

/** Whether a character is an XML `Char`: the only ones a document may contain. */
fn is_char(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
    )
}

/** The character a numeric reference (`#65` or `#x41`) names, or None when it is malformed or not
an XML character. */
fn numeric(name: &str) -> Option<char> {
    let digits = name.strip_prefix('#')?;
    let (radix, body) = match digits.strip_prefix('x') {
        Some(hex) => (16, hex),
        None => (10, digits),
    };
    if body.is_empty() || !body.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    u32::from_str_radix(body, radix)
        .ok()
        .and_then(char::from_u32)
        .filter(|c| is_char(*c))
}

/** The character a reference name stands for: the five predefined entities or a numeric
reference. Any other name is undefined, because a DTD is not read. */
pub fn reference_char(name: &str) -> Option<char> {
    match name {
        "lt" => Some('<'),
        "gt" => Some('>'),
        "amp" => Some('&'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => numeric(name),
    }
}

/** A reference name as a message may quote it. */
pub fn short(name: &str) -> String {
    name.chars().take(MESSAGE_CHARS).collect()
}

/** Replace each reference in `raw` with its character. Scanning goes left to right and every
name ends at the next `;`, so the work is linear in the text's length. */
pub fn decode(raw: &str) -> Result<String, String> {
    if !raw.contains('&') {
        return Ok(raw.to_string());
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let end = after
            .find(';')
            .ok_or_else(|| "a reference is missing its ';'".to_string())?;
        let name = &after[..end];
        let c = reference_char(name)
            .ok_or_else(|| format!("undefined entity &{};", short(name)))?;
        out.push(c);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/** Normalize line endings: CRLF and a lone CR both become LF, as the XML end-of-line rule says.
Borrows when there is no CR, which is the common case. */
pub fn normalize_newlines(source: &str) -> std::borrow::Cow<'_, str> {
    if source.contains('\r') {
        std::borrow::Cow::Owned(source.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        std::borrow::Cow::Borrowed(source)
    }
}
