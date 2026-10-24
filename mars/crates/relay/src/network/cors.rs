/**
 * The Fetch standard's CORS rules as pure functions: which request headers a page may set,
 * which ones make a request need a preflight, whether a response passes the CORS check, what a
 * preflight allows, which response headers a script may read, and how a redirect taints the
 * request's origin. Relay sends only what these allow; the coordinator decides with them hop by
 * hop, so a page never reads a cross-origin response the server did not share with it.
 */
use crate::network::constants::{
    PREFLIGHT_DEFAULT_AGE, PREFLIGHT_ENTRIES, PREFLIGHT_MAX_AGE, SAFELISTED_TOTAL_BYTES,
    SAFELISTED_VALUE_BYTES, SCRIPT_RESPONSE_HEADERS,
};
use ::url::{Origin, Url};
use std::time::{Duration, Instant};

/// Request headers no page may set (Fetch's forbidden request-header names), plus the ones Relay
/// owns for the presented browser (`user-agent`, `priority`), so a script cannot contradict them.
const FORBIDDEN_REQUEST: [&str; 27] = [
    "accept-charset",
    "accept-encoding",
    "access-control-request-headers",
    "access-control-request-method",
    "access-control-request-private-network",
    "connection",
    "content-length",
    "cookie",
    "cookie2",
    "date",
    "dnt",
    "expect",
    "host",
    "keep-alive",
    "origin",
    "referer",
    "set-cookie",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "via",
    "x-http-method",
    "x-http-method-override",
    "x-method-override",
    "user-agent",
    "priority",
];

/// Response headers a script never sees, whatever the server or a CORS grant says.
const FORBIDDEN_RESPONSE: [&str; 2] = ["set-cookie", "set-cookie2"];

/// Fetch's CORS-safelisted response-header names, readable on any CORS response.
const SAFELISTED_RESPONSE: [&str; 7] = [
    "cache-control",
    "content-language",
    "content-length",
    "content-type",
    "expires",
    "last-modified",
    "pragma",
];

/// Content types a page may send cross-origin without a preflight (an HTML form could too).
const SIMPLE_TYPES: [&str; 3] = [
    "application/x-www-form-urlencoded",
    "multipart/form-data",
    "text/plain",
];

/// A script request's mode, which decides what a cross-origin response may be.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// Cross-origin responses are readable only when the server passes the CORS check.
    #[default]
    Cors,
    /// Cross-origin responses come back opaque: status 0, no headers, no body.
    NoCors,
    /// Any cross-origin hop is a network error.
    SameOrigin,
}

impl Mode {
    /// The mode a page named, or None for one the Fetch standard does not let it use.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "" | "cors" => Some(Self::Cors),
            "no-cors" => Some(Self::NoCors),
            "same-origin" => Some(Self::SameOrigin),
            _ => None,
        }
    }
    /// The Sec-Fetch-Mode value Chrome sends for it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Cors => "cors",
            Self::NoCors => "no-cors",
            Self::SameOrigin => "same-origin",
        }
    }
}

/// What a response is to the page, the Fetch standard's response tainting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tainting {
    /// Same-origin all the way: everything but Set-Cookie is readable.
    Basic,
    /// Cross-origin under CORS: readable only past the CORS check, and only exposed headers.
    Cors,
    /// Cross-origin under no-cors: nothing is readable.
    Opaque,
}

/// One request's origin bookkeeping across redirects: its tainting and whether a redirect
/// tainted its origin, which makes it send and be checked against `Origin: null`.
#[derive(Clone, Debug)]
pub struct Flow {
    /// The page's origin, the request's origin.
    origin: Origin,
    /// The request's mode.
    mode: Mode,
    /// Set once a cross-origin hop happened; it never goes back to basic.
    tainting: Tainting,
    /// Fetch's tainted origin flag.
    tainted: bool,
}

