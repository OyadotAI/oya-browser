/**
 * The public-key half of TLS's RSA key exchange: the leaf certificate's RSA key and Key Usage
 * read from its DER, and RSAES-PKCS1-v1_5 encryption (RFC 8017 section 7.2.1) over a small
 * Montgomery bignum. Only public-key operations run in production; the private operation exists
 * for the loopback test server.
 */
use std::io;

/// rsaEncryption, 1.2.840.113549.1.1.1, as its DER OID body.
const RSA_ENCRYPTION: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01];
/// id-ce-keyUsage, 2.5.29.15.
const KEY_USAGE: &[u8] = &[0x55, 0x1d, 0x0f];
/// keyEncipherment is bit 2 of the KeyUsage BIT STRING, so 0x20 of its first byte.
const KEY_ENCIPHERMENT: u8 = 0x20;
/// Public CAs have issued no smaller RSA key since 2013; a smaller one is refused.
const MIN_MODULUS_BYTES: usize = 256;
/// Larger keys than any CA issues would only cost time.
const MAX_MODULUS_BYTES: usize = 1024;
/// PKCS #1 v1.5 needs 0x00 0x02, eight nonzero padding bytes and a 0x00 separator.
const PADDING_OVERHEAD: usize = 11;
/// The longest public exponent accepted; 65537 needs three.
const MAX_EXPONENT_BYTES: usize = 4;

/** A leaf certificate's RSA public key. */
pub(crate) struct PublicKey {
    /// Modulus, big-endian, without a leading zero.
    n: Vec<u8>,
    /// Public exponent, big-endian.
    e: Vec<u8>,
}

/** One DER element reader over a byte slice; it never reads past the slice. */
struct Der<'a>(&'a [u8]);

impl<'a> Der<'a> {
    /** The next element's tag and contents; definite lengths of up to four bytes. */
    fn next(&mut self) -> Option<(u8, &'a [u8])> {
        let (&tag, rest) = self.0.split_first()?;
        let (&first, rest) = rest.split_first()?;
        let (length, rest) = if first < 0x80 {
            (usize::from(first), rest)
        } else {
            let count = usize::from(first & 0x7f);
            if count == 0 || count > 4 || rest.len() < count {
                return None;
            }
            let length = rest[..count]
                .iter()
                .fold(0usize, |sum, &byte| (sum << 8) | usize::from(byte));
            (length, &rest[count..])
        };
        if rest.len() < length {
            return None;
        }
        self.0 = &rest[length..];
        Some((tag, &rest[..length]))
    }

    /** The next element's contents when it has `tag`. */
    fn expect(&mut self, tag: u8) -> Option<&'a [u8]> {
        self.next()
            .filter(|(found, _)| *found == tag)
            .map(|(_, body)| body)
    }
}

/**
 * The RSA key of an end-entity certificate whose chain was already verified. Refuses a
 * non-RSA key, a modulus under 2048 bits, and (as BoringSSL does for RSA key exchange) a Key
 * Usage extension that omits keyEncipherment.
 */
pub(crate) fn leaf_key(certificate: &[u8]) -> io::Result<PublicKey> {
    leaf_key_parts(certificate).ok_or_else(|| invalid("certificate key"))
}

/** The parse behind `leaf_key`; None on any malformed or refused field. */
fn leaf_key_parts(certificate: &[u8]) -> Option<PublicKey> {
    let mut tbs = Der(Der(Der(certificate).expect(0x30)?).expect(0x30)?);
    if tbs.next()?.0 == 0xa0 {
        tbs.next()?;
    }
    // serial (already read), signature, issuer, validity, subject, then the key.
    for _ in 0..4 {
        tbs.next()?;
    }
    let key = spki(tbs.expect(0x30)?)?;
    while let Some((tag, body)) = tbs.next() {
        if tag == 0xa3 && !permits_key_encipherment(body)? {
            return None;
        }
    }
    Some(key)
}

