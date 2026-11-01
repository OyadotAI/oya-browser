/**
 * Character reference decoding with the WHATWG rules, shared by every engine that reads HTML or
 * XML text: the document parser, innerHTML in V9 and RCDATA text. One decoder so a reference means
 * the same thing wherever a page writes it. Decoding grows text by at most one byte per reference
 * (`&nGt;` is two code points, six bytes, for five source bytes).
 */
mod table;

use table::{MAX_NAME, NAMED};

/** Bytes of `&` that open every reference. */
const AMPERSAND: usize = "&".len();
/** Bytes of `&#` that open a numeric reference. */
const NUMERIC_PREFIX: usize = "&#".len();
/** Bytes of the `;` that completes a name. */
const SEMICOLON: usize = ";".len();
/** Radix of decimal numeric references. */
const DECIMAL: u32 = 10;
/** Radix of hexadecimal numeric references. */
const HEXADECIMAL: u32 = 16;
/** First code point of the C1 range that numeric references map through Windows-1252. */
const C1_START: u32 = 0x80;
/** Last code point of the C1 range that numeric references map through Windows-1252. */
const C1_END: u32 = 0x9f;
/** Windows-1252 meanings of the C1 range; the five unmapped slots keep their code point. */
const C1: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '\u{2014}', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž',
    'Ÿ',
];

/**
 * Decodes the character references in `source`. In an attribute value (`attribute`) a legacy
 * name without its semicolon stays literal when `=` or an ASCII alphanumeric follows it, so
 * query strings such as `?a=1&copy=2` survive. Text decodes every legacy name.
 */
pub fn decode(source: &str, attribute: bool) -> String {
    let mut output = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(start) = rest.find('&') {
        output.push_str(&rest[..start]);
        rest = &rest[start..];
        match reference(rest, attribute) {
            Some((value, used)) => {
                output.push_str(&value);
                rest = &rest[used..];
            }
            None => {
                output.push('&');
                rest = &rest[1..];
            }
        }
    }
    output.push_str(rest);
    output
}

/**
 * One reference at the start of `rest` (which begins with `&`): its replacement and the source
 * bytes it consumed, or nothing when the `&` is literal text.
 */
fn reference(rest: &str, attribute: bool) -> Option<(String, usize)> {
    let body = &rest[1..];
    match body.strip_prefix('#') {
        Some(number) => {
            numeric(number).map(|(value, used)| (value.to_string(), used + NUMERIC_PREFIX))
        }
        None => named(body, attribute).map(|(value, used)| (value.to_string(), used + AMPERSAND)),
    }
}

/**
 * The named reference at the start of `body`, after its `&`: the longest name the table knows,
 * or nothing when an attribute value continues the legacy name it matched.
 */
fn named(body: &str, attribute: bool) -> Option<(&'static str, usize)> {
    let (value, used) = longest_name(body)?;
    let ends_with_semicolon = body[..used].ends_with(';');
    let next = body[used..].chars().next();
    let continues = next.is_some_and(|c| c == '=' || c.is_ascii_alphanumeric());
    (!(attribute && !ends_with_semicolon && continues)).then_some((value, used))
}

/**
 * The longest table name at the start of `body`. A name with its semicolon wins whenever the
 * alphanumeric run is followed by `;`, because it is longer than any legacy prefix of the run.
 */
fn longest_name(body: &str) -> Option<(&'static str, usize)> {
    let run = body.bytes().take_while(u8::is_ascii_alphanumeric).count();
    if body[run..].starts_with(';')
        && let Some(value) = lookup(&body[..=run])
    {
        return Some((value, run + SEMICOLON));
    }
    (1..=run.min(MAX_NAME))
        .rev()
        .find_map(|len| lookup(&body[..len]).map(|value| (value, len)))
}

/** Exact table lookup; the table is sorted by byte order for binary search. */
fn lookup(name: &str) -> Option<&'static str> {
    NAMED
        .binary_search_by(|(key, _)| key.cmp(&name))
        .ok()
        .map(|index| NAMED[index].1)
}