impl Flow {
    /// A request a page at `origin` makes in `mode`.
    pub fn new(origin: Origin, mode: Mode) -> Self {
        Self {
            origin,
            mode,
            tainting: Tainting::Basic,
            tainted: false,
        }
    }
    /// Enter a hop to `url`: a cross-origin one makes the response CORS or opaque from then on.
    /// Returns false when the mode forbids the hop (same-origin mode leaving the origin).
    pub fn enter(&mut self, url: &Url) -> bool {
        if url.origin() == self.origin || self.tainting != Tainting::Basic {
            return true;
        }
        self.tainting = match self.mode {
            Mode::Cors => Tainting::Cors,
            Mode::NoCors => Tainting::Opaque,
            Mode::SameOrigin => return false,
        };
        true
    }
    /// Follow a redirect from `from` to `to`. Fetch taints the origin when the redirect crosses
    /// origins and the request had already left its own origin.
    pub fn redirect(&mut self, from: &Url, to: &Url) {
        if from.origin() != to.origin() && self.origin != from.origin() {
            self.tainted = true;
        }
    }
    /// The response tainting so far.
    pub fn tainting(&self) -> Tainting {
        self.tainting
    }
    /// The Origin value the request sends and CORS checks compare against.
    pub fn serialized(&self) -> String {
        if self.tainted {
            "null".into()
        } else {
            self.origin.ascii_serialization()
        }
    }
}

/// Whether a page may set this request header at all; forbidden ones are dropped.
pub fn forbidden_request_header(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    FORBIDDEN_REQUEST.contains(&name.as_str())
        || name.starts_with("proxy-")
        || name.starts_with("sec-")
}

/// Fetch's CORS-unsafe request-header bytes.
fn unsafe_byte(byte: u8) -> bool {
    (byte < 0x20 && byte != b'\t') || b"\"():<>?@[\\]{}\x7f".contains(&byte)
}

/// Whether one header is CORS-safelisted, so it needs no preflight.
pub fn safelisted_request_header(name: &str, value: &str) -> bool {
    if value.len() > SAFELISTED_VALUE_BYTES {
        return false;
    }
    let language = |value: &str| {
        value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b" *,-.;=".contains(&byte))
    };
    match name.to_ascii_lowercase().as_str() {
        "accept" => !value.bytes().any(unsafe_byte),
        "accept-language" | "content-language" => language(value),
        "content-type" => {
            !value.bytes().any(unsafe_byte) && SIMPLE_TYPES.contains(&essence(value).as_str())
        }
        _ => false,
    }
}

/** A media type's lowercase essence, without parameters. */
fn essence(value: &str) -> String {
    value
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

/// Fetch's CORS-unsafe request-header names: the sorted lowercase names a preflight must ask
/// for. Safelisted headers join them when their values pass 1024 bytes in total.
pub fn unsafe_header_names(headers: &[(String, String)]) -> Vec<String> {
    let mut unsafe_names = Vec::new();
    let mut safelisted = Vec::new();
    let mut size = 0;
    for (name, value) in headers {
        let name = name.to_ascii_lowercase();
        if safelisted_request_header(&name, value) {
            size += value.len();
            safelisted.push(name);
        } else {
            unsafe_names.push(name);
        }
    }
    if size > SAFELISTED_TOTAL_BYTES {
        unsafe_names.extend(safelisted);
    }
    unsafe_names.sort();
    unsafe_names.dedup();
    unsafe_names
}

/// GET, HEAD and POST are CORS-safelisted methods.
pub fn safelisted_method(method: &str) -> bool {
    ["GET", "HEAD", "POST"].contains(&method)
}

/// Whether a cross-origin request needs a preflight before it is sent.
pub fn needs_preflight(method: &str, headers: &[(String, String)]) -> bool {
    !safelisted_method(method) || !unsafe_header_names(headers).is_empty()
}

/** The one value a response sent for `name`, joined as Relay joins repeats. */
fn value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(field, _)| field.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim())
}

/// The CORS check. Access-Control-Allow-Origin must be `*` (only without credentials) or the
/// request's serialized origin exactly; with credentials, Access-Control-Allow-Credentials must
/// be `true`. A repeated Allow-Origin arrives joined with a comma and so never matches.
pub fn allows(headers: &[(String, String)], origin: &str, credentialed: bool) -> bool {
    let Some(allowed) = value(headers, "access-control-allow-origin") else {
        return false;
    };
    if !credentialed && allowed == "*" {
        return true;
    }
    if allowed != origin {
        return false;
    }
    !credentialed || value(headers, "access-control-allow-credentials") == Some("true")
}

/** A comma-separated token list, or None when an item is not an HTTP token. */
fn tokens(headers: &[(String, String)], name: &str) -> Option<Vec<String>> {
    let Some(list) = value(headers, name) else {
        return Some(Vec::new());
    };
    let mut items = Vec::new();
    for item in list
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        if !item
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
        {
            return None;
        }
        items.push(item.to_owned());
    }
    Some(items)
}

