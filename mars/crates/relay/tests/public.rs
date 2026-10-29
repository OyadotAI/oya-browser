/** Independent public consumers pin Relay policy and request failures without network I/O. */
use mars_relay::{CookieJar, Method, Prepared, WebRequest, fetch, fetch_web, policy};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::SystemTime;

/** Rejected literal targets must fail before opening a socket or resolving a name. */
const PREPARATION_MS: u64 = 1_000;

/** Public request construction and literal-address policy require no browser runtime. */
#[test]
fn public_request_and_policy_are_usable_without_the_browser() {
    let request = WebRequest::get("https://1.1.1.1/document?view=plain");
    request.validate().expect("bounded navigation request");
    let url = policy::web_url(&request.url).expect("public HTTPS URL syntax");
    let address = url.host_str().expect("literal host").parse().expect("IP");
    assert!(policy::public_address(address));
    assert_eq!(url.path(), "/document");
    assert_eq!(request.method, Method::Get);
}

/** Credential refusal happens before the transport can resolve or connect. */
#[test]
fn credential_urls_are_rejected_before_preparation() {
    let error = Prepared::connect(
        WebRequest::get("https://user:secret@1.1.1.1/"),
        &[],
        PREPARATION_MS,
    )
    .err()
    .expect("credential URL must be refused before connection");
    assert_eq!(error.code, "permission_denied");
    assert!(!error.message.contains("secret"));
}

/** Literal private addresses fail locally, without fixture grants or network effects. */
#[test]
fn private_literal_targets_without_grants_are_rejected_before_connection() {
    for url in ["http://127.0.0.1:1/", "http://10.0.0.1/", "http://[::1]:1/"] {
        let error = Prepared::connect(WebRequest::get(url), &[], PREPARATION_MS)
            .err()
            .expect("private address must be refused before connection");
        assert_eq!(error.code, "permission_denied", "{url}");
    }
}

/** An origin grant does not authorize another loopback address or port. */
#[test]
fn fixture_grants_apply_only_to_the_exact_loopback_origin() {
    let url = policy::web_url("http://127.0.0.1:8123/document").expect("fixture syntax");
    assert!(policy::fixture_granted(
        &url,
        &["http://127.0.0.1:8123".into()]
    ));
    assert!(!policy::fixture_granted(
        &url,
        &["http://127.0.0.1:8124".into()]
    ));
    assert!(!policy::fixture_granted(
        &url,
        &["http://[::1]:8123".into()]
    ));
}

/** Malformed request fields cannot become an injected HTTP header or GET submission. */
#[test]
fn request_validation_rejects_header_injection_and_get_bodies() {
    let mut request = WebRequest::get("https://1.1.1.1/");
    request.cookie = "session=x\r\nInjected: yes".into();
    assert_eq!(
        request.validate().expect_err("header injection").code,
        "invalid_request"
    );
    request.cookie.clear();
    request.body = "field=value".into();
    assert_eq!(
        request.validate().expect_err("GET body").code,
        "invalid_request"
    );
}

/** The library's pure cookie record API neither opens Keychain nor weakens Secure matching. */
#[test]
fn cookies_round_trip_through_pure_records_without_profile_io() {
    let url = policy::web_url("https://1.1.1.1/document").expect("HTTPS URL");
    let now = SystemTime::UNIX_EPOCH;
    let mut jar = CookieJar::default();
    assert!(
        jar.receive(&url, &["session=value; Secure; Path=/".into()], now)
            .is_empty()
    );
    let record = jar.profile_record().expect("bounded cookie record");
    let mut restored = CookieJar::from_profile(&record, now).expect("complete valid record");
    assert_eq!(
        restored.header(&url, Some(&url), true, now),
        "session=value"
    );
    let insecure = policy::web_url("http://1.1.1.1/").expect("HTTP URL");
    assert!(
        restored
            .header(&insecure, Some(&insecure), true, now)
            .is_empty()
    );
}

/** Navigation, subresource and loopback fetches each open one connection. */
const PATHS: usize = 3;

