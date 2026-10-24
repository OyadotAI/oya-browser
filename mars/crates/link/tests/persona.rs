use mars_link::Persona;
/**
 * Persona rules, ported from Oya's browser/tests/unit/main/native-policy/persona.test.ts and
 * browser/tests/unit/anonymity/fingerprint.test.js, plus the unit rules: a persona loads whole,
 * its device is compared without its per-connection proxy, and its proxy has one shape.
 */
use mars_link::identity::native_policy;
use mars_link::persona::unmasked_webgl;
use mars_link::profile::Profile;
use serde_json::{Value, json};

/// The engine User-Agent Oya's tests claim.
const ENGINE: &str = "Mozilla/5.0 Chrome/153.0.1234.56 Safari/537.36";

/// Oya's test persona: a real shape with protections outside the native subset.
fn persona(platform: &str) -> Value {
    json!({
        "id": "p-test",
        "navigator": {"platform": platform, "languages": ["en-US", "en"], "hardwareConcurrency": 8, "userAgent": "Untrusted/999"},
        "timezone": "America/New_York", "locale": "en-US", "webgl": {"renderer": "Apple M2"},
        "fonts": {"available": ["Arial"]}, "screen": {"width": 1440, "height": 900},
    })
}

/// A profile from JSON.
fn profile(value: Value) -> Profile {
    serde_json::from_value(value).unwrap()
}

/// The refusal text for a persona.
fn refusal(value: Value, engine: &str) -> String {
    native_policy(&profile(value), engine).unwrap_err().0
}

#[test]
fn one_deterministic_policy_binds_persona_platform_engine_version_and_metadata() {
    for (platform, hint, architecture) in [
        ("MacIntel", "macOS", "arm"),
        ("Win32", "Windows", "x86"),
        ("Linux x86_64", "Linux", "x86"),
    ] {
        let policy = native_policy(&profile(persona(platform)), ENGINE).unwrap();
        assert_eq!(
            policy,
            native_policy(&profile(persona(platform)), ENGINE).unwrap()
        );
        assert_eq!(policy.platform, platform);
        assert_eq!(policy.user_agent_metadata.platform, hint);
        assert_eq!(policy.user_agent_metadata.architecture, architecture);
        assert_eq!(policy.user_agent_metadata.full_version, "153.0.1234.56");
        assert!(policy.user_agent.contains("Chrome/153.0.0.0"));
        assert!(
            !policy.user_agent.contains("Untrusted"),
            "a caller-supplied UA is never used"
        );
        let chromium = |list: &[mars_link::identity::Brand]| {
            list.iter()
                .find(|b| b.brand == "Chromium")
                .unwrap()
                .version
                .clone()
        };
        assert_eq!(chromium(&policy.user_agent_metadata.brands), "153");
        assert_eq!(
            chromium(&policy.user_agent_metadata.full_version_list),
            "153.0.1234.56"
        );
        assert_eq!(policy.user_agent_metadata.form_factors, ["Desktop"]);
    }
}

#[test]
fn missing_and_unknown_platforms_never_inherit_the_host_identity() {
    for platform in ["", "Android", "macOS"] {
        assert!(refusal(persona(platform), ENGINE).contains("platform"));
    }
}

#[test]
fn engine_version_evidence_is_mandatory_instead_of_a_fixed_fallback() {
    for engine in ["", "Chrome/999", "Chrome/153.0.0.0junk", "Chrome/0.1.2.3"] {
        assert!(
            refusal(persona("MacIntel"), engine).contains("engine Chrome version"),
            "{engine}"
        );
    }
}

#[test]
fn locale_languages_timezone_and_processors_cannot_silently_lose_identity() {
    let mut input = persona("MacIntel");
    input["navigator"]["languages"] = json!(["fr-FR", "fr"]);
    assert!(refusal(input.clone(), ENGINE).contains("languages"));
    input["navigator"]["languages"] = json!(["en-US", "en", "fr"]);
    assert!(refusal(input.clone(), ENGINE).contains("languages"));
    input["navigator"]["languages"] = json!(["en-US"]);
    assert_eq!(
        native_policy(&profile(input.clone()), ENGINE)
            .unwrap()
            .languages,
        ["en-US", "en"]
    );
    input["navigator"]["hardwareConcurrency"] = json!(0);
    assert!(refusal(input.clone(), ENGINE).contains("processor"));
    input["navigator"]["hardwareConcurrency"] = json!(8);
    input["timezone"] = json!("Mars/Olympus");
    assert!(refusal(input.clone(), ENGINE).contains("timezone"));
    input["timezone"] = json!("+05:00");
    assert!(refusal(input, ENGINE).contains("timezone"));
}

#[test]
fn a_gpu_less_mac_is_presented_as_intel() {
    let mut input = persona("MacIntel");
    input.as_object_mut().unwrap().remove("webgl");
    assert_eq!(
        native_policy(&profile(input), ENGINE)
            .unwrap()
            .user_agent_metadata
            .architecture,
        "x86"
    );
}

/// A profile with these WebGL strings.
fn webgl(chrome: bool, gl: Value) -> Profile {
    profile(
        json!({"id": "p", "navigator": {"platform": "MacIntel"}, "webgl": gl, "webglChrome": chrome}),
    )
}

/// Apple's ANGLE renderer string.
const ANGLE: &str = "ANGLE (Apple, ANGLE Metal Renderer: Apple M4, Unspecified Version)";

#[test]
fn a_webgl_chrome_persona_answers_with_chromes_angle_strings() {
    let gl = json!({"vendor": "Google Inc. (Apple)", "renderer": ANGLE, "unmaskedVendor": "Apple", "unmaskedRenderer": "Apple M4"});
    assert_eq!(
        unmasked_webgl(&webgl(true, gl)),
        (Some("Google Inc. (Apple)".into()), Some(ANGLE.into()))
    );
}

