/** Content-Security-Policy from metadata and responses governs script and worker acquisition. */
use ring::digest::{SHA256, SHA384, SHA512, digest};
use url::Url;

mod handlers;

/** Refresh both independent script policies from the same current document policy list. */
pub(super) fn apply_environment(policies: &[String], environment: &mut crate::v9::PageEnvironment) {
    environment.disallow_code_generation_from_strings = !allows_eval(policies);
    environment.handler_policy = handlers::compile(policies);
}

/** HTTP fields may combine several serialized policies; each remains independently enforced. */
pub(super) fn response_policies(headers: &[(String, String)]) -> Vec<String> {
    headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("content-security-policy"))
        .flat_map(|(_, value)| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

/** Dynamic source is allowed only when every effective response policy permits unsafe-eval. */
pub(super) fn allows_eval(policies: &[String]) -> bool {
    policies
        .iter()
        .filter_map(|policy| effective_sources(policy, &["script-src", "default-src"]))
        .all(|sources| {
            sources
                .iter()
                .any(|source| source.eq_ignore_ascii_case("'unsafe-eval'"))
        })
}

/// The script facts a policy decides on.
pub(super) struct Candidate<'a> {
    /// Resolved URL for an external script, None for inline.
    pub url: Option<&'a Url>,
    /// The element's `nonce` attribute.
    pub nonce: Option<&'a str>,
    /// Inline source text, hashed for `'sha*-'` sources.
    pub source: &'a str,
}

/// True when every policy allows the script, as browsers require for multiple policies.
pub(super) fn allows(policies: &[String], document: &Url, script: &Candidate<'_>) -> bool {
    policies
        .iter()
        .filter_map(|policy| script_sources(policy))
        .all(|sources| allowed(&sources, document, script))
}

/** The effective script source list: script-src-elem, then script-src, then default-src. */
fn script_sources(policy: &str) -> Option<Vec<String>> {
    effective_sources(policy, &["script-src-elem", "script-src", "default-src"])
}

/** Worker fetches enforce connect-src on every hop, including requests that need preflight. */
pub(super) fn allows_connect(policies: &[String], origin: &Url, url: &Url) -> bool {
    policies
        .iter()
        .filter_map(|policy| effective_sources(policy, &["connect-src", "default-src"]))
        .all(|sources| {
            sources
                .iter()
                .any(|source| matches_source(source, origin, url))
        })
}

/** Worker creation and imported scripts have distinct fallback lists from script elements. */
pub(super) fn allows_worker(policies: &[String], origin: &Url, url: &Url, initial: bool) -> bool {
    policies.iter().all(|policy| {
        if initial && let Some(sources) = effective_sources(policy, &["worker-src", "child-src"]) {
            return sources
                .iter()
                .any(|source| matches_source(source, origin, url));
        }
        let Some(sources) = effective_sources(policy, &["script-src", "default-src"]) else {
            return true;
        };
        sources
            .iter()
            .any(|source| source.eq_ignore_ascii_case("'strict-dynamic'"))
            || allowed(
                &sources,
                origin,
                &Candidate {
                    url: Some(url),
                    nonce: None,
                    source: "",
                },
            )
    })
}

/** The first effective directive wins; a later duplicate never broadens its policy. */
fn effective_sources(policy: &str, fallback: &[&str]) -> Option<Vec<String>> {
    let directives: Vec<(String, Vec<String>)> = policy
        .split(';')
        .filter_map(|directive| {
            let mut parts = directive.split_ascii_whitespace();
            let name = parts.next()?.to_ascii_lowercase();
            Some((name, parts.map(str::to_owned).collect()))
        })
        .collect();
    fallback.iter().find_map(|wanted| {
        directives
            .iter()
            .find(|(name, _)| name == wanted)
            .map(|(_, sources)| sources.clone())
    })
}

/** One source list's verdict; nonces and hashes also disable 'unsafe-inline', per CSP3. */
fn allowed(sources: &[String], document: &Url, script: &Candidate<'_>) -> bool {
    let lower: Vec<String> = sources.iter().map(|s| s.to_ascii_lowercase()).collect();
    if script
        .nonce
        .is_some_and(|nonce| sources.iter().any(|s| s == &format!("'nonce-{nonce}'")))
    {
        return true;
    }
    let Some(url) = script.url else {
        let keyed = lower
            .iter()
            .any(|s| s.starts_with("'nonce-") || s.starts_with("'sha"));
        return hash_matches(sources, script.source)
            || (!keyed && lower.iter().any(|s| s == "'unsafe-inline'"));
    };
    if lower.iter().any(|s| s == "'strict-dynamic'") {
        return false;
    }
    sources
        .iter()
        .any(|source| matches_source(source, document, url))
}