/// What one preflight allowed, for as long as its max-age says.
#[derive(Clone, Debug)]
pub struct Allowance {
    /// Access-Control-Allow-Methods, as sent (methods compare case-sensitively).
    pub methods: Vec<String>,
    /// Access-Control-Allow-Headers, lowercased (header names compare case-insensitively).
    pub headers: Vec<String>,
    /// Access-Control-Max-Age, defaulted and capped.
    pub max_age: Duration,
}

impl Allowance {
    /// Whether this allowance covers `method` and every one of `unsafe_names`. A `*` wildcard
    /// counts only without credentials, and never stands in for Authorization.
    pub fn covers(&self, method: &str, unsafe_names: &[String], credentialed: bool) -> bool {
        let any = |list: &[String]| !credentialed && list.iter().any(|item| item == "*");
        let method_ok = safelisted_method(method)
            || self.methods.iter().any(|item| item == method)
            || any(&self.methods);
        method_ok
            && unsafe_names.iter().all(|name| {
                self.headers.contains(name) || (any(&self.headers) && name != "authorization")
            })
    }
}

/// Read a preflight response: a 2xx that passes the CORS check and lists valid tokens gives an
/// allowance; anything else (including a redirect) is a network error for the real request.
pub fn preflight(
    status: u16,
    headers: &[(String, String)],
    origin: &str,
    credentialed: bool,
) -> Option<Allowance> {
    if !(200..300).contains(&status) || !allows(headers, origin, credentialed) {
        return None;
    }
    let methods = tokens(headers, "access-control-allow-methods")?;
    let allowed = tokens(headers, "access-control-allow-headers")?
        .into_iter()
        .map(|name| name.to_ascii_lowercase())
        .collect();
    let max_age = value(headers, "access-control-max-age")
        .and_then(|age| age.parse::<u64>().ok())
        .map_or(PREFLIGHT_DEFAULT_AGE, Duration::from_secs)
        .min(PREFLIGHT_MAX_AGE);
    Some(Allowance {
        methods,
        headers: allowed,
        max_age,
    })
}

/// The response headers a script may read: all but Set-Cookie on a basic response, the
/// safelisted names plus Access-Control-Expose-Headers on a CORS one (`*` exposes all only
/// without credentials), none on an opaque one. Bounded to SCRIPT_RESPONSE_HEADERS.
pub fn exposed(
    headers: &[(String, String)],
    tainting: Tainting,
    credentialed: bool,
) -> Vec<(String, String)> {
    let listed = tokens(headers, "access-control-expose-headers").unwrap_or_default();
    let listed: Vec<String> = listed
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect();
    let everything = !credentialed && listed.iter().any(|name| name == "*");
    headers
        .iter()
        .filter(|(name, _)| !FORBIDDEN_RESPONSE.contains(&name.as_str()))
        .filter(|(name, _)| match tainting {
            Tainting::Basic => true,
            Tainting::Cors => {
                everything || SAFELISTED_RESPONSE.contains(&name.as_str()) || listed.contains(name)
            }
            Tainting::Opaque => false,
        })
        .take(SCRIPT_RESPONSE_HEADERS)
        .cloned()
        .collect()
}

