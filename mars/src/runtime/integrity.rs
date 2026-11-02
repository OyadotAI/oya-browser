/** Subresource Integrity: a script carrying `integrity` runs only if its body matches the metadata. */
use ring::digest::{Algorithm, SHA256, SHA384, SHA512, digest};

/** Strength order, weakest first; per the SRI spec only the strongest listed algorithm counts. */
const ALGORITHMS: [(&str, &Algorithm); 3] = [
    ("sha256", &SHA256),
    ("sha384", &SHA384),
    ("sha512", &SHA512),
];

/// True when the body matches one of the strongest-algorithm hashes, or the metadata names none.
pub(super) fn matches(metadata: &str, body: &[u8]) -> bool {
    let hashes: Vec<(usize, Vec<u8>)> = metadata
        .split_ascii_whitespace()
        .filter_map(|token| {
            let (name, value) = token.split_once('-')?;
            let rank = ALGORITHMS.iter().position(|(known, _)| *known == name)?;
            let value = value.split('?').next()?;
            Some((rank, base64(value)?))
        })
        .collect();
    let Some(strongest) = hashes.iter().map(|(rank, _)| *rank).max() else {
        return true;
    };
    let actual = digest(ALGORITHMS[strongest].1, body);
    hashes
        .iter()
        .any(|(rank, expected)| *rank == strongest && expected.as_slice() == actual.as_ref())
}

/** Standard or URL-safe base64, padding optional; None on any other byte. */
pub(super) fn base64(text: &str) -> Option<Vec<u8>> {
    let mut bits = 0u32;
    let mut count = 0;
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    for byte in text.bytes().filter(|byte| *byte != b'=') {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        };
        bits = (bits << 6) | u32::from(value);
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::matches;

    /** sha256 of "alert(1)" as published by `openssl dgst -sha256 -binary | base64`. */
    const ALERT_SHA256: &str = "sha256-bhHHL3z2vDgxUt0W3dWQOrprscmda2Y5pLsLg4GF+pI=";

    #[test]
    fn matching_hash_passes_and_mismatch_fails() {
        assert!(matches(ALERT_SHA256, b"alert(1)"));
        assert!(!matches(ALERT_SHA256, b"alert(2)"));
    }

    #[test]
    fn only_the_strongest_algorithm_counts() {
        let metadata = format!("{ALERT_SHA256} sha512-AAAA");
        assert!(!matches(&metadata, b"alert(1)"));
    }

    #[test]
    fn unknown_algorithms_alone_do_not_block() {
        assert!(matches("md5-abc", b"anything"));
        assert!(matches("", b"anything"));
    }
}
