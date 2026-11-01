/**
 * AES-CBC for crypto.subtle: the raw key import, and the PKCS#7-padded CBC seal and open that Chrome
 * runs natively. The block cipher is the `aes` crate already in the workspace (the Clear Key CDM
 * uses it); ring has no CBC mode, so the chaining and the padding are here. A block is 16 bytes and
 * the IV is one block: any other IV length is refused, as Web Crypto requires.
 */
use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use aes::{Aes128, Aes192, Aes256};

/** The AES block and CBC IV size in bytes. */
pub(super) const BLOCK: usize = 16;
/** The AES-128 key length in bytes. */
const KEY_128_BYTES: usize = 16;
/** The AES-192 key length in bytes. */
const KEY_192_BYTES: usize = 24;
/** The AES-256 key length in bytes. */
const KEY_256_BYTES: usize = 32;

/** Why an AES-CBC operation could not run. Padding failures share one variant, so the caller reports
them with one message and one error name, whichever check failed. */
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Fault {
    /// The key is not 128, 192 or 256 bits.
    KeyLength,
    /// The IV is not one block long.
    IvLength,
    /// The ciphertext is empty or not whole blocks, or its padding is not valid PKCS#7.
    Padding,
}

/** One AES key at its own size; the variant picks the block cipher, the bytes never leave it. */
enum Cipher {
    /** AES-128. */
    Small(Aes128),
    /** AES-192. */
    Medium(Aes192),
    /** AES-256. */
    Large(Aes256),
}

/** A validated AES-CBC key, ready to seal or open. */
pub(super) struct Key(Cipher);

impl Key {
    /** A key from its raw bytes; the length selects AES-128, AES-192 or AES-256. */
    pub(super) fn raw(bytes: &[u8]) -> Result<Self, Fault> {
        let cipher = match bytes.len() {
            KEY_128_BYTES => {
                Cipher::Small(Aes128::new_from_slice(bytes).map_err(|_| Fault::KeyLength)?)
            }
            KEY_192_BYTES => {
                Cipher::Medium(Aes192::new_from_slice(bytes).map_err(|_| Fault::KeyLength)?)
            }
            KEY_256_BYTES => {
                Cipher::Large(Aes256::new_from_slice(bytes).map_err(|_| Fault::KeyLength)?)
            }
            _ => return Err(Fault::KeyLength),
        };
        Ok(Self(cipher))
    }

    /** The PKCS#7-padded plaintext sealed in CBC under `iv`: whole blocks, always at least one pad byte. */
    pub(super) fn seal(&self, iv: &[u8], data: &[u8]) -> Result<Vec<u8>, Fault> {
        let mut previous = iv_block(iv)?;
        let pad = BLOCK - data.len() % BLOCK;
        let mut out = data.to_vec();
        out.extend(std::iter::repeat_n(pad as u8, pad));
        for block in out.chunks_exact_mut(BLOCK) {
            xor_into(block, &previous);
            self.encrypt_block(block);
            previous.copy_from_slice(block);
        }
        Ok(out)
    }

    /** The plaintext of CBC `ciphertext` under `iv` with its padding removed, or a padding fault. */
    pub(super) fn open(&self, iv: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, Fault> {
        let mut previous = iv_block(iv)?;
        if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(BLOCK) {
            return Err(Fault::Padding);
        }
        let mut out = ciphertext.to_vec();
        for (plain, cipher) in out
            .chunks_exact_mut(BLOCK)
            .zip(ciphertext.chunks_exact(BLOCK))
        {
            plain.copy_from_slice(cipher);
            self.decrypt_block(plain);
            xor_into(plain, &previous);
            previous.copy_from_slice(cipher);
        }
        unpad(out)
    }

    /** Encrypts one block in place. */
    fn encrypt_block(&self, block: &mut [u8]) {
        let block = GenericArray::from_mut_slice(block);
        match &self.0 {
            Cipher::Small(cipher) => cipher.encrypt_block(block),
            Cipher::Medium(cipher) => cipher.encrypt_block(block),
            Cipher::Large(cipher) => cipher.encrypt_block(block),
        }
    }

    /** Decrypts one block in place. */
    fn decrypt_block(&self, block: &mut [u8]) {
        let block = GenericArray::from_mut_slice(block);
        match &self.0 {
            Cipher::Small(cipher) => cipher.decrypt_block(block),
            Cipher::Medium(cipher) => cipher.decrypt_block(block),
            Cipher::Large(cipher) => cipher.decrypt_block(block),
        }
    }
}

/** The IV as a block, or an IV fault when it is not one block long. */
fn iv_block(iv: &[u8]) -> Result<[u8; BLOCK], Fault> {
    let mut block = [0u8; BLOCK];
    if iv.len() != BLOCK {
        return Err(Fault::IvLength);
    }
    block.copy_from_slice(iv);
    Ok(block)
}

/** XORs `mask` into `block`, byte for byte. */
fn xor_into(block: &mut [u8], mask: &[u8]) {
    for (byte, other) in block.iter_mut().zip(mask) {
        *byte ^= other;
    }
}

/**
 * Strips the PKCS#7 padding. Every check runs over the final block whatever the outcome, and all
 * failures return the same fault, so the error reveals neither which check failed nor where.
 */
fn unpad(mut out: Vec<u8>) -> Result<Vec<u8>, Fault> {
    let length = out.len();
    let pad = usize::from(out[length - 1]);
    let tail = &out[length - BLOCK..];
    let mut mismatch = u8::from(pad == 0 || pad > BLOCK);
    for (index, byte) in tail.iter().enumerate() {
        let in_padding = tail.len() - index <= pad;
        mismatch |= u8::from(in_padding && usize::from(*byte) != pad);
    }
    if mismatch != 0 {
        return Err(Fault::Padding);
    }
    out.truncate(length - pad);
    Ok(out)
}
