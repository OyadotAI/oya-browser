/** Content-Encoding: advertised codings decode, and every decode is bounded and refuses bad data. */
/** Shared loopback server; each test binary uses a different part of it. */
#[allow(dead_code)]
mod support;
use flate2::Compression;
use mars_relay::constants::{HTML_BYTES, IMAGE_BYTES};
use mars_relay::protocol::EngineResult;
use mars_relay::{Prepared, WebRequest, WebResponse};
use std::io::Write;
use support::{REMAINING_MS, Server, header, html};

/** Brotli encoder quality and window for test fixtures. */
const BROTLI_QUALITY: u32 = 5;
const BROTLI_WINDOW: u32 = 22;

/** gzip of `bytes`. */
fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).expect("gzip");
    encoder.finish().expect("gzip")
}

/** zlib-wrapped deflate of `bytes`. */
fn zlib(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).expect("zlib");
    encoder.finish().expect("zlib")
}

/** Raw deflate of `bytes`, which some servers send as "deflate". */
fn raw_deflate(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).expect("deflate");
    encoder.finish().expect("deflate")
}

/** Brotli of `bytes`. */
fn brotli(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut encoder =
        brotli::CompressorWriter::new(&mut output, 4096, BROTLI_QUALITY, BROTLI_WINDOW);
    encoder.write_all(bytes).expect("brotli");
    drop(encoder);
    output
}

/** Zstandard of `bytes`. */
fn zstd(bytes: &[u8]) -> Vec<u8> {
    ruzstd::encoding::compress_to_vec(bytes, ruzstd::encoding::CompressionLevel::Fastest)
}

/** Serves `body` with the given Content-Encoding and fetches it once. */
fn fetch(coding: &str, body: Vec<u8>, image: bool) -> (EngineResult<WebResponse>, Server) {
    let extra = format!("Content-Encoding: {coding}\r\n");
    let server = Server::start(move |_, _| html(&extra, &body));
    let request = WebRequest {
        image,
        ..WebRequest::get(&server.url("/"))
    };
    let result =
        Prepared::connect(request, &server.grants(), REMAINING_MS).and_then(Prepared::transmit);
    (result, server)
}

/** Plain HTTP advertises what Chrome does there; HTTPS adds br and zstd (unit-tested). */
#[test]
fn plain_http_requests_advertise_gzip_and_deflate_like_chrome() {
    let (result, server) = fetch("identity", b"x".to_vec(), false);
    result.expect("identity");
    assert_eq!(
        header(&server.requests()[0], "accept-encoding").as_deref(),
        Some("gzip, deflate")
    );
}

/** gzip, zlib deflate, raw deflate, brotli and a stacked list all decode to the same text. */
#[test]
fn every_advertised_coding_decodes() {
    let text = b"<p>hello compressed world</p>";
    for (coding, body) in [
        ("gzip", gzip(text)),
        ("deflate", zlib(text)),
        ("deflate", raw_deflate(text)),
        ("br", brotli(text)),
        ("gzip, br", brotli(&gzip(text))),
        ("zstd", zstd(text)),
        ("GZIP", gzip(text)),
    ] {
        let (result, _) = fetch(coding, body, false);
        assert_eq!(result.expect(coding).source.as_bytes(), text, "{coding}");
    }
}

/** A decoded body exactly at HTML_BYTES is accepted; one byte more is refused as a limit. */
#[test]
fn decoded_bodies_are_bounded_at_html_bytes() {
    for (coding, encode) in [
        ("gzip", gzip as fn(&[u8]) -> Vec<u8>),
        ("br", brotli),
        ("zstd", zstd),
    ] {
        let (fits, _) = fetch(coding, encode(&vec![b'a'; HTML_BYTES]), false);
        assert_eq!(
            fits.expect("at the bound").source.len(),
            HTML_BYTES,
            "{coding}"
        );
        let (bomb, _) = fetch(coding, encode(&vec![b'a'; HTML_BYTES + 1]), false);
        let error = bomb.expect_err("over the bound");
        assert_eq!(error.code, "limit_exceeded", "{coding}: {}", error.message);
    }
}

/** A 64 MiB zero bomb is refused without ever being fully decoded. */
#[test]
fn decompression_bombs_are_refused() {
    let bomb = gzip(&vec![0; 64 * 1024 * 1024]);
    assert!(bomb.len() < HTML_BYTES);
    let (result, _) = fetch("gzip", bomb, false);
    assert_eq!(result.expect_err("bomb").code, "limit_exceeded");
}

/** Images use their smaller budget for decoded bytes too. */
#[test]
fn image_decodes_are_bounded_at_image_bytes() {
    let (result, _) = fetch("gzip", gzip(&vec![0; IMAGE_BYTES + 1]), true);
    assert_eq!(result.expect_err("over image bound").code, "limit_exceeded");
}

/** Corrupt, truncated, unknown or over-stacked codings are refused, never partly used. */
#[test]
fn malformed_and_unknown_codings_are_refused() {
    let text = b"<p>hello</p>";
    let mut truncated = gzip(text);
    truncated.truncate(truncated.len() - 4);
    let mut short_br = brotli(&vec![b'a'; 10_000]);
    short_br.truncate(short_br.len() / 2);
    for (coding, body) in [
        ("gzip", b"not gzip at all".to_vec()),
        ("gzip", truncated),
        ("br", short_br),
        ("deflate", b"\xff\xff\xff".to_vec()),
        ("zstd", text.to_vec()),
        // A frame header declaring a 2 TiB window, far past RFC 9659's 8 MiB.
        ("zstd", b"\x28\xb5\x2f\xfd\x00\xf8\x01\x00\x00".to_vec()),
        ("gzip, gzip, gzip", gzip(&gzip(&gzip(text)))),
        ("gzip,", gzip(text)),
    ] {
        let (result, _) = fetch(coding, body, false);
        assert_eq!(
            result.expect_err(coding).code,
            "unsupported_feature",
            "{coding}"
        );
    }
}
