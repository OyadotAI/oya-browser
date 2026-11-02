use rustls::RootCertStore;
/** System root certificates complement Mozilla roots for platform and enterprise trust anchors. */
use rustls::pki_types::CertificateDer;
use std::path::PathBuf;

/// PEM header identifying the start of an ASCII-armored certificate.
const BEGIN_MARKER: &str = "-----BEGIN CERTIFICATE-----";
/// PEM trailer identifying the end of an ASCII-armored certificate.
const END_MARKER: &str = "-----END CERTIFICATE-----";
/// Standard paths for system and OS trust stores across macOS and Linux distributions.
const BUNDLE_PATHS: [&str; 4] = [
    "/etc/ssl/cert.pem",
    "/etc/ssl/certs/ca-certificates.crt",
    "/etc/pki/tls/certs/ca-bundle.crt",
    "/etc/ssl/ca-bundle.pem",
];

/** Decodes one base64 character according to the standard RFC 4648 alphabet. */
fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/** Drains full eight-bit bytes from the decoding buffer into the output vector. */
fn drain_bytes(buffer: &mut u32, bits: &mut u32, out: &mut Vec<u8>) {
    while *bits >= 8 {
        *bits -= 8;
        out.push((*buffer >> *bits) as u8);
        *buffer &= (1 << *bits) - 1;
    }
}

/** Decodes standard base64 text into raw binary bytes. */
pub(crate) fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let clean = text.trim_end_matches('=');
    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for byte in clean.bytes() {
        buffer = (buffer << 6) | u32::from(base64_value(byte)?);
        bits += 6;
        drain_bytes(&mut buffer, &mut bits, &mut out);
    }
    Some(out)
}

/** Handles transition marker lines for PEM block boundaries. */
fn handle_pem_marker(trimmed: &str, capturing: &mut bool) -> bool {
    if trimmed == BEGIN_MARKER {
        *capturing = true;
        true
    } else if trimmed == END_MARKER {
        *capturing = false;
        true
    } else {
        false
    }
}

/** Ingests one certificate line, appending complete parsed DER structures on boundary. */
fn handle_pem_line(
    line: &str,
    capturing: &mut bool,
    current: &mut String,
    certs: &mut Vec<CertificateDer<'static>>,
) {
    let trimmed = line.trim();
    if handle_pem_marker(trimmed, capturing) {
        if trimmed == BEGIN_MARKER {
            current.clear();
        } else if let Some(der) = base64_decode(current) {
            certs.push(CertificateDer::from(der));
        }
    } else if *capturing {
        current.push_str(trimmed);
    }
}

/** Extracts DER certificate bodies from an ASCII-armored PEM bundle. */
pub(crate) fn parse_pem_bundle(pem: &str) -> Vec<CertificateDer<'static>> {
    let (mut certs, mut current, mut capturing) = (Vec::new(), String::new(), false);
    for line in pem.lines() {
        handle_pem_line(line, &mut capturing, &mut current, &mut certs);
    }
    certs
}

/** Gathers potential root certificate file paths from environment and system locations. */
fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(env_path) = std::env::var("SSL_CERT_FILE") {
        paths.push(PathBuf::from(env_path));
    }
    for p in BUNDLE_PATHS {
        paths.push(PathBuf::from(p));
    }
    paths
}

/** Appends parsable system and platform trust anchors into the certificate store. */
fn load_system_roots(store: &mut RootCertStore) {
    for path in candidate_paths() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            let certs = parse_pem_bundle(&content);
            let (added, _) = store.add_parsable_certificates(certs);
            if added > 0 {
                return;
            }
        }
    }
}

/** Assembles the full root trust store containing Mozilla roots and platform anchors. */
pub(super) fn root_store() -> RootCertStore {
    let mut store = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    load_system_roots(&mut store);
    store
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Minimal valid X.509 certificate DER encoded into PEM format. */
    const SAMPLE_PEM: &str = "\
-----BEGIN CERTIFICATE-----\n\
AQIDBA==\n\
-----END CERTIFICATE-----\n";

    #[test]
    fn parses_pem_blocks_into_der_certificates() {
        let certs = parse_pem_bundle(SAMPLE_PEM);
        assert_eq!(certs.len(), 1);
        assert_eq!(certs[0].as_ref(), &[1, 2, 3, 4]);
    }

    #[test]
    fn decodes_base64_correctly() {
        assert_eq!(base64_decode("AQIDBA==").unwrap(), vec![1, 2, 3, 4]);
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
    }

    #[test]
    fn assembled_store_contains_webpki_roots() {
        let store = root_store();
        assert!(!store.is_empty());
    }
}
