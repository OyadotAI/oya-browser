/** The private HTTP cache's storage, freshness, revalidation, Vary and partition rules over loopback. */
/** Shared loopback server; each test binary uses a different part of it. */
#[allow(dead_code)]
mod support;
use mars_relay::{Method, Prepared, WebRequest, WebResponse};
use support::{REMAINING_MS, Server, header, html};

/** A GET partitioned under `top`, as the coordinator sends it once wired. */
fn get(url: &str, top: &str) -> WebRequest {
    WebRequest {
        top_level: Some(top.into()),
        ..WebRequest::get(url)
    }
}

/** One prepare and transmit with the server's own grant. */
fn fetch(server: &Server, request: WebRequest) -> WebResponse {
    Prepared::connect(request, &server.grants(), REMAINING_MS)
        .and_then(Prepared::transmit)
        .expect("fetch")
}

/** Fetches `path` twice under one partition and returns how many requests reached the server. */
fn twice(extra: &'static str) -> usize {
    let server = Server::start(move |_, index| html(extra, format!("v{index}").as_bytes()));
    let url = server.url("/page");
    let first = fetch(&server, get(&url, &server.origin));
    let second = fetch(&server, get(&url, &server.origin));
    assert_eq!(first.source, "v0");
    if server.requests().len() == 1 {
        assert_eq!(second.source, "v0", "a cache hit serves the stored body");
    }
    server.requests().len()
}

/** max-age keeps a response fresh, so the second visit sends nothing. */
#[test]
fn fresh_max_age_responses_are_served_without_the_network() {
    assert_eq!(twice("Cache-Control: max-age=600\r\n"), 1);
}

/** no-store is never stored. */
#[test]
fn no_store_responses_are_refetched() {
    assert_eq!(twice("Cache-Control: no-store, max-age=600\r\n"), 2);
}

/** A private cache stores `private` responses. */
#[test]
fn private_responses_are_cached() {
    assert_eq!(twice("Cache-Control: private, max-age=600\r\n"), 1);
}

/** s-maxage is a shared-cache directive; alone it gives this cache no freshness. */
#[test]
fn s_maxage_alone_does_not_make_a_response_fresh() {
    assert_eq!(twice("Cache-Control: s-maxage=600\r\n"), 2);
}

/** An Age at or past max-age makes a response stale on arrival. */
#[test]
fn age_past_max_age_is_stale() {
    assert_eq!(twice("Cache-Control: max-age=60\r\nAge: 61\r\n"), 2);
}

/** Expires in the future is fresh; a past or malformed Expires is stale. */
#[test]
fn expires_sets_freshness_from_date() {
    let now = std::time::SystemTime::now();
    let later = httpdate::fmt_http_date(now + std::time::Duration::from_secs(600));
    let earlier = httpdate::fmt_http_date(now - std::time::Duration::from_secs(600));
    let future: &'static str = Box::leak(format!("Expires: {later}\r\n").into_boxed_str());
    let past: &'static str = Box::leak(format!("Expires: {earlier}\r\n").into_boxed_str());
    assert_eq!(twice(future), 1);
    assert_eq!(twice(past), 2);
    assert_eq!(twice("Expires: 0\r\n"), 2);
}

/** Without explicit freshness, a 200 is fresh for a tenth of its Last-Modified age. */
#[test]
fn last_modified_gives_ten_percent_heuristic_freshness() {
    let now = std::time::SystemTime::now();
    let old = httpdate::fmt_http_date(now - std::time::Duration::from_secs(86_400));
    let recent = httpdate::fmt_http_date(now);
    let old: &'static str = Box::leak(format!("Last-Modified: {old}\r\n").into_boxed_str());
    let recent: &'static str = Box::leak(format!("Last-Modified: {recent}\r\n").into_boxed_str());
    assert_eq!(twice(old), 1, "a day old: fresh for 2.4 hours");
    assert_eq!(
        twice(recent),
        2,
        "modified just now: no heuristic freshness"
    );
}

