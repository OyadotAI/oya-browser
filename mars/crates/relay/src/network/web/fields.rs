/** Chrome-shaped request header fields, in Chrome 152's order, shared by HTTP/1.1 and HTTP/2. */
use super::encoding::{ACCEPT_ENCODING, ACCEPT_ENCODING_PLAIN};
use crate::network::cookies::site;
use crate::network::{Destination, Method, WebRequest};
use ::url::Url;

/** The Chrome major version Mars presents, as a literal so `concat!` can build every string from it. */
macro_rules! chrome_major {
    () => {
        "152"
    };
}

/// The Chrome version Mars presents everywhere: these request headers and, through the persona
/// glue, every persona's identity. Chrome 152 is the Chromium the Oya desktop ships (Electron 44).
/// Reduced form, as Chrome's own User-Agent carries it; moving it also means recomputing
/// `SEC_CH_UA`'s GREASE brand, which a unit test pins to the major.
pub const CHROME_ENGINE: &str = concat!("Chrome/", chrome_major!(), ".0.0.0");

/// Chrome on macOS. Chrome freezes the macOS version at 10_15_7 and the minor version at 0.0.0 in
/// the reduced User-Agent.
pub(super) const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/",
    chrome_major!(),
    ".0.0.0 Safari/537.36"
);

/// Chrome 152's low-entropy brand list. Chromium seeds the GREASE brand and the order with the
/// major version: 152 % 11 and 153 % 11 pick `?` and `_`, 152 % 3 picks "24", and 152 % 6
/// picks the order grease, Chromium, Google Chrome placed at slots 1, 0, 2.
pub(super) const SEC_CH_UA: &str = concat!(
    r#""Chromium";v=""#,
    chrome_major!(),
    r#"", "Not?A_Brand";v="24", "Google Chrome";v=""#,
    chrome_major!(),
    r#"""#
);

/// Chrome's default for an English macOS install.
pub(super) const ACCEPT_LANGUAGE: &str = "en-US,en;q=0.9";

/** Chrome's Accept for a navigation. */
const ACCEPT_DOCUMENT: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7";

/** Chrome's Accept for an image. */
const ACCEPT_IMAGE: &str = "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8";

/** Chrome's Accept for a stylesheet. */
const ACCEPT_STYLE: &str = "text/css,*/*;q=0.1";

/// The destination a request really has; the image budget flag always wins.
pub(super) fn destination(request: &WebRequest) -> Destination {
    if request.image {
        Destination::Image
    } else {
        request.destination
    }
}