/** SubjectPublicKeyInfo holding an rsaEncryption RSAPublicKey. */
pub(super) fn spki(body: &[u8]) -> Option<PublicKey> {
    let mut spki = Der(body);
    let mut algorithm = Der(spki.expect(0x30)?);
    if algorithm.expect(0x06)? != RSA_ENCRYPTION {
        return None;
    }
    let bits = spki.expect(0x03)?;
    let (0, key) = bits.split_first()? else {
        return None;
    };
    let mut key = Der(Der(key).expect(0x30)?);
    let n = unsigned(key.expect(0x02)?)?;
    let e = unsigned(key.expect(0x02)?)?;
    let sized = (MIN_MODULUS_BYTES..=MAX_MODULUS_BYTES).contains(&n.len());
    (sized && n.last()? & 1 == 1 && exponent_ok(&e)).then_some(PublicKey { n, e })
}

/**
 * A public exponent is odd, at least 3 and at most 4 bytes: e = 1 would send the premaster in
 * the clear, an even e is not invertible, and a long one only slows the bignum down.
 */
fn exponent_ok(e: &[u8]) -> bool {
    (1..=MAX_EXPONENT_BYTES).contains(&e.len())
        && e.last().is_some_and(|byte| byte & 1 == 1)
        && e != [1]
}

/** A positive DER INTEGER without its sign byte; None for a negative or zero value. */
fn unsigned(body: &[u8]) -> Option<Vec<u8>> {
    if body.first()? & 0x80 != 0 {
        return None;
    }
    let start = body.iter().position(|&byte| byte != 0)?;
    Some(body[start..].to_vec())
}

/** Whether the [3] extensions allow key encipherment; absent Key Usage allows every use. */
fn permits_key_encipherment(explicit: &[u8]) -> Option<bool> {
    let mut extensions = Der(Der(explicit).expect(0x30)?);
    while let Some((_, extension)) = extensions.next() {
        let mut fields = Der(extension);
        if fields.expect(0x06)? != KEY_USAGE {
            continue;
        }
        let mut value = fields.next()?;
        if value.0 == 0x01 {
            value = fields.next()?;
        }
        let bits = Der(value.1).expect(0x03)?;
        return Some(bits.get(1).is_some_and(|byte| byte & KEY_ENCIPHERMENT != 0));
    }
    Some(true)
}

impl PublicKey {
    /** The modulus length in bytes, which is also the ciphertext length. */
    pub(crate) fn size(&self) -> usize {
        self.n.len()
    }

    /**
     * RSAES-PKCS1-v1_5: 0x00 0x02, nonzero random padding, 0x00, the message, raised to e.
     * `random` fills the padding and must be a CSPRNG.
     */
    pub(crate) fn encrypt(
        &self,
        message: &[u8],
        random: &dyn ring::rand::SecureRandom,
    ) -> io::Result<Vec<u8>> {
        let k = self.size();
        if message.len() + PADDING_OVERHEAD > k {
            return Err(invalid("RSA message length"));
        }
        let mut block = vec![0u8; k];
        block[1] = 2;
        let padding = k - message.len() - 1;
        for byte in &mut block[2..padding] {
            while *byte == 0 {
                let mut one = [0u8];
                random.fill(&mut one).map_err(|_| invalid("random"))?;
                *byte = one[0];
            }
        }
        block[padding + 1..].copy_from_slice(message);
        Ok(modpow(&block, &self.e, &self.n))
    }
}

/** `base^exponent mod modulus` for an odd modulus and base < modulus, all big-endian. */
pub(crate) fn modpow(base: &[u8], exponent: &[u8], modulus: &[u8]) -> Vec<u8> {
    let n = limbs(modulus, None);
    let field = Montgomery::new(&n);
    let one = field.enter(&limbs(&[1], Some(n.len())));
    let x = field.enter(&limbs(base, Some(n.len())));
    let mut result = one;
    for byte in exponent {
        for bit in (0..8).rev() {
            result = field.multiply(&result, &result);
            if byte >> bit & 1 == 1 {
                result = field.multiply(&result, &x);
            }
        }
    }
    let plain = field.multiply(&result, &limbs(&[1], Some(n.len())));
    big_endian(&plain, modulus.len())
}

