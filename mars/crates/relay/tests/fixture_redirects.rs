/** A redirect between granted loopback fixtures follows, and one to an ungranted private
destination is still refused (Relay's fixture-origin bound). */
use url::Url;

/** The two fixture origins a test grants. */
fn grants() -> Vec<String> {
    vec![
        "http://localhost:8000".into(),
        "http://www1.localhost:8000".into(),
    ]
}

#[test]
fn a_redirect_to_another_granted_fixture_origin_follows() {
    let from = Url::parse("http://localhost:8000/a").unwrap();
    let next = mars_relay::redirect_url(&from, "http://www1.localhost:8000/b", &grants()).unwrap();
    assert_eq!(next.as_str(), "http://www1.localhost:8000/b");
}

#[test]
fn a_redirect_from_a_fixture_to_an_ungranted_private_origin_is_refused() {
    let from = Url::parse("http://localhost:8000/a").unwrap();
    for location in [
        "http://www2.localhost:8000/",
        "http://127.0.0.1:9/",
        "http://10.0.0.1/",
    ] {
        assert!(
            mars_relay::redirect_url(&from, location, &grants()).is_err(),
            "{location}"
        );
    }
}
