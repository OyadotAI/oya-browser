/** Header joining, singleton refusals and every framing budget, pinned on each loopback fetch path. */
use mars_relay::constants::{CHUNK_FRAMING_BYTES, HEADER_BYTES, HTML_BYTES};
use mars_relay::protocol::{EngineError, EngineResult};
use mars_relay::{Prepared, WebRequest, WebResponse, fetch, fetch_web};
use std::io::{Read, Write};
use std::net::TcpListener;

/** Each subresource fetch gets the whole navigation budget. */
const REMAINING_MS: u64 = 8_000;

/** Chunk size used for the in-budget chunked body. */
const CHUNK: usize = 4 * 1024;

/** Message the shared field helper returns for a repeated singleton. */
const REPEATED: &str = "Repeated HTTP framing or singleton header.";

/** Serves one raw response to `connections` loopback connections, ignoring early hangups. */
fn serve(response: Vec<u8>, connections: usize) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback listener");
    let origin = format!("http://{}", listener.local_addr().expect("address"));
    let worker = std::thread::spawn(move || {
        for stream in listener.incoming().take(connections) {
            let mut stream = stream.expect("connection");
            let mut request = Vec::new();
            let mut byte = [0; 1];
            while !request.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 {
                request.push(byte[0]);
            }
            let _ = stream.write_all(&response);
        }
    });
    (origin, worker)
}

/** Runs the web navigation and web subresource paths against one response. */
fn web(response: &[u8]) -> (EngineResult<String>, EngineResult<WebResponse>) {
    let (origin, worker) = serve(response.to_vec(), 2);
    let url = format!("{origin}/");
    let grants = std::slice::from_ref(&origin);
    let navigation = fetch_web(&url, grants).map(|page| page.source);
    let subresource =
        Prepared::connect(WebRequest::get(&url), grants, REMAINING_MS).and_then(Prepared::transmit);
    worker.join().expect("fixture");
    (navigation, subresource)
}

/** Runs the loopback fixture path against one response. */
fn loopback(response: &[u8]) -> EngineResult<String> {
    let (origin, worker) = serve(response.to_vec(), 1);
    let result = fetch(&format!("{origin}/"));
    worker.join().expect("fixture");
    result
}

/** All three paths' sources, in navigation, subresource, loopback order. */
fn every_path(response: &[u8]) -> [EngineResult<String>; 3] {
    let (navigation, subresource) = web(response);
    [
        navigation,
        subresource.map(|response| response.source),
        loopback(response),
    ]
}

/** A 200 text/html response with extra header lines and a Content-Length body. */
fn page(extra: &str, body: &[u8]) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n{extra}Content-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

/** A 200 text/html response whose body is framed by `framing`. */
fn framed(framing: &str, body: &[u8]) -> Vec<u8> {
    let mut response =
        format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n{framing}\r\n").into_bytes();
    response.extend_from_slice(body);
    response
}

/** Encodes `body` as chunks of `size` bytes followed by an empty trailer. */
fn chunked(body: &[u8], size: usize) -> Vec<u8> {
    let mut output = Vec::new();
    for chunk in body.chunks(size) {
        output.extend_from_slice(format!("{:x}\r\n", chunk.len()).as_bytes());
        output.extend_from_slice(chunk);
        output.extend_from_slice(b"\r\n");
    }
    output.extend_from_slice(b"0\r\n\r\n");
    output
}

/** A response head padded with one header so it totals exactly `total` bytes. */
fn head_of(total: usize) -> Vec<u8> {
    let fixed =
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 2\r\nX-Pad: \r\n\r\n";
    let pad = "a".repeat(total - fixed.len());
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 2\r\nX-Pad: {pad}\r\n\r\n"
    )
    .into_bytes();
    assert_eq!(response.len(), total);
    response.extend_from_slice(b"ok");
    response
}

/** Asserts the error is the named limit. */
fn assert_limit(result: EngineResult<impl std::fmt::Debug>, name: &str, bound: usize) {
    let error: EngineError = result.expect_err("limit");
    assert_eq!(error.code, "limit_exceeded", "{}", error.message);
    assert_eq!(
        error.message,
        format!("{name} exceeds the limit of {bound}.")
    );
}

/** RFC 9110 list fields sent twice, as iana.org and lite.duckduckgo.com do, are joined. */
#[test]
fn repeated_list_headers_are_accepted_on_every_path() {
    let response = page(
        "Vary: Accept\r\nvary: Accept-Encoding\r\nX-Content-Type-Options: nosniff\r\nx-content-type-options: nosniff\r\n",
        b"ok",
    );
    for result in every_path(&response) {
        assert_eq!(result.expect("joined list headers"), "ok");
    }
}

/** Framing and destination fields stay refused when repeated, in any case. */
#[test]
fn repeated_singleton_headers_are_refused_on_every_path() {
    for repeat in [
        "Content-Length: 2\r\ncontent-length: 2\r\n",
        "Transfer-Encoding: chunked\r\nTRANSFER-ENCODING: chunked\r\n",
        "Content-Encoding: identity\r\ncontent-encoding: identity\r\n",
        "Content-Type: text/html\r\nCONTENT-TYPE: text/html\r\n",
        "Location: /a\r\nlocation: /b\r\n",
    ] {
        let response = framed(repeat, b"ok");
        let [navigation, subresource, loopback] = every_path(&response);
        for result in [navigation, subresource] {
            let error = result.expect_err(repeat);
            assert_eq!(error.code, "unsupported_feature", "{repeat}");
            assert_eq!(error.message, REPEATED, "{repeat}");
        }
        // Loopback refuses any Transfer-Encoding before the repeat matters.
        assert_eq!(loopback.expect_err(repeat).code, "unsupported_feature");
    }
}