#[test]
fn an_older_persona_keeps_its_bare_driver_strings() {
    let gl = json!({"vendor": "Google Inc. (Apple)", "renderer": ANGLE, "unmaskedVendor": "Apple", "unmaskedRenderer": "Apple M1"});
    assert_eq!(
        unmasked_webgl(&webgl(false, gl)),
        (Some("Apple".into()), Some("Apple M1".into()))
    );
}

#[test]
fn a_masked_webkit_renderer_never_answers() {
    let gl = json!({"vendor": "WebKit", "renderer": "WebKit WebGL", "unmaskedVendor": "Google Inc. (Apple)", "unmaskedRenderer": ANGLE});
    assert_eq!(
        unmasked_webgl(&webgl(true, gl)),
        (Some("Google Inc. (Apple)".into()), Some(ANGLE.into()))
    );
}

/// An auth_ok carrying `fingerprint`.
fn welcome(fingerprint: Value) -> Value {
    json!({"type": "auth_ok", "browser_id": "b", "persona": {"id": "p-test", "name": "Work"}, "fingerprint": fingerprint, "cookies": [{"name": "sid", "value": "1", "domain": ".example.com"}], "origins": {}})
}

#[test]
fn a_persona_loads_fingerprint_cookies_and_proxy_as_one_unit() {
    let mut fingerprint = persona("MacIntel");
    fingerprint["proxy"] =
        json!({"url": "http://proxy.test:8080", "username": "u", "password": "secret"});
    let unit = Persona::from_welcome(&welcome(fingerprint)).unwrap();
    assert_eq!((unit.id(), unit.name()), ("p-test", "Work"));
    assert_eq!(unit.cookies().len(), 1);
    let proxy = unit.proxy().unwrap();
    assert_eq!(
        (proxy.scheme.as_str(), proxy.host.as_str(), proxy.port),
        ("http", "proxy.test", 8080)
    );
    assert!(
        !serde_json::to_string(proxy).unwrap().contains("secret"),
        "the proxy password is never serialized"
    );
    assert_eq!(unit.identity(ENGINE).unwrap().proxy.as_ref(), Some(proxy));
}

#[test]
fn proxy_shapes_normalize_and_direct_is_no_proxy() {
    let cases = [
        (
            json!({"url": "https://gw.test", "metered": true}),
            Some(("https", "gw.test", 443, true)),
        ),
        (
            json!({"host": "socks.test", "port": 1080, "type": "socks5"}),
            Some(("socks5", "socks.test", 1080, false)),
        ),
        (json!({"direct": true}), None),
        (Value::Null, None),
    ];
    for (proxy, expected) in cases {
        let mut fingerprint = persona("Win32");
        fingerprint["proxy"] = proxy;
        let unit = Persona::from_welcome(&welcome(fingerprint)).unwrap();
        let got = unit
            .proxy()
            .map(|p| (p.scheme.as_str(), p.host.as_str(), p.port, p.metered));
        assert_eq!(got, expected);
    }
}

#[test]
fn a_fingerprint_without_an_id_or_device_is_refused() {
    assert!(Persona::from_welcome(&json!({"type": "auth_ok"})).is_err());
    assert!(
        Persona::from_welcome(&welcome(
            json!({"id": "", "navigator": {"platform": "Win32"}})
        ))
        .is_err()
    );
    assert!(Persona::from_welcome(&welcome(json!({"id": "p", "navigator": 5}))).is_err());
}

#[test]
fn the_same_device_ignores_only_the_per_connection_proxy() {
    let a = Persona::from_welcome(&welcome(persona("MacIntel"))).unwrap();
    let mut proxied = persona("MacIntel");
    proxied["proxy"] = json!({"url": "http://gw.test:1"});
    assert!(a.same_device(&Persona::from_welcome(&welcome(proxied)).unwrap()));
    let mut moved = persona("MacIntel");
    moved["navigator"]["hardwareConcurrency"] = json!(4);
    assert!(!a.same_device(&Persona::from_welcome(&welcome(moved)).unwrap()));
}

#[test]
fn https_requests_carry_the_personas_hints_in_chromes_order() {
    let identity = Persona::from_welcome(&welcome(persona("Win32")))
        .unwrap()
        .identity(ENGINE)
        .unwrap();
    let base = [
        ("Accept", "text/html"),
        ("User-Agent", "Mars/1"),
        ("sec-ch-ua", "\"Mars\""),
    ]
    .map(|(k, v)| (k.to_owned(), v.to_owned()));
    let names = |headers: Vec<(String, String)>| {
        headers
            .into_iter()
            .map(|(k, _)| k.to_ascii_lowercase())
            .collect::<Vec<_>>()
    };
    let navigation = identity.request_headers(&base, true, true, &[]);
    assert!(
        navigation
            .iter()
            .any(|(k, v)| k == "User-Agent" && v == &identity.policy.user_agent),
        "the engine UA is replaced"
    );
    assert_eq!(
        names(navigation),
        [
            "sec-ch-ua",
            "sec-ch-ua-mobile",
            "sec-ch-ua-platform",
            "accept-language",
            "accept",
            "user-agent"
        ]
    );
    let fetch = identity.request_headers(&base, true, false, &["sec-ch-ua-arch"]);
    assert_eq!(
        names(fetch),
        [
            "sec-ch-ua-platform",
            "accept-language",
            "sec-ch-ua",
            "user-agent",
            "sec-ch-ua-mobile",
            "sec-ch-ua-arch",
            "accept"
        ]
    );
    assert_eq!(
        names(identity.request_headers(&base, false, true, &[])),
        ["accept", "user-agent", "accept-language"],
        "no hints over plain HTTP"
    );
}
