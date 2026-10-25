/** Round-5 "network" port: Chromium's localhost names (net::IsLocalHostname and the host
resolver's ResolveLocalHostname). `localhost` and every `*.localhost` name mean loopback without
DNS, so WPT's www1/www2 subdomains reach the local server, and only under the exact fixture
grant, like a literal loopback address. */
#[allow(dead_code)]
mod support;
use mars_relay::policy::{fixture_granted, local_hostname};
use url::Url;

#[test]
fn localhost_and_its_subdomains_are_local_names() {
    for host in [
        "localhost",
        "LocalHost",
        "localhost.",
        "www1.localhost",
        "a.b.localhost.",
    ] {
        assert!(local_hostname(host), "{host}");
    }
    for host in [
        "localhost.example",
        "notlocalhost",
        "localhostx",
        "127.0.0.1",
        "example.com",
    ] {
        assert!(!local_hostname(host), "{host}");
    }
}

#[test]
fn a_localhost_name_is_granted_only_for_its_exact_origin() {
    let url = Url::parse("http://www1.localhost:8001/x").unwrap();
    assert!(fixture_granted(
        &url,
        &["http://www1.localhost:8001".into()]
    ));
    assert!(!fixture_granted(&url, &["http://localhost:8001".into()]));
    assert!(!fixture_granted(&url, &[]));
}

#[test]
fn a_granted_localhost_subdomain_reaches_the_loopback_server() {
    let server = support::Server::start(|_, _| {
        b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 2\r\n\r\nok".to_vec()
    });
    let port = server.origin.rsplit(':').next().unwrap();
    let origin = format!("http://www1.localhost:{port}");
    let page = mars_relay::fetch_web(&format!("{origin}/"), std::slice::from_ref(&origin)).unwrap();
    assert_eq!(page.source, "ok");
    assert!(mars_relay::fetch_web(&format!("{origin}/"), &[]).is_err());
}
