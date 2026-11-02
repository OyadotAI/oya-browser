/** Hermetic desktop networking tests cover owned framing, policy, redirects and verified TLS. */
#[allow(dead_code)]
mod support;
use mars::network::{
    fetch_web,
    policy::{public_address, web_url},
};
use support::Fixture;

/** Neither alternate address spellings nor URL credentials can bypass the desktop egress policy. */
#[test]
fn public_policy_rejects_reserved_transition_and_credential_destinations() {
    for address in [
        "127.0.0.1",
        "10.0.0.1",
        "100.64.0.1",
        "169.254.169.254",
        "192.0.2.1",
        "224.0.0.1",
        "::1",
        "::ffff:127.0.0.1",
        "2001:db8::1",
        "2002:7f00:1::",
        "3ffe::1",
        "3fff::1",
    ] {
        assert!(!public_address(address.parse().unwrap()), "{address}");
    }
    for address in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111"] {
        assert!(public_address(address.parse().unwrap()));
    }
    for url in [
        "http://@example.com",
        "http://user:pass@example.com",
        "http://example.com:0",
        "file:///tmp/x",
        "http://example.com\\@localhost",
        "http://example.com/\r\nx",
    ] {
        assert!(web_url(url).is_err(), "{url}");
    }
    assert_eq!(
        fetch_web("http://127.1/", &[]).unwrap_err().code,
        "permission_denied"
    );
}
/** Exact startup-origin grants permit the owned fixture, never neighboring loopback services. */
#[test]
fn loopback_requires_an_exact_operator_origin_grant() {
    let body = "<html><body><p>owned</p></body></html>";
    let fixture = Fixture::raw(reply(body));
    assert!(fetch_web(&fixture.url, &[]).is_err());
    assert!(fetch_web(&fixture.url, &[format!("{}/path", fixture.url)]).is_err());
    assert_eq!(
        fetch_web(&fixture.url, std::slice::from_ref(&fixture.url))
            .unwrap()
            .source,
        body
    );
}
/** Chunk sizes and terminators are Oya-owned and bounded independently of decoded HTML. */
#[test]
fn chunked_html_decodes_and_conflicting_or_truncated_framing_fails() {
    let fixture=Fixture::raw(b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nTransfer-Encoding: chunked\r\n\r\n6\r\n<p>ok<\r\n3\r\n/p>\r\n0\r\n\r\n".to_vec());
    assert_eq!(
        fetch_web(&fixture.url, std::slice::from_ref(&fixture.url))
            .unwrap()
            .source,
        "<p>ok</p>"
    );
    for response in [b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nTransfer-Encoding: chunked\r\nContent-Length: 1\r\n\r\n0\r\n\r\n".as_slice(),b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 9\r\n\r\nshort",b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nx"] {let fixture=Fixture::raw(response.to_vec());assert!(fetch_web(&fixture.url,std::slice::from_ref(&fixture.url)).is_err());}
}
/** Redirects cannot grant private network access or loop forever. */
#[test]
fn redirects_revalidate_destinations_and_enforce_the_hop_budget() {
    for location in ["http://169.254.169.254/latest", "/again"] {
        let fixture = Fixture::raw(
            format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\n\r\n")
                .into_bytes(),
        );
        assert!(fetch_web(&fixture.url, std::slice::from_ref(&fixture.url)).is_err());
        assert!(fixture.requests.load(std::sync::atomic::Ordering::Relaxed) <= 6);
    }
}
/** Fixture body framing is kept literal to exercise the real network codec. */
fn reply(body: &str) -> Vec<u8> {
    format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{body}",body.len()).into_bytes()
}

/** Even an exact operator fixture grant never bypasses Rustls certificate validation. */
#[test]
fn untrusted_tls_is_rejected_without_a_verification_bypass() {
    use rustls::{
        ServerConfig, ServerConnection, StreamOwned,
        pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
    };
    use std::{io::Read, net::TcpListener, sync::Arc, thread, time::Duration};
    let cert = CertificateDer::from(include_bytes!("fixtures/untrusted-cert.der").to_vec());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
        include_bytes!("fixtures/untrusted-key.der").to_vec(),
    ));
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("https://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        socket
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut tls = StreamOwned::new(ServerConnection::new(Arc::new(config)).unwrap(), socket);
        let mut byte = [0];
        let _ = tls.read(&mut byte);
    });
    assert_eq!(
        fetch_web(&origin, std::slice::from_ref(&origin))
            .unwrap_err()
            .code,
        "tls_error"
    );
    server.join().unwrap();
}
