/**
 * The one TLS client configuration, shaped like Chrome's ClientHello: Chrome's cipher suite and
 * signature scheme order, Chrome's groups (X25519MLKEM768 from our own ML-KEM, then X25519,
 * P-256, P-384) with Chrome's two key shares, ALPN h2 then http/1.1, Brotli certificate
 * compression and a randomized extension order. The vendored Rustls (vendor/rustls, its
 * `ClientHelloShaping`) adds what stock Rustls cannot send and Google's bot check keys on:
 * GREASE values, the renegotiation_info extension in place of the SCSV,
 * signed_certificate_timestamp, a GREASE ECH outer, ALPS for h2 (with the client
 * EncryptedExtensions message an accepting server waits for), and Chrome's six legacy suites,
 * which are advertised here and served by legacy.rs when a server picks one. The result is
 * Chrome's JA4; README.md records what tls.peet.ws measured.
 */
mod hybrid;
mod mlkem;
#[cfg(test)]
mod tests_hello;

use crate::protocol::{EngineError, EngineResult};
use rustls::client::{ClientHelloShaping, WebPkiServerVerifier};
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

/// Chrome's legacy suites after the ones Rustls runs, in Chrome's order: ECDHE-RSA CBC, kRSA
/// GCM, kRSA CBC. A server that picks one fails the Rustls handshake and legacy.rs takes over;
/// it speaks the kRSA GCM pair, the CBC four are a ceiling (see README.md).
const ADVERTISED_SUITES: [u16; 6] = [0xc013, 0xc014, 0x009c, 0x009d, 0x002f, 0x0035];
/// signed_certificate_timestamp (RFC 6962): Chrome offers it empty; the answer is ignored.
const SCT_EXTENSION: u16 = 18;
/// The GREASE ECH payload Chrome sends most often, in bytes.
// ponytail: fixed size; Chrome sizes it like a padded inner hello, vary it if a check keys on it.
const GREASE_ECH_PAYLOAD: usize = 208;

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

/** The Mozilla and system roots both TLS paths trust, built once. */
fn roots() -> Arc<RootCertStore> {
    static ROOTS: OnceLock<Arc<RootCertStore>> = OnceLock::new();
    ROOTS
        .get_or_init(|| Arc::new(super::roots::root_store()))
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
    build_with(roots())
}

/** The shared configuration over any roots: tests hand it their own CA. */
pub(super) fn build_with(roots: Arc<RootCertStore>) -> Option<Arc<ClientConfig>> {
    let mut config = ClientConfig::builder_with_provider(Arc::new(provider()))
        .with_safe_default_protocol_versions()
        .ok()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    config.hello_shaping = shaping();
    Some(Arc::new(config))
}

/** Chrome's shape beyond what Rustls negotiates; every value is inert on the wire. */
fn shaping() -> ClientHelloShaping {
    ClientHelloShaping {
        grease: true,
        advertised_cipher_suites: ADVERTISED_SUITES.to_vec(),
        renegotiation_info_extension: true,
        extra_extensions: vec![(SCT_EXTENSION, Vec::new())],
        grease_ech_payload_len: Some(GREASE_ECH_PAYLOAD),
        alps_protocols: vec![b"h2".to_vec()],
    }
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