/** Little-endian 64-bit limbs of a big-endian number, padded to `width` limbs if given. */
fn limbs(bytes: &[u8], width: Option<usize>) -> Vec<u64> {
    let mut out: Vec<u64> = bytes
        .rchunks(8)
        .map(|chunk| chunk.iter().fold(0u64, |sum, &b| (sum << 8) | u64::from(b)))
        .collect();
    out.resize(width.unwrap_or(out.len()).max(out.len()), 0);
    out
}

/** A number's big-endian bytes, left-padded or truncated to `length`. */
fn big_endian(limbs: &[u64], length: usize) -> Vec<u8> {
    let bytes: Vec<u8> = limbs
        .iter()
        .rev()
        .flat_map(|limb| limb.to_be_bytes())
        .collect();
    bytes[bytes.len() - length..].to_vec()
}

/** Montgomery arithmetic modulo one odd modulus (CIOS multiplication). */
struct Montgomery<'a> {
    n: &'a [u64],
    /// -n^-1 mod 2^64.
    inverse: u64,
    /// R^2 mod n, R = 2^(64 * limbs), to move numbers into the Montgomery domain.
    r2: Vec<u64>,
}

impl<'a> Montgomery<'a> {
    /** Precomputes the limb inverse and R^2 mod n. */
    fn new(n: &'a [u64]) -> Self {
        let mut inverse = 1u64;
        for _ in 0..6 {
            inverse = inverse.wrapping_mul(2u64.wrapping_sub(n[0].wrapping_mul(inverse)));
        }
        let mut wide = vec![0u64; 2 * n.len() + 1];
        wide[2 * n.len()] = 1;
        Self {
            n,
            inverse: inverse.wrapping_neg(),
            r2: remainder(&wide, n),
        }
    }

    /** x * R mod n. */
    fn enter(&self, x: &[u64]) -> Vec<u64> {
        self.multiply(x, &self.r2)
    }

    /** a * b * R^-1 mod n for a, b < n. */
    fn multiply(&self, a: &[u64], b: &[u64]) -> Vec<u64> {
        let k = self.n.len();
        let mut t = vec![0u64; k + 2];
        for &bi in b.iter().take(k) {
            let mut carry = 0u128;
            for j in 0..k {
                let sum = u128::from(t[j]) + u128::from(a[j]) * u128::from(bi) + carry;
                t[j] = sum as u64;
                carry = sum >> 64;
            }
            let sum = u128::from(t[k]) + carry;
            t[k] = sum as u64;
            t[k + 1] = (sum >> 64) as u64;
            let m = t[0].wrapping_mul(self.inverse);
            let mut carry = (u128::from(t[0]) + u128::from(m) * u128::from(self.n[0])) >> 64;
            for j in 1..k {
                let sum = u128::from(t[j]) + u128::from(m) * u128::from(self.n[j]) + carry;
                t[j - 1] = sum as u64;
                carry = sum >> 64;
            }
            let sum = u128::from(t[k]) + carry;
            t[k - 1] = sum as u64;
            t[k] = t[k + 1] + (sum >> 64) as u64;
        }
        if t[k] != 0 || !less(&t[..k], self.n) {
            subtract(&mut t, self.n);
        }
        t.truncate(k);
        t
    }
}

/** Whether a < b, both with the same limb count. */
fn less(a: &[u64], b: &[u64]) -> bool {
    for (x, y) in a.iter().rev().zip(b.iter().rev()) {
        if x != y {
            return x < y;
        }
    }
    false
}

