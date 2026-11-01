/**
 * AES-GCM for crypto.subtle, with no VM in it: the raw key import and the seal and open steps that
 * Chrome runs natively, on ring. Anti-bot challenges take the native path when SubtleCrypto has
 * importKey and encrypt, so these must answer as Chrome does (the same bytes for the same key,
 * IV and data). Ring seals only 96-bit IVs and 128-bit tags, so those are the sizes accepted here;
 * a request for any other size is refused by the caller as unsupported, never silently truncated.
 */
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey};

/** The IV length ring accepts for GCM: 96 bits. */
pub(super) const IV_BYTES: usize = 12;
/** The AES-128 key length in bytes. */
const KEY_128_BYTES: usize = 16;
/** The AES-256 key length in bytes. */
const KEY_256_BYTES: usize = 32;

/** Why an AES-GCM operation could not run. */
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Fault {
    /// The key is neither 128 nor 256 bits.
    KeyLength,
    /// The IV is not the 96-bit size ring seals under.
    IvLength,
    /// Decryption failed authentication: the tag, key, IV or data does not match.
    Authentication,
}

/** A validated AES-GCM key, ready to seal or open. */
pub(super) struct Key(LessSafeKey);

impl Key {
    /** A key from its raw bytes; the length selects AES-128 or AES-256. */
    pub(super) fn raw(bytes: &[u8]) -> Result<Self, Fault> {
        let algorithm = match bytes.len() {
            KEY_128_BYTES => &ring::aead::AES_128_GCM,
            KEY_256_BYTES => &ring::aead::AES_256_GCM,
            _ => return Err(Fault::KeyLength),
        };
        let unbound = UnboundKey::new(algorithm, bytes).map_err(|_| Fault::KeyLength)?;
        Ok(Self(LessSafeKey::new(unbound)))
    }

    /** The plaintext sealed under `iv` and `aad`: ciphertext followed by the 16-byte tag. */
    pub(super) fn seal(&self, iv: &[u8], aad: &[u8], data: &[u8]) -> Result<Vec<u8>, Fault> {
        let nonce = nonce(iv)?;
        let mut buffer = data.to_vec();
        self.0
            .seal_in_place_append_tag(nonce, Aad::from(aad), &mut buffer)
            .map_err(|_| Fault::Authentication)?;
        Ok(buffer)
    }

    /** The plaintext of `ciphertext` (which ends in its tag), or an authentication failure. */
    pub(super) fn open(&self, iv: &[u8], aad: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, Fault> {
        let nonce = nonce(iv)?;
        let mut buffer = ciphertext.to_vec();
        let plain = self
            .0
            .open_in_place(nonce, Aad::from(aad), &mut buffer)
            .map_err(|_| Fault::Authentication)?;
        Ok(plain.to_vec())
    }
}

/** The GCM nonce for an IV of the one size ring accepts. */
fn nonce(iv: &[u8]) -> Result<Nonce, Fault> {
    if iv.len() != IV_BYTES {
        return Err(Fault::IvLength);
    }
    Nonce::try_assume_unique_for_key(iv).map_err(|_| Fault::IvLength)
}
