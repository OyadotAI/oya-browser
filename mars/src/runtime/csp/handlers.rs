/** Compile attribute-specific CSP lists into inert V9 admission records without granting eval. */
use crate::v9::{HandlerHash, HandlerPolicy, HandlerRestriction};

/** Base64 source expressions permit at most two trailing padding characters. */
const MAX_BASE64_PADDING: usize = 2;

/** Effective directives intersect; absent directives impose no restriction. */
pub(super) fn compile(policies: &[String]) -> HandlerPolicy {
    HandlerPolicy {
        restrictions: policies
            .iter()
            .filter_map(|policy| {
                super::effective_sources(policy, &["script-src-attr", "script-src", "default-src"])
            })
            .map(|sources| restriction(&sources))
            .collect(),
    }
}

/** Keywords authorize only their own operation; nonce and hash expressions suppress unsafe-inline. */
fn restriction(sources: &[String]) -> HandlerRestriction {
    let keyword = |name: &str| {
        sources
            .iter()
            .any(|source| source.eq_ignore_ascii_case(name))
    };
    let keyed = sources.iter().any(|source| {
        expression(source).is_some_and(|(kind, _)| {
            kind.eq_ignore_ascii_case("nonce") || hash_algorithm(kind).is_some()
        })
    });
    HandlerRestriction {
        allow_inline: keyword("'unsafe-inline'") && !keyed && !keyword("'strict-dynamic'"),
        hashes: if keyword("'unsafe-hashes'") {
            sources.iter().filter_map(|source| hash(source)).collect()
        } else {
            Vec::new()
        },
    }
}

/** Source-expression base64 grammar permits URL-safe characters and at most two trailing pads. */
pub(super) fn expression(source: &str) -> Option<(&str, &str)> {
    let (kind, encoded) = source
        .strip_prefix('\'')?
        .strip_suffix('\'')?
        .split_once('-')?;
    let data = encoded.trim_end_matches('=');
    let padding = encoded.len() - data.len();
    (!data.is_empty()
        && padding <= MAX_BASE64_PADDING
        && data
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"+/-_".contains(&byte)))
    .then_some((kind, encoded))
}

/** A closed set of CSP hash algorithms shares ring's existing digest definitions. */
pub(super) fn hash_algorithm(name: &str) -> Option<&'static ring::digest::Algorithm> {
    match name.to_ascii_lowercase().as_str() {
        "sha256" => Some(&ring::digest::SHA256),
        "sha384" => Some(&ring::digest::SHA384),
        "sha512" => Some(&ring::digest::SHA512),
        _ => None,
    }
}

/** Malformed digests never become permissions and never enter the validated V9 contract. */
fn hash(source: &str) -> Option<HandlerHash> {
    let (kind, encoded) = expression(source)?;
    let algorithm = hash_algorithm(kind)?;
    let bytes = super::super::integrity::base64(encoded)?;
    if bytes.len() != algorithm.output_len()
        || super::super::images::base64(&bytes) != encoded.replace('-', "+").replace('_', "/")
    {
        return None;
    }
    match kind.to_ascii_lowercase().as_str() {
        "sha256" => Some(HandlerHash::Sha256(bytes)),
        "sha384" => Some(HandlerHash::Sha384(bytes)),
        "sha512" => Some(HandlerHash::Sha512(bytes)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** One effective list is enough for these grammar and fallback cases. */
    fn rule(policy: &str) -> HandlerRestriction {
        compile(&[policy.into()]).restrictions.remove(0)
    }

    /** Attribute fallback ignores script-src-elem and respects first duplicate directives. */
    #[test]
    fn effective_directive_preserves_precedence_and_first_occurrence() {
        for policy in [
            "default-src 'unsafe-inline'",
            "script-src 'unsafe-inline'; default-src 'none'",
            "script-src-attr 'unsafe-inline'; script-src 'none'",
            "script-src-elem 'none'; script-src 'unsafe-inline'",
            "script-src-attr 'unsafe-inline'; script-src-attr 'none'",
        ] {
            assert!(rule(policy).allow_inline, "{policy}");
        }
        assert!(!rule("script-src-attr 'none'; script-src-attr 'unsafe-inline'").allow_inline);
        assert!(compile(&["img-src 'none'".into()]).restrictions.is_empty());
    }

    /** Nonces, hashes and strict-dynamic suppress inline, while eval and hosts never grant it. */
    #[test]
    fn unrelated_permissions_do_not_authorize_attributes() {
        for source in [
            "'nonce-abc' 'unsafe-inline'",
            "'sha256-AAAA' 'unsafe-inline'",
            "'strict-dynamic' 'unsafe-inline'",
            "'self' 'unsafe-eval'",
            "'none'",
        ] {
            assert!(
                !rule(&format!("script-src {source}")).allow_inline,
                "{source}"
            );
        }
        assert_eq!(
            compile(&[
                "script-src 'unsafe-inline'".into(),
                "script-src-attr 'none'".into()
            ])
            .restrictions
            .len(),
            2
        );
    }

    /** Hash grants require valid canonical encoding; embedded padding cannot be ignored. */
    #[test]
    fn malformed_hashes_never_become_permissions() {
        let valid = "bhHHL3z2vDgxUt0W3dWQOrprscmda2Y5pLsLg4GF+pI=";
        assert_eq!(
            rule(&format!("script-src-attr 'unsafe-hashes' 'sha256-{valid}'"))
                .hashes
                .len(),
            1
        );
        for invalid in [
            format!("={valid}"),
            valid.replace('H', "=H"),
            format!("{valid}="),
            valid.trim_end_matches('=').into(),
            "AAAA".into(),
        ] {
            assert!(
                rule(&format!(
                    "script-src-attr 'unsafe-hashes' 'sha256-{invalid}'"
                ))
                .hashes
                .is_empty()
            );
        }
        assert!(
            rule(&format!("script-src-attr 'sha256-{valid}'"))
                .hashes
                .is_empty()
        );
    }
}
