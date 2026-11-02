/** Keep-alive connection reuse, its refusals, idle-close recovery and parallel batch fetches. */
/** Shared loopback server; each test binary uses a different part of it. */
#[allow(dead_code)]
mod support;
use mars_relay::{Method, Prepared, WebRequest, fetch_batch};
use std::time::{Duration, Instant};
use support::{REMAINING_MS, Server, header, html};

/** One prepare and transmit with the server's own grant; returns the body. */
fn fetch(server: &Server, request: WebRequest) -> String {
    Prepared::connect(request, &server.grants(), REMAINING_MS)
        .and_then(Prepared::transmit)
        .expect("fetch")
        .source
}

/** Sequential GETs to one origin share a single connection and advertise keep-alive. */
#[test]
fn sequential_gets_reuse_one_connection() {
    let server = Server::start(|_, index| html("", format!("v{index}").as_bytes()));
    for index in 0..3 {
        assert_eq!(
            fetch(&server, WebRequest::get(&server.url("/a"))),
            format!("v{index}")
        );
    }
    assert_eq!(server.connections(), 1);
    assert_eq!(
        header(&server.requests()[0], "connection").as_deref(),
        Some("keep-alive")
    );
}

/** A response saying Connection: close is never reused. */
#[test]
fn connection_close_responses_are_not_reused() {
    let server = Server::start(|_, _| html("Connection: close\r\n", b"x"));
    fetch(&server, WebRequest::get(&server.url("/a")));
    fetch(&server, WebRequest::get(&server.url("/a")));
    assert_eq!(server.connections(), 2);
}

/** HTTP/1.0 responses and connection-delimited bodies end with the connection. */
#[test]
fn http_1_0_and_unframed_bodies_are_not_reused() {
    let old = Server::start(|_, _| {
        b"HTTP/1.0 200 OK\r\nContent-Type: text/html\r\nContent-Length: 1\r\n\r\nx".to_vec()
    });
    fetch(&old, WebRequest::get(&old.url("/")));
    fetch(&old, WebRequest::get(&old.url("/")));
    assert_eq!(old.connections(), 2);
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
 * A server that answers once per connection, then reads the next request and hangs up without
 * answering, is the idle-close race at its worst: the reused write succeeds and the read sees
 * EOF. Each GET is resent once on a fresh connection and succeeds.
 */
#[test]
fn a_reused_connection_closed_before_answering_is_replaced_once() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
    let origin = format!("http://{}", listener.local_addr().expect("address"));
    let worker = std::thread::spawn(move || {
        for (index, stream) in listener.incoming().take(3).enumerate() {
            let mut stream = stream.expect("connection");
            assert!(read_head(&mut stream));
            std::io::Write::write_all(&mut stream, &html("", b"ok")).expect("response");
            // The last connection stays pooled by the client; waiting on it would never end.
            if index < 2 {
                read_head(&mut stream);
            }
        }
    });
    let grants = vec![origin.clone()];
    for _ in 0..3 {
        let response = Prepared::connect(
            WebRequest::get(&format!("{origin}/")),
            &grants,
            REMAINING_MS,
        )
        .and_then(Prepared::transmit)
        .expect("each GET succeeds on a replacement connection");
        assert_eq!(response.source, "ok");
    }
    worker.join().expect("server");
}

/** POST never takes a pooled connection and asks the server to close. */
#[test]
fn post_uses_a_fresh_connection_and_closes_it() {
    let server = Server::start(|_, _| html("", b"x"));
    fetch(&server, WebRequest::get(&server.url("/a")));
    let post = WebRequest {
        method: Method::Post,
        body: "a=1".into(),
        ..WebRequest::get(&server.url("/a"))
    };
    fetch(&server, post);
    assert_eq!(server.connections(), 2);
    assert_eq!(
        header(&server.requests()[1], "connection").as_deref(),
        Some("close")
    );
}

/** 204 and 304 carry no body, so a kept-alive connection does not wait for one. */
#[test]
fn bodyless_statuses_do_not_wait_for_a_body() {
    let server = Server::start(|_, _| {
        b"HTTP/1.1 204 No Content\r\nContent-Type: text/html\r\n\r\n".to_vec()
    });
    let start = Instant::now();
    let response = Prepared::connect(
        WebRequest::get(&server.url("/")),
        &server.grants(),
        REMAINING_MS,
    )
    .and_then(Prepared::transmit)
    .expect("204");
    assert_eq!(response.status, 204);
    assert!(start.elapsed() < Duration::from_secs(2));
    fetch(&server, WebRequest::get(&server.url("/")));
    assert_eq!(server.connections(), 1);
}

/** A batch runs requests concurrently and returns results in request order. */
#[test]
fn batch_fetches_run_in_parallel_and_keep_order() {
    const DELAY: Duration = Duration::from_millis(200);
    let server = Server::start(|head, _| {
        std::thread::sleep(DELAY);
        let path = head.split(' ').nth(1).unwrap_or_default().to_owned();
        html("", path.as_bytes())
    });
    let requests: Vec<_> = (0..6)
        .map(|index| WebRequest::get(&server.url(&format!("/{index}"))))
        .collect();
    let start = Instant::now();
    let results = fetch_batch(requests, &server.grants(), REMAINING_MS);
    assert!(start.elapsed() < DELAY * 3, "six requests overlap");
    let bodies: Vec<_> = results
        .into_iter()
        .map(|result| result.expect("fetch").source)
        .collect();
    assert_eq!(bodies, ["/0", "/1", "/2", "/3", "/4", "/5"]);
}

/** Batches refuse POST and report each failure in its own slot. */
#[test]
fn batch_refuses_post_per_request() {
    let server = Server::start(|_, _| html("", b"x"));
    let post = WebRequest {
        method: Method::Post,
        body: "a=1".into(),
        ..WebRequest::get(&server.url("/"))
    };
    let results = fetch_batch(
        vec![WebRequest::get(&server.url("/")), post],
        &server.grants(),
        REMAINING_MS,
    );
    assert!(results[0].is_ok());
    assert_eq!(
        results[1].as_ref().expect_err("POST").code,
        "invalid_request"
    );
}