/**
 * Decimal or hexadecimal numeric reference after `&#`: the scalar and the bytes consumed after
 * `&#`, including the `x` and the optional semicolon. Digits are required; without them the
 * whole reference is literal text.
 */
fn numeric(number: &str) -> Option<(char, usize)> {
    let (radix, prefix) = match number.as_bytes().first() {
        Some(b'x' | b'X') => (HEXADECIMAL, 1),
        _ => (DECIMAL, 0),
    };
    let digits = &number[prefix..];
    let end = digits
        .find(|c: char| !c.is_digit(radix))
        .unwrap_or(digits.len());
    if end == 0 {
        return None;
    }
    let value = u32::from_str_radix(&digits[..end], radix).unwrap_or(u32::MAX);
    let semicolon = usize::from(digits[end..].starts_with(';'));
    Some((scalar(value), prefix + end + semicolon))
}

/**
 * The character a numeric reference names. Zero, surrogates and values past U+10FFFF become
 * U+FFFD; the C1 range maps through Windows-1252; everything else, noncharacters included, is kept.
 */
fn scalar(value: u32) -> char {
    match value {
        C1_START..=C1_END => C1[(value - C1_START) as usize],
        0 => char::REPLACEMENT_CHARACTER,
        _ => char::from_u32(value).unwrap_or(char::REPLACEMENT_CHARACTER),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Binary search needs the table in byte order with no duplicates. */
    #[test]
    fn named_table_is_sorted_and_bounded() {
        assert!(NAMED.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert!(NAMED.iter().all(|(name, _)| name.len() <= MAX_NAME));
    }

    /** Named references decode with their semicolon, and the longest match wins. */
    #[test]
    fn named_references_decode_with_semicolon() {
        assert_eq!(decode("&eacute;&hellip;&nbsp;&notin;", false), "é…\u{a0}∉");
        assert_eq!(decode("&amp;amp;", false), "&amp;");
    }

    /** Without a semicolon a legacy name decodes in text, the longest legacy prefix first. */
    #[test]
    fn legacy_names_decode_in_text() {
        assert_eq!(decode("&copy 2026", false), "© 2026");
        assert_eq!(decode("&notit;", false), "¬it;");
    }

    /** In an attribute, a legacy name followed by `=` or a letter stays literal. */
    #[test]
    fn legacy_names_stay_literal_in_attributes() {
        assert_eq!(decode("&copy=2", true), "&copy=2");
        assert_eq!(decode("&copyx", true), "&copyx");
        assert_eq!(decode("&copy 2", true), "© 2");
        assert_eq!(decode("&copy;", true), "©");
    }

    /** `&copyright;` is the name `copyright` is not, so the longest legacy prefix `copy` decodes. */
    #[test]
    fn longest_legacy_prefix_decodes_before_extra_letters() {
        assert_eq!(decode("&copyright;", false), "©right;");
    }

    /** Names that the table lacks, or that have no name at all, stay literal. */
    #[test]
    fn unknown_references_stay_literal() {
        assert_eq!(decode("&bogus; & &#; &#x;", false), "&bogus; & &#; &#x;");
    }

    /** Numeric references map C1 code points through Windows-1252 and reject bad scalars. */
    #[test]
    fn numeric_references_follow_the_spec() {
        assert_eq!(decode("&#x80;", false), "€");
        assert_eq!(decode("&#xD800;", false), "\u{fffd}");
        assert_eq!(
            decode("&#0;&#x110000;&#x81;", false),
            "\u{fffd}\u{fffd}\u{81}"
        );
        assert_eq!(decode("&#xFDD0;&#65", false), "\u{fdd0}A");
    }

    /** Decoding grows text by at most one byte per reference (`&nGt;` is six bytes). */
    #[test]
    fn decoding_grows_by_at_most_one_byte_per_reference() {
        let source = "&nGt;&eacute;&#x1F600;";
        assert!(decode(source, false).len() <= source.len() + 2);
        assert_eq!(decode("&nGt;", false), "\u{226b}\u{20d2}");
    }
}