/** no-cache stores the body but revalidates it with If-None-Match; a 304 reuses the body. */
#[test]
fn no_cache_revalidates_with_etag_and_304_reuses_the_body() {
    let server = Server::start(|head, _| {
        if header(head, "if-none-match").as_deref() == Some("\"v1\"") {
            // Real servers repeat Content-Encoding on a 304; there is still nothing to decode.
            b"HTTP/1.1 304 Not Modified\r\nETag: \"v1\"\r\nContent-Encoding: gzip\r\nCache-Control: no-cache\r\n\r\n".to_vec()
        } else {
            html("Cache-Control: no-cache\r\nETag: \"v1\"\r\n", b"body")
        }
    });
    let url = server.url("/page");
    assert_eq!(fetch(&server, get(&url, &server.origin)).source, "body");
    let second = fetch(&server, get(&url, &server.origin));
    assert_eq!((second.status, second.source.as_str()), (200, "body"));
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(header(&requests[0], "if-none-match"), None);
    assert_eq!(
        header(&requests[1], "if-none-match").as_deref(),
        Some("\"v1\"")
    );
}

/** A stale entry with Last-Modified revalidates with If-Modified-Since; a 200 replaces it. */
#[test]
fn stale_last_modified_entries_revalidate_and_a_200_replaces_them() {
    let modified = "Last-Modified: Mon, 01 Jan 2024 00:00:00 GMT\r\n";
    let server = Server::start(move |_, index| {
        let extra = format!("Cache-Control: max-age=0\r\n{modified}");
        html(&extra, format!("v{index}").as_bytes())
    });
    let url = server.url("/page");
    fetch(&server, get(&url, &server.origin));
    assert_eq!(fetch(&server, get(&url, &server.origin)).source, "v1");
    let requests = server.requests();
    assert_eq!(
        header(&requests[1], "if-modified-since").as_deref(),
        Some("Mon, 01 Jan 2024 00:00:00 GMT")
    );
}

/** A 304 refreshes freshness from its own headers, so the next use needs no request. */
#[test]
fn a_304_with_max_age_makes_the_entry_fresh_again() {
    let server = Server::start(|head, _| {
        if header(head, "if-none-match").is_some() {
            b"HTTP/1.1 304 Not Modified\r\nCache-Control: max-age=600\r\n\r\n".to_vec()
        } else {
            html("Cache-Control: max-age=0\r\nETag: \"a\"\r\n", b"body")
        }
    });
    let url = server.url("/page");
    for _ in 0..3 {
        assert_eq!(fetch(&server, get(&url, &server.origin)).source, "body");
    }
    assert_eq!(server.requests().len(), 2);
}

/** Vary: Cookie stores one variant per cookie value. */
#[test]
fn vary_stores_one_variant_per_request_value() {
    let server = Server::start(|head, _| {
        let cookie = header(head, "cookie").unwrap_or_default();
        html(
            "Cache-Control: max-age=600\r\nVary: Cookie\r\n",
            cookie.as_bytes(),
        )
    });
    let url = server.url("/page");
    let with = |cookie: &str| WebRequest {
        cookie: cookie.into(),
        ..get(&url, &server.origin)
    };
    assert_eq!(fetch(&server, with("a=1")).source, "a=1");
    assert_eq!(fetch(&server, with("a=2")).source, "a=2");
    assert_eq!(fetch(&server, with("a=1")).source, "a=1");
    assert_eq!(fetch(&server, with("a=2")).source, "a=2");
    assert_eq!(server.requests().len(), 2);
}

/** Vary: * can never match, so it is not stored. */
#[test]
fn vary_star_is_not_cached() {
    assert_eq!(twice("Cache-Control: max-age=600\r\nVary: *\r\n"), 2);
}

/** A cache hit never replays Set-Cookie; cookies come only from network responses. */
#[test]
fn cached_responses_carry_no_set_cookie() {
    let server =
        Server::start(|_, _| html("Cache-Control: max-age=600\r\nSet-Cookie: id=1\r\n", b"x"));
    let url = server.url("/page");
    assert_eq!(
        fetch(&server, get(&url, &server.origin)).set_cookies,
        ["id=1"]
    );
    assert!(
        fetch(&server, get(&url, &server.origin))
            .set_cookies
            .is_empty()
    );
    assert_eq!(server.requests().len(), 1);
}

