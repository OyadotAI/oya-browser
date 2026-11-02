/** Pure cookie rules use explicit time and URLs without touching the network. */
use mars::network::CookieJar;
use mars::network::network::constants::DOMAIN_COOKIES;
use std::time::{Duration, SystemTime};
use url::Url;
fn url(value: &str) -> Url {
    Url::parse(value).unwrap()
}

/** Session cookies survive restart while expiry and deletion remain authoritative. */
#[test]
fn profile_roundtrip_preserves_session_cookies_and_prunes_expired_entries() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000_000);
    let origin = url("https://example.com/login");
    let mut jar = CookieJar::default();
    assert!(
        receive(
            &mut jar,
            &origin,
            &[
                "session=active; Path=/; Secure; SameSite=Lax",
                "short=lived; Path=/; Secure; Max-Age=1"
            ],
            now
        )
        .is_empty()
    );
    let record = jar.profile_record().unwrap();
    let restored = CookieJar::from_profile(&record, now + Duration::from_secs(2)).unwrap();
    let mut restored = restored;
    assert_eq!(
        restored.header(&origin, Some(&origin), false, now + Duration::from_secs(2)),
        "session=active"
    );
    assert!(
        receive(
            &mut restored,
            &origin,
            &["session=; Path=/; Secure; Max-Age=0"],
            now + Duration::from_secs(2)
        )
        .is_empty()
    );
    let mut deleted = CookieJar::from_profile(
        &restored.profile_record().unwrap(),
        now + Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(
        deleted.header(&origin, Some(&origin), false, now + Duration::from_secs(2)),
        ""
    );
}

/** Corrupt and future-version profile data fails whole-record rather than restoring a prefix. */
#[test]
fn profile_rejects_corruption_unknown_fields_and_future_versions() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000_000);
    for bytes in [br"{".as_slice(), br#"{"version":3,"cookies":[]}"#.as_slice(), br#"{"version":1,"cookies":[{"name":"x","value":"y","host":"example.com","path":"/","secure":false,"same_site":"lax","expires":null,"extra":true}]}"#.as_slice()] {
        assert!(CookieJar::from_profile(bytes, now).is_err());
    }
}
fn receive(jar: &mut CookieJar, url: &Url, fields: &[&str], now: SystemTime) -> Vec<String> {
    jar.receive(
        url,
        &fields.iter().map(|v| v.to_string()).collect::<Vec<_>>(),
        now,
    )
}
/** Host-only, slash-boundary paths and Secure govern every emitted header. */
#[test]
fn host_path_secure_and_samesite_boundaries_are_enforced() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000_000);
    let origin = url("https://example.com/account/login");
    let mut jar = CookieJar::default();
    assert!(
        receive(
            &mut jar,
            &origin,
            &[
                "a=one; Secure; HttpOnly; SameSite=Strict",
                "b=two; Path=/; SameSite=Lax",
                "c=three; Secure; SameSite=None; Path=/"
            ],
            now
        )
        .is_empty()
    );
    assert_eq!(
        jar.header(
            &url("https://example.com/account/x"),
            Some(&origin),
            false,
            now
        ),
        "a=one; b=two; c=three"
    );
    assert_eq!(
        jar.header(
            &url("https://example.com/accounting"),
            Some(&origin),
            false,
            now
        ),
        "b=two; c=three"
    );
    assert_eq!(
        jar.header(
            &url("https://other.example.com/account/x"),
            Some(&origin),
            true,
            now
        ),
        ""
    );
    assert_eq!(
        jar.header(
            &url("http://example.com/account/x"),
            Some(&origin),
            true,
            now
        ),
        "b=two"
    );
    assert_eq!(
        jar.header(&origin, Some(&url("https://other.com")), false, now),
        "c=three"
    );
}
/** Max-Age wins over Expires, including deletion and bounded explicit expiry. */
#[test]
fn max_age_expiry_deletion_prefixes_and_domain_refusals_are_value_free() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000_000);
    let origin = url("https://example.com/");
    let mut jar = CookieJar::default();
    assert!(
        receive(
            &mut jar,
            &origin,
            &["a=one; Max-Age=2; Expires=Wed, 01 Jan 1997 00:00:00 GMT"],
            now
        )
        .is_empty()
    );
    assert_eq!(jar.header(&origin, Some(&origin), true, now), "a=one");
    assert_eq!(
        jar.header(&origin, Some(&origin), true, now + Duration::from_secs(3)),
        ""
    );
    for field in [
        "__Host-a=secret; Secure",
        "__Secure-a=secret",
        "a=secret; Domain=com",
        "a=secret; SameSite=None",
    ] {
        let reports = receive(&mut jar, &origin, &[field], now);
        assert_eq!(reports.len(), 1);
        assert!(!reports[0].contains("secret"));
    }
    assert!(
        receive(
            &mut jar,
            &origin,
            &["__Host-ok=yes; Secure; Path=/", "a=one"],
            now
        )
        .is_empty()
    );
    receive(&mut jar, &origin, &["a=removed; Max-Age=0"], now);
    assert_eq!(
        jar.header(&origin, Some(&origin), true, now),
        "__Host-ok=yes"
    );
}
/** Insecure overlays cannot replace or delete a previously Secure name at overlapping paths. */
#[test]
fn insecure_overlay_cannot_replace_a_secure_cookie() {
    let now = SystemTime::now();
    let origin = url("https://example.com/");
    let mut jar = CookieJar::default();
    receive(&mut jar, &origin, &["a=one; Secure; Path=/"], now);
    assert_eq!(
        receive(
            &mut jar,
            &url("http://example.com/x"),
            &["a=two; Path=/x"],
            now
        )
        .len(),
        1
    );
    assert_eq!(jar.header(&origin, Some(&origin), true, now), "a=one");
}
/** A full site evicts its least recently used cookie, so the per-site bound always holds. */
#[test]
fn site_capacity_evicts_the_least_recently_used_cookie() {
    let now = SystemTime::now();
    let origin = url("https://example.com/");
    let mut jar = CookieJar::default();
    for index in 0..DOMAIN_COOKIES {
        assert!(receive(&mut jar, &origin, &[&format!("c{index}=x")], now).is_empty());
    }
    assert!(receive(&mut jar, &origin, &["overflow=yes"], now).is_empty());
    let header = jar.header(&origin, Some(&origin), true, now);
    let names: Vec<&str> = header.split("; ").collect();
    assert_eq!(names.len(), DOMAIN_COOKIES);
    assert!(!names.contains(&"c0=x"));
    assert!(names.contains(&"overflow=yes"));
}
