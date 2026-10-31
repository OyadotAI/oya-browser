/**
 * The Clear Key content decryption module's key store and its decryption interface. Sessions in
 * a page (`eme.js`) hand usable keys here when a license is accepted and withdraw them on close
 * or remove. The media pipeline, which another module owns, decrypts through `Keys::decrypt_cenc`
 * only: given the page's context identity, the sample's key ID, its IV and its subsample map, it
 * decrypts the protected ranges in place with AES-128-CTR as ISO/IEC 23001-7 'cenc' specifies.
 * A missing key is reported so the pipeline can wait for one (the spec's "waiting for key").
 */
use super::{Ctx, Handler, Reply, fail, ok};
use crate::protocol::EngineResult;
use crate::runtime::supervisor::Runtime;
use aes::Aes128;
use aes::cipher::{BlockEncrypt, KeyInit, generic_array::GenericArray};
use serde_json::Value;
use std::collections::BTreeMap;

/** Clear Key operations. */
pub(super) const HANDLERS: &[(&str, Handler)] = &[("eme.keys", keys), ("eme.close", close)];

/** AES block and Clear Key key and key ID size. */
const BLOCK: usize = 16;
/** Sessions one context may hold keys for. */
const SESSIONS: usize = 64;
/** Keys one session may hold. */
const SESSION_KEYS: usize = 64;

/** Why a sample could not be decrypted. */
#[derive(Debug, PartialEq, Eq)]
pub enum DecryptError {
    /** No session of this context holds the key: wait for a license. */
    NoKey,
    /** The IV or subsample map does not describe this sample. */
    Malformed,
}

/** Usable keys by context identity, then session id, then key ID. */
#[derive(Default)]
pub(in crate::runtime) struct Keys {
    contexts: BTreeMap<String, BTreeMap<String, BTreeMap<[u8; BLOCK], [u8; BLOCK]>>>,
}

impl Keys {
    /** The key for `kid` in any of the context's sessions. */
    fn find(&self, identity: &str, kid: &[u8; BLOCK]) -> Option<[u8; BLOCK]> {
        self.contexts
            .get(identity)?
            .values()
            .find_map(|keys| keys.get(kid).copied())
    }

    /**
     * Decrypt one 'cenc' sample in place: `subsamples` are (clear, protected) byte counts in
     * order (empty means the whole sample is protected); the protected bytes form one AES-CTR
     * stream starting from the 8- or 16-byte IV.
     */
    pub fn decrypt_cenc(
        &self,
        identity: &str,
        kid: &[u8; BLOCK],
        iv: &[u8],
        subsamples: &[(usize, usize)],
        sample: &mut [u8],
    ) -> Result<(), DecryptError> {
        let key = self.find(identity, kid).ok_or(DecryptError::NoKey)?;
        let ranges = protected_ranges(subsamples, sample.len()).ok_or(DecryptError::Malformed)?;
        let mut stream = CtrStream::new(&key, iv).ok_or(DecryptError::Malformed)?;
        for (start, end) in ranges {
            stream.apply(&mut sample[start..end]);
        }
        Ok(())
    }
}

/** The protected byte ranges of a sample, or None when the map overruns it. */
fn protected_ranges(subsamples: &[(usize, usize)], length: usize) -> Option<Vec<(usize, usize)>> {
    if subsamples.is_empty() {
        return Some(vec![(0, length)]);
    }
    let mut offset = 0usize;
    let mut ranges = Vec::new();
    for (clear, protected) in subsamples {
        let start = offset.checked_add(*clear)?;
        offset = start.checked_add(*protected)?;
        ranges.push((start, offset));
    }
    (offset == length).then_some(ranges)
}

/** AES-128 in counter mode with a 128-bit big-endian counter. */
struct CtrStream {
    cipher: Aes128,
    counter: [u8; BLOCK],
    pad: [u8; BLOCK],
    used: usize,
}

impl CtrStream {
    /** A stream from an 8-byte (zero-extended) or 16-byte IV. */
    fn new(key: &[u8; BLOCK], iv: &[u8]) -> Option<Self> {
        let mut counter = [0u8; BLOCK];
        match iv.len() {
            8 | BLOCK => counter[..iv.len()].copy_from_slice(iv),
            _ => return None,
        }
        Some(Self {
            cipher: Aes128::new(GenericArray::from_slice(key)),
            counter,
            pad: [0; BLOCK],
            used: BLOCK,
        })
    }
    /** XOR the keystream into `bytes`, continuing where the last range stopped. */
    fn apply(&mut self, bytes: &mut [u8]) {
        for byte in bytes {
            if self.used == BLOCK {
                let mut block = GenericArray::clone_from_slice(&self.counter);
                self.cipher.encrypt_block(&mut block);
                self.pad.copy_from_slice(&block);
                self.used = 0;
                increment(&mut self.counter);
            }
            *byte ^= self.pad[self.used];
            self.used += 1;
        }
    }
}