/// Response headers as Relay hands them to the coordinator: lowercase, Set-Cookie never among
/// them, bounded to SCRIPT_RESPONSE_HEADERS.
pub fn response_headers<'a>(
    headers: impl IntoIterator<Item = (&'a String, &'a String)>,
) -> Vec<(String, String)> {
    headers
        .into_iter()
        .filter(|(name, _)| !FORBIDDEN_RESPONSE.contains(&name.as_str()))
        .take(SCRIPT_RESPONSE_HEADERS)
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

/** One remembered preflight. */
#[derive(Clone, Debug)]
struct Entry {
    /// The serialized request origin it answered (`null` included).
    origin: String,
    /// The exact URL it answered.
    url: String,
    /// Whether it was for a credentialed request.
    credentialed: bool,
    /// What it allowed.
    allowance: Allowance,
    /// When it stops counting.
    expires: Instant,
}

/// Fetch's CORS-preflight cache for one page: keyed by origin, URL and credentials, honoring
/// max-age (capped), and bounded to PREFLIGHT_ENTRIES so a page cannot grow it without end.
#[derive(Clone, Debug, Default)]
pub struct PreflightCache {
    /// Newest last.
    entries: Vec<Entry>,
}

impl PreflightCache {
    /// Whether a live entry already allows this request, so no preflight is sent.
    pub fn covers(
        &self,
        origin: &str,
        url: &Url,
        credentialed: bool,
        method: &str,
        unsafe_names: &[String],
        now: Instant,
    ) -> bool {
        self.entries.iter().any(|entry| {
            entry.origin == origin
                && entry.url == url.as_str()
                && entry.credentialed == credentialed
                && entry.expires > now
                && entry.allowance.covers(method, unsafe_names, credentialed)
        })
    }
    /// Remember a preflight's allowance, replacing an older one for the same key.
    pub fn store(
        &mut self,
        origin: &str,
        url: &Url,
        credentialed: bool,
        allowance: Allowance,
        now: Instant,
    ) {
        self.entries.retain(|entry| {
            entry.expires > now
                && !(entry.origin == origin
                    && entry.url == url.as_str()
                    && entry.credentialed == credentialed)
        });
        if allowance.max_age.is_zero() {
            return;
        }
        if self.entries.len() >= PREFLIGHT_ENTRIES {
            self.entries.remove(0);
        }
        self.entries.push(Entry {
            origin: origin.into(),
            url: url.as_str().into(),
            credentialed,
            expires: now + allowance.max_age,
            allowance,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Header pairs from string slices. */
    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(name, value)| ((*name).into(), (*value).into()))
            .collect()
    }

    /** A URL from text. */
    fn url(text: &str) -> Url {
        Url::parse(text).expect("url")
    }

    #[test]
    fn wildcard_allows_only_requests_without_credentials() {
        let star = pairs(&[("access-control-allow-origin", "*")]);
        assert!(allows(&star, "https://a.example", false));
        assert!(!allows(&star, "https://a.example", true));
    }

    #[test]
    fn credentialed_requests_need_the_exact_origin_and_allow_credentials() {
        let exact = pairs(&[("access-control-allow-origin", "https://a.example")]);
        assert!(allows(&exact, "https://a.example", false));
        assert!(!allows(&exact, "https://a.example", true));
        let mut both = exact.clone();
        both.push(("access-control-allow-credentials".into(), "true".into()));
        assert!(allows(&both, "https://a.example", true));
        assert!(!allows(&both, "https://b.example", true));
    }

    #[test]
    fn repeated_allow_origin_values_never_pass() {
        let joined = pairs(&[(
            "access-control-allow-origin",
            "https://a.example, https://b.example",
        )]);
        assert!(!allows(&joined, "https://a.example", false));
        assert!(!allows(&[], "https://a.example", false));
    }

    #[test]
    fn a_redirect_across_origins_after_leaving_the_origin_taints_it() {
        let page = url("http://a.example/");
        let mut flow = Flow::new(page.origin(), Mode::Cors);
        let other = url("http://b.example/hop");
        assert!(flow.enter(&other));
        assert_eq!(flow.tainting(), Tainting::Cors);
        assert_eq!(flow.serialized(), "http://a.example");
        let back = url("http://a.example/data");
        flow.redirect(&other, &back);
        assert!(flow.enter(&back));
        assert_eq!(flow.serialized(), "null");
        assert_eq!(flow.tainting(), Tainting::Cors);
    }

    #[test]
    fn a_redirect_out_of_the_page_origin_does_not_taint_it() {
        let page = url("http://a.example/");
        let mut flow = Flow::new(page.origin(), Mode::Cors);
        let other = url("http://b.example/data");
        flow.redirect(&page, &other);
        assert_eq!(flow.serialized(), "http://a.example");
    }

    #[test]
    fn same_origin_mode_refuses_a_cross_origin_hop_and_no_cors_goes_opaque() {
        let page = url("http://a.example/");
        let other = url("http://b.example/");
        assert!(!Flow::new(page.origin(), Mode::SameOrigin).enter(&other));
        let mut flow = Flow::new(page.origin(), Mode::NoCors);
        assert!(flow.enter(&other));
        assert_eq!(flow.tainting(), Tainting::Opaque);
    }

    #[test]
    fn simple_headers_need_no_preflight_and_others_do() {
        let simple = pairs(&[
            ("content-type", "text/plain;charset=UTF-8"),
            ("accept", "*/*"),
        ]);
        assert!(!needs_preflight("POST", &simple));
        let json = pairs(&[("content-type", "application/json")]);
        assert_eq!(unsafe_header_names(&json), ["content-type"]);
        let custom = pairs(&[("X-Token", "1"), ("Authorization", "a")]);
        assert_eq!(unsafe_header_names(&custom), ["authorization", "x-token"]);
        assert!(needs_preflight("PUT", &[]));
    }

    #[test]
    fn forbidden_request_headers_include_prefixes_and_relay_owned_names() {
        for name in [
            "Cookie",
            "sec-fetch-site",
            "Proxy-Authorization",
            "user-agent",
        ] {
            assert!(forbidden_request_header(name), "{name}");
        }
        assert!(!forbidden_request_header("x-token"));
    }

    #[test]
    fn a_preflight_allows_listed_headers_and_wildcards_only_without_credentials() {
        let headers = pairs(&[
            ("access-control-allow-origin", "https://a.example"),
            ("access-control-allow-headers", "X-Token"),
            ("access-control-allow-methods", "PUT"),
            ("access-control-max-age", "999999"),
        ]);
        let allowance = preflight(204, &headers, "https://a.example", false).expect("allowed");
        assert_eq!(allowance.max_age, PREFLIGHT_MAX_AGE);
        assert!(allowance.covers("PUT", &["x-token".into()], false));
        assert!(!allowance.covers("DELETE", &[], false));
        assert!(!allowance.covers("GET", &["x-other".into()], false));
        let star = Allowance {
            methods: vec!["*".into()],
            headers: vec!["*".into()],
            max_age: PREFLIGHT_DEFAULT_AGE,
        };
        assert!(star.covers("PUT", &["x-token".into()], false));
        assert!(!star.covers("PUT", &[], true));
        assert!(!star.covers("GET", &["authorization".into()], false));
    }

    #[test]
    fn a_failed_or_redirected_preflight_allows_nothing() {
        let headers = pairs(&[("access-control-allow-origin", "*")]);
        assert!(preflight(302, &headers, "https://a.example", false).is_none());
        assert!(preflight(200, &[], "https://a.example", false).is_none());
        let bad = pairs(&[
            ("access-control-allow-origin", "*"),
            ("access-control-allow-headers", "x token"),
        ]);
        assert!(preflight(200, &bad, "https://a.example", false).is_none());
    }

    #[test]
    fn cors_responses_expose_safelisted_and_listed_headers_never_cookies() {
        let headers = pairs(&[
            ("content-type", "text/plain"),
            ("x-secret", "1"),
            ("x-shared", "2"),
            ("set-cookie", "a=b"),
            ("access-control-expose-headers", "X-Shared, set-cookie"),
        ]);
        let names = |list: Vec<(String, String)>| -> Vec<String> {
            list.into_iter().map(|(name, _)| name).collect()
        };
        assert_eq!(
            names(exposed(&headers, Tainting::Cors, false)),
            ["content-type", "x-shared"]
        );
        assert!(exposed(&headers, Tainting::Opaque, false).is_empty());
        assert!(!names(exposed(&headers, Tainting::Basic, false)).contains(&"set-cookie".into()));
        let mut star = headers.clone();
        star[4].1 = "*".into();
        assert_eq!(names(exposed(&star, Tainting::Cors, false)).len(), 4);
        assert_eq!(names(exposed(&star, Tainting::Cors, true)).len(), 1);
    }

    #[test]
    fn the_preflight_cache_honors_key_expiry_and_bound() {
        let now = Instant::now();
        let target = url("https://b.example/api");
        let allowance = Allowance {
            methods: Vec::new(),
            headers: vec!["x-token".into()],
            max_age: PREFLIGHT_DEFAULT_AGE,
        };
        let mut cache = PreflightCache::default();
        cache.store("https://a.example", &target, false, allowance.clone(), now);
        let names = ["x-token".to_string()];
        assert!(cache.covers("https://a.example", &target, false, "GET", &names, now));
        assert!(!cache.covers("https://a.example", &target, true, "GET", &names, now));
        assert!(!cache.covers("null", &target, false, "GET", &names, now));
        let later = now + PREFLIGHT_DEFAULT_AGE;
        assert!(!cache.covers("https://a.example", &target, false, "GET", &names, later));
        for index in 0..=PREFLIGHT_ENTRIES {
            let other = url(&format!("https://b.example/{index}"));
            cache.store("https://a.example", &other, false, allowance.clone(), now);
        }
        assert_eq!(cache.entries.len(), PREFLIGHT_ENTRIES);
    }
}
