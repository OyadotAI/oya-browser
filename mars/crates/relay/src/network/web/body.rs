use super::super::headers::insert_field;
/** Bounded HTTP status, headers and chunk framing are implemented in Oya. */
use super::encoding;
use super::transport::{Connection, READ_CHUNK};
use crate::constants::{CHUNK_FRAMING_BYTES, HEADER_BYTES, HTML_BYTES, IMAGE_BYTES};
use crate::protocol::{EngineError, EngineResult};
use std::collections::BTreeMap;
use std::time::Instant;

/** A complete decoded response, not a prematurely published page. */
pub(super) struct Response {
    /** Valid three-digit HTTP status. */
    pub status: u16,
    /** Lowercase names; repeated list fields joined, repeated singletons refused. */
    pub headers: BTreeMap<String, String>,
    /** Cookies preserve individual fields without relaxing singleton framing. */
    pub set_cookies: Vec<String>,
    /** Transfer- and content-decoded bounded bytes. */
    pub body: Vec<u8>,
    /** HTTP/1.1, framed, not `Connection: close`, and nothing unread: the socket may be reused. */
    pub reusable: bool,
}

/** RFC 9110 section 6.4.1: these statuses never carry content, whatever the headers say. */
const BODYLESS: [u16; 2] = [204, 304];

/**
 * Header bytes and decoded body have independent budgets. `initial` holds bytes the caller
 * already read (the reuse probe for a pooled connection), consumed before the socket. A `head`
 * response has no body whatever its Content-Length says (RFC 9110 section 9.3.2).
 */
pub(super) fn response(
    connection: &mut Connection,
    deadline: Instant,
    image: bool,
    head: bool,
    initial: Vec<u8>,
) -> EngineResult<Response> {
    let mut input = Input {
        connection,
        deadline,
        bytes: initial,
        position: 0,
    };
    let status_line = input.line(HEADER_BYTES, header_limit)?;
    let (persistent, status) = status(&status_line)?;
    let mut used = status_line.len() + 2;
    let (headers, set_cookies) = headers(&mut input, &mut used)?;
    let limit = if image { IMAGE_BYTES } else { HTML_BYTES };
    let (body, framed) = if head || BODYLESS.contains(&status) {
        (Vec::new(), true)
    } else {
        body(&mut input, &headers, limit)?
    };
    let body = encoding::decode(
        body,
        headers.get("content-encoding").map(String::as_str),
        limit,
    )?;
    let reusable = persistent
        && framed
        && input.position == input.bytes.len()
        && !headers.get("connection").is_some_and(|value| {
            value
                .split(',')
                .any(|token| token.trim().eq_ignore_ascii_case("close"))
        });
    Ok(Response {
        status,
        headers,
        set_cookies,
        body,
        reusable,
    })
}

/** Set-Cookie stays a list of separate fields; every other name goes through `insert_field`. */
fn headers(
    input: &mut Input<'_>,
    used: &mut usize,
) -> EngineResult<(BTreeMap<String, String>, Vec<String>)> {
    let mut headers = BTreeMap::new();
    let mut cookies = Vec::new();
    loop {
        let line = input.line(HEADER_BYTES.saturating_sub(*used), header_limit)?;
        *used += line.len() + 2;
        if *used > HEADER_BYTES {
            return Err(header_limit());
        }
        if line.is_empty() {
            return Ok((headers, cookies));
        }
        let (name, value) = line.split_once(':').ok_or_else(unsupported)?;
        if !crate::network::mime::token(name) || value.chars().any(|c| c.is_control() && c != '\t')
        {
            return Err(unsupported());
        }
        if name.eq_ignore_ascii_case("set-cookie") {
            cookies.push(value.trim().to_owned());
            continue;
        }
        insert_field(&mut headers, name, value)?;
    }
}

/** Content-Length and chunked framing cannot be mixed; the flag says the body was self-delimited. */
fn body(
    input: &mut Input<'_>,
    headers: &BTreeMap<String, String>,
    limit: usize,
) -> EngineResult<(Vec<u8>, bool)> {
    let transfer = headers.get("transfer-encoding");
    let length = headers.get("content-length");
    if transfer.is_some() && length.is_some() {
        return Err(unsupported());
    }
    if let Some(transfer) = transfer {
        if !transfer.eq_ignore_ascii_case("chunked") {
            return Err(unsupported());
        }
        return chunked(input, limit).map(|body| (body, true));
    }
    if let Some(length) = length {
        if length.is_empty() || !length.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(unsupported());
        }
        let length = length.parse::<usize>().map_err(|_| unsupported())?;
        if length > limit {
            return Err(EngineError::limit("HTTP body bytes", limit));
        }
        return input.exact(length).map(|body| (body, true));
    }
    let mut body = Vec::new();
    while let Some(byte) = input.byte()? {
        if body.len() == limit {
            return Err(EngineError::limit("HTTP body bytes", limit));
        }
        body.push(byte);
    }
    Ok((body, false))
}

