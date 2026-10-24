/** Cookie jar rules, one test per rule, through the jar's public methods. */
use super::*;

/** A fixed clock keeps expiry deterministic. */
fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000)
}

/** Parses a test URL. */
fn url(value: &str) -> Url {
    Url::parse(value).expect("test URL")
}

/** Stores one Set-Cookie field from `origin` and returns the diagnostics. */
fn set(jar: &mut CookieJar, origin: &str, field: &str) -> Vec<String> {
    jar.receive(&url(origin), &[field.to_owned()], now())
}

/** The header a person's own navigation to `target` would carry. */
fn sent(jar: &mut CookieJar, target: &str) -> String {
    jar.header(&url(target), None, true, now())
}

#[test]
fn domain_cookies_reach_subdomains_and_host_cookies_do_not() {
    let mut jar = CookieJar::default();
    assert!(
        set(
            &mut jar,
            "https://accounts.google.com/",
            "SID=a; Domain=.google.com; Path=/"
        )
        .is_empty()
    );
    set(&mut jar, "https://accounts.google.com/", "LSID=b; Path=/");
    assert_eq!(sent(&mut jar, "https://mail.google.com/"), "SID=a");
    assert_eq!(
        sent(&mut jar, "https://accounts.google.com/"),
        "SID=a; LSID=b"
    );
    assert_eq!(sent(&mut jar, "https://notgoogle.com/"), "");
}

#[test]
fn domain_must_cover_the_setting_host() {
    let mut jar = CookieJar::default();
    assert!(!set(&mut jar, "https://a.example.com/", "x=1; Domain=other.com").is_empty());
    assert!(
        !set(
            &mut jar,
            "https://a.example.com/",
            "x=1; Domain=b.example.com"
        )
        .is_empty()
    );
    assert!(jar.is_empty());
}

#[test]
fn public_suffix_domains_are_refused() {
    let mut jar = CookieJar::default();
    for (origin, field) in [
        ("https://a.example.co.uk/", "x=1; Domain=co.uk"),
        ("https://me.github.io/", "x=1; Domain=github.io"),
        ("https://a.example.com/", "x=1; Domain=com"),
    ] {
        assert!(!set(&mut jar, origin, field).is_empty(), "{field}");
    }
    assert!(jar.is_empty());
    set(
        &mut jar,
        "https://a.example.co.uk/",
        "x=1; Domain=example.co.uk",
    );
    assert_eq!(sent(&mut jar, "https://b.example.co.uk/"), "x=1");
}

#[test]
fn a_public_suffix_host_naming_itself_gets_a_host_only_cookie() {
    let mut jar = CookieJar::default();
    assert!(set(&mut jar, "https://github.io/", "x=1; Domain=github.io").is_empty());
    assert_eq!(sent(&mut jar, "https://github.io/"), "x=1");
    assert_eq!(sent(&mut jar, "https://me.github.io/"), "");
}

#[test]
fn ip_hosts_never_share_cookies_with_other_hosts() {
    let mut jar = CookieJar::default();
    assert!(!set(&mut jar, "https://1.1.1.1/", "x=1; Domain=1.1").is_empty());
    assert!(set(&mut jar, "https://1.1.1.1/", "y=2; Domain=1.1.1.1").is_empty());
    assert_eq!(sent(&mut jar, "https://1.1.1.1/"), "y=2");
}

#[test]
fn strict_cookies_stay_same_site_lax_cookies_follow_top_level_gets() {
    let mut jar = CookieJar::default();
    set(&mut jar, "https://shop.example/", "s=1; SameSite=Strict");
    set(&mut jar, "https://shop.example/", "l=2; SameSite=Lax");
    set(
        &mut jar,
        "https://shop.example/",
        "n=3; SameSite=None; Secure",
    );
    set(&mut jar, "https://shop.example/", "d=4");
    let target = url("https://shop.example/");
    let sub = url("https://www.shop.example/");
    let other = url("https://evil.test/");
    assert_eq!(
        jar.header(&target, Some(&sub), false, now()),
        "s=1; l=2; n=3; d=4"
    );
    assert_eq!(
        jar.header(&target, Some(&other), true, now()),
        "l=2; n=3; d=4"
    );
    assert_eq!(jar.header(&target, Some(&other), false, now()), "n=3");
}

#[test]
fn same_site_is_schemeful() {
    let mut jar = CookieJar::default();
    set(&mut jar, "https://shop.example/", "s=1; SameSite=Strict");
    let target = url("https://shop.example/");
    let insecure = url("http://shop.example/");
    assert_eq!(jar.header(&target, Some(&insecure), false, now()), "");
}

#[test]
fn same_site_none_requires_secure() {
    let mut jar = CookieJar::default();
    assert!(!set(&mut jar, "https://a.example/", "n=1; SameSite=None").is_empty());
    assert!(jar.is_empty());
}

