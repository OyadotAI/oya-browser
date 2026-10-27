/** Charset labels and the meta prescan, after the WHATWG Encoding standard. Every label resolves through encoding_rs, and a page's own meta declaration is read from its first bytes before anything is decoded, so the page is read the way its markup says. Relay decodes with it and Eye parses the result. */
pub use encoding_rs::Encoding;

use crate::http::UTF_8;
use encoding_rs::{Encoder, EncoderResult, UTF_16BE, UTF_16LE};

/** Bytes the prescan reads for a meta declaration: the HTML standard's prescan bound. */
pub const PRESCAN_BYTES: usize = 1024;
/** The start of a meta tag, lowercase because the prescan works on a lowercased copy. */
const META_TAG: &[u8] = b"<meta";
/** ASCII whitespace the HTML standard skips between attributes and around `=`. */
const SPACE: &[u8] = b" \t\r\n\x0c";
/** Bytes that end an attribute name. */
const NAME_END: &[u8] = b"=/> \t\r\n\x0c";
/** Bytes that end an unquoted attribute value. */
const VALUE_END: &[u8] = b"> \t\r\n\x0c";
/** The attribute that names a charset directly. */
const CHARSET_ATTR: &[u8] = b"charset";
/** The attribute that makes a meta an http-equiv pragma. */
const HTTP_EQUIV: &[u8] = b"http-equiv";
/** The pragma value whose content may name a charset. */
const CONTENT_TYPE: &[u8] = b"content-type";
/** The attribute that carries a pragma's content. */
const CONTENT_ATTR: &[u8] = b"content";
/** The keyword that introduces the label inside a content type. */
const CHARSET_KEYWORD: &str = "charset";

/// The encoding a label names, ASCII case and surrounding whitespace ignored. None only for a label
/// WHATWG does not define.
pub fn for_label(label: &str) -> Option<&'static Encoding> {
    Encoding::for_label(label.as_bytes())
}

/// The canonical name a meta declaration settles to. WHATWG reads a meta that names UTF-16 as
/// UTF-8, since the markup around it is ASCII, so only the transport can select UTF-16.
pub fn meta_name(label: &str) -> Option<&'static str> {
    let encoding = for_label(label)?;
    if encoding == UTF_16LE || encoding == UTF_16BE {
        Some(UTF_8)
    } else {
        Some(encoding.name())
    }
}

/// The canonical charset the first meta declaration in `bytes` names, read within the prescan
/// bound. None when no meta names a charset, or the first one names an unknown label, in which
/// case the page keeps UTF-8 decoding.
pub fn prescan(bytes: &[u8]) -> Option<&'static str> {
    let head = bytes[..bytes.len().min(PRESCAN_BYTES)].to_ascii_lowercase();
    let mut rest = head.as_slice();
    while let Some(at) = find(rest, META_TAG) {
        let after = &rest[at + META_TAG.len()..];
        if after
            .first()
            .is_some_and(|b| SPACE.contains(b) || *b == b'/')
        {
            return tag_label(&attributes(after)).and_then(meta_name);
        }
        rest = after;
    }
    None
}

/** The first occurrence of `needle` in `haystack`. */
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/** The label a meta tag declares: its charset attribute, else the charset in an http-equiv content type. */
fn tag_label<'a>(attrs: &[(&'a [u8], &'a [u8])]) -> Option<&'a str> {
    if let Some(value) = value_of(attrs, CHARSET_ATTR) {
        return std::str::from_utf8(value).ok();
    }
    if value_of(attrs, HTTP_EQUIV)? != CONTENT_TYPE {
        return None;
    }
    let content = std::str::from_utf8(value_of(attrs, CONTENT_ATTR)?).ok()?;
    content_charset(content)
}

/** The value of the first attribute called `name`. */
fn value_of<'a>(attrs: &[(&'a [u8], &'a [u8])], name: &[u8]) -> Option<&'a [u8]> {
    attrs
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value)
}

