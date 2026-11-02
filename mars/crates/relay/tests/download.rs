/** Top-level downloads: attachments and undisplayable types come back as raw bytes, never as a page. */
/** Shared loopback server; each test binary uses a different part of it. */
#[allow(dead_code)]
mod support;
use mars_relay::{Destination, Prepared, WebRequest, WebResponse};
use support::{REMAINING_MS, Server};

/** A binary body that is not valid UTF-8, so a text decode would refuse it. */
const BINARY: &[u8] = &[0x50, 0x4b, 0x03, 0x04, 0xff, 0xfe, 0x00, 0x80];

/** A 200 response with this Content-Type, extra header lines and body. */
fn reply(mime: &str, extra: &str, body: &[u8]) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\n{extra}\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

/** One prepare and transmit with the server's own grant. */
fn fetch(server: &Server, request: WebRequest) -> WebResponse {
    Prepared::connect(request, &server.grants(), REMAINING_MS)
        .and_then(Prepared::transmit)
        .expect("fetch")
}

/** Standard base64, to compare against the response source. */
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                char::from(ALPHABET[(n >> (18 - 6 * i) & 63) as usize])
            } else {
                '='
            });
        }
    }
    out
}

/** An undisplayable type at the top level is a download with its exact bytes. */
#[test]
fn undisplayable_top_level_types_are_downloads() {
    let server = Server::start(|_, _| reply("application/zip", "", BINARY));
    let response = fetch(&server, WebRequest::get(&server.url("/file.zip")));
    assert!(response.download);
    assert_eq!(response.mime, "application/zip");
    assert_eq!(response.source, base64(BINARY));
    assert_eq!(response.charset, None);
}

/** Content-Disposition: attachment downloads even an HTML body, and keeps the header for naming. */
#[test]
fn attachments_are_downloads_with_their_disposition() {
    let disposition = "attachment; filename=\"report.html\"";
    let server = Server::start(move |_, _| {
        reply(
            "text/html; charset=utf-8",
            &format!("Content-Disposition: {disposition}\r\n"),
            b"<p>hi</p>",
        )
    });
    let response = fetch(&server, WebRequest::get(&server.url("/report")));
    assert!(response.download);
    assert_eq!(response.disposition.as_deref(), Some(disposition));
    assert_eq!(response.source, base64(b"<p>hi</p>"));
}

/** Pages, inline dispositions and subresources stay ordinary responses. */
#[test]
fn pages_and_subresources_are_never_downloads() {
    let server = Server::start(|path, _| {
        if path.contains("inline") {
            reply("text/html", "Content-Disposition: inline\r\n", b"<p>x</p>")
        } else {
            reply("application/octet-stream", "", b"bytes")
        }
    });
    let inline = fetch(&server, WebRequest::get(&server.url("/inline")));
    assert!(!inline.download);
    assert_eq!(inline.source, "<p>x</p>");
    let mut script = WebRequest::get(&server.url("/data"));
    script.destination = Destination::Script;
    assert!(!fetch(&server, script).download);
}