/** On the web path cookies stay separate fields; loopback has no cookies and refuses the repeat. */
#[test]
fn set_cookie_values_stay_separate() {
    let response = page("Set-Cookie: a=1\r\nset-cookie: b=2\r\n", b"ok");
    let (_, subresource) = web(&response);
    assert_eq!(
        subresource.expect("cookies").set_cookies,
        vec!["a=1".to_owned(), "b=2".to_owned()]
    );
    let error = loopback(&response).expect_err("loopback repeat");
    assert_eq!(error.message, REPEATED);
}

/** The whole head, CRLFs included, may total exactly HEADER_BYTES. */
#[test]
fn headers_at_the_budget_are_accepted_and_one_byte_more_is_refused() {
    for result in every_path(&head_of(HEADER_BYTES)) {
        assert_eq!(result.expect("head at budget"), "ok");
    }
    for result in every_path(&head_of(HEADER_BYTES + 1)) {
        assert_limit(result, "HTTP header bytes", HEADER_BYTES);
    }
}

/** One over-long header line reports the header budget, not a generic framing error. */
#[test]
fn a_single_overlong_header_line_reports_header_bytes() {
    let line = format!("X-Pad: {}\r\n", "a".repeat(HEADER_BYTES));
    for result in every_path(&page(&line, b"ok")) {
        assert_limit(result, "HTTP header bytes", HEADER_BYTES);
    }
}

/** Content-Length bodies fit at HTML_BYTES and are refused one byte later. */
#[test]
fn content_length_bodies_are_bounded_at_html_bytes() {
    let body = vec![b'a'; HTML_BYTES];
    for result in every_path(&page("", &body)) {
        assert_eq!(result.expect("body at budget").len(), HTML_BYTES);
    }
    let body = vec![b'a'; HTML_BYTES + 1];
    let [navigation, subresource, loopback] = every_path(&page("", &body));
    assert_limit(navigation, "HTTP body bytes", HTML_BYTES);
    assert_limit(subresource, "HTTP body bytes", HTML_BYTES);
    assert_limit(loopback, "HTML bytes", HTML_BYTES);
}

/** Connection-delimited bodies fit at HTML_BYTES and are refused one byte later. */
#[test]
fn connection_delimited_bodies_are_bounded_at_html_bytes() {
    let response = framed("Connection: close\r\n", &vec![b'a'; HTML_BYTES]);
    for result in every_path(&response) {
        assert_eq!(result.expect("body at budget").len(), HTML_BYTES);
    }
    let response = framed("Connection: close\r\n", &vec![b'a'; HTML_BYTES + 1]);
    let [navigation, subresource, loopback] = every_path(&response);
    assert_limit(navigation, "HTTP body bytes", HTML_BYTES);
    assert_limit(subresource, "HTTP body bytes", HTML_BYTES);
    assert_limit(loopback, "HTML bytes", HTML_BYTES);
}

/** A full body in small chunks fits both the body and the chunk framing budgets. */
#[test]
fn chunked_bodies_are_bounded_at_html_bytes() {
    let te = "Transfer-Encoding: chunked\r\n";
    let (navigation, subresource) = web(&framed(te, &chunked(&vec![b'a'; HTML_BYTES], CHUNK)));
    assert_eq!(navigation.expect("chunked at budget").len(), HTML_BYTES);
    assert_eq!(
        subresource.expect("chunked at budget").source.len(),
        HTML_BYTES
    );
    let (navigation, subresource) = web(&framed(te, &chunked(&vec![b'a'; HTML_BYTES + 1], CHUNK)));
    assert_limit(navigation, "HTTP body bytes", HTML_BYTES);
    assert_limit(subresource, "HTTP body bytes", HTML_BYTES);
}

/** Many tiny chunks exhaust the framing budget before the body budget. */
#[test]
fn chunk_framing_past_its_budget_is_refused() {
    let te = "Transfer-Encoding: chunked\r\n";
    let body = vec![b'a'; CHUNK_FRAMING_BYTES / 4];
    let (navigation, subresource) = web(&framed(te, &chunked(&body, 1)));
    assert_limit(navigation, "HTTP chunk framing bytes", CHUNK_FRAMING_BYTES);
    assert_limit(subresource, "HTTP chunk framing bytes", CHUNK_FRAMING_BYTES);
}

/** The declared charset is reported by encoding, not by the label the server spelled. */
#[test]
fn web_responses_report_the_declared_charset() {
    for (content_type, expected) in [
        ("text/html; charset=ISO-8859-1", Some("windows-1252")),
        ("text/html; charset=utf8", Some("utf-8")),
        ("text/html", None),
    ] {
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: 2\r\n\r\nok"
        )
        .into_bytes();
        let (_, subresource) = web(&response);
        assert_eq!(
            subresource.expect("text response").charset.as_deref(),
            expected,
            "{content_type}"
        );
    }
}