/** The label after `charset=` in a content attribute, bare or quoted. */
fn content_charset(content: &str) -> Option<&str> {
    let at = content.find(CHARSET_KEYWORD)? + CHARSET_KEYWORD.len();
    let value = content[at..].trim_start().strip_prefix('=')?.trim_start();
    match value.chars().next()? {
        quote @ ('"' | '\'') => value[1..].find(quote).map(|end| &value[1..][..end]),
        _ => value
            .split(|c: char| c == ';' || c.is_ascii_whitespace())
            .next()
            .filter(|label| !label.is_empty()),
    }
}

/** The name and value of each attribute in the bytes after a meta tag's name, up to its `>`. */
fn attributes(mut rest: &[u8]) -> Vec<(&[u8], &[u8])> {
    let mut out = Vec::new();
    loop {
        rest = skip_space_and_slashes(rest);
        if rest.first().is_none_or(|b| *b == b'>') {
            return out;
        }
        let (name, after) = attribute_name(rest);
        let (value, after) = attribute_value(after);
        out.push((name, value));
        rest = after;
    }
}

/** Skips whitespace and the `/` that separates attributes. */
fn skip_space_and_slashes(bytes: &[u8]) -> &[u8] {
    let skipped = bytes
        .iter()
        .take_while(|b| SPACE.contains(b) || **b == b'/')
        .count();
    &bytes[skipped..]
}

/** Skips ASCII whitespace. */
fn skip_space(bytes: &[u8]) -> &[u8] {
    &bytes[bytes.iter().take_while(|b| SPACE.contains(b)).count()..]
}

/** One attribute name and the bytes after it. The name is at least one byte, even when it starts with `=`. */
fn attribute_name(rest: &[u8]) -> (&[u8], &[u8]) {
    let end = 1 + rest[1..]
        .iter()
        .position(|b| NAME_END.contains(b))
        .unwrap_or(rest.len() - 1);
    (&rest[..end], &rest[end..])
}

/** An optional `= value` after an attribute name; a missing value is empty. */
fn attribute_value(rest: &[u8]) -> (&[u8], &[u8]) {
    let rest = skip_space(rest);
    let Some(after_equals) = rest.strip_prefix(b"=") else {
        return (&[][..], rest);
    };
    let rest = skip_space(after_equals);
    match rest.first() {
        Some(&quote @ (b'"' | b'\'')) => quoted_value(&rest[1..], quote),
        _ => bare_value(rest),
    }
}

/** A quoted value up to its closing quote; an unterminated one runs to the end of the tag. */
fn quoted_value(body: &[u8], quote: u8) -> (&[u8], &[u8]) {
    let end = body.iter().position(|&b| b == quote).unwrap_or(body.len());
    (&body[..end], &body[(end + 1).min(body.len())..])
}

/** An unquoted value up to whitespace or the tag's `>`. */
fn bare_value(rest: &[u8]) -> (&[u8], &[u8]) {
    let end = rest
        .iter()
        .position(|b| VALUE_END.contains(b))
        .unwrap_or(rest.len());
    (&rest[..end], &rest[end..])
}

/** Bytes one encoder call may write for a character: an ISO-2022-JP escape and its two bytes fit well inside. */
const CHAR_BUF_BYTES: usize = 32;
/** Bytes a URL's percent-encoded `&#` opens an unmappable character's reference with. */
const NCR_OPEN: &str = "%26%23";
/** Bytes that close a percent-encoded reference: the escaped `;`. */
const NCR_CLOSE: &str = "%3B";
/** Bytes of one UTF-8 character at most. */
const UTF8_CHAR_BYTES: usize = 4;

