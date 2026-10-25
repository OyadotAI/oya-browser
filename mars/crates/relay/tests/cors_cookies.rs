/** Round-2 "cors" owner ports: RFC 6265bis cookie-pair parsing as Chromium does it (WPT
cookies/name, cookies/value, cookies/path), through CookieJar's public surface. */
use mars_relay::CookieJar;
use std::time::{Duration, SystemTime};
use url::Url;

/** A fixed clock keeps expiry deterministic. */
fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000)
}

/** Store each Set-Cookie field from one page, in order, and read document.cookie back. */
fn after(fields: &[&str]) -> String {
    let page = Url::parse("http://a.example/cookies/name/name.html").unwrap();
    let mut jar = CookieJar::default();
    for field in fields {
        jar.receive(&page, &[(*field).to_owned()], now());
    }
    jar.document_cookie(&page, now())
}

#[test]
fn a_pair_without_an_equals_sign_is_a_nameless_cookie() {
    assert_eq!(after(&["test6;cool=dude"]), "test6");
    assert_eq!(after(&["=test=2"]), "test=2");
    assert_eq!(after(&["===test=2b"]), "==test=2b");
    assert_eq!(after(&["test17a", "test17b"]), "test17b");
    assert_eq!(
        after(&["testA=16", "=test16", "testB=16"]),
        "testA=16; test16; testB=16"
    );
}

#[test]
fn names_keep_inner_whitespace_and_any_printable_byte() {
    assert_eq!(after(&["test test=8"]), "test test=8");
    assert_eq!(after(&["test =3"]), "test=3");
    assert_eq!(after(&["\"test9;test\"=9"]), "\"test9");
    assert_eq!(after(&["(a=test"]), "(a=test");
    assert_eq!(after(&["\"a=b\"=test"]), "\"a=b\"=test");
    assert_eq!(after(&["test\u{9}name=9"]), "test\u{9}name=9");
}

#[test]
fn empty_pairs_and_nameless_prefix_impostors_are_ignored() {
    assert_eq!(after(&["=", ""]), "");
    assert_eq!(after(&["=__Secure-abc=123", "__HoSt-abc"]), "");
}

#[test]
fn control_characters_reject_the_cookie_but_utf8_is_kept() {
    assert_eq!(after(&["test\u{1}name=1"]), "");
    assert_eq!(after(&["a=\u{7f}"]), "");
    assert_eq!(after(&["caf\u{e9}=\u{2603}"]), "caf\u{e9}=\u{2603}");
}

#[test]
fn an_invalid_path_attribute_falls_back_to_the_default_path() {
    let page = Url::parse("http://a.example/dir/page.html").unwrap();
    let mut jar = CookieJar::default();
    jar.receive(&page, &["a=1; Path=/elsewhere; Path=x".to_owned()], now());
    assert_eq!(jar.document_cookie(&page, now()), "a=1");
}
