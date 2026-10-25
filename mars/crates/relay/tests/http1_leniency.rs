/** Round-6 "cors" owner ports: Chromium's HTTP/1 response leniency (bare-LF lines, any value
byte but NUL, HTTP-whitespace-only trimming), Fetch's tab-or-space trim in CORS lists, and the
headers-only preflight read (WPT cors/response-headers, access-control-expose-headers-parsing,
fetch/api/cors/cors-preflight-unterminated-body). */
use mars_relay::cors::{self, Tainting};
use mars_relay::{Destination, Preflight, Prepared, WebRequest, WebResponse};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

/** Generous preparation budget for loopback. */
const PREPARATION_MS: u64 = 10_000;

/** Serves `response` to one connection, then holds the socket open for `hold`. */
fn serve(response: Vec<u8>, hold: Duration) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut byte = [0; 1];
        while !request.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 {
            request.push(byte[0]);
        }
        stream.write_all(&response).unwrap();
        std::thread::sleep(hold);
    });
    origin
}

/** One script-style request to `origin`, as the coordinator sends it. */
fn send(origin: &str, preflight: Option<Preflight>) -> WebResponse {
    let request = WebRequest {
        destination: Destination::Empty,
        preflight,
        ..WebRequest::get(&format!("{origin}/"))
    };
    Prepared::connect(request, &[origin.to_owned()], PREPARATION_MS)
        .and_then(Prepared::transmit)
        .unwrap()
}

/** The joined value of `name`. */
fn header<'a>(response: &'a WebResponse, name: &str) -> Option<&'a str> {
    response
        .headers
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.as_str())
}

#[test]
fn bare_lf_header_lines_parse_and_repeats_join() {
    let raw = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 4\nX-Custom: test\r\nX-Custom: test\r\n\r\nTEST";
    let response = send(&serve(raw.to_vec(), Duration::ZERO), None);
    assert_eq!(response.source, "TEST");
    assert_eq!(header(&response, "x-custom"), Some("test, test"));
}

#[test]
fn a_form_feed_in_a_value_is_kept_and_spoils_the_expose_token() {
    let raw = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nAccess-Control-Expose-Headers: bb-8\x0c\r\nbb-8: hey\r\nContent-Length: 2\r\n\r\nok";
    let response = send(&serve(raw.to_vec(), Duration::ZERO), None);
    assert_eq!(
        header(&response, "access-control-expose-headers"),
        Some("bb-8\x0c")
    );
    let exposed = cors::exposed(&response.headers, Tainting::Cors, false);
    assert!(!exposed.iter().any(|(name, _)| name == "bb-8"));
    let listed = vec![(
        "access-control-expose-headers".to_owned(),
        " bb-8\t".to_owned(),
    )];
    let mut headers = listed.clone();
    headers.push(("bb-8".into(), "hey".into()));
    assert!(
        cors::exposed(&headers, Tainting::Cors, false)
            .iter()
            .any(|(n, _)| n == "bb-8")
    );
}

#[test]
fn a_preflight_reads_headers_only_so_an_unending_body_cannot_stall_it() {
    let raw =
        b"HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: 1000\r\n\r\npartial"
            .to_vec();
    let origin = serve(raw, Duration::from_secs(5));
    let started = Instant::now();
    let response = send(
        &origin,
        Some(Preflight {
            method: "PUT".into(),
            headers: String::new(),
        }),
    );
    assert_eq!(response.status, 200);
    assert!(started.elapsed() < Duration::from_secs(4));
}
