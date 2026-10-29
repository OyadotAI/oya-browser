/*!
 * Whether a document lets another origin's page frame it: CSP `frame-ancestors` (CSP Level 3),
 * or without it `X-Frame-Options`, read from the document's response. Only frames that run their
 * own scripts consult it (`frame_pages`); a refused document is not embedded as a page. The
 * both same-origin and cross-origin frames use the same policy checks.
 */

/** Whether `headers` admit `embedder` as the framing page. Every policy that names
`frame-ancestors` must allow it; XFO DENY or SAMEORIGIN refuses when none does. */
pub(super) fn may_frame(
    headers: &[(String, String)],
    embedder: &url::Url,
    framed: &url::Url,
) -> bool {
    let header = |name: &str| {
        headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    };
    let ancestors: Vec<&str> = header("content-security-policy")
        .map(|value| value.split(',').filter_map(frame_ancestors).collect())
        .unwrap_or_default();
    if !ancestors.is_empty() {
        return ancestors
            .iter()
            .all(|sources| allows(sources, embedder, framed));
    }
    !header("x-frame-options").is_some_and(|value| {
        let value = value.split(',').next().unwrap_or("").trim();
        value.eq_ignore_ascii_case("deny")
            || value.eq_ignore_ascii_case("sameorigin") && embedder.origin() != framed.origin()
    })
}

/** The source list of one policy's `frame-ancestors` directive, if it has one. */
fn frame_ancestors(policy: &str) -> Option<&str> {
    policy.split(';').find_map(|directive| {
        let directive = directive.trim();
        let (name, sources) = directive
            .split_once(char::is_whitespace)
            .unwrap_or((directive, ""));
        name.eq_ignore_ascii_case("frame-ancestors")
            .then_some(sources)
    })
}

/** Whether one source list matches `embedder`: `*`, a scheme source or a host source. */
fn allows(sources: &str, embedder: &url::Url, framed: &url::Url) -> bool {
    sources
        .split_ascii_whitespace()
        .any(|source| matches(source, embedder, framed))
}

/** One source expression against the embedder's origin. */
fn matches(source: &str, embedder: &url::Url, framed: &url::Url) -> bool {
    if source.eq_ignore_ascii_case("'self'") {
        return embedder.origin() == framed.origin();
    }
    let scheme = embedder.scheme();
    if source == "*" {
        return matches!(scheme, "http" | "https");
    }
    if source.starts_with('\'') {
        return false;
    }
    if let Some(only) = source.strip_suffix(':') {
        return only.eq_ignore_ascii_case(scheme) || (only == "http" && scheme == "https");
    }
    let (source_scheme, rest) = match source.split_once("://") {
        Some((s, rest)) => (Some(s), rest),
        None => (None, source),
    };
    if let Some(s) = source_scheme
        && !s.eq_ignore_ascii_case(scheme)
        && !(s == "http" && scheme == "https")
    {
        return false;
    }
    let host_port = rest.split('/').next().unwrap_or("");
    let (host, port) = match host_port.rsplit_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (host_port, None),
    };
    let Some(embedder_host) = embedder.host_str() else {
        return false;
    };
    let host_ok = match host.strip_prefix("*.") {
        Some(suffix) => embedder_host
            .strip_suffix(suffix)
            .is_some_and(|prefix| prefix.ends_with('.')),
        None => host.eq_ignore_ascii_case(embedder_host),
    };
    let port_ok = match port {
        Some("*") => true,
        Some(port) => port.parse::<u16>().ok() == embedder.port_or_known_default(),
        None => embedder.port().is_none(),
    };
    host_ok && port_ok
}

#[cfg(test)]
mod tests {
    use super::may_frame;

    fn page() -> url::Url {
        url::Url::parse("https://www.google.com/").unwrap()
    }
    /** A different origin used to prove same-origin policies reject cross-origin framing. */
    fn other() -> url::Url {
        url::Url::parse("https://accounts.google.com/").unwrap()
    }
    fn csp(value: &str) -> Vec<(String, String)> {
        vec![("content-security-policy".into(), value.into())]
    }

    /** A document with no framing headers may be framed. */
    #[test]
    fn no_headers_allow_framing() {
        assert!(may_frame(&[], &page(), &other()));
    }

    /** X-Frame-Options DENY and SAMEORIGIN refuse another origin. */
    #[test]
    fn x_frame_options_refuses() {
        for value in ["DENY", "sameorigin"] {
            assert!(!may_frame(
                &[("x-frame-options".into(), value.into())],
                &page(),
                &other()
            ));
        }
    }

    /** frame-ancestors host wildcards and schemes admit, 'none' and 'self' refuse. */
    #[test]
    fn frame_ancestors_sources() {
        assert!(may_frame(
            &csp("frame-ancestors https://*.google.com"),
            &page(),
            &other()
        ));
        assert!(may_frame(
            &csp("default-src 'self'; frame-ancestors https:"),
            &page(),
            &other()
        ));
        assert!(!may_frame(
            &csp("frame-ancestors 'none'"),
            &page(),
            &other()
        ));
        assert!(may_frame(&csp("frame-ancestors 'self'"), &page(), &page()));
        assert!(!may_frame(
            &csp("frame-ancestors 'self'"),
            &page(),
            &other()
        ));
        assert!(!may_frame(
            &csp("frame-ancestors https://example.com"),
            &page(),
            &other()
        ));
    }

    /** frame-ancestors overrides X-Frame-Options. */
    #[test]
    fn frame_ancestors_overrides_x_frame_options() {
        let mut headers = csp("frame-ancestors https://www.google.com");
        headers.push(("x-frame-options".into(), "DENY".into()));
        assert!(may_frame(&headers, &page(), &page()));
    }
}
