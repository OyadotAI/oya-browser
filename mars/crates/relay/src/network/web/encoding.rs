/** Content-Encoding decoding with every layer's output bounded by the response's body budget. */
use crate::network::constants::CONTENT_CODINGS;
use crate::protocol::{EngineError, EngineResult};
use std::io::Read;

/// What HTTPS requests advertise, Chrome's value; only codings decoded below may appear here.
pub(super) const ACCEPT_ENCODING: &str = "gzip, deflate, br, zstd";

/// Chrome advertises Brotli and Zstandard only over HTTPS; plain HTTP gets this.
pub(super) const ACCEPT_ENCODING_PLAIN: &str = "gzip, deflate";

/// Media element requests ask for the bytes as stored, Chrome's exact value.
pub(super) const MEDIA_ACCEPT_ENCODING: &str = "identity;q=1, *;q=0";

/** Brotli's internal window buffer; any size works, this one matches its documented default. */
const BROTLI_BUFFER: usize = 4096;

/// RFC 9659 section 3: HTTP Zstandard windows are at most 8 MiB, so a frame header cannot make
/// the decoder reserve more.
const ZSTD_WINDOW: u64 = 8 * 1024 * 1024;

/**
 * Decodes `body` according to the Content-Encoding list, last coding first (RFC 9110 section
 * 8.4). Decoded output above `limit` is refused before it is fully produced, so a small
 * compressed body cannot expand into an unbounded allocation; malformed data is refused too.
 */
pub(super) fn decode(body: Vec<u8>, header: Option<&str>, limit: usize) -> EngineResult<Vec<u8>> {
    // A 304, a 204 or an empty 200 keeps its Content-Encoding header but has nothing to decode.
    let Some(header) = header.filter(|_| !body.is_empty()) else {
        return Ok(body);
    };
    let codings: Vec<String> = header
        .split(',')
        .map(|coding| coding.trim().to_ascii_lowercase())
        .filter(|coding| coding != "identity")
        .collect();
    if codings.len() > CONTENT_CODINGS || codings.iter().any(String::is_empty) {
        return Err(unsupported());
    }
    codings
        .iter()
        .rev()
        .try_fold(body, |body, coding| layer(&body, coding, limit))
}

/** One coding, read through a `limit + 1` window so overflow is detected without decoding it all. */
fn layer(body: &[u8], coding: &str, limit: usize) -> EngineResult<Vec<u8>> {
    let reader: Box<dyn Read + '_> = match coding {
        "gzip" | "x-gzip" => Box::new(flate2::read::MultiGzDecoder::new(body)),
        "deflate" if zlib_wrapped(body) => Box::new(flate2::read::ZlibDecoder::new(body)),
        "deflate" => Box::new(flate2::read::DeflateDecoder::new(body)),
        "br" => Box::new(brotli_decompressor::Decompressor::new(body, BROTLI_BUFFER)),
        "zstd" => Box::new(
            ruzstd::decoding::StreamingDecoder::new_with_max_window_size(body, ZSTD_WINDOW)
                .map_err(|_| malformed())?,
        ),
        _ => return Err(unsupported()),
    };
    let mut output = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut output)
        .map_err(|_| malformed())?;
    if output.len() > limit {
        return Err(EngineError::limit("decoded HTTP body bytes", limit));
    }
    Ok(output)
}

/**
 * HTTP "deflate" means zlib (RFC 9110 section 8.4.1.2), but some servers send raw DEFLATE, and
 * browsers accept both. A zlib header is a CM=8 byte whose two-byte value is a multiple of 31.
 */
fn zlib_wrapped(body: &[u8]) -> bool {
    const ZLIB_CHECK: u16 = 31;
    const DEFLATE_METHOD: u8 = 8;
    const METHOD_MASK: u8 = 0x0f;
    body.len() >= 2
        && body[0] & METHOD_MASK == DEFLATE_METHOD
        && u16::from_be_bytes([body[0], body[1]]).is_multiple_of(ZLIB_CHECK)
}

/** A coding Relay never advertised, or an over-long coding list. */
fn unsupported() -> EngineError {
    EngineError::unsupported("Unsupported HTTP Content-Encoding.")
}

/** The compressed stream is corrupt or truncated; nothing partial becomes a document. */
fn malformed() -> EngineError {
    EngineError::unsupported("The compressed HTTP body is malformed or truncated.")
}