/**
 * Chunk-size lines and their CRLFs share `CHUNK_FRAMING_BYTES`, sized so a full body in small
 * chunks still fits. Trailers get a fresh `HEADER_BYTES` budget, since the framing count says
 * nothing about how many header bytes remain.
 */
fn chunked(input: &mut Input<'_>, limit: usize) -> EngineResult<Vec<u8>> {
    let mut body = Vec::new();
    let mut framing = 0;
    loop {
        let size = input.line(CHUNK_FRAMING_BYTES.saturating_sub(framing), framing_limit)?;
        framing += size.len() + 2;
        if size.is_empty() || !size.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(unsupported());
        }
        let size = usize::from_str_radix(&size, 16).map_err(|_| unsupported())?;
        if size == 0 {
            let mut trailer_used = 0;
            let _ = headers(input, &mut trailer_used)?;
            return Ok(body);
        }
        if size > limit.saturating_sub(body.len()) {
            return Err(EngineError::limit("HTTP body bytes", limit));
        }
        body.extend(input.exact(size)?);
        if input.exact(2)? != b"\r\n" {
            return Err(unsupported());
        }
        framing += 2;
        if framing > CHUNK_FRAMING_BYTES {
            return Err(framing_limit());
        }
    }
}

/** A small buffered cursor avoids allocating for the entire encoded response. */
struct Input<'a> {
    /** Exact-address transport with an absolute deadline. */
    connection: &'a mut Connection,
    /** Shared across every read and redirect. */
    deadline: Instant,
    /** At most one socket read chunk (or the caller's probe bytes) is retained. */
    bytes: Vec<u8>,
    /** Next unread byte within that chunk. */
    position: usize,
}

impl Input<'_> {
    /** Stream consumption never retries after EOF. */
    fn byte(&mut self) -> EngineResult<Option<u8>> {
        if self.position == self.bytes.len() {
            self.bytes.resize(READ_CHUNK, 0);
            let length = self.connection.read(&mut self.bytes, self.deadline)?;
            self.bytes.truncate(length);
            self.position = 0;
            if length == 0 {
                return Ok(None);
            }
        }
        let byte = self.bytes[self.position];
        self.position += 1;
        Ok(Some(byte))
    }
    /** A promised body or CRLF must arrive completely; whole buffered runs are copied at once. */
    fn exact(&mut self, length: usize) -> EngineResult<Vec<u8>> {
        let mut output = Vec::with_capacity(length);
        while output.len() < length {
            let byte = self.byte()?.ok_or_else(unsupported)?;
            output.push(byte);
            let run = (length - output.len()).min(self.bytes.len() - self.position);
            output.extend_from_slice(&self.bytes[self.position..self.position + run]);
            self.position += run;
        }
        Ok(output)
    }
    /** Header and chunk lines stop at their budget rather than growing until newline; `limit` names that budget in the error. */
    fn line(&mut self, budget: usize, limit: fn() -> EngineError) -> EngineResult<String> {
        let mut output = Vec::new();
        loop {
            let byte = self.byte()?.ok_or_else(unsupported)?;
            if output.len() >= budget {
                return Err(limit());
            }
            output.push(byte);
            if output.ends_with(b"\r\n") {
                output.truncate(output.len() - 2);
                return String::from_utf8(output).map_err(|_| unsupported());
            }
            if byte == b'\n' {
                return Err(unsupported());
            }
        }
    }
}

/** Malformed statuses are not normalized into a successful response; true means HTTP/1.1. */
fn status(line: &str) -> EngineResult<(bool, u16)> {
    let mut fields = line.splitn(3, ' ');
    let version = fields.next().unwrap_or_default();
    if !["HTTP/1.1", "HTTP/1.0"].contains(&version) {
        return Err(unsupported());
    }
    let value = fields.next().ok_or_else(unsupported)?;
    if value.len() != 3
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || fields.next().is_none()
    {
        return Err(unsupported());
    }
    let status = value.parse().map_err(|_| unsupported())?;
    Ok((version == "HTTP/1.1", status))
}

/** The status line plus every header line, CRLFs included, exceeded `HEADER_BYTES`. */
fn header_limit() -> EngineError {
    EngineError::limit("HTTP header bytes", HEADER_BYTES)
}

/** Chunk-size lines and chunk CRLFs exceeded `CHUNK_FRAMING_BYTES`. */
fn framing_limit() -> EngineError {
    EngineError::limit("HTTP chunk framing bytes", CHUNK_FRAMING_BYTES)
}

/** EOF before the first response byte: the peer closed without answering. */
pub(super) fn truncated() -> EngineError {
    unsupported()
}

/** An unsupported response remains an error instead of being partially interpreted. */
fn unsupported() -> EngineError {
    EngineError::unsupported(
        "Unsupported or incomplete HTTP status, headers, encoding or body framing.",
    )
}
