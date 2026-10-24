/** One owned Content-Type parser and charset decoder keeps every fetch path's verdict identical. */
use crate::protocol::{EngineError, EngineResult};
use mars_core::http::{self, encoding_for_label};

/** First byte of the windows-1252 range that differs from Latin-1. */
const WINDOWS_1252_HIGH: u8 = 0x80;

/** Code points for 0x80 to 0x9F; the five undefined bytes map to themselves. */
const WINDOWS_1252: [char; 32] = [
    '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}', '\u{017D}', '\u{008F}',
    '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
];

/** A UTF-8 byte order mark is not document content. */
const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

/** The parts of a Content-Type value that Relay acts on. */
#[derive(Debug, PartialEq)]
pub(crate) struct ContentType {
    /** Lowercase `type/subtype`, without parameters. */
    pub essence: String,
    /** First valid charset parameter, unvalidated; `decode` decides whether it is supported. */
    pub charset: Option<String>,
}

/** WHATWG MIME parsing (case-insensitive essence, quoted values, first duplicate wins), also tolerating whitespace around `=`. */
pub(crate) fn parse(value: &str) -> Option<ContentType> {
    let value = value.trim_matches(http_space);
    let (essence, mut rest) = value.split_once(';').unwrap_or((value, ""));
    let (kind, subtype) = essence.trim_end_matches(http_space).split_once('/')?;
    if !token(kind) || !token(subtype) {
        return None;
    }
    let mut charset = None;
    while !rest.is_empty() {
        let (name, value, next) = parameter(rest.trim_start_matches(http_space));
        if charset.is_none() && name.eq_ignore_ascii_case("charset") && token(name) {
            charset = value.filter(|value| !value.is_empty());
        }
        rest = next;
    }
    Some(ContentType {
        essence: essence.trim_end_matches(http_space).to_ascii_lowercase(),
        charset,
    })
}

/** Strict decoding to UTF-8; an unknown charset is refused rather than guessed. */
pub(crate) fn decode(body: Vec<u8>, charset: Option<&str>) -> EngineResult<String> {
    let encoding = encoding_for_label(charset.unwrap_or(http::UTF_8));
    if encoding == Some(http::UTF_8) {
        let start = if body.starts_with(UTF8_BOM) {
            UTF8_BOM.len()
        } else {
            0
        };
        return std::str::from_utf8(&body[start..])
            .map(str::to_owned)
            .map_err(|_| EngineError::unsupported("The resource is not valid UTF-8."));
    }
    if encoding == Some(http::WINDOWS_1252) {
        return Ok(body.into_iter().map(windows_1252).collect());
    }
    Err(EngineError::unsupported("Unsupported resource encoding."))
}

/** Splits one `name[=value]` parameter and returns the input after its `;`. */
fn parameter(input: &str) -> (&str, Option<String>, &str) {
    let end = input.find([';', '=']).unwrap_or(input.len());
    let name = input[..end].trim_end_matches(http_space);
    if !input[end..].starts_with('=') {
        return (name, None, input.get(end + 1..).unwrap_or_default());
    }
    let input = input[end + 1..].trim_start_matches(http_space);
    if let Some(quoted) = input.strip_prefix('"') {
        let (value, after) = quoted_string(quoted);
        let next = after.split_once(';').map_or("", |(_, next)| next);
        return (name, Some(value), next);
    }
    let (value, next) = input.split_once(';').unwrap_or((input, ""));
    (
        name,
        Some(value.trim_end_matches(http_space).to_owned()),
        next,
    )
}

/** Collects a quoted-string body with backslash escapes; an unterminated one ends at the input end. */
fn quoted_string(input: &str) -> (String, &str) {
    let mut value = String::new();
    let mut chars = input.char_indices();
    while let Some((index, c)) = chars.next() {
        match c {
            '"' => return (value, &input[index + 1..]),
            '\\' => match chars.next() {
                Some((_, escaped)) => value.push(escaped),
                None => value.push('\\'),
            },
            _ => value.push(c),
        }
    }
    (value, "")
}

/** HTTP token characters; an empty token is invalid. */
fn token(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

/** HTTP whitespace per WHATWG: space, tab, CR and LF. */
fn http_space(c: char) -> bool {
    [' ', '\t', '\r', '\n'].contains(&c)
}

/** Bytes outside 0x80 to 0x9F are identity Latin-1. */
fn windows_1252(byte: u8) -> char {
    byte.checked_sub(WINDOWS_1252_HIGH)
        .and_then(|index| WINDOWS_1252.get(usize::from(index)))
        .copied()
        .unwrap_or(char::from(byte))
}

#[cfg(test)]
mod tests {
    use super::{decode, parse};

    /** Parses a value and decodes a body under its charset. */
    fn text(value: &str, body: &[u8]) -> Result<String, String> {
        let parsed = parse(value).ok_or("invalid")?;
        decode(body.to_vec(), parsed.charset.as_deref()).map_err(|error| error.code)
    }

    #[test]
    fn utf8_spellings_are_accepted() {
        for value in [
            "text/html;charset=UTF-8",
            "TEXT/HTML; Charset=\"utf-8\"",
            "text/html; charset=utf8; foo=bar",
            "text/html ; charset = utf-8",
        ] {
            assert_eq!(parse(value).unwrap().essence, "text/html", "{value}");
            assert_eq!(text(value, "é".as_bytes()).as_deref(), Ok("é"), "{value}");
        }
    }

    #[test]
    fn first_duplicate_parameter_wins() {
        let value = "text/html; charset=\"utf-8\"; charset=shift_jis";
        assert_eq!(parse(value).unwrap().charset.as_deref(), Some("utf-8"));
    }

    #[test]
    fn quoted_values_unescape_backslashes() {
        let value = "text/plain; x=\"a;b\"; charset=\"u\\tf-8\"";
        assert_eq!(parse(value).unwrap().charset.as_deref(), Some("utf-8"));
    }

    #[test]
    fn windows_1252_maps_the_high_range() {
        let value = "text/html; charset=windows-1252";
        assert_eq!(
            text(value, b"\x80\x93\xE9").as_deref(),
            Ok("\u{20AC}\u{201C}é")
        );
        assert_eq!(text(value, b"\x81\x9D").as_deref(), Ok("\u{81}\u{9D}"));
    }

    #[test]
    fn iso_8859_1_and_ascii_use_the_windows_1252_table() {
        for charset in ["iso-8859-1", "Latin1", "us-ascii"] {
            let value = format!("text/css; charset={charset}");
            assert_eq!(text(&value, b"\x80\xE9").as_deref(), Ok("\u{20AC}é"));
        }
    }

    #[test]
    fn utf8_bom_is_stripped() {
        assert_eq!(decode(b"\xEF\xBB\xBFok".to_vec(), None).unwrap(), "ok");
    }

    #[test]
    fn unknown_charsets_are_unsupported() {
        let error = decode(b"ok".to_vec(), Some("shift_jis")).unwrap_err();
        assert_eq!(error.code, "unsupported_feature");
        assert_eq!(error.message, "Unsupported resource encoding.");
    }

    #[test]
    fn invalid_utf8_under_a_utf8_label_is_refused() {
        for charset in [None, Some("utf-8")] {
            let error = decode(b"\xFF".to_vec(), charset).unwrap_err();
            assert_eq!(error.code, "unsupported_feature");
        }
    }

    #[test]
    fn invalid_essence_returns_none() {
        for value in ["", "text", "text/", "/html", "te xt/html", "text/html/x;"] {
            assert_eq!(parse(value), None, "{value}");
        }
    }
}