#[test]
fn max_age_wins_over_expires_and_zero_deletes() {
    let mut jar = CookieJar::default();
    set(
        &mut jar,
        "https://a.example/",
        "x=1; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Max-Age=60",
    );
    assert_eq!(sent(&mut jar, "https://a.example/"), "x=1");
    let later = now() + Duration::from_secs(61);
    assert_eq!(
        jar.header(&url("https://a.example/"), None, true, later),
        ""
    );
    set(&mut jar, "https://a.example/", "y=1");
    set(&mut jar, "https://a.example/", "y=1; Max-Age=0");
    assert!(jar.is_empty());
}

#[test]
fn expires_accepts_the_cookie_date_formats_servers_send() {
    let mut jar = CookieJar::default();
    set(
        &mut jar,
        "https://a.example/",
        "a=1; expires=Sat, 24-Apr-2027 10:00:00 GMT",
    );
    set(
        &mut jar,
        "https://a.example/",
        "b=1; expires=Sunday, 06-Nov-94 08:49:37 GMT",
    );
    set(&mut jar, "https://a.example/", "c=1; expires=garbage");
    assert_eq!(sent(&mut jar, "https://a.example/"), "a=1; c=1");
    assert_eq!(
        cookie_date("Wed, 21 Oct 2015 07:28:00 GMT"),
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1_445_412_480))
    );
    assert_eq!(cookie_date("Wed, 32 Oct 2015 07:28:00 GMT"), None);
}

#[test]
fn lifetimes_are_clamped_to_four_hundred_days() {
    let mut jar = CookieJar::default();
    set(
        &mut jar,
        "https://a.example/",
        "a=1; Max-Age=999999999999999999999",
    );
    set(
        &mut jar,
        "https://a.example/",
        "b=1; Expires=Fri, 01 Jan 9999 00:00:00 GMT",
    );
    let after = now() + COOKIE_LIFETIME + Duration::from_secs(1);
    assert_eq!(
        jar.header(&url("https://a.example/"), None, true, after),
        ""
    );
}

#[test]
fn secure_cookies_need_https_both_ways() {
    let mut jar = CookieJar::default();
    assert!(!set(&mut jar, "http://a.example/", "x=1; Secure").is_empty());
    set(&mut jar, "https://a.example/", "x=1; Secure");
    assert_eq!(sent(&mut jar, "http://a.example/"), "");
    assert!(
        !set(&mut jar, "http://a.example/", "x=2").is_empty(),
        "insecure overwrite"
    );
    assert_eq!(sent(&mut jar, "https://a.example/"), "x=1");
}

#[test]
fn http_only_cookies_are_hidden_from_and_protected_against_script() {
    let mut jar = CookieJar::default();
    set(&mut jar, "https://a.example/", "sid=1; HttpOnly");
    set(&mut jar, "https://a.example/", "theme=dark");
    let page = url("https://a.example/");
    assert_eq!(jar.document_cookie(&page, now()), "theme=dark");
    assert!(jar.set_document_cookie(&page, "sid=forged", now()).is_err());
    assert!(
        jar.set_document_cookie(&page, "x=1; HttpOnly", now())
            .is_err()
    );
    assert!(jar.set_document_cookie(&page, "lang=en", now()).is_ok());
    assert_eq!(
        sent(&mut jar, "https://a.example/"),
        "sid=1; theme=dark; lang=en"
    );
}

#[test]
fn prefixed_cookies_enforce_their_requirements() {
    let mut jar = CookieJar::default();
    for field in [
        "__Secure-a=1",
        "__Host-a=1; Secure",
        "__Host-a=1; Secure; Path=/app",
        "__Host-a=1; Secure; Path=/; Domain=a.example",
        "__host-a=1; Path=/",
    ] {
        assert!(
            !set(&mut jar, "https://a.example/", field).is_empty(),
            "{field}"
        );
    }
    assert!(
        set(
            &mut jar,
            "https://a.example/",
            "__Secure-a=1; Secure; Domain=a.example"
        )
        .is_empty()
    );
    assert!(set(&mut jar, "https://a.example/", "__Host-b=1; Secure; Path=/").is_empty());
    assert_eq!(
        sent(&mut jar, "https://a.example/"),
        "__Secure-a=1; __Host-b=1"
    );
}

#[test]
fn paths_match_on_segment_boundaries_and_longest_path_goes_first() {
    let mut jar = CookieJar::default();
    set(&mut jar, "https://a.example/", "root=1; Path=/");
    set(&mut jar, "https://a.example/", "acct=2; Path=/account");
    assert_eq!(
        sent(&mut jar, "https://a.example/account/x"),
        "acct=2; root=1"
    );
    assert_eq!(sent(&mut jar, "https://a.example/accounting"), "root=1");
}

#[test]
fn oversized_pairs_are_refused_and_oversized_attributes_ignored() {
    let mut jar = CookieJar::default();
    let big = "v".repeat(COOKIE_PAIR_BYTES);
    assert!(!set(&mut jar, "https://a.example/", &format!("x={big}")).is_empty());
    let long_path = format!("y=1; Path=/{}", "p".repeat(COOKIE_ATTRIBUTE_BYTES));
    assert!(set(&mut jar, "https://a.example/a/b", &long_path).is_empty());
    assert_eq!(sent(&mut jar, "https://a.example/a/x"), "y=1");
    let huge = format!("z=1; {}", "a".repeat(COOKIE_FIELD_BYTES));
    assert!(!set(&mut jar, "https://a.example/", &huge).is_empty());
}

