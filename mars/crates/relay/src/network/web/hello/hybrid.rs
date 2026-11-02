/**
 * X25519MLKEM768 (draft-ietf-tls-ecdhe-mlkem), the hybrid post-quantum group Chrome offers
 * first: the client share is the ML-KEM-768 encapsulation key then an X25519 public key, the
 * server answers with the ML-KEM ciphertext then its X25519 key, and the shared secret is the
 * ML-KEM key then the X25519 secret. The X25519 half is Rustls's own ring group, exposed as a
 * hybrid component so the hello also carries a plain X25519 share, as Chrome's does.
 */
use super::mlkem;
use rustls::crypto::{ActiveKeyExchange, GetRandomFailed, SharedSecret, SupportedKxGroup};
use rustls::{Error, NamedGroup, PeerMisbehaved, ProtocolVersion};

/// The X25519 public key length.
const X25519_BYTES: usize = 32;

/** The group Rustls offers; TLS 1.3 only, as the draft requires. */
#[derive(Debug)]
pub(super) struct X25519MlKem768;

/** One handshake's hybrid key: the ML-KEM pair, the X25519 key, and their joined share. */
struct Active {
    /// The ML-KEM-768 key pair.
    kem: mlkem::KeyPair,
    /// Rustls's X25519 exchange.
    x25519: Box<dyn ActiveKeyExchange>,
    /// ek || X25519 public key, as sent.
    share: Vec<u8>,
}

impl SupportedKxGroup for X25519MlKem768 {
    fn start(&self) -> Result<Box<dyn ActiveKeyExchange>, Error> {
        let mut seeds = [0u8; 64];
        ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut seeds)
            .map_err(|_| Error::from(GetRandomFailed))?;
        let (d, z) = seeds.split_at(32);
        let kem = mlkem::generate(
            d.try_into().unwrap_or(&[0; 32]),
            z.try_into().unwrap_or(&[0; 32]),
        );
        let x25519 = rustls::crypto::ring::kx_group::X25519.start()?;
        let share = [kem.public.as_slice(), x25519.pub_key()].concat();
        Ok(Box::new(Active { kem, x25519, share }))
    }

    fn name(&self) -> NamedGroup {
        NamedGroup::X25519MLKEM768
    }

    fn usable_for_version(&self, version: ProtocolVersion) -> bool {
        version == ProtocolVersion::TLSv1_3
    }
}

impl ActiveKeyExchange for Active {
    /** Decapsulates the server's ciphertext and completes X25519 with its key. */
    fn complete(self: Box<Self>, peer: &[u8]) -> Result<SharedSecret, Error> {
        if peer.len() != mlkem::CT_BYTES + X25519_BYTES {
            return Err(PeerMisbehaved::InvalidKeyShare.into());
        }
        let (ciphertext, x25519) = peer.split_at(mlkem::CT_BYTES);
        let kem = mlkem::decapsulate(&self.kem, ciphertext)
            .ok_or(Error::from(PeerMisbehaved::InvalidKeyShare))?;
        let classic = self.x25519.complete(x25519)?;
        Ok(SharedSecret::from(
            [&kem[..], classic.secret_bytes()].concat(),
        ))
    }

    fn hybrid_component(&self) -> Option<(NamedGroup, &[u8])> {
        Some((NamedGroup::X25519, self.x25519.pub_key()))
    }

    /** The server chose plain X25519 from the second share. */
    fn complete_hybrid_component(self: Box<Self>, peer: &[u8]) -> Result<SharedSecret, Error> {
        self.x25519.complete(peer)
    }

    fn pub_key(&self) -> &[u8] {
        &self.share
    }

    fn group(&self) -> NamedGroup {
        NamedGroup::X25519MLKEM768
    }
}