/** `'sha256-…'` style sources against the inline text. */
fn hash_matches(sources: &[String], text: &str) -> bool {
    let algorithms = [
        ("'sha256-", &SHA256),
        ("'sha384-", &SHA384),
        ("'sha512-", &SHA512),
    ];
    sources.iter().any(|source| {
        algorithms.iter().any(|(prefix, algorithm)| {
            source
                .strip_prefix(prefix)
                .and_then(|rest| rest.strip_suffix('\''))
                .and_then(super::integrity::base64)
                .is_some_and(|hash| hash == digest(algorithm, text.as_bytes()).as_ref())
        })
    })
}

/** 'self', scheme sources (`https:`), `*`, and host sources with optional scheme, port and path. */
fn matches_source(source: &str, document: &Url, url: &Url) -> bool {
    // Blob workers inherit the creator policy, including its scheme for scheme-less host sources.
    // Keep the actual Blob URL elsewhere: it remains the module and relative-request base.
    let inherited = (document.scheme() == "blob")
        .then(|| Url::parse(&document.origin().ascii_serialization()).ok())
        .flatten();
    let document = inherited.as_ref().unwrap_or(document);
    let lower = source.to_ascii_lowercase();
    match lower.as_str() {
        "'self'" => url.origin() == document.origin(),
        "*" => matches!(url.scheme(), "http" | "https"),
        _ if lower.starts_with('\'') => false,
        _ if lower.ends_with(':') && !lower.contains('/') => {
            let scheme = &lower[..lower.len() - 1];
            url.scheme() == scheme || (scheme == "http" && url.scheme() == "https")
        }
        _ => host_source(source, document, url),
    }
}

/** `[scheme://]host[:port][/path]`, with `*.` host wildcards and secure-scheme upgrades. */
fn host_source(source: &str, document: &Url, url: &Url) -> bool {
    let (scheme, rest) = match source.split_once("://") {
        Some((scheme, rest)) => (Some(scheme.to_ascii_lowercase()), rest),
        None => (None, source),
    };
    let scheme_ok = match scheme.as_deref() {
        Some(scheme) => url.scheme() == scheme || (scheme == "http" && url.scheme() == "https"),
        None => url.scheme() == document.scheme() || url.scheme() == "https",
    };
    let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    let host = host.to_ascii_lowercase();
    let target = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let host_ok = match host.strip_prefix("*.") {
        Some(suffix) => target.ends_with(&format!(".{suffix}")),
        None => target == host,
    };
    let port_ok = match port {
        None => url.port().is_none(),
        Some("*") => true,
        Some(port) => {
            url.port_or_known_default()
                .map(|p| p.to_string())
                .as_deref()
                == Some(port)
        }
    };
    let path_ok = path.is_empty()
        || if path.ends_with('/') {
            url.path().starts_with(path)
        } else {
            url.path() == path
        };
    scheme_ok && host_ok && port_ok && path_ok
}

#[cfg(test)]
mod tests {
    use super::{Candidate, allows};
    use url::Url;

