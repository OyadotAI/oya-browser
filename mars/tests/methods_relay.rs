/** PUT, PATCH, DELETE, OPTIONS and HEAD through Relay over loopback: wire names and framing, a
HEAD with no body, no retry or connection sharing for unsafe methods, and cache invalidation. */
#[allow(dead_code)]
#[path = "../crates/relay/tests/support/mod.rs"]
mod support;
use mars_relay::{Method, Prepared, WebRequest, WebResponse};
use std::time::{Duration, Instant};
use support::{REMAINING_MS, Server, header, html};

/** The methods that are not GET or HEAD and so must behave like POST. */
const UNSAFE: [Method; 5] = [
    Method::Post,
    Method::Put,
    Method::Patch,
    Method::Delete,
    Method::Options,
];

/** A request partitioned under `top` with `method` and `body`, so the cache applies. */
fn request(method: Method, url: &str, top: &str, body: &str) -> WebRequest {
    WebRequest {
        method,
        body: body.into(),
        top_level: Some(top.into()),
        ..WebRequest::get(url)
    }
}

/** One prepare and transmit with the server's own grant. */
fn send(server: &Server, request: WebRequest) -> mars_relay::protocol::EngineResult<WebResponse> {
    Prepared::connect(request, &server.grants(), REMAINING_MS).and_then(Prepared::transmit)
}

/** Each method goes out under its own name, with Content-Length only where it carries content. */
#[test]
fn every_method_is_sent_by_name_with_its_framing() {
    let server = Server::start(|_, _| html("", b"ok"));
    let url = server.url("/r");
    for (method, body, length) in [
        (Method::Put, "a", Some("1")),
        (Method::Patch, "", Some("0")),
        (Method::Delete, "", None),
        (Method::Delete, "id", Some("2")),
        (Method::Options, "", None),
        (Method::Head, "", None),
    ] {
        let name = method.name();
        send(&server, request(method, &url, &server.origin, body)).expect("sent");
        let head = server.requests().pop().expect("received");
        assert!(head.starts_with(&format!("{name} /r HTTP/1.1")), "{head}");
        assert_eq!(header(&head, "content-length").as_deref(), length, "{head}");
    }
}

/** GET and HEAD never carry a body. */
#[test]
fn a_head_with_a_body_is_refused_before_sending() {
    let server = Server::start(|_, _| html("", b"ok"));
    let error = send(
        &server,
        request(Method::Head, &server.url("/"), &server.origin, "x"),
    )
    .expect_err("HEAD has no body");
    assert_eq!(error.code, "invalid_request");
    assert!(server.requests().is_empty());
}

/** A HEAD answer ends at its headers: Content-Length describes the GET and no body is read. */
#[test]
fn a_head_response_has_no_body_whatever_its_content_length() {
    let server = Server::start(|head, _| {
        if head.starts_with("HEAD") {
            b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 5\r\n\r\n".to_vec()
        } else {
            html("", b"hello")
        }
    });
    let start = Instant::now();
    let response = send(
        &server,
        request(Method::Head, &server.url("/"), &server.origin, ""),
    )
    .expect("HEAD");
    assert_eq!((response.status, response.source.as_str()), (200, ""));
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "no wait for a body"
    );
}

/** A HEAD's empty answer never stands in for the GET's cached body. */
#[test]
fn a_head_is_not_cached_as_the_get() {
    let server = Server::start(|_, index| {
        html(
            "Cache-Control: max-age=600\r\n",
            format!("v{index}").as_bytes(),
        )
    });
    let url = server.url("/page");
    send(&server, request(Method::Head, &url, &server.origin, "")).expect("HEAD");
    let get = send(&server, request(Method::Get, &url, &server.origin, "")).expect("GET");
    assert_eq!(get.source, "v1");
}

/** PUT and DELETE are never cached and drop the GET cached for their URL. */
#[test]
fn put_and_delete_invalidate_the_cached_get_and_are_never_cached() {
    let server = Server::start(|_, index| {
        html(
            "Cache-Control: max-age=600\r\n",
            format!("v{index}").as_bytes(),
        )
    });
    let url = server.url("/item");
    let get = || request(Method::Get, &url, &server.origin, "");
    assert_eq!(send(&server, get()).unwrap().source, "v0");
    assert_eq!(send(&server, get()).unwrap().source, "v0", "cached");
    let put = || request(Method::Put, &url, &server.origin, "a");
    assert_eq!(send(&server, put()).unwrap().source, "v1");
    assert_eq!(
        send(&server, put()).unwrap().source,
        "v2",
        "PUT is not cached"
    );
    assert_eq!(
        send(&server, get()).unwrap().source,
        "v3",
        "PUT invalidated"
    );
    let delete = request(Method::Delete, &url, &server.origin, "");
    assert_eq!(send(&server, delete).unwrap().source, "v4");
    assert_eq!(
        send(&server, get()).unwrap().source,
        "v5",
        "DELETE invalidated"
    );
}

/** Unsafe methods never take a pooled connection and ask the server to close theirs. */
#[test]
fn unsafe_methods_use_a_fresh_connection_and_close_it() {
    let server = Server::start(|_, _| html("", b"x"));
    let url = server.url("/a");
    send(&server, request(Method::Get, &url, &server.origin, "")).expect("GET");
    for (index, method) in UNSAFE.into_iter().enumerate() {
        send(&server, request(method, &url, &server.origin, "")).expect("sent");
        assert_eq!(server.connections(), index + 2);
        let head = server.requests().pop().expect("received");
        assert_eq!(header(&head, "connection").as_deref(), Some("close"));
    }
}

/** Reads one request head; false at EOF. */
fn read_head(stream: &mut std::net::TcpStream) -> bool {
    let mut head = Vec::new();
    let mut byte = [0; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if std::io::Read::read(stream, &mut byte).unwrap_or(0) == 0 {
            return false;
        }
        head.push(byte[0]);
    }
    true
}

/**
 * The server answers the first GET and keeps that connection, then hangs up on every later
 * request without answering. A GET would be resent; an unsafe method fails once, unretried.
 */
#[test]
fn unsafe_methods_are_never_retried_when_the_server_hangs_up() {
    for method in UNSAFE {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
        let origin = format!("http://{}", listener.local_addr().expect("address"));
        let worker = std::thread::spawn(move || {
            let mut accepted = 0;
            let mut kept = Vec::new();
            listener.set_nonblocking(true).expect("nonblocking");
            let until = Instant::now() + Duration::from_secs(1);
            while Instant::now() < until {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                };
                stream.set_nonblocking(false).expect("blocking");
                accepted += 1;
                if read_head(&mut stream) && accepted == 1 {
                    std::io::Write::write_all(&mut stream, &html("", b"ok")).expect("response");
                    kept.push(stream);
                }
            }
            accepted
        });
        let grants = vec![origin.clone()];
        let url = format!("{origin}/");
        let first = Prepared::connect(WebRequest::get(&url), &grants, REMAINING_MS)
            .and_then(Prepared::transmit);
        assert!(first.is_ok());
        let unsafe_request = WebRequest {
            method: method.clone(),
            ..WebRequest::get(&url)
        };
        let sent =
            Prepared::connect(unsafe_request, &grants, REMAINING_MS).and_then(Prepared::transmit);
        assert!(sent.is_err(), "{method:?} fails");
        assert_eq!(worker.join().expect("server"), 2, "{method:?} sent once");
    }
}