/** Serves one raw response to exactly `PATHS` loopback connections. */
fn serve(response: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback listener");
    let origin = format!("http://{}", listener.local_addr().expect("address"));
    let worker = std::thread::spawn(move || {
        for stream in listener.incoming().take(PATHS) {
            let mut stream = stream.expect("connection");
            let mut request = Vec::new();
            let mut byte = [0; 1];
            while !request.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 {
                request.push(byte[0]);
            }
            stream.write_all(&response).expect("response");
        }
    });
    (origin, worker)
}

/** Every fetch path parses Content-Type and decodes charsets identically. */
#[test]
fn content_type_verdicts_match_on_every_fetch_path() {
    let accepted = "\u{e9}".as_bytes();
    for (content_type, body, expected) in [
        ("text/html;charset=UTF-8", accepted, Ok("\u{e9}")),
        ("TEXT/HTML; Charset=\"utf-8\"", accepted, Ok("\u{e9}")),
        (
            "text/html; charset=windows-1252",
            b"\x80\x93\xE9".as_slice(),
            Ok("\u{20AC}\u{201C}\u{e9}"),
        ),
        (
            "text/html; charset=shift_jis",
            b"\x82\xA0".as_slice(),
            Ok("\u{3042}"),
        ),
        (
            "text/html; charset=nope",
            accepted,
            Err("unsupported_feature"),
        ),
    ] {
        let mut response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        response.extend_from_slice(body);
        let (origin, worker) = serve(response);
        let url = format!("{origin}/");
        let grants = std::slice::from_ref(&origin);
        let navigation = fetch_web(&url, grants).map(|page| page.source);
        let subresource = Prepared::connect(WebRequest::get(&url), grants, PREPARATION_MS)
            .and_then(Prepared::transmit)
            .map(|response| {
                assert_eq!(response.mime, "text/html", "{content_type}");
                response.source
            });
        let loopback = fetch(&url);
        worker.join().expect("fixture");
        for result in [navigation, subresource, loopback] {
            assert_eq!(
                result.as_deref().map_err(|error| error.code.as_str()),
                expected,
                "{content_type}"
            );
        }
    }
}

/** A `data:` resource (facebook.com links its CSS and scripts that way) is refused as
unsupported, so a page loader skips that one resource instead of abandoning the navigation as an
egress denial. It is still never fetched, and a redirect to one is still refused. */
#[test]
fn data_urls_are_unsupported_resources_rather_than_egress_denials() {
    let data = "data:text/css; charset=utf-8,.a%7Bcolor%3Ared%7D";
    assert_eq!(
        policy::web_url(data).expect_err("inline").code,
        "unsupported_feature"
    );
    let error = Prepared::connect(WebRequest::get(data), &[], PREPARATION_MS)
        .err()
        .expect("never fetched");
    assert_eq!(error.code, "unsupported_feature");
    let page = url::Url::parse("https://1.1.1.1/").expect("url");
    assert!(mars_relay::redirect_url(&page, data, &[]).is_err());
    for other in [
        "file:///etc/passwd",
        "javascript:alert(1)",
        "ftp://1.1.1.1/",
    ] {
        assert_eq!(
            policy::web_url(other).expect_err("denied").code,
            "permission_denied"
        );
    }
}

/** Budget a caller gives a request to a server that never answers. */
const STALLED_MS: u64 = 1_000;
/** Parallel test load may delay the observing thread after Relay's own timeout fires. */
const LOAD_SLACK_MS: u64 = 500;

/** Relay reports its own timeout while the fixture is still stalled, with room for scheduler
delay in a parallel test run. The production reply margin still protects the caller's deadline. */
#[test]
fn a_stalled_server_times_out_before_the_callers_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback listener");
    let origin = format!("http://{}", listener.local_addr().expect("address"));
    let holder = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("connection");
        std::thread::sleep(std::time::Duration::from_millis(STALLED_MS * 2));
        drop(stream);
    });
    let started = std::time::Instant::now();
    let error = Prepared::connect(
        WebRequest::get(&format!("{origin}/")),
        std::slice::from_ref(&origin),
        STALLED_MS,
    )
    .and_then(Prepared::transmit)
    .expect_err("stalled");
    let elapsed = started.elapsed();
    assert_eq!(error.code, "timeout");
    assert!(error.service.is_none());
    assert!(
        elapsed < std::time::Duration::from_millis(STALLED_MS + LOAD_SLACK_MS),
        "{elapsed:?}"
    );
    holder.join().expect("holder");
}
