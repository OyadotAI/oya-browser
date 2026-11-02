/**
 * Request bodies reach the socket byte for byte: a body that is not UTF-8 (a protobuf, as YouTube's SABR requests
 * carry) and a text body both arrive at a raw loopback listener exactly as the request held them. The shared test
 * server discards bodies, so this one keeps them.
 */
#[allow(dead_code)]
mod support;
use mars_relay::{Method, Prepared, WebRequest};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use support::REMAINING_MS;

/// The response the listener answers every request with.
const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";

/** A loopback listener that keeps the raw body of the one request it serves: (origin, the body once read). */
fn raw_listener() -> (String, Arc<Mutex<Option<Vec<u8>>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let origin = format!("http://{}", listener.local_addr().expect("address"));
    let body = Arc::new(Mutex::new(None));
    let kept = Arc::clone(&body);
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let raw = read_body(&mut stream);
        *kept.lock().expect("body") = Some(raw);
        stream.write_all(OK).expect("respond");
    });
    (origin, body)
}

/** The body of one request: the head is read to its end, then Content-Length bytes. */
fn read_body(stream: &mut TcpStream) -> Vec<u8> {
    let mut head = Vec::new();
    let mut byte = [0; 1];
    while !head.ends_with(b"\r\n\r\n") {
        assert_ne!(
            stream.read(&mut byte).expect("read"),
            0,
            "request cut short"
        );
        head.push(byte[0]);
    }
    let length = String::from_utf8_lossy(&head)
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|value| value.trim().to_owned())
        })
        .and_then(|value| value.parse::<usize>().ok())
        .expect("a Content-Length");
    let mut body = vec![0; length];
    stream.read_exact(&mut body).expect("body");
    body
}

/** POST `body` to the listener and return what it received. */
fn post(body: Vec<u8>) -> Vec<u8> {
    let (origin, received) = raw_listener();
    let request = WebRequest {
        method: Method::Post,
        body,
        ..WebRequest::get(&format!("{origin}/upload"))
    };
    let response = Prepared::connect(request, &[origin], REMAINING_MS)
        .and_then(Prepared::transmit)
        .expect("post");
    assert_eq!(response.source, "ok");
    received.lock().expect("body").clone().expect("a body")
}

/** Bytes that are not UTF-8 arrive unchanged, every one of them. */
#[test]
fn a_binary_body_arrives_byte_for_byte() {
    let body = vec![8, 0xff, 0, 0x80, 0xc3, 0x28, 10, 0xfe];
    assert_eq!(post(body.clone()), body);
}

/** A text body arrives as its UTF-8 bytes. */
#[test]
fn a_text_body_arrives_as_its_utf8_bytes() {
    let body = "field=v\u{e9}lue".as_bytes().to_vec();
    assert_eq!(post(body.clone()), body);
}
