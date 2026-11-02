/** Real loopback-only HTTP fixtures test framing and absolute network deadlines. */
#[allow(dead_code)]
mod support;
use mars::constants::{HEADER_BYTES, HTML_BYTES, NETWORK_TIMEOUT, RPC_TIMEOUT};
use mars::network::fetch;
use std::sync::atomic::Ordering;
use std::time::Instant;
use support::Fixture;

/** Deliberately separates connect, first header segment and final delimiter delivery. */
const DELIVERY_GAP: std::time::Duration = std::time::Duration::from_millis(25);

/** A connection-delimited response is supported without inventing Content-Length. */
#[test]
fn connection_delimited_utf8_html_is_supported() {
    let fixture = Fixture::raw(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body></body></html>".to_vec(),
    );
    assert_eq!(
        fetch(&format!("{}/", fixture.url)).unwrap(),
        "<html><body></body></html>"
    );
}

/** Ambiguous framing, encoding and redirects cannot become successful pages. */
#[test]
fn unsupported_http_features_are_explicitly_rejected() {
    for headers in [
        "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/\r\n",
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n",
        "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\n",
        "HTTP/1.1 200 OK\r\nContent-Length: 0\r\ncontent-length: 0\r\n",
        "HTTP/1.1 200 OK\r\nContent-Length: -1\r\n",
        "HTTP/1.1 200 OK\r\nContent-Length: 9999999999999999999999\r\n",
    ] {
        let fixture =
            Fixture::raw(format!("{headers}Content-Type: text/html\r\n\r\n").into_bytes());
        assert_eq!(
            fetch(&format!("{}/", fixture.url)).unwrap_err().code,
            "unsupported_feature"
        );
        assert_eq!(fixture.requests.load(Ordering::Relaxed), 1);
    }
}

/** A peer closing early cannot publish a partial body; malformed UTF-8 is decoded, not refused. */
#[test]
fn truncated_bodies_are_rejected_and_malformed_utf8_is_replaced() {
    let fixture = Fixture::raw(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 100\r\n\r\nshort".to_vec(),
    );
    assert_eq!(
        fetch(&format!("{}/", fixture.url)).unwrap_err().code,
        "network_error"
    );
    let fixture = Fixture::raw(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 1\r\n\r\n\xff".to_vec(),
    );
    // A malformed byte decodes to U+FFFD, as the Encoding standard's UTF-8 decoder does.
    assert_eq!(fetch(&format!("{}/", fixture.url)).unwrap(), "\u{fffd}");
}

/** Body and header budgets cannot be bypassed using connection-delimited responses. */
#[test]
fn http_allocations_have_independent_bounds() {
    for response in [
        format!(
            "HTTP/1.1 200 OK\r\nX-Long: {}\r\nContent-Type: text/html\r\n\r\n",
            "x".repeat(HEADER_BYTES)
        ),
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n",
            HTML_BYTES + 1
        ),
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n{}",
            "x".repeat(HTML_BYTES + 1)
        ),
    ] {
        let fixture = Fixture::raw(response.into_bytes());
        assert_eq!(
            fetch(&format!("{}/", fixture.url)).unwrap_err().code,
            "limit_exceeded"
        );
    }
}

/** A held-open socket cannot extend a request beyond the total request budget. */
#[test]
fn slow_response_ends_at_the_absolute_deadline() {
    let fixture = Fixture::slow();
    let start = Instant::now();
    assert_eq!(
        fetch(&format!("{}/", fixture.url)).unwrap_err().code,
        "timeout"
    );
    assert!(start.elapsed() >= NETWORK_TIMEOUT);
    assert!(start.elapsed() < RPC_TIMEOUT);
}

/** Delayed and fragmented requests cannot trigger an early reply or a reset. */
#[test]
fn fixtures_wait_for_complete_headers_on_accepted_sockets() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    let fixture = Fixture::pages();
    let mut socket = TcpStream::connect(fixture.url.strip_prefix("http://").unwrap()).unwrap();
    std::thread::sleep(DELIVERY_GAP);
    socket.write_all(b"GET /plain HTTP/1.1\r\nHost:").unwrap();
    socket.set_read_timeout(Some(DELIVERY_GAP)).unwrap();
    let error = socket.read(&mut [0]).unwrap_err();
    assert!([std::io::ErrorKind::WouldBlock, std::io::ErrorKind::TimedOut].contains(&error.kind()));
    socket.write_all(b" 127.0.0.1\r\n\r\n").unwrap();
    socket.set_read_timeout(Some(NETWORK_TIMEOUT)).unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("original"));
}
