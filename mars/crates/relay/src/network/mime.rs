/** One owned Content-Type parser and charset decoder keeps every fetch path's verdict identical. */
use crate::protocol::{EngineError, EngineResult};
use mars_core::charset;
use mars_core::http::{self, encoding_for_label};

/** The essence a page is decoded as HTML under, the only type whose body may name its charset in markup. */
const HTML_TYPE: &str = "text/html";

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

/** Strict UTF-8 for the UTF-8 label; every other label decodes with its WHATWG encoding, malformed
bytes becoming U+FFFD. An unknown label is refused rather than guessed. */
pub(crate) fn decode(body: Vec<u8>, charset: Option<&str>) -> EngineResult<String> {
    let encoding = encoding_for_label(charset.unwrap_or(http::UTF_8));
    if encoding == Some(http::UTF_8) {
        return strict_utf8(&body);
    }
    let encoding = encoding
        .and_then(charset::for_label)
        .ok_or_else(|| EngineError::unsupported("Unsupported resource encoding."))?;
    Ok(encoding.decode(&body).0.into_owned())
}

/** UTF-8 with its byte order mark dropped; any malformed sequence refuses the body. */
fn strict_utf8(body: &[u8]) -> EngineResult<String> {
    let start = if body.starts_with(UTF8_BOM) {
        UTF8_BOM.len()
    } else {
        0
    };
    std::str::from_utf8(&body[start..])
        .map(str::to_owned)
        .map_err(|_| EngineError::unsupported("The resource is not valid UTF-8."))
}

/** The charset an HTML body is decoded with: a UTF-8 byte order mark, else the transport's declared
charset, else the body's own meta declaration, read before decoding so a legacy page is not misread
as UTF-8. Other types keep the declared charset alone, since only markup names a charset in its bytes. */
pub(crate) fn html_charset(mime: &str, declared: Option<String>, body: &[u8]) -> Option<String> {
    if mime != HTML_TYPE {
        return declared;
    }
    // WHATWG sniffs a UTF-8 byte order mark before any label, transport or meta, so it wins.
    if body.starts_with(UTF8_BOM) {
        return Some(http::UTF_8.to_owned());
    }
    declared.or_else(|| charset::prescan(body).map(str::to_owned))
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

/** HTTP token characters (RFC 9110 tchar); an empty token is invalid. Field names are tokens,
so `x-netflix.request.id` is as valid as `x-request-id`. */
pub(crate) fn token(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

/** HTTP whitespace per WHATWG: space, tab, CR and LF. */
fn http_space(c: char) -> bool {
    [' ', '\t', '\r', '\n'].contains(&c)
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
        let error = decode(b"ok".to_vec(), Some("nope")).unwrap_err();
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

    /** Legacy labels decode through their WHATWG tables, malformed bytes becoming U+FFFD. */
    #[test]
    fn legacy_charsets_decode_with_their_tables() {
        let cases: [(&str, &[u8], &str); 4] = [
            ("shift_jis", b"\x82\xA0", "\u{3042}"),
            ("euc-jp", b"\xA4\xA2", "\u{3042}"),
            ("big5", b"\xA4\x40", "\u{4E00}"),
            ("shift_jis", b"\x81", "\u{FFFD}"),
        ];
        for (charset, body, expected) in cases {
            let value = format!("text/html; charset={charset}");
            assert_eq!(text(&value, body).as_deref(), Ok(expected), "{charset}");
        }
    }

    /** A UTF-8 byte order mark decides the charset over a declared label, as WHATWG sniffs it. */
    #[test]
    fn utf8_bom_overrides_the_declared_charset() {
        let body = b"\xEF\xBB\xBFok";
        assert_eq!(super::html_charset("text/html", Some("windows-1252".into()), body).as_deref(), Some("UTF-8"));
        assert_eq!(decode(body.to_vec(), Some("UTF-8")).unwrap(), "ok");
    }

    /** A body with no declared charset names its meta charset before it is decoded. */
    #[test]
    fn undeclared_html_reads_its_meta_charset() {
        let body = b"<meta charset=big5>\xA4\x40";
        let charset = super::html_charset("text/html", None, body);
        assert_eq!(charset.as_deref(), Some("Big5"));
        let decoded = decode(body.to_vec(), charset.as_deref()).unwrap();
        assert!(decoded.ends_with('\u{4E00}'));
    }

    /** A declared charset is kept, and non-HTML bodies are never prescanned. */
    #[test]
    fn declared_or_non_html_bodies_keep_their_charset() {
        let body = b"<meta charset=big5>";
        assert_eq!(super::html_charset("text/html", Some("UTF-8".into()), body).as_deref(), Some("UTF-8"));
        assert_eq!(super::html_charset("text/css", None, body), None);
    }

    #[test]
    fn invalid_essence_returns_none() {
        for value in ["", "text", "text/", "/html", "te xt/html", "text/html/x;"] {
            assert_eq!(parse(value), None, "{value}");
        }
    }
}