/// Every header after the request line or pseudo-headers, in Chrome's order, with lowercase
/// names. `conditional` holds the cache's validators. Client hints and Sec-Fetch-* go only to
/// HTTPS, as Chrome sends them only to potentially trustworthy origins.
pub(super) fn fields(
    url: &Url,
    request: &WebRequest,
    conditional: &[(&'static str, String)],
) -> Vec<(&'static str, String)> {
    let secure = url.scheme() == "https";
    let destination = destination(request);
    let navigate = destination == Destination::Document;
    let post = request.method == Method::Post;
    let initiator = request
        .initiator
        .as_deref()
        .and_then(|value| Url::parse(value).ok());
    let mut fields: Vec<(&'static str, String)> = Vec::new();
    if post && navigate {
        fields.push(("cache-control", "max-age=0".into()));
    }
    if secure {
        fields.push(("sec-ch-ua", SEC_CH_UA.into()));
        fields.push(("sec-ch-ua-mobile", "?0".into()));
        fields.push(("sec-ch-ua-platform", "\"macOS\"".into()));
        fields.extend(super::hints::high_entropy(url));
    }
    if let Some(origin) = origin(request, initiator.as_ref()) {
        fields.push(("origin", origin));
    }
    if let Some(preflight) = &request.preflight {
        fields.push(("access-control-request-method", preflight.method.clone()));
        if !preflight.headers.is_empty() {
            fields.push(("access-control-request-headers", preflight.headers.clone()));
        }
    }
    if post && destination != Destination::Empty {
        fields.push(("content-type", "application/x-www-form-urlencoded".into()));
    }
    if navigate {
        fields.push(("upgrade-insecure-requests", "1".into()));
    }
    fields.push(("user-agent", USER_AGENT.into()));
    if !set_by_page(request, "accept") {
        fields.push(("accept", accept(destination).into()));
    }
    if secure {
        fetch_metadata(&mut fields, url, initiator.as_ref(), request);
    }
    if let Some(referer) = initiator.as_ref().and_then(|from| referer(from, url)) {
        fields.push(("referer", referer));
    }
    let encoding = if secure {
        ACCEPT_ENCODING
    } else {
        ACCEPT_ENCODING_PLAIN
    };
    fields.push(("accept-encoding", encoding.into()));
    if !set_by_page(request, "accept-language") {
        fields.push(("accept-language", ACCEPT_LANGUAGE.into()));
    }
    fields.extend(conditional.iter().cloned());
    if !request.cookie.is_empty() {
        fields.push(("cookie", request.cookie.clone()));
    }
    fields
}

/// The Origin a request sends: a script request's own (the coordinator serialized it the Fetch
/// way, `null` included), or a form POST's initiator origin.
fn origin(request: &WebRequest, initiator: Option<&Url>) -> Option<String> {
    if request.destination == Destination::Empty {
        return request.origin.clone();
    }
    initiator
        .filter(|_| request.method == Method::Post)
        .map(|initiator| initiator.origin().ascii_serialization())
}

/** Whether a script set this header itself, replacing Relay's default for it. */
fn set_by_page(request: &WebRequest, name: &str) -> bool {
    request.headers.iter().any(|(field, _)| field == name)
}

/// The headers a page's script set, sent after Relay's own fields. Validation already refused
/// forbidden names, so none of them can override what Relay owns.
pub(super) fn page_fields(request: &WebRequest) -> impl Iterator<Item = (&str, &str)> {
    request
        .headers
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
}

/// The request method on the wire: OPTIONS for a CORS preflight, otherwise the method.
pub(super) fn method(request: &WebRequest) -> &'static str {
    match &request.preflight {
        Some(_) => "OPTIONS",
        None => request.method.name(),
    }
}

/// The Content-Length a request declares: always for methods defined to carry content (POST,
/// PUT, PATCH), so an empty body reads as 0 rather than as a missing one, and otherwise only
/// when there is a body (RFC 9110 section 8.6).
pub(super) fn content_length(request: &WebRequest) -> Option<usize> {
    let content = matches!(request.method, Method::Post | Method::Put | Method::Patch);
    (content || !request.body.is_empty()).then_some(request.body.len())
}

/// The value this request sends for `name`, or empty when it sends none. The cache compares it
/// for Vary, so it must agree with `fields` and `page_fields` exactly.
pub(super) fn sent(request: &WebRequest, name: &str) -> String {
    let Ok(url) = crate::network::policy::web_url(&request.url) else {
        return String::new();
    };
    fields(&url, request, &[])
        .into_iter()
        .find(|(field, _)| *field == name)
        .map(|(_, value)| value)
        .or_else(|| {
            page_fields(request)
                .find(|(field, _)| *field == name)
                .map(|(_, value)| value.to_owned())
        })
        .unwrap_or_default()
}

/// Chrome's `priority` header (RFC 9218) for HTTP/2, and its HEADERS-frame weight (256 is
/// highest), by destination.
pub(super) fn priority(request: &WebRequest) -> (&'static str, u8) {
    match destination(request) {
        Destination::Document => ("u=0, i", 255),
        Destination::Style => ("u=0", 255),
        Destination::Script => ("u=1", 219),
        Destination::Image => ("u=2, i", 182),
        Destination::Font => ("u=1", 219),
        Destination::Empty => ("u=1, i", 219),
    }
}

/** Accept by destination; scripts and fetches take anything. */
fn accept(destination: Destination) -> &'static str {
    match destination {
        Destination::Document => ACCEPT_DOCUMENT,
        Destination::Image => ACCEPT_IMAGE,
        Destination::Style => ACCEPT_STYLE,
        Destination::Script | Destination::Font | Destination::Empty => "*/*",
    }
}

/** Sec-Fetch-Site, -Mode, -User and -Dest, in Chrome's order; a script request names its mode. */
fn fetch_metadata(
    fields: &mut Vec<(&'static str, String)>,
    url: &Url,
    initiator: Option<&Url>,
    request: &WebRequest,
) {
    let destination = destination(request);
    let (mode, dest) = match destination {
        Destination::Document => ("navigate", "document"),
        Destination::Style => ("no-cors", "style"),
        Destination::Script => ("no-cors", "script"),
        Destination::Image => ("no-cors", "image"),
        // Chrome fetches fonts in CORS mode, as CSS Fonts requires; Mars sends the metadata and
        // does not yet refuse a font whose response lacks Access-Control-Allow-Origin.
        Destination::Font => ("cors", "font"),
        Destination::Empty => (
            request
                .mode
                .filter(|_| request.preflight.is_none())
                .unwrap_or_default()
                .name(),
            "empty",
        ),
    };
    fields.push(("sec-fetch-site", fetch_site(url, initiator).into()));
    fields.push(("sec-fetch-mode", mode.into()));
    if destination == Destination::Document {
        fields.push(("sec-fetch-user", "?1".into()));
    }
    fields.push(("sec-fetch-dest", dest.into()));
}

/** `none` for the person's own navigations, otherwise the initiator's relation to the target. */
fn fetch_site(url: &Url, initiator: Option<&Url>) -> &'static str {
    let Some(initiator) = initiator else {
        return "none";
    };
    if initiator.origin() == url.origin() {
        "same-origin"
    } else if initiator.scheme() == url.scheme()
        && site(initiator.host_str().unwrap_or_default())
            == site(url.host_str().unwrap_or_default())
    {
        "same-site"
    } else {
        "cross-site"
    }
}

/// Chrome's default `strict-origin-when-cross-origin`: the full URL (no fragment or
/// credentials) same-origin, the origin cross-origin, nothing from HTTPS down to HTTP.
fn referer(from: &Url, to: &Url) -> Option<String> {
    if from.scheme() == "https" && to.scheme() != "https" {
        return None;
    }
    if from.origin() == to.origin() {
        let mut full = from.clone();
        full.set_fragment(None);
        return Some(full.into());
    }
    Some(format!("{}/", from.origin().ascii_serialization()))
}

/// HTTP/1.1 spelling: Chrome sends client hints lowercase and title-cases the rest.
pub(super) fn http1_name(name: &str) -> String {
    if name.starts_with("sec-ch-") {
        return name.to_owned();
    }
    name.split('-')
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + chars.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    /** The GREASE brand depends on the major version, so a version move must fail here until
    the brand list is recomputed for it. */
    #[test]
    fn the_brand_list_belongs_to_the_presented_major() {
        assert_eq!(CHROME_ENGINE, "Chrome/152.0.0.0");
        assert_eq!(
            SEC_CH_UA,
            r#""Chromium";v="152", "Not?A_Brand";v="24", "Google Chrome";v="152""#
        );
        assert!(USER_AGENT.contains(CHROME_ENGINE));
    }

    /** Names only, to compare order. */
    fn names(fields: &[(&'static str, String)]) -> Vec<&'static str> {
        fields.iter().map(|(name, _)| *name).collect()
    }

    #[test]
    fn https_navigations_send_chromes_headers_in_chromes_order() {
        let url = Url::parse("https://www.example.com/").expect("url");
        let request = WebRequest {
            cookie: "a=1".into(),
            ..WebRequest::get(url.as_str())
        };
        assert_eq!(
            names(&fields(&url, &request, &[])),
            [
                "sec-ch-ua",
                "sec-ch-ua-mobile",
                "sec-ch-ua-platform",
                "upgrade-insecure-requests",
                "user-agent",
                "accept",
                "sec-fetch-site",
                "sec-fetch-mode",
                "sec-fetch-user",
                "sec-fetch-dest",
                "accept-encoding",
                "accept-language",
                "cookie",
            ]
        );
        assert_eq!(sent(&request, "sec-fetch-site"), "none");
        assert_eq!(sent(&request, "accept-encoding"), "gzip, deflate, br, zstd");
    }

    #[test]
    fn plain_http_omits_client_hints_fetch_metadata_and_br() {
        let request = WebRequest::get("http://example.com/");
        assert_eq!(sent(&request, "sec-ch-ua"), "");
        assert_eq!(sent(&request, "sec-fetch-mode"), "");
        assert_eq!(sent(&request, "accept-encoding"), "gzip, deflate");
    }

    #[test]
    fn subresources_report_their_site_relation_and_destination() {
        let image = WebRequest {
            image: true,
            initiator: Some("https://www.example.com/page#top".into()),
            ..WebRequest::get("https://cdn.example.com/a.png")
        };
        assert_eq!(sent(&image, "sec-fetch-site"), "same-site");
        assert_eq!(sent(&image, "sec-fetch-dest"), "image");
        assert_eq!(sent(&image, "referer"), "https://www.example.com/");
        assert_eq!(sent(&image, "upgrade-insecure-requests"), "");
        let style = WebRequest {
            destination: Destination::Style,
            initiator: Some("https://www.example.com/page#top".into()),
            ..WebRequest::get("https://www.example.com/a.css")
        };
        assert_eq!(sent(&style, "sec-fetch-site"), "same-origin");
        assert_eq!(sent(&style, "referer"), "https://www.example.com/page");
        let other = WebRequest {
            destination: Destination::Script,
            initiator: Some("https://evil.test/".into()),
            ..WebRequest::get("https://www.example.com/a.js")
        };
        assert_eq!(sent(&other, "sec-fetch-site"), "cross-site");
    }

    #[test]
    fn posts_carry_origin_only_when_the_initiator_is_known() {
        let mut post = WebRequest {
            method: Method::Post,
            ..WebRequest::get("https://a.example/login")
        };
        assert_eq!(sent(&post, "origin"), "");
        post.initiator = Some("https://a.example/form".into());
        assert_eq!(sent(&post, "origin"), "https://a.example");
        assert_eq!(sent(&post, "cache-control"), "max-age=0");
    }

    #[test]
    fn script_requests_send_their_own_origin_headers_and_mode() {
        let fetch = WebRequest {
            destination: Destination::Empty,
            method: Method::Post,
            initiator: Some("https://a.example/".into()),
            origin: Some("null".into()),
            mode: Some(crate::network::cors::Mode::NoCors),
            headers: vec![
                ("content-type".into(), "text/plain;charset=UTF-8".into()),
                ("accept".into(), "application/json".into()),
            ],
            ..WebRequest::get("https://b.example/api")
        };
        assert_eq!(sent(&fetch, "origin"), "null");
        assert_eq!(sent(&fetch, "content-type"), "text/plain;charset=UTF-8");
        assert_eq!(sent(&fetch, "accept"), "application/json");
        assert_eq!(sent(&fetch, "sec-fetch-mode"), "no-cors");
        let url = Url::parse(&fetch.url).expect("url");
        assert_eq!(
            fields(&url, &fetch, &[])
                .iter()
                .filter(|(name, _)| *name == "accept")
                .count(),
            0
        );
        let plain = WebRequest {
            destination: Destination::Empty,
            ..WebRequest::get("https://b.example/api")
        };
        assert_eq!(sent(&plain, "origin"), "");
        assert_eq!(sent(&plain, "sec-fetch-mode"), "cors");
    }

    #[test]
    fn a_preflight_is_an_options_request_naming_the_real_one() {
        let preflight = WebRequest {
            destination: Destination::Empty,
            origin: Some("https://a.example".into()),
            preflight: Some(crate::network::Preflight {
                method: "POST".into(),
                headers: "content-type,x-token".into(),
            }),
            ..WebRequest::get("https://b.example/api")
        };
        assert_eq!(method(&preflight), "OPTIONS");
        assert_eq!(sent(&preflight, "access-control-request-method"), "POST");
        assert_eq!(
            sent(&preflight, "access-control-request-headers"),
            "content-type,x-token"
        );
        assert_eq!(sent(&preflight, "sec-fetch-mode"), "cors");
        assert_eq!(sent(&preflight, "cookie"), "");
        preflight.validate().expect("valid");
    }

    #[test]
    fn page_set_fields_are_refused_outside_script_requests_and_when_forbidden() {
        let navigation = WebRequest {
            headers: vec![("x-token".into(), "1".into())],
            ..WebRequest::get("https://b.example/")
        };
        assert!(navigation.validate().is_err());
        let forbidden = WebRequest {
            destination: Destination::Empty,
            headers: vec![("cookie".into(), "a=b".into())],
            ..WebRequest::get("https://b.example/")
        };
        assert!(forbidden.validate().is_err());
        let injected = WebRequest {
            destination: Destination::Empty,
            headers: vec![("x-token".into(), "1\r\nhost: evil".into())],
            ..WebRequest::get("https://b.example/")
        };
        assert!(injected.validate().is_err());
        let spoofed = WebRequest {
            destination: Destination::Empty,
            origin: Some("https://a.example/path".into()),
            ..WebRequest::get("https://b.example/")
        };
        assert!(spoofed.validate().is_err());
    }

    #[test]
    fn https_pages_send_no_referer_to_http() {
        let request = WebRequest {
            initiator: Some("https://a.example/secret".into()),
            ..WebRequest::get("http://b.example/")
        };
        assert_eq!(sent(&request, "referer"), "");
    }

    #[test]
    fn http1_names_are_spelled_like_chrome() {
        assert_eq!(http1_name("sec-ch-ua-mobile"), "sec-ch-ua-mobile");
        assert_eq!(http1_name("sec-fetch-site"), "Sec-Fetch-Site");
        assert_eq!(
            http1_name("upgrade-insecure-requests"),
            "Upgrade-Insecure-Requests"
        );
    }
}
