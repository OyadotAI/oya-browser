/** URL policy is pure and never resolves names or opens a network connection. */
use mars::constants::URL_BYTES;
use mars::network::Endpoint;

/** Only the exact advertised loopback spellings reach a socket address. */
#[test]
fn only_explicit_literal_loopback_http_is_allowed() {
    for url in [
        "http://127.0.0.1:80/fixture?q=x",
        "http://[::1]:8080/fixture",
    ] {
        let endpoint = Endpoint::parse(url).unwrap();
        assert!(endpoint.address.ip().is_loopback());
        assert!(endpoint.request().contains("Connection: close\r\n"));
    }
    for url in [
        "https://127.0.0.1:80/",
        "http://localhost:80/",
        "http://127.1:80/",
        "http://2130706433:80/",
        "http://127.0.0.1:0/",
        "http://127.0.0.1:65536/",
        "http://127.0.0.1/",
        "http://127.0.0.1:80",
        "http://user@127.0.0.1:80/",
        "http://169.254.169.254:80/",
        "http://[::ffff:127.0.0.1]:80/",
        "http://127.0.0.1:80/#fragment",
        "http://127.0.0.1:80/\r\nHost: evil",
        "http://127.0.0.1:80/a b",
    ] {
        assert_eq!(
            Endpoint::parse(url).err().unwrap().code,
            "permission_denied",
            "{url:?}"
        );
    }
}

/** Large URLs are refused before parsing or allocating request headers. */
#[test]
fn url_bytes_are_bounded() {
    assert_eq!(
        Endpoint::parse(&"x".repeat(URL_BYTES + 1))
            .err()
            .unwrap()
            .code,
        "limit_exceeded"
    );
}