/// Percent-encodes the non-ASCII characters of `text` as bytes of `encoding`, the way a link written in
/// a page's own charset carries its query. ASCII passes through for the URL parser to escape; a
/// character the encoding cannot represent becomes its decimal reference, escaped, as `%26%23N%3B`.
/// One encoder runs over the whole text, so stateful encodings keep their state across characters.
pub fn encode_query_text(encoding: &'static Encoding, text: &str) -> String {
    let mut encoder = encoding.new_encoder();
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            push_char(&mut encoder, c, &mut out);
        }
    }
    let mut buf = [0u8; CHAR_BUF_BYTES];
    let (_, _, written) = encoder.encode_from_utf8_without_replacement("", &mut buf, true);
    push_bytes(&buf[..written], &mut out);
    out
}

/** First printable byte the query set leaves alone: space is escaped, `!` is not. */
const QUERY_FIRST_PRINTABLE: u8 = 0x21;
/** Last printable ASCII byte; DEL and above are escaped. */
const QUERY_LAST_PRINTABLE: u8 = 0x7E;

/// Encodes one non-ASCII character with the shared encoder: its bytes, or its escaped reference when
/// the encoding has no mapping for it. The encoder may write bytes before reporting the character
/// unmappable (ISO-2022-JP returns to ASCII first), so those bytes are kept either way.
fn push_char(encoder: &mut Encoder, c: char, out: &mut String) {
    let mut utf8 = [0u8; UTF8_CHAR_BYTES];
    let mut rest: &str = c.encode_utf8(&mut utf8);
    let mut buf = [0u8; CHAR_BUF_BYTES];
    loop {
        let (result, read, written) =
            encoder.encode_from_utf8_without_replacement(rest, &mut buf, false);
        push_bytes(&buf[..written], out);
        rest = &rest[read..];
        match result {
            EncoderResult::OutputFull => continue,
            EncoderResult::Unmappable(_) => {
                out.push_str(&format!("{NCR_OPEN}{}{NCR_CLOSE}", c as u32));
                return;
            }
            EncoderResult::InputEmpty => return,
        }
    }
}

/** Appends bytes to a query as percent escapes, each one uppercase hex. */
fn push_bytes(bytes: &[u8], out: &mut String) {
    for &byte in bytes {
        if in_query_set(byte) {
            out.push_str(&format!("%{byte:02X}"));
        } else {
            out.push(char::from(byte));
        }
    }
}

/** Bytes the WHATWG query percent-encode set escapes: controls, space and the characters `"`, `#`,
`<` and `>`. Everything else, such as `@`, is written as it is. */
fn in_query_set(byte: u8) -> bool {
    byte < QUERY_FIRST_PRINTABLE
        || byte > QUERY_LAST_PRINTABLE
        || matches!(byte, b'"' | b'#' | b'<' | b'>')
}

/// The URL with the characters of its query re-encoded in `encoding` (see `encode_query_text`). Only
/// the query is touched: the part before it and the fragment after `#` stay as written.
pub fn encode_url_query(url: &str, encoding: &'static Encoding) -> String {
    let hash = url.find('#').unwrap_or(url.len());
    let Some(start) = url[..hash].find('?') else {
        return url.to_string();
    };
    format!(
        "{}{}{}",
        &url[..start],
        encode_query_text(encoding, &url[start..hash]),
        &url[hash..]
    )
}

#[cfg(test)]
mod tests {
    use super::{encode_query_text, encode_url_query, for_label, meta_name, prescan};

    /** Labels resolve to their WHATWG encoding; x-user-defined and unknown labels do not. */
    #[test]
    fn labels_resolve_to_whatwg_names() {
        assert_eq!(for_label("sjis").map(|e| e.name()), Some("Shift_JIS"));
        assert_eq!(
            for_label("cseucpkdfmtjapanese").map(|e| e.name()),
            Some("EUC-JP")
        );
        assert_eq!(
            for_label(" Latin1 ").map(|e| e.name()),
            Some("windows-1252")
        );
        assert_eq!(
            for_label("x-user-defined").map(|e| e.name()),
            Some("x-user-defined")
        );
        assert!(for_label("nope").is_none());
    }