/** a -= b in place, where a has at least as many limbs and a >= b. */
fn subtract(a: &mut [u64], b: &[u64]) {
    let mut borrow = 0u64;
    for (i, limb) in a.iter_mut().enumerate() {
        let (x, under1) = limb.overflowing_sub(b.get(i).copied().unwrap_or(0));
        let (x, under2) = x.overflowing_sub(borrow);
        *limb = x;
        borrow = u64::from(under1 || under2);
    }
}

/**
 * x mod n by binary long division. It runs once per key for R^2, so its quadratic bit loop
 * costs microseconds. ponytail: bitwise division; Knuth D if it ever runs per record.
 */
fn remainder(x: &[u64], n: &[u64]) -> Vec<u64> {
    let mut r = vec![0u64; n.len() + 1];
    for bit in (0..x.len() * 64).rev() {
        let mut carry = x[bit / 64] >> (bit % 64) & 1;
        for limb in &mut r {
            let next = *limb >> 63;
            *limb = (*limb << 1) | carry;
            carry = next;
        }
        if r[n.len()] != 0 || !less(&r[..n.len()], n) {
            subtract(&mut r, n);
        }
    }
    r.truncate(n.len());
    r
}

/** A refused field, surfaced as TLS failure by the transport. */
fn invalid(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("legacy TLS: {what}"))
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// The loopback fixtures: an RSA leaf for key exchange, and one whose Key Usage forbids it.
    const ENC: &[u8] = include_bytes!("../fixtures/rsa-enc.der");
    const SIGN: &[u8] = include_bytes!("../fixtures/rsa-sign.der");
    const ENC_KEY: &[u8] = include_bytes!("../fixtures/rsa-enc.key.der");

    /** The PKCS #1 RSAPrivateKey's modulus and private exponent. */
    pub(crate) fn private(der: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let mut key = Der(Der(der).expect(0x30).expect("key"));
        key.expect(0x02).expect("version");
        let n = unsigned(key.expect(0x02).expect("n")).expect("n");
        key.expect(0x02).expect("e");
        let d = unsigned(key.expect(0x02).expect("d")).expect("d");
        (n, d)
    }

    #[test]
    fn modpow_matches_small_known_values() {
        // 4^13 mod 497 = 445, the textbook example; 3^200 mod 1e9+7 checked by plain arithmetic.
        assert_eq!(modpow(&[4], &[13], &[0x01, 0xf1]), vec![0x01, 0xbd]);
        let big = modpow(&[3], &[200], &1_000_000_007u32.to_be_bytes());
        assert_eq!(
            u32::from_be_bytes(big.try_into().expect("four bytes")),
            expected_3_200()
        );
    }

    /** 3^200 mod 1e9+7 by repeated u128 multiplication. */
    fn expected_3_200() -> u32 {
        (0..200).fold(1u128, |acc, _| acc * 3 % 1_000_000_007) as u32
    }

    #[test]
    fn pkcs1_encryption_round_trips_through_the_private_key() {
        let key = leaf_key(ENC).expect("rsa leaf");
        let (n, d) = private(ENC_KEY);
        assert_eq!(n, key.n);
        let message = [3u8, 3, 7, 7, 7];
        let cipher = key
            .encrypt(&message, &ring::rand::SystemRandom::new())
            .expect("encrypt");
        assert_eq!(cipher.len(), 256);
        let block = modpow(&cipher, &d, &n);
        assert_eq!(&block[..2], &[0, 2]);
        let separator = block[2..].iter().position(|&b| b == 0).expect("separator") + 2;
        assert!(separator >= 10, "eight nonzero padding bytes");
        assert_eq!(&block[separator + 1..], &message);
    }

    #[test]
    fn a_key_usage_without_key_encipherment_refuses_rsa_key_exchange() {
        assert!(leaf_key(SIGN).is_err());
    }

    #[test]
    fn a_non_rsa_leaf_is_refused() {
        assert!(leaf_key(include_bytes!("../fixtures/leaf.der")).is_err());
    }
}
