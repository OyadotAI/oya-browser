/** Owned HTTP over OS sockets; no HTTP client or browser engine dependency. */
pub mod constants;
mod cookies;
pub mod cors;
mod headers;
mod mime;
mod partition;
pub mod policy;
mod request;
mod url;
mod web;
use crate::constants::{
    HEADER_BYTES, HEADER_END, HTML_BYTES, NETWORK_CHUNK_BYTES, NETWORK_TIMEOUT,
};
use crate::protocol::{EngineError, EngineResult};
pub use cookies::{CookieEntry, CookieJar};
pub use partition::partition_site;
pub use request::{Destination, Method, Preflight, WebRequest, WebResponse};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};
pub use url::Endpoint;
pub use web::Prepared;
pub use web::redirect_url;
pub use web::{CHROME_ENGINE, clear_cache, fetch_batch, forget, set_cache_directory};
pub use web::{WebPage, fetch as fetch_web};

/// Every call revalidates the literal destination inside the network service.
pub fn fetch(url: &str) -> EngineResult<String> {
    fetch_decoded(url).map(|(source, _)| source)
}

/** The decoded body and the canonical charset it was decoded with, so Eye parses the page's meta
against the same encoding. The charset is None for UTF-8 pages that declare none. */
pub fn fetch_decoded(url: &str) -> EngineResult<(String, Option<String>)> {
    let endpoint = Endpoint::parse(url)?;
    let deadline = Instant::now() + NETWORK_TIMEOUT;
    let mut socket =
        TcpStream::connect_timeout(&endpoint.address, remaining(deadline)?).map_err(io_error)?;
    socket
        .set_write_timeout(Some(remaining(deadline)?))
        .map_err(io_error)?;
    socket
        .write_all(endpoint.request().as_bytes())
        .map_err(io_error)?;
    let (header, mut body) = read_headers(&mut socket, deadline)?;
    let head = headers::parse(&header)?;
    read_body(&mut socket, &mut body, head.length, deadline)?;
    let charset = mime::html_charset("text/html", head.charset, &body);
    let source = mime::decode(body, charset.as_deref())?;
    let canonical = charset
        .as_deref()
        .and_then(mars_core::http::encoding_for_label)
        .map(str::to_owned);
    Ok((source, canonical))
}

/** A header budget is independent of the response body budget. */
fn read_headers(socket: &mut TcpStream, deadline: Instant) -> EngineResult<(Vec<u8>, Vec<u8>)> {
    let mut bytes = Vec::new();
    loop {
        if let Some(end) = bytes
            .windows(HEADER_END.len())
            .position(|part| part == HEADER_END)
        {
            if end + HEADER_END.len() > HEADER_BYTES {
                return Err(EngineError::limit("HTTP header bytes", HEADER_BYTES));
            }
            return Ok((
                bytes[..end].to_vec(),
                bytes[end + HEADER_END.len()..].to_vec(),
            ));
        }
        if bytes.len() >= HEADER_BYTES {
            return Err(EngineError::limit("HTTP header bytes", HEADER_BYTES));
        }
        if read_chunk(socket, &mut bytes, deadline)? == 0 {
            return Err(truncated());
        }
    }
}

/** Content-Length finishes promptly; connection-delimited bodies require actual EOF. */
fn read_body(
    socket: &mut TcpStream,
    body: &mut Vec<u8>,
    length: Option<usize>,
    deadline: Instant,
) -> EngineResult<()> {
    loop {
        if body.len() > HTML_BYTES {
            return Err(EngineError::limit("HTML bytes", HTML_BYTES));
        }
        if let Some(length) = length {
            if body.len() > length {
                return Err(truncated());
            }
            if body.len() == length {
                return Ok(());
            }
        }
        if read_chunk(socket, body, deadline)? == 0 {
            return if length.is_none() {
                Ok(())
            } else {
                Err(truncated())
            };
        }
    }
}

/** Each read uses the remaining total budget; slow bytes cannot extend a request. */
fn read_chunk(
    socket: &mut TcpStream,
    output: &mut Vec<u8>,
    deadline: Instant,
) -> EngineResult<usize> {
    socket
        .set_read_timeout(Some(remaining(deadline)?))
        .map_err(io_error)?;
    let mut bytes = [0; NETWORK_CHUNK_BYTES];
    let count = socket.read(&mut bytes).map_err(io_error)?;
    output.extend_from_slice(&bytes[..count]);
    Ok(count)
}

/** A total deadline bounds connect, write and every read together. */
fn remaining(deadline: Instant) -> EngineResult<Duration> {
    let duration = deadline.saturating_duration_since(Instant::now());
    if duration.is_zero() {
        return Err(timeout());
    }
    Ok(duration)
}

/** HTTP failures retain the old committed page and do not poison healthy services. */
fn io_error(error: std::io::Error) -> EngineError {
    if [std::io::ErrorKind::TimedOut, std::io::ErrorKind::WouldBlock].contains(&error.kind()) {
        return timeout();
    }
    EngineError::new(
        "network_error",
        "The local HTTP fixture could not be reached or read.",
        "Start the loopback fixture and navigate again.",
    )
}

/** Timeout copy makes the total network budget explicit. */
fn timeout() -> EngineError {
    EngineError::new(
        "timeout",
        "The local HTTP request exceeded its two-second deadline.",
        "Fix the local fixture and navigate again.",
    )
}

/** A partial framed response must never become a loaded document. */
fn truncated() -> EngineError {
    EngineError::new(
        "network_error",
        "The HTTP response is incomplete or has inconsistent framing.",
        "Return one complete supported HTTP response.",
    )
}
