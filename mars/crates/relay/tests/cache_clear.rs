/** Clearing the HTTP cache; its own binary, because the cache is process-wide. */
/** Shared loopback server; each test binary uses a different part of it. */
#[allow(dead_code)]
mod support;
use mars_relay::{Prepared, WebRequest};
use support::{REMAINING_MS, Server, html};

/** One partitioned GET's body. */
fn source(server: &Server, url: &str) -> String {
    let request = WebRequest {
        top_level: Some(server.origin.clone()),
        ..WebRequest::get(url)
    };
    Prepared::connect(request, &server.grants(), REMAINING_MS)
        .and_then(Prepared::transmit)
        .expect("fetch")
        .source
}

/** Clearing the cache makes the next visit go back to the network. */
#[test]
fn clearing_the_cache_refetches() {
    let server = Server::start(|_, index| {
        html(
            "Cache-Control: max-age=600\r\n",
            format!("v{index}").as_bytes(),
        )
    });
    let url = server.url("/cleared");
    assert_eq!(source(&server, &url), "v0");
    assert_eq!(source(&server, &url), "v0", "served from the cache");
    mars_relay::clear_cache();
    assert_eq!(source(&server, &url), "v1");
}
