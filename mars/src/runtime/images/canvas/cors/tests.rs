/** CORS image admission is tested with inert responses, never a network or page-supplied clean flag. */
use super::*;

/** Responses expose only the two admission fields retained by Relay for image destinations. */
fn response(url: &Url, allow: Option<&str>, credentials: bool) -> WebResponse {
    let mut headers = Vec::new();
    if let Some(allow) = allow {
        headers.push(("access-control-allow-origin".into(), allow.into()));
    }
    if credentials {
        headers.push(("access-control-allow-credentials".into(), "true".into()));
    }
    WebResponse {
        url: url.to_string(),
        status: 200,
        source: String::new(),
        image_data: true,
        mime: "image/png".into(),
        location: None,
        set_cookies: Vec::new(),
        charset: None,
        download: false,
        disposition: None,
        headers,
    }
}

#[test]
fn anonymous_cross_origin_omits_credentials_and_accepts_a_wildcard() {
    let base = Url::parse("https://a.test/page").unwrap();
    let cross = Url::parse("https://b.test/pixel").unwrap();
    let mut flow = ImageFlow::new(&base, ImageCors::Anonymous);
    assert!(flow.request(&base, true).1);
    let (request, credentials) = flow.request(&cross, true);
    assert!(!credentials);
    assert_eq!(request.origin.as_deref(), Some("https://a.test"));
    request.validate().unwrap();
    flow.admit(&cross, &response(&cross, Some("*"), false))
        .unwrap();
    assert!(flow.origin_clean());
}

#[test]
fn credentialed_cross_origin_requires_both_exact_origin_and_credentials_grant() {
    let base = Url::parse("https://a.test/page").unwrap();
    let cross = Url::parse("https://b.test/pixel").unwrap();
    let mut flow = ImageFlow::new(&base, ImageCors::UseCredentials);
    assert!(flow.request(&cross, true).1);
    assert!(
        flow.admit(&cross, &response(&cross, Some("*"), true))
            .is_err()
    );
    assert!(
        flow.admit(&cross, &response(&cross, Some("https://a.test"), false))
            .is_err()
    );
    flow.admit(&cross, &response(&cross, Some("https://a.test"), true))
        .unwrap();
}

#[test]
fn no_cors_cross_origin_returning_home_stays_tainted() {
    let base = Url::parse("https://a.test/page").unwrap();
    let cross = Url::parse("https://b.test/pixel").unwrap();
    let mut flow = ImageFlow::new(&base, ImageCors::NoCors);
    let (request, credentials) = flow.request(&cross, true);
    assert!(credentials);
    assert!(request.origin.is_none());
    flow.admit(&cross, &response(&cross, Some("*"), false))
        .unwrap();
    flow.redirect(&cross, &base);
    flow.request(&base, true);
    flow.admit(&base, &response(&base, None, false)).unwrap();
    assert!(!flow.origin_clean());
}

#[test]
fn each_redirect_response_requires_admission_and_crossing_again_taints_origin() {
    let base = Url::parse("https://a.test/page").unwrap();
    let cross = Url::parse("https://b.test/pixel").unwrap();
    let last = Url::parse("https://c.test/pixel").unwrap();
    let mut flow = ImageFlow::new(&base, ImageCors::Anonymous);
    flow.request(&cross, true);
    let refusal = flow
        .admit(&cross, &response(&cross, None, false))
        .unwrap_err();
    assert_ne!(refusal.code, "unsupported_feature");
    flow.admit(&cross, &response(&cross, Some("https://a.test"), false))
        .unwrap();
    flow.redirect(&cross, &last);
    let (request, credentials) = flow.request(&last, true);
    assert_eq!(request.origin.as_deref(), Some("null"));
    assert!(!credentials);
    assert!(
        flow.admit(&last, &response(&last, Some("https://a.test"), false))
            .is_err()
    );
    flow.admit(&last, &response(&last, Some("null"), false))
        .unwrap();
}

#[test]
fn hidden_redirects_and_duplicate_grants_cannot_create_clean_pixels() {
    let base = Url::parse("https://a.test/page").unwrap();
    let cross = Url::parse("https://b.test/pixel").unwrap();
    let mut flow = ImageFlow::new(&base, ImageCors::Anonymous);
    flow.request(&cross, true);
    assert!(
        flow.admit(&cross, &response(&base, Some("*"), false))
            .is_err()
    );
    let mut duplicated = response(&cross, Some("*"), false);
    duplicated
        .headers
        .push(("Access-Control-Allow-Origin".into(), "*".into()));
    assert!(flow.admit(&cross, &duplicated).is_err());
}

/** A Basic same-origin response needs no cross-origin permission even if its CORS headers are malformed. */
#[test]
fn same_origin_images_ignore_irrelevant_duplicate_cors_headers() {
    let base = Url::parse("https://a.test/page").unwrap();
    let image = Url::parse("https://a.test/pixel").unwrap();
    let mut flow = ImageFlow::new(&base, ImageCors::Anonymous);
    assert!(flow.request(&image, true).1);
    let mut duplicated = response(&image, Some("https://unrelated.test"), false);
    duplicated
        .headers
        .push(("Access-Control-Allow-Origin".into(), "*".into()));
    flow.admit(&image, &duplicated).unwrap();
    assert!(flow.origin_clean());
}