    /** Builds an external candidate. */
    fn external(url: &Url) -> Candidate<'_> {
        Candidate {
            url: Some(url),
            nonce: None,
            source: "",
        }
    }

    #[test]
    fn blob_policies_match_scheme_less_hosts_using_the_creator_origin() {
        let blob =
            Url::parse("blob:http://creator.test/12345678-1234-4123-8123-123456789abc").unwrap();
        let policies = vec!["script-src cdn.test; connect-src api.test".into()];
        assert!(super::allows_worker(
            &policies,
            &blob,
            &Url::parse("http://cdn.test/module.js").unwrap(),
            false
        ));
        assert!(super::allows_connect(
            &policies,
            &blob,
            &Url::parse("http://api.test/data").unwrap()
        ));
        assert!(!super::allows_worker(
            &policies,
            &blob,
            &Url::parse("http://other.test/module.js").unwrap(),
            false
        ));
        let secure =
            Url::parse("blob:https://creator.test/12345678-1234-4123-8123-123456789abc").unwrap();
        assert!(!super::allows_connect(
            &policies,
            &secure,
            &Url::parse("http://api.test/data").unwrap()
        ));
        let opaque = Url::parse("blob:null/12345678-1234-4123-8123-123456789abc").unwrap();
        assert!(!super::allows_connect(
            &policies,
            &opaque,
            &Url::parse("http://api.test/data").unwrap()
        ));
    }

    #[test]
    fn self_and_hosts_allow_and_others_block() {
        let page = Url::parse("https://a.test/").unwrap();
        let policy = vec!["script-src 'self' cdn.test *.b.test".to_owned()];
        for (url, ok) in [
            ("https://a.test/x.js", true),
            ("https://cdn.test/x.js", true),
            ("https://s.b.test/x.js", true),
            ("https://evil.test/x.js", false),
        ] {
            let url = Url::parse(url).unwrap();
            assert_eq!(allows(&policy, &page, &external(&url)), ok, "{url}");
        }
    }

    #[test]
    fn inline_needs_unsafe_inline_nonce_or_hash() {
        let page = Url::parse("https://a.test/").unwrap();
        let inline = |nonce, source| Candidate {
            url: None,
            nonce,
            source,
        };
        assert!(!allows(
            &["script-src 'self'".into()],
            &page,
            &inline(None, "x")
        ));
        assert!(allows(
            &["script-src 'unsafe-inline'".into()],
            &page,
            &inline(None, "x")
        ));
        let nonce = vec!["script-src 'nonce-abc' 'unsafe-inline'".to_owned()];
        assert!(allows(&nonce, &page, &inline(Some("abc"), "x")));
        assert!(!allows(&nonce, &page, &inline(None, "x")));
        // sha256 of "alert(1)".
        let hash =
            vec!["script-src 'sha256-bhHHL3z2vDgxUt0W3dWQOrprscmda2Y5pLsLg4GF+pI='".to_owned()];
        assert!(allows(&hash, &page, &inline(None, "alert(1)")));
    }

    #[test]
    fn default_src_applies_and_every_policy_must_allow() {
        let page = Url::parse("https://a.test/").unwrap();
        let url = Url::parse("https://cdn.test/x.js").unwrap();
        assert!(!allows(
            &["default-src 'self'".into()],
            &page,
            &external(&url)
        ));
        let both = vec!["script-src *".to_owned(), "script-src 'self'".to_owned()];
        assert!(!allows(&both, &page, &external(&url)));
        assert!(allows(&["img-src 'none'".into()], &page, &external(&url)));
    }

    #[test]
    fn strict_dynamic_requires_a_nonce() {
        let page = Url::parse("https://a.test/").unwrap();
        let url = Url::parse("https://a.test/x.js").unwrap();
        let policy = vec!["script-src 'self' 'strict-dynamic' 'nonce-n'".to_owned()];
        assert!(!allows(&policy, &page, &external(&url)));
        let nonced = Candidate {
            url: Some(&url),
            nonce: Some("n"),
            source: "",
        };
        assert!(allows(&policy, &page, &nonced));
    }

    #[test]
    fn host_source_enforces_default_port_and_case_sensitive_path() {
        let page = Url::parse("https://app.test/index.html").unwrap();
        let port_policy = vec!["script-src https://cdn.test".to_owned()];
        let evil_port = Url::parse("https://cdn.test:8443/worker.js").unwrap();
        let default_port = Url::parse("https://cdn.test:443/worker.js").unwrap();
        let wildcard_port_policy = vec!["script-src https://cdn.test:*".to_owned()];
        assert!(!allows(&port_policy, &page, &external(&evil_port)));
        assert!(allows(&port_policy, &page, &external(&default_port)));
        assert!(allows(&wildcard_port_policy, &page, &external(&evil_port)));

        let path_policy = vec!["script-src https://cdn.test/Allowed/worker.js".to_owned()];
        let wrong_case = Url::parse("https://cdn.test/allowed/worker.js").unwrap();
        let exact_case = Url::parse("https://cdn.test/Allowed/worker.js").unwrap();
        let upper_host = Url::parse("HTTPS://CDN.TEST/Allowed/worker.js").unwrap();
        assert!(!allows(&path_policy, &page, &external(&wrong_case)));
        assert!(allows(&path_policy, &page, &external(&exact_case)));
        assert!(allows(&path_policy, &page, &external(&upper_host)));
    }
}
