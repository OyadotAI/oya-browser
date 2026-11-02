/** Inert host restrictions admit markup handlers independently from dynamic JavaScript code. */
use crate::protocol::{EngineError, EngineResult};
use ring::digest::{SHA256, SHA384, SHA512, digest};
use serde::{Deserialize, Serialize};

/** Independent policies and their combined digest work share the script-count bound. */
pub const HANDLER_POLICY_LIMIT: usize = crate::constants::SCRIPT_COUNT;

/** Every enforced source list must admit the exact decoded attribute source. */
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct HandlerPolicy {
    /// Empty means the host supplied no restrictive attribute directive.
    pub restrictions: Vec<HandlerRestriction>,
}

/** One effective script-src-attr list, compiled by the host without network authority. */
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct HandlerRestriction {
    /// True only when unsafe-inline is effective without nonce/hash/strict-dynamic suppression.
    pub allow_inline: bool,
    /// Only hashes explicitly authorized by unsafe-hashes are supplied here.
    pub hashes: Vec<HandlerHash>,
}

/** Digest algorithm is a closed wire enum so unknown names cannot broaden policy. */
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum HandlerHash {
    /// A SHA-256 digest, validated to the algorithm's output length.
    Sha256(Vec<u8>),
    /// A SHA-384 digest, validated to the algorithm's output length.
    Sha384(Vec<u8>),
    /// A SHA-512 digest, validated to the algorithm's output length.
    Sha512(Vec<u8>),
}

impl HandlerHash {
    /** Return immutable digest facts without accepting an arbitrary algorithm name. */
    fn parts(&self) -> (&'static ring::digest::Algorithm, &[u8]) {
        match self {
            Self::Sha256(bytes) => (&SHA256, bytes),
            Self::Sha384(bytes) => (&SHA384, bytes),
            Self::Sha512(bytes) => (&SHA512, bytes),
        }
    }

    /** Hash the original source; generated function wrapper text is never part of CSP. */
    fn matches(&self, source: &str) -> bool {
        let (algorithm, expected) = self.parts();
        digest(algorithm, source.as_bytes()).as_ref() == expected
    }
}

impl HandlerPolicy {
    /** Refuse malformed or excessive host records before any page code executes. */
    pub fn validate(&self) -> EngineResult<()> {
        let count = self
            .restrictions
            .iter()
            .try_fold(0usize, |count, rule| count.checked_add(rule.hashes.len()));
        if self.restrictions.len() > HANDLER_POLICY_LIMIT
            || count.is_none_or(|count| count > HANDLER_POLICY_LIMIT)
        {
            return Err(EngineError::limit(
                "CSP handler policy",
                HANDLER_POLICY_LIMIT,
            ));
        }
        for hash in self.restrictions.iter().flat_map(|rule| &rule.hashes) {
            let (algorithm, bytes) = hash.parts();
            if bytes.len() != algorithm.output_len() {
                return Err(EngineError::invalid("Invalid CSP handler digest length."));
            }
        }
        Ok(())
    }

    /** Intersection preserves independent policies instead of unioning their permissions. */
    pub(in crate::v9) fn allows(&self, source: &str) -> bool {
        self.restrictions
            .iter()
            .all(|rule| rule.allow_inline || rule.hashes.iter().any(|hash| hash.matches(source)))
    }
}
