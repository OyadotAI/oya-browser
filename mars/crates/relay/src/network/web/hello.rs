/**
 * The one TLS client configuration, shaped like Chrome's ClientHello as far as Rustls allows:
 * Chrome's cipher suite and signature scheme order, Chrome's groups (X25519MLKEM768 from our
 * own ML-KEM, then X25519, P-256, P-384) with Chrome's two key shares, ALPN h2 then http/1.1,
 * Brotli certificate compression and a randomized extension order. Rustls cannot send GREASE,
 * ALPS, the renegotiation_info extension (it sends the SCSV), signed_certificate_timestamp, or
 * the legacy CBC and RSA key-exchange suites, and its ECH GREASE forces TLS 1.3 only, which
 * would lose every TLS 1.2 server; README.md records what tls.peet.ws measured.
 */
mod hybrid;
mod mlkem;
#[cfg(test)]
mod tests_hello;

use crate::protocol::{EngineError, EngineResult};
use rustls::client::WebPkiServerVerifier;
use rustls::crypto::{CryptoProvider, WebPkiSupportedAlgorithms};
use rustls::{CipherSuite, ClientConfig, RootCertStore, SignatureScheme};
use std::sync::{Arc, OnceLock};

/// Chrome's suites in Chrome's order, minus the CBC and static-RSA ones Rustls never offers.
const SUITES: [CipherSuite; 9] = [
    CipherSuite::TLS13_AES_128_GCM_SHA256,
    CipherSuite::TLS13_AES_256_GCM_SHA384,
    CipherSuite::TLS13_CHACHA20_POLY1305_SHA256,
    CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256,
    CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
    CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384,
    CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384,
    CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256,
    CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256,
];

/// Chrome's signature_algorithms list, exactly; Ed25519 and P-521 are dropped as Chrome does.
const SCHEMES: [SignatureScheme; 8] = [
    SignatureScheme::ECDSA_NISTP256_SHA256,
    SignatureScheme::RSA_PSS_SHA256,
    SignatureScheme::RSA_PKCS1_SHA256,
    SignatureScheme::ECDSA_NISTP384_SHA384,
    SignatureScheme::RSA_PSS_SHA384,
    SignatureScheme::RSA_PKCS1_SHA384,
    SignatureScheme::RSA_PSS_SHA512,
    SignatureScheme::RSA_PKCS1_SHA512,
];

/** What Rustls's `mapping` field holds, in order. */
type Mapping = Vec<(
    SignatureScheme,
    &'static [&'static dyn rustls::pki_types::SignatureVerificationAlgorithm],
)>;

/// The shared configuration. One instance means one in-memory session cache, so later
/// connections resume (as Chrome does) instead of repeating full handshakes.
pub(super) fn config() -> EngineResult<Arc<ClientConfig>> {
    static CONFIG: OnceLock<Option<Arc<ClientConfig>>> = OnceLock::new();
    CONFIG.get_or_init(build).clone().ok_or_else(unconfigured)
}

/** The Mozilla roots both TLS paths trust, built once. */
fn roots() -> Arc<RootCertStore> {
    static ROOTS: OnceLock<Arc<RootCertStore>> = OnceLock::new();
    ROOTS
        .get_or_init(|| {
            Arc::new(RootCertStore::from_iter(
                webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
            ))
        })
        .clone()
}

/// The WebPKI verifier Rustls builds from the same roots and provider, for the legacy RSA key
/// exchange in legacy.rs, so both paths verify certificates identically.
pub(super) fn verifier() -> EngineResult<Arc<WebPkiServerVerifier>> {
    static VERIFIER: OnceLock<Option<Arc<WebPkiServerVerifier>>> = OnceLock::new();
    VERIFIER
        .get_or_init(|| {
            WebPkiServerVerifier::builder_with_provider(roots(), Arc::new(provider()))
                .build()
                .ok()
        })
        .clone()
        .ok_or_else(unconfigured)
}

/** The configuration could not be built. */
fn unconfigured() -> EngineError {
    EngineError::new(
        "tls_error",
        "The TLS client could not be configured.",
        "Restart the browser.",
    )
}

/** Builds the provider and configuration; None only if Rustls rejects its own defaults. */
fn build() -> Option<Arc<ClientConfig>> {
    let roots = roots();
    let mut config = ClientConfig::builder_with_provider(Arc::new(provider()))
        .with_safe_default_protocol_versions()
        .ok()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Some(Arc::new(config))
}

/** The ring provider, reordered and trimmed to Chrome's lists. */
fn provider() -> CryptoProvider {
    let mut provider = rustls::crypto::ring::default_provider();
    provider.kx_groups.insert(0, &hybrid::X25519MlKem768);
    let available = provider.cipher_suites.clone();
    provider.cipher_suites = SUITES
        .iter()
        .filter_map(|id| available.iter().find(|suite| suite.suite() == *id).copied())
        .collect();
    let algorithms = provider.signature_verification_algorithms;
    provider.signature_verification_algorithms = WebPkiSupportedAlgorithms {
        all: algorithms.all,
        mapping: mapping(algorithms),
    };
    provider
}

/** The default mapping filtered into Chrome's order, built once for the `'static` slice. */
fn mapping(algorithms: WebPkiSupportedAlgorithms) -> &'static Mapping {
    static MAPPING: OnceLock<Mapping> = OnceLock::new();
    MAPPING.get_or_init(|| {
        SCHEMES
            .iter()
            .filter_map(|scheme| {
                algorithms
                    .mapping
                    .iter()
                    .find(|(id, _)| id == scheme)
                    .copied()
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_provider_offers_chromes_suites_and_schemes_in_chromes_order() {
        let provider = provider();
        let suites: Vec<_> = provider
            .cipher_suites
            .iter()
            .map(|suite| suite.suite())
            .collect();
        assert_eq!(suites, SUITES);
        let schemes: Vec<_> = provider
            .signature_verification_algorithms
            .mapping
            .iter()
            .map(|(scheme, _)| *scheme)
            .collect();
        assert_eq!(schemes, SCHEMES);
        let groups: Vec<_> = provider
            .kx_groups
            .iter()
            .map(|group| group.name())
            .collect();
        assert_eq!(
            groups,
            [
                rustls::NamedGroup::X25519MLKEM768,
                rustls::NamedGroup::X25519,
                rustls::NamedGroup::secp256r1,
                rustls::NamedGroup::secp384r1
            ]
        );
    }

    #[test]
    fn alpn_offers_h2_before_http1() {
        let config = config().expect("config");
        assert_eq!(
            config.alpn_protocols,
            [b"h2".to_vec(), b"http/1.1".to_vec()]
        );
    }
}