    /** A meta that names UTF-16 reads as UTF-8, while the transport keeps UTF-16. */
    #[test]
    fn meta_utf16_reads_as_utf8() {
        assert_eq!(meta_name("utf-16le"), Some("UTF-8"));
        assert_eq!(meta_name("unicodefeff"), Some("UTF-8"));
        assert_eq!(meta_name("big5"), Some("Big5"));
    }

    /** A charset attribute is found in a plain meta, with or without quotes. */
    #[test]
    fn prescan_reads_a_charset_attribute() {
        assert_eq!(prescan(b"<head><meta charset=\"big5\">"), Some("Big5"));
        assert_eq!(prescan(b"<meta charset=shift_jis>"), Some("Shift_JIS"));
        assert_eq!(prescan(b"<meta charset='gbk'/>"), Some("GBK"));
    }

    /** An http-equiv content type names its charset when no charset attribute does. */
    #[test]
    fn prescan_reads_an_http_equiv_content_type() {
        let page = b"<meta http-equiv='Content-Type' content='text/html; charset=EUC-JP'>";
        assert_eq!(prescan(page), Some("EUC-JP"));
    }

    /** The charset attribute wins over an http-equiv content type in the same tag. */
    #[test]
    fn prescan_prefers_the_charset_attribute() {
        let page = b"<meta http-equiv=content-type content='text/html; charset=big5' charset=gbk>";
        assert_eq!(prescan(page), Some("GBK"));
    }

    /** Tags that name no charset, and tags that only look like meta, do not decide the encoding. */
    #[test]
    fn prescan_skips_unlabelled_and_look_alike_tags() {
        assert_eq!(prescan(b"<meta name=viewport content='x'>"), None);
        assert_eq!(prescan(b"<metadata charset=big5>"), None);
    }

    /** An unknown label settles nothing: the page keeps UTF-8 decoding. */
    #[test]
    fn prescan_unknown_label_is_none() {
        assert_eq!(prescan(b"<meta charset=nope>"), None);
    }

    /** A meta past the prescan bound is not read. */
    #[test]
    fn prescan_stops_at_the_bound() {
        let mut page = vec![b' '; super::PRESCAN_BYTES];
        page.extend_from_slice(b"<meta charset=big5>");
        assert_eq!(prescan(&page), None);
    }

    /** A character a legacy encoding maps is written as its bytes, escaped; ASCII stays as it is. */
    #[test]
    fn query_text_writes_mapped_characters_in_the_encoding() {
        let big5 = for_label("big5").expect("big5");
        assert_eq!(encode_query_text(big5, "ab"), "ab");
        assert_eq!(encode_query_text(big5, "\u{3000}"), "%A1@");
        assert_eq!(encode_query_text(big5, "\u{20AC}"), "%A3%E1");
    }

    /** An unmappable character becomes its escaped decimal reference, one per character. */
    #[test]
    fn query_text_writes_unmappable_characters_as_references() {
        let big5 = for_label("big5").expect("big5");
        assert_eq!(encode_query_text(big5, "\u{9EA6}"), "%26%2340614%3B");
        assert_eq!(encode_query_text(big5, "\u{2603}"), "%26%239731%3B");
        assert_eq!(encode_query_text(big5, "\u{1F4A9}"), "%26%23128169%3B");
    }

    /** Only the query is re-encoded: the path before it and the fragment after it stay as written. */
    #[test]
    fn url_query_is_re_encoded_and_the_rest_is_kept() {
        let big5 = for_label("big5").expect("big5");
        assert_eq!(
            encode_url_query("https://example.com/\u{3000}?X\u{3000}X#\u{3000}", big5),
            "https://example.com/\u{3000}?X%A1@X#\u{3000}"
        );
        assert_eq!(
            encode_url_query("https://example.com/a#b?\u{3000}", big5),
            "https://example.com/a#b?\u{3000}"
        );
        assert_eq!(
            encode_url_query("https://example.com/", big5),
            "https://example.com/"
        );
    }
}