/** The same URL under another top-level site is a separate entry. */
#[test]
fn entries_are_partitioned_by_top_level_site() {
    let server = Server::start(|_, index| {
        html(
            "Cache-Control: max-age=600\r\n",
            format!("v{index}").as_bytes(),
        )
    });
    let url = server.url("/shared.css");
    assert_eq!(fetch(&server, get(&url, "https://a.example/")).source, "v0");
    assert_eq!(fetch(&server, get(&url, "https://b.example/")).source, "v1");
    assert_eq!(
        fetch(&server, get(&url, "https://a.example/page")).source,
        "v0"
    );
    assert_eq!(server.requests().len(), 2);
}

/** Without a top-level partition the cache is bypassed entirely. */
#[test]
fn unpartitioned_requests_bypass_the_cache() {
    let server = Server::start(|_, _| html("Cache-Control: max-age=600\r\n", b"x"));
    let url = server.url("/page");
    fetch(&server, WebRequest::get(&url));
    fetch(&server, WebRequest::get(&url));
    assert_eq!(server.requests().len(), 2);
}

/** POST responses are never stored, and a POST invalidates the stored GET for that URL. */
#[test]
fn post_is_never_cached_and_invalidates_the_url() {
    let server = Server::start(|_, index| {
        html(
            "Cache-Control: max-age=600\r\n",
            format!("v{index}").as_bytes(),
        )
    });
    let url = server.url("/form");
    let post = || WebRequest {
        method: Method::Post,
        body: "a=1".into(),
        ..get(&url, &server.origin)
    };
    assert_eq!(fetch(&server, get(&url, &server.origin)).source, "v0");
    assert_eq!(fetch(&server, post()).source, "v1");
    assert_eq!(fetch(&server, post()).source, "v2");
    assert_eq!(fetch(&server, get(&url, &server.origin)).source, "v3");
}

/** A cached loopback entry is unreachable once the fixture grant is gone. */
#[test]
fn cached_entries_still_need_the_destination_grant() {
    let server = Server::start(|_, _| html("Cache-Control: max-age=600\r\n", b"x"));
    let url = server.url("/page");
    fetch(&server, get(&url, &server.origin));
    let error = Prepared::connect(get(&url, &server.origin), &[], REMAINING_MS)
        .err()
        .expect("no grant, no cached answer");
    assert_eq!(error.code, "permission_denied");
}

/** A cached document still goes through the media type checks: an image request misses it. */
#[test]
fn cached_documents_are_not_served_to_image_requests() {
    let server = Server::start(|_, _| html("Cache-Control: max-age=600\r\n", b"x"));
    let url = server.url("/page");
    fetch(&server, get(&url, &server.origin));
    let image = WebRequest {
        image: true,
        ..get(&url, &server.origin)
    };
    let error = Prepared::connect(image, &server.grants(), REMAINING_MS)
        .and_then(Prepared::transmit)
        .expect_err("HTML is not an image");
    assert_eq!(error.code, "unsupported_feature");
}

/** Non-200 statuses are not stored. */
#[test]
fn error_statuses_are_not_cached() {
    let server = Server::start(|_, _| {
        b"HTTP/1.1 404 Not Found\r\nContent-Type: text/html\r\nCache-Control: max-age=600\r\nContent-Length: 1\r\n\r\nx".to_vec()
    });
    let url = server.url("/missing");
    assert_eq!(fetch(&server, get(&url, &server.origin)).status, 404);
    fetch(&server, get(&url, &server.origin));
    assert_eq!(server.requests().len(), 2);
}

/** A malformed partition is a faulty coordinator request, refused before any I/O. */
#[test]
fn malformed_partitions_are_refused() {
    let request = get("https://1.1.1.1/", "javascript:alert(1)");
    assert_eq!(
        request.validate().expect_err("bad partition").code,
        "invalid_request"
    );
}