/** Add one to a big-endian 128-bit counter. */
fn increment(counter: &mut [u8; BLOCK]) {
    for byte in counter.iter_mut().rev() {
        *byte = byte.wrapping_add(1);
        if *byte != 0 {
            break;
        }
    }
}

/** Sixteen bytes from 32 hex digits. */
fn hex16(text: &str) -> Option<[u8; BLOCK]> {
    if text.len() != BLOCK * 2 {
        return None;
    }
    let mut out = [0u8; BLOCK];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

impl Runtime {
    /** The media pipeline's Clear Key decryption entry point; see `Keys::decrypt_cenc`. */
    pub fn decrypt_clear_key(
        &self,
        identity: &str,
        kid: &[u8; BLOCK],
        iv: &[u8],
        subsamples: &[(usize, usize)],
        sample: &mut [u8],
    ) -> Result<(), DecryptError> {
        self.platform
            .keys
            .decrypt_cenc(identity, kid, iv, subsamples, sample)
    }
}

/** A session accepted a license: its keys become usable for this context. */
fn keys(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let session = args["session"].as_str().unwrap_or_default().to_string();
    let mut accepted = BTreeMap::new();
    for key in args["keys"].as_array().into_iter().flatten() {
        let (Some(kid), Some(value)) = (
            hex16(key["kid"].as_str().unwrap_or_default()),
            hex16(key["key"].as_str().unwrap_or_default()),
        ) else {
            return fail("TypeError", "The license is not a valid JSON Web Key Set.");
        };
        accepted.insert(kid, value);
    }
    let sessions = runtime
        .platform
        .keys
        .contexts
        .entry(ctx.identity.clone())
        .or_default();
    if accepted.len() > SESSION_KEYS
        || (!sessions.contains_key(&session) && sessions.len() >= SESSIONS)
    {
        return fail("QuotaExceededError", "Too many keys for this context.");
    }
    sessions.entry(session).or_default().extend(accepted);
    ok(Value::Null)
}

/** A session closed or was removed: its keys stop being usable. */
fn close(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let session = args["session"].as_str().unwrap_or_default();
    if let Some(sessions) = runtime.platform.keys.contexts.get_mut(&ctx.identity) {
        sessions.remove(session);
    }
    ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    /** A store holding one key for context "page" under key ID 01..01. */
    fn store(key: [u8; BLOCK]) -> Keys {
        let mut keys = Keys::default();
        let session = BTreeMap::from([([1u8; BLOCK], key)]);
        keys.contexts
            .insert("page".into(), BTreeMap::from([("1".into(), session)]));
        keys
    }

    /** NIST SP 800-38A F.5.1 CTR-AES128 vector: the first block decrypts to the known plaintext. */
    #[test]
    fn ctr_matches_nist_vector() {
        let key = hex16("2b7e151628aed2a6abf7158809cf4f3c").unwrap();
        let iv = hex16("f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff").unwrap();
        let mut sample = hex16("874d6191b620e3261bef6864990db6ce").unwrap().to_vec();
        store(key)
            .decrypt_cenc("page", &[1; BLOCK], &iv, &[], &mut sample)
            .unwrap();
        assert_eq!(sample, hex16("6bc1bee22e409f96e93d7e117393172a").unwrap());
    }

    /** Clear bytes stay; protected bytes across subsamples share one continuous keystream. */
    #[test]
    fn subsamples_share_one_keystream() {
        let key = [7u8; BLOCK];
        let keys = store(key);
        let plain: Vec<u8> = (0..40u8).collect();
        let mut whole = plain[4..24]
            .iter()
            .chain(&plain[30..40])
            .copied()
            .collect::<Vec<_>>();
        let mut stream = CtrStream::new(&key, &[9; 8]).unwrap();
        stream.apply(&mut whole);
        let mut sample = plain.clone();
        sample[4..24].copy_from_slice(&whole[..20]);
        sample[30..40].copy_from_slice(&whole[20..]);
        keys.decrypt_cenc(
            "page",
            &[1; BLOCK],
            &[9; 8],
            &[(4, 20), (6, 10)],
            &mut sample,
        )
        .unwrap();
        assert_eq!(sample, plain);
    }

    /** Unknown keys and overrunning subsample maps are reported, not guessed. */
    #[test]
    fn missing_keys_and_bad_maps_are_errors() {
        let keys = store([0; BLOCK]);
        let mut sample = vec![0u8; 8];
        assert_eq!(
            keys.decrypt_cenc("other", &[1; BLOCK], &[0; 8], &[], &mut sample),
            Err(DecryptError::NoKey)
        );
        assert_eq!(
            keys.decrypt_cenc("page", &[1; BLOCK], &[0; 8], &[(4, 8)], &mut sample),
            Err(DecryptError::Malformed)
        );
        assert_eq!(
            keys.decrypt_cenc("page", &[1; BLOCK], &[0; 3], &[], &mut sample),
            Err(DecryptError::Malformed)
        );
    }
}
