/** Minimal HTTP framing is strict to prevent accepting ambiguous responses. */
use super::mime;
use crate::constants::{HTML_BYTES, HTTP_OK};
use crate::protocol::{EngineError, EngineResult};
use std::collections::BTreeMap;

/** A validated loopback response's exact body length and declared charset. */
pub(super) struct Head {
    /** Exact body length, or connection-delimited when absent. */
    pub length: Option<usize>,
    /** Charset parameter for `mime::decode`. */
    pub charset: Option<String>,
}

/** Returns the body length and charset after validating the response. */
pub(super) fn parse(bytes: &[u8]) -> EngineResult<Head> {
    let source = std::str::from_utf8(bytes).map_err(|_| unsupported())?;
    let mut lines = source.split("\r\n");
    status(lines.next().unwrap_or_default())?;
    let headers = fields(lines)?;
    let charset = encoding(&headers)?;
    Ok(Head {
        length: content_length(&headers)?,
        charset,
    })
}

/** Redirects and error statuses are unsupported and never followed. */
fn status(line: &str) -> EngineResult<()> {
    let mut parts = line.splitn(3, ' ');
    if !["HTTP/1.1", "HTTP/1.0"].contains(&parts.next().unwrap_or_default()) {
        return Err(unsupported());
    }
    if parts.next().and_then(|status| status.parse::<u16>().ok()) != Some(HTTP_OK) {
        return Err(unsupported());
    }
    if parts.next().is_none() {
        return Err(unsupported());
    }
    Ok(())
}

/** Repeated list fields are joined; repeated framing and singleton fields are rejected. */
fn fields<'a>(lines: impl Iterator<Item = &'a str>) -> EngineResult<BTreeMap<String, String>> {
    let mut headers = BTreeMap::new();
    for line in lines {
        let (name, value) = line.split_once(':').ok_or_else(unsupported)?;
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(unsupported());
        }
        if value.chars().any(|c| c.is_control() && c != '\t') {
            return Err(unsupported());
        }
        insert_field(&mut headers, name, value)?;
    }
    Ok(headers)
}

/** Fields Relay reads as one value, so a repeat is ambiguous framing rather than a list. */
const SINGLETON_FIELDS: [&str; 6] = [
    "content-length",
    "transfer-encoding",
    "content-encoding",
    "content-type",
    "location",
    "set-cookie",
];

/**
 * Adds one field under its lowercase name. RFC 9110 section 5.3 lets a recipient join repeated
 * list fields with ", ", which is how a browser reads `Vary` or `X-Content-Type-Options` sent
 * twice. Singleton fields stay refused, since picking either copy could disagree with another
 * parser about the framing or the destination. Joined values never exceed the raw header bytes,
 * so the caller's header budget still bounds them.
 */
pub(super) fn insert_field(
    headers: &mut BTreeMap<String, String>,
    name: &str,
    value: &str,
) -> EngineResult<()> {
    let name = name.to_ascii_lowercase();
    let value = value.trim();
    let Some(existing) = headers.get_mut(&name) else {
        headers.insert(name, value.to_owned());
        return Ok(());
    };
    if SINGLETON_FIELDS.contains(&name.as_str()) {
        return Err(EngineError::unsupported(
            "Repeated HTTP framing or singleton header.",
        ));
    }
    if existing.is_empty() {
        value.clone_into(existing);
    } else if !value.is_empty() {
        existing.push_str(", ");
        existing.push_str(value);
    }
    Ok(())
}

/** Transfer and compression codecs must be implemented before they can be advertised. */
fn encoding(headers: &BTreeMap<String, String>) -> EngineResult<Option<String>> {
    if headers.contains_key("transfer-encoding") {
        return Err(unsupported());
    }
    if headers
        .get("content-encoding")
        .is_some_and(|value| !value.eq_ignore_ascii_case("identity"))
    {
        return Err(unsupported());
    }
    headers
        .get("content-type")
        .and_then(|value| mime::parse(value))
        .filter(|content_type| content_type.essence == "text/html")
        .map(|content_type| content_type.charset)
        .ok_or_else(unsupported)
}

/** Length is bounded before any matching body allocation is attempted. */
fn content_length(headers: &BTreeMap<String, String>) -> EngineResult<Option<usize>> {
    let Some(value) = headers.get("content-length") else {
        return Ok(None);
    };
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(unsupported());
    }
    let length = value.parse::<usize>().map_err(|_| unsupported())?;
    if length > HTML_BYTES {
        return Err(EngineError::limit("HTML bytes", HTML_BYTES));
    }
    Ok(Some(length))
}

/** Includes the supported framing in the recovery hint. */
fn unsupported() -> EngineError {
    EngineError::unsupported(
        "HTTP requires status 200, text/html in a supported charset, identity encoding, no repeated framing headers or transfer encoding, and a valid optional Content-Length.",
    )
}
