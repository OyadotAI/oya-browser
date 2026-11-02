//! Mars: shaping the ClientHello like Chrome's.
//!
//! Servers fingerprint the ClientHello. rustls offers only what it implements; Chrome also
//! advertises GREASE values, legacy suites it would rather not use, and extensions rustls has
//! no use for, and servers treat the difference as "not a browser". This module lets a client
//! opt into Chrome's shape. Nothing here changes what is negotiated: a GREASE value is never
//! selected, an advertised-only suite fails the handshake if a server picks it, and the extra
//! extensions carry no state.

use alloc::vec::Vec;

use crate::crypto::SecureRandom;
use crate::rand::GetRandomFailed;

/// Extra ClientHello content beyond what the protocol needs. Off by default: a default
/// configuration writes exactly the hello upstream rustls writes.
#[derive(Clone, Debug, Default)]
pub struct ClientHelloShaping {
    /// Send GREASE (RFC 8701) values: a cipher suite, a group with a one-byte key share, a
    /// version and two extensions (an empty first one, a one-byte last one), drawn fresh per
    /// handshake and kept across a HelloRetryRequest, as Chrome does.
    pub grease: bool,
    /// Cipher suite codes appended after the provider's suites, advertised only. A server that
    /// selects one fails the handshake as an unsupported suite.
    pub advertised_cipher_suites: Vec<u16>,
    /// Send an empty `renegotiation_info` extension instead of the SCSV cipher suite.
    pub renegotiation_info_extension: bool,
    /// Extensions appended verbatim as (type, body) and shuffled with the rest. A body for a
    /// type rustls would write itself replaces rustls's encoding of it.
    pub extra_extensions: Vec<(u16, Vec<u8>)>,
    /// Send a GREASE `encrypted_client_hello` outer extension built from random bytes with a
    /// payload of this many bytes. Retry configs a server answers with are ignored, exactly as
    /// for rustls's own GREASE ECH, and the same extension is repeated after a retry request.
    pub grease_ech_payload_len: Option<usize>,
    /// Offer ALPS (`application_settings`, draft-vvv-tls-alps) for these ALPN protocols. A
    /// server that accepts answers in its EncryptedExtensions and then waits for the client's
    /// own EncryptedExtensions message, which carries empty settings, as Chrome's does.
    pub alps_protocols: Vec<Vec<u8>>,
}

/// The GREASE values of one handshake; a retried hello repeats them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GreaseValues {
    pub(crate) cipher_suite: u16,
    pub(crate) group: u16,
    pub(crate) version: u16,
    pub(crate) first_extension: u16,
    pub(crate) last_extension: u16,
}

impl GreaseValues {
    /// Five random draws; the two extension values are kept distinct, as BoringSSL does.
    pub(crate) fn random(secure_random: &dyn SecureRandom) -> Result<Self, GetRandomFailed> {
        let mut seed = [0u8; 5];
        secure_random.fill(&mut seed)?;
        let first_extension = grease(seed[3]);
        let mut last_extension = grease(seed[4]);
        if last_extension == first_extension {
            last_extension = grease(seed[4].wrapping_add(0x10));
        }
        Ok(Self {
            cipher_suite: grease(seed[0]),
            group: grease(seed[1]),
            version: grease(seed[2]),
            first_extension,
            last_extension,
        })
    }
}

/// One of the sixteen GREASE values, `0x?a?a`, from one random byte.
fn grease(byte: u8) -> u16 {
    let half = u16::from((byte & 0xf0) | 0x0a);
    (half << 8) | half
}

/// `true` when any value in the hello is a GREASE value (RFC 8701).
pub fn is_grease(value: u16) -> bool {
    value & 0x0f0f == 0x0a0a && (value >> 8) == (value & 0xff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_grease_value_has_the_rfc_8701_shape() {
        for byte in 0..=u8::MAX {
            assert!(is_grease(grease(byte)));
        }
        assert!(!is_grease(0x1301));
    }
}