#[test]
fn a_domain_keeps_at_most_its_limit_evicting_least_recently_used() {
    let mut jar = CookieJar::default();
    for index in 0..DOMAIN_COOKIES {
        set(
            &mut jar,
            "https://www.example.com/",
            &format!("c{index}=1; Domain=example.com"),
        );
    }
    jar.header(&url("https://www.example.com/"), None, true, now());
    set(&mut jar, "https://www.example.com/", "late=1");
    set(&mut jar, "https://other.example.com/", "fresh=1");
    assert_eq!(jar.records.len(), DOMAIN_COOKIES);
    let header = sent(&mut jar, "https://www.example.com/");
    assert!(!header.contains("c0=1") && !header.contains("c1=1"));
    assert!(header.contains("late=1") && header.contains("c2=1"));
}

#[test]
fn the_jar_keeps_at_most_its_total_limit() {
    let mut jar = CookieJar::default();
    for site in 0..COOKIE_COUNT / 100 + 1 {
        for index in 0..100 {
            set(
                &mut jar,
                &format!("https://s{site}.example/"),
                &format!("c{index}=1"),
            );
        }
    }
    assert_eq!(jar.records.len(), COOKIE_COUNT);
    assert_eq!(sent(&mut jar, "https://s0.example/"), "");
}

#[test]
fn the_jar_stays_within_its_byte_budget() {
    let mut jar = CookieJar::default();
    let value = "v".repeat(COOKIE_PAIR_BYTES - 8);
    for site in 0..COOKIE_JAR_BYTES / COOKIE_PAIR_BYTES + 10 {
        set(
            &mut jar,
            &format!("https://s{site}.example/"),
            &format!("c={value}"),
        );
    }
    assert!(jar.bytes() <= COOKIE_JAR_BYTES);
    assert!(jar.profile_record().is_ok());
}

#[test]
fn version_two_records_round_trip_every_attribute() {
    let mut jar = CookieJar::default();
    set(
        &mut jar,
        "https://www.example.com/",
        "d=1; Domain=example.com; HttpOnly; Max-Age=600",
    );
    set(
        &mut jar,
        "https://www.example.com/",
        "h=2; Secure; SameSite=None; Path=/",
    );
    let record = jar.profile_record().expect("record");
    let mut restored = CookieJar::from_profile(&record, now()).expect("restore");
    assert_eq!(sent(&mut restored, "https://api.example.com/"), "d=1");
    assert_eq!(
        restored.document_cookie(&url("https://www.example.com/"), now()),
        "h=2"
    );
}

#[test]
fn version_one_records_still_load_as_host_only() {
    let record = br#"{"version":1,"cookies":[{"name":"a","value":"1","host":"example.com","path":"/","secure":false,"same_site":"lax","expires":null}]}"#;
    let mut jar = CookieJar::from_profile(record, now()).expect("old shape");
    assert_eq!(sent(&mut jar, "https://example.com/"), "a=1");
    assert_eq!(sent(&mut jar, "https://www.example.com/"), "");
    let upgraded = String::from_utf8(jar.profile_record().expect("record")).expect("utf8");
    assert!(upgraded.starts_with(r#"{"version":2,"#));
}

#[test]
fn corrupt_or_unauthorized_records_are_rejected_whole() {
    for record in [
        r#"{"version":3,"cookies":[]}"#,
        r#"{"version":255,"cookies":[]}"#,
        r#"{"version":1,"cookies":[{"name":"a","value":"1","host":"example.com","path":"/","secure":false,"same_site":"lax","expires":null,"host_only":false}]}"#,
        r#"{"version":2,"cookies":[{"name":"a","value":"1","host":"co.uk","path":"/","secure":false,"same_site":"lax","expires":null,"host_only":false}]}"#,
        r#"{"version":2,"cookies":[{"name":"a","value":"1","host":"x.test","path":"/","secure":false,"same_site":"lax","expires":null},{"name":"a","value":"2","host":"x.test","path":"/","secure":false,"same_site":"lax","expires":null}]}"#,
    ] {
        assert!(
            CookieJar::from_profile(record.as_bytes(), now()).is_err(),
            "{record}"
        );
    }
    let huge = vec![b' '; COOKIE_PROFILE_BYTES + 1];
    assert!(CookieJar::from_profile(&huge, now()).is_err());
}

#[test]
fn hostile_attribute_soup_never_panics() {
    let mut jar = CookieJar::default();
    for field in [
        "=",
        ";",
        "a",
        "a=1;;;;",
        "a=1; Domain=.",
        "a=1; Domain=..",
        "a=1; Max-Age=-",
        "a=1; Max-Age=-99999999999999999999",
        "a=1; Expires=99:99:99 1 jan 1600",
        "a=1; Path",
        "a=1; SameSite",
        "a=\u{1}",
        "a=1; Expires=1:2:3 31 dec 69",
    ] {
        let _ = set(&mut jar, "https://a.example/", field);
    }
}
