/** Cache matching and storage rules, through the model the handlers use. */
use super::*;

/** A GET request for `url` with these headers. */
fn get(url: &str, headers: &[(&str, &str)]) -> CachedRequest {
    CachedRequest {
        url: url.into(),
        method: "GET".into(),
        headers: headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

/** An entry for `request` whose response carries `vary` (if any) and body text. */
fn entry(request: CachedRequest, vary: Option<&str>, body: &str) -> Entry {
    Entry {
        request,
        response: CachedResponse {
            status: 200,
            kind: "basic".into(),
            headers: vary
                .map(|v| vec![("vary".to_string(), v.to_string())])
                .unwrap_or_default(),
            body: crate::runtime::base64(body.as_bytes()),
            ..CachedResponse::default()
        },
    }
}

/** Fragments never matter; the query matters unless ignoreSearch. */
#[test]
fn urls_match_without_fragment_and_optionally_query() {
    let stored = entry(get("https://a.test/x?q=1#top", &[]), None, "");
    assert!(matches(
        &get("https://a.test/x?q=1", &[]),
        &stored,
        Query::default()
    ));
    assert!(!matches(
        &get("https://a.test/x?q=2", &[]),
        &stored,
        Query::default()
    ));
    let ignore = Query {
        ignore_search: true,
        ..Query::default()
    };
    assert!(matches(&get("https://a.test/x", &[]), &stored, ignore));
}

/** Non-GET queries match only with ignoreMethod. */
#[test]
fn methods_other_than_get_need_ignore_method() {
    let stored = entry(get("https://a.test/x", &[]), None, "");
    let mut post = get("https://a.test/x", &[]);
    post.method = "POST".into();
    assert!(!matches(&post, &stored, Query::default()));
    assert!(matches(
        &post,
        &stored,
        Query {
            ignore_method: true,
            ..Query::default()
        }
    ));
}

/** Vary compares the named request headers; `*` never matches; ignoreVary skips the check. */
#[test]
fn vary_headers_must_agree() {
    let stored = entry(
        get("https://a.test/x", &[("Accept", "text/html")]),
        Some("accept"),
        "",
    );
    assert!(matches(
        &get("https://a.test/x", &[("accept", "text/html")]),
        &stored,
        Query::default()
    ));
    assert!(!matches(
        &get("https://a.test/x", &[("accept", "image/png")]),
        &stored,
        Query::default()
    ));
    assert!(matches(
        &get("https://a.test/x", &[]),
        &stored,
        Query {
            ignore_vary: true,
            ..Query::default()
        }
    ));
    let star = entry(get("https://a.test/x", &[]), Some("*"), "");
    assert!(!matches(
        &get("https://a.test/x", &[]),
        &star,
        Query::default()
    ));
}

/** Put replaces the matching entry and keeps order; delete reports whether anything went. */
#[test]
fn put_replaces_and_delete_reports() {
    let mut cache = NamedCache::default();
    cache.put(entry(get("https://a.test/1", &[]), None, "one"));
    cache.put(entry(get("https://a.test/2", &[]), None, "two"));
    cache.put(entry(get("https://a.test/1", &[]), None, "uno"));
    let urls: Vec<_> = cache
        .lookup(None, Query::default())
        .iter()
        .map(|e| e.request.url.clone())
        .collect();
    assert_eq!(urls, ["https://a.test/2", "https://a.test/1"]);
    assert!(cache.delete(&get("https://a.test/2", &[]), Query::default()));
    assert!(!cache.delete(&get("https://a.test/2", &[]), Query::default()));
}

/** Caches are per storage key; open is idempotent by name; bodies come back in order. */
#[test]
fn caches_are_partitioned_and_bodies_round_trip() {
    let mut storage = CacheStorage::default();
    let first = storage.open("https://a.test https://a.test", "v1");
    assert_eq!(storage.open("https://a.test https://a.test", "v1"), first);
    let other = storage.open("https://a.test https://b.test", "v1");
    assert_ne!(first, other);
    let cache = storage
        .cache("https://a.test https://a.test", first)
        .unwrap();
    cache.put(entry(get("https://a.test/1", &[]), None, "ab"));
    cache.put(entry(get("https://a.test/2", &[]), None, "cde"));
    let (records, bytes) = responses(&cache.lookup(None, Query::default()));
    assert_eq!(bytes, b"abcde");
    assert_eq!(records[1]["length"], 3);
    assert!(
        storage
            .cache("https://a.test https://b.test", first)
            .is_none()
    );
}

/** The whole store survives its JSON record. */
#[test]
fn storage_round_trips_through_its_record() {
    let mut storage = CacheStorage::default();
    let id = storage.open("k", "v1");
    storage
        .cache("k", id)
        .unwrap()
        .put(entry(get("https://a.test/1", &[]), None, "x"));
    let bytes = serde_json::to_vec(&storage).unwrap();
    let mut back: CacheStorage = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(back.cache("k", id).unwrap().entries.len(), 1);
    assert_eq!(back.open("k", "v2"), id + 1);
}
