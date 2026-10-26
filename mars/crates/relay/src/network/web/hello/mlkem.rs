/**
 * ML-KEM-768 (FIPS 203), the post-quantum half of Chrome's X25519MLKEM768 key share, with the
 * Keccak (FIPS 202) it is built on. Written from the standard's algorithms: K-PKE key
 * generation, encryption and decryption, and the Fujisaki-Okamoto transform with implicit
 * rejection in decapsulation. The client only generates keys and decapsulates; encapsulation
 * exists because decapsulation re-encrypts, and so tests can run both sides.
 */
/// The prime modulus q.
const Q: u32 = 3329;
/// Coefficients per polynomial.
const N: usize = 256;
/// Module rank for ML-KEM-768.
const K: usize = 3;
/// Compression width of u.
const DU: u32 = 10;
/// Compression width of v.
const DV: u32 = 4;
/// Bytes of one 12-bit encoded polynomial.
const POLY_BYTES: usize = 384;
/// Encapsulation key length: t-hat and rho.
pub(super) const EK_BYTES: usize = K * POLY_BYTES + 32;
/// Ciphertext length: c1 then c2.
pub(super) const CT_BYTES: usize = K * 32 * DU as usize + 32 * DV as usize;
/// Decapsulation key length: s-hat, ek, H(ek), z.
const DK_BYTES: usize = K * POLY_BYTES + EK_BYTES + 64;

/** One polynomial, coefficients in [0, q). */
type Poly = [u32; N];

/** The Keccak-f[1600] round constants. */
const ROUND: [u64; 24] = [
    0x0000000000000001,
    0x0000000000008082,
    0x800000000000808a,
    0x8000000080008000,
    0x000000000000808b,
    0x0000000080000001,
    0x8000000080008081,
    0x8000000000008009,
    0x000000000000008a,
    0x0000000000000088,
    0x0000000080008009,
    0x000000008000000a,
    0x000000008000808b,
    0x800000000000008b,
    0x8000000000008089,
    0x8000000000008003,
    0x8000000000008002,
    0x8000000000000080,
    0x000000000000800a,
    0x800000008000000a,
    0x8000000080008081,
    0x8000000000008080,
    0x0000000080000001,
    0x8000000080008008,
];

/** Rho rotation offsets, indexed x + 5y. */
const ROTATE: [u32; 25] = [
    0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39, 41, 45, 15, 21, 8, 18, 2, 61, 56, 14,
];

/** Keccak-f[1600]: theta, rho and pi, chi, iota, 24 rounds. */
fn keccak(a: &mut [u64; 25]) {
    for constant in ROUND {
        let c: [u64; 5] =
            std::array::from_fn(|x| a[x] ^ a[x + 5] ^ a[x + 10] ^ a[x + 15] ^ a[x + 20]);
        for x in 0..5 {
            let d = c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1);
            for y in 0..5 {
                a[x + 5 * y] ^= d;
            }
        }
        let mut b = [0u64; 25];
        for x in 0..5 {
            for y in 0..5 {
                b[y + 5 * ((2 * x + 3 * y) % 5)] = a[x + 5 * y].rotate_left(ROTATE[x + 5 * y]);
            }
        }
        for y in 0..5 {
            for x in 0..5 {
                a[x + 5 * y] = b[x + 5 * y] ^ (!b[(x + 1) % 5 + 5 * y] & b[(x + 2) % 5 + 5 * y]);
            }
        }
        a[0] ^= constant;
    }
}

/** A Keccak sponge: absorb everything, pad once, then squeeze any length. */
pub(super) struct Sponge {
    /// The 1600-bit state.
    state: [u64; 25],
    /// Bytes per block.
    rate: usize,
    /// Position in the current block.
    at: usize,
    /// The domain separation byte (0x06 for SHA-3, 0x1f for SHAKE).
    pad: u8,
}

impl Sponge {
    /** A sponge of `rate` bytes with SHA-3 or SHAKE padding. */
    pub(super) fn new(rate: usize, pad: u8) -> Self {
        Self {
            state: [0; 25],
            rate,
            at: 0,
            pad,
        }
    }
    /** XORs one byte into the state at the current position. */
    fn xor(&mut self, byte: u8) {
        self.state[self.at / 8] ^= u64::from(byte) << (8 * (self.at % 8));
        self.at += 1;
        if self.at == self.rate {
            keccak(&mut self.state);
            self.at = 0;
        }
    }
    /** Absorbs input. */
    pub(super) fn absorb(mut self, bytes: &[u8]) -> Self {
        bytes.iter().for_each(|byte| self.xor(*byte));
        self
    }
    /** Pads and switches to squeezing. */
    pub(super) fn finish(mut self) -> Self {
        self.state[self.at / 8] ^= u64::from(self.pad) << (8 * (self.at % 8));
        let last = self.rate - 1;
        self.state[last / 8] ^= 0x80 << (8 * (last % 8));
        keccak(&mut self.state);
        self.at = 0;
        self
    }
    /** Squeezes the next `out.len()` bytes. */
    pub(super) fn squeeze(&mut self, out: &mut [u8]) {
        for byte in out {
            if self.at == self.rate {
                keccak(&mut self.state);
                self.at = 0;
            }
            *byte = (self.state[self.at / 8] >> (8 * (self.at % 8))) as u8;
            self.at += 1;
        }
    }
}

/** A fixed-length digest of the concatenated parts. */
fn digest<const L: usize>(rate: usize, pad: u8, parts: &[&[u8]]) -> [u8; L] {
    let mut sponge = parts
        .iter()
        .fold(Sponge::new(rate, pad), |sponge, part| sponge.absorb(part))
        .finish();
    let mut out = [0; L];
    sponge.squeeze(&mut out);
    out
}

/** SHA3-256, FIPS 203's H. */
pub(super) fn sha3_256(parts: &[&[u8]]) -> [u8; 32] {
    digest(136, 0x06, parts)
}

/** SHA3-512, FIPS 203's G. */
fn sha3_512(parts: &[&[u8]]) -> [u8; 64] {
    digest(72, 0x06, parts)
}

/** SHAKE256 to 32 bytes, FIPS 203's J. */
fn shake256_32(parts: &[&[u8]]) -> [u8; 32] {
    digest(136, 0x1f, parts)
}

/** 17^BitRev7(i) mod q, the NTT's zetas, computed once. */
fn zetas() -> &'static [u32; 128] {
    static ZETAS: std::sync::OnceLock<[u32; 128]> = std::sync::OnceLock::new();
    ZETAS.get_or_init(|| std::array::from_fn(|i| power(17, (i as u8).reverse_bits() as u32 >> 1)))
}

/** 17^(2 BitRev7(i) + 1) mod q, the base-case multiplication's gammas, computed once. */
fn gammas() -> &'static [u32; 128] {
    static GAMMAS: std::sync::OnceLock<[u32; 128]> = std::sync::OnceLock::new();
    GAMMAS.get_or_init(|| {
        std::array::from_fn(|i| power(17, 2 * ((i as u8).reverse_bits() as u32 >> 1) + 1))
    })
}

/** base^exponent mod q. */
fn power(base: u32, exponent: u32) -> u32 {
    (0..exponent).fold(1, |acc, _| acc * base % Q)
}

/** Algorithm 9, NTT. */
fn ntt(f: &mut Poly) {
    let mut i = 1;
    let mut len = 128;
    while len >= 2 {
        for start in (0..N).step_by(2 * len) {
            let zeta = zetas()[i];
            i += 1;
            for j in start..start + len {
                let t = zeta * f[j + len] % Q;
                f[j + len] = (f[j] + Q - t) % Q;
                f[j] = (f[j] + t) % Q;
            }
        }
        len /= 2;
    }
}

/** Algorithm 10, inverse NTT, scaled by 128^-1 = 3303. */
fn inverse_ntt(f: &mut Poly) {
    let mut i = 127;
    let mut len = 2;
    while len <= 128 {
        for start in (0..N).step_by(2 * len) {
            let zeta = zetas()[i];
            i -= 1;
            for j in start..start + len {
                let t = f[j];
                f[j] = (t + f[j + len]) % Q;
                f[j + len] = zeta * ((f[j + len] + Q - t) % Q) % Q;
            }
        }
        len *= 2;
    }
    f.iter_mut().for_each(|c| *c = *c * 3303 % Q);
}

/** Algorithms 11 and 12: the product of two NTT-domain polynomials. */
fn multiply(f: &Poly, g: &Poly) -> Poly {
    let mut h = [0; N];
    for i in 0..128 {
        let gamma = gammas()[i];
        let (a0, a1, b0, b1) = (f[2 * i], f[2 * i + 1], g[2 * i], g[2 * i + 1]);
        h[2 * i] = (a0 * b0 % Q + a1 * b1 % Q * gamma) % Q;
        h[2 * i + 1] = (a0 * b1 % Q + a1 * b0 % Q) % Q;
    }
    h
}

/** Coefficient-wise sum mod q. */
fn add(f: &Poly, g: &Poly) -> Poly {
    std::array::from_fn(|i| (f[i] + g[i]) % Q)
}

/** The sum over j of the products of two vectors in the NTT domain. */
fn dot(f: &[Poly; K], g: &[Poly; K]) -> Poly {
    (0..K).fold([0; N], |acc, j| add(&acc, &multiply(&f[j], &g[j])))
}

/** Algorithm 7, SampleNTT: rejection sampling from SHAKE128(rho || j || i). */
fn sample_ntt(rho: &[u8; 32], j: u8, i: u8) -> Poly {
    let mut xof = Sponge::new(168, 0x1f).absorb(rho).absorb(&[j, i]).finish();
    let mut a = [0; N];
    let mut count = 0;
    while count < N {
        let mut c = [0u8; 3];
        xof.squeeze(&mut c);
        let d1 = u32::from(c[0]) + 256 * (u32::from(c[1]) & 15);
        let d2 = u32::from(c[1]) / 16 + 16 * u32::from(c[2]);
        for d in [d1, d2] {
            if d < Q && count < N {
                a[count] = d;
                count += 1;
            }
        }
    }
    a
}

/** Algorithm 8 with eta = 2, from PRF(seed, nonce) = SHAKE256(seed || nonce, 128). */
fn cbd(seed: &[u8; 32], nonce: u8) -> Poly {
    let mut bytes = [0u8; 128];
    Sponge::new(136, 0x1f)
        .absorb(seed)
        .absorb(&[nonce])
        .finish()
        .squeeze(&mut bytes);
    let bit = |k: usize| u32::from(bytes[k / 8] >> (k % 8) & 1);
    std::array::from_fn(|i| (Q + bit(4 * i) + bit(4 * i + 1) - bit(4 * i + 2) - bit(4 * i + 3)) % Q)
}

/** Algorithm 5, ByteEncode_d: `d` bits per coefficient, little-endian. */
fn encode(f: &Poly, d: u32, out: &mut Vec<u8>) {
    let mut acc: u64 = 0;
    let mut bits = 0;
    for c in f {
        acc |= u64::from(*c) << bits;
        bits += d;
        while bits >= 8 {
            out.push(acc as u8);
            acc >>= 8;
            bits -= 8;
        }
    }
}

/** Algorithm 6, ByteDecode_d; 12-bit values are reduced mod q as the standard says. */
fn decode(bytes: &[u8], d: u32) -> Poly {
    let bit = |k: usize| u32::from(bytes[k / 8] >> (k % 8) & 1);
    std::array::from_fn(|i| {
        let value = (0..d as usize).fold(0, |acc, b| acc | bit(i * d as usize + b) << b);
        if d == 12 { value % Q } else { value }
    })
}

/** Compress_d: round(2^d / q * x) mod 2^d. */
fn compress(f: &Poly, d: u32) -> Poly {
    std::array::from_fn(|i| {
        ((((f[i] as u64) << (d + 1)) + Q as u64) / (2 * Q as u64)) as u32 & ((1 << d) - 1)
    })
}

/** Decompress_d: round(q / 2^d * y). */
fn decompress(f: &Poly, d: u32) -> Poly {
    std::array::from_fn(|i| (f[i] * Q + (1 << (d - 1))) >> d)
}

/** Â from rho; `transpose` gives Â^T, entry [i][j] = SampleNTT(rho || i || j). */
fn matrix(rho: &[u8; 32], transpose: bool) -> [[Poly; K]; K] {
    std::array::from_fn(|i| {
        std::array::from_fn(|j| {
            let (a, b) = if transpose { (i, j) } else { (j, i) };
            sample_ntt(rho, a as u8, b as u8)
        })
    })
}

/** A vector of K noise polynomials from consecutive nonces, moved into the NTT domain. */
fn noise(seed: &[u8; 32], first: u8, transform: bool) -> [Poly; K] {
    std::array::from_fn(|i| {
        let mut f = cbd(seed, first + i as u8);
        if transform {
            ntt(&mut f);
        }
        f
    })
}

/** A key pair: the encapsulation key sent to the server, the decapsulation key kept. */
pub(super) struct KeyPair {
    /// ek: ByteEncode12(t-hat) || rho.
    pub(super) public: Vec<u8>,
    /// dk: ByteEncode12(s-hat) || ek || H(ek) || z.
    secret: Vec<u8>,
}

/** Algorithms 13 and 16: ML-KEM.KeyGen_internal(d, z). */
pub(super) fn generate(d: &[u8; 32], z: &[u8; 32]) -> KeyPair {
    let g = sha3_512(&[d, &[K as u8]]);
    let (rho, sigma): ([u8; 32], [u8; 32]) = (
        g[..32].try_into().unwrap_or_default(),
        g[32..].try_into().unwrap_or_default(),
    );
    let a = matrix(&rho, false);
    let s = noise(&sigma, 0, true);
    let e = noise(&sigma, K as u8, true);
    let mut public = Vec::with_capacity(EK_BYTES);
    for i in 0..K {
        encode(&add(&dot(&a[i], &s), &e[i]), 12, &mut public);
    }
    public.extend_from_slice(&rho);
    let mut secret = Vec::with_capacity(DK_BYTES);
    s.iter().for_each(|f| encode(f, 12, &mut secret));
    secret.extend_from_slice(&public);
    secret.extend_from_slice(&sha3_256(&[&public]));
    secret.extend_from_slice(z);
    KeyPair { public, secret }
}

/** Algorithm 14, K-PKE.Encrypt(ek, m, r). */
fn encrypt(ek: &[u8], m: &[u8; 32], r: &[u8; 32]) -> Vec<u8> {
    let t: [Poly; K] = std::array::from_fn(|i| decode(&ek[i * POLY_BYTES..], 12));
    let rho: [u8; 32] = ek[K * POLY_BYTES..].try_into().unwrap_or_default();
    let a = matrix(&rho, true);
    let y = noise(r, 0, true);
    let e1 = noise(r, K as u8, false);
    let e2 = cbd(r, 2 * K as u8);
    let mut out = Vec::with_capacity(CT_BYTES);
    for i in 0..K {
        let mut u = dot(&a[i], &y);
        inverse_ntt(&mut u);
        encode(&compress(&add(&u, &e1[i]), DU), DU, &mut out);
    }
    let mut v = dot(&t, &y);
    inverse_ntt(&mut v);
    let mu = decompress(&decode(m, 1), 1);
    encode(&compress(&add(&add(&v, &e2), &mu), DV), DV, &mut out);
    out
}

/** Algorithm 15, K-PKE.Decrypt(dk_pke, c). */
fn decrypt(dk: &[u8], c: &[u8]) -> [u8; 32] {
    let chunk = 32 * DU as usize;
    let mut u: [Poly; K] = std::array::from_fn(|i| decompress(&decode(&c[i * chunk..], DU), DU));
    u.iter_mut().for_each(ntt);
    let v = decompress(&decode(&c[K * chunk..], DV), DV);
    let s: [Poly; K] = std::array::from_fn(|i| decode(&dk[i * POLY_BYTES..], 12));
    let mut su = dot(&s, &u);
    inverse_ntt(&mut su);
    let w: Poly = std::array::from_fn(|i| (v[i] + Q - su[i]) % Q);
    let mut m = Vec::with_capacity(32);
    encode(&compress(&w, 1), 1, &mut m);
    m.try_into().unwrap_or_default()
}

/** Algorithm 17, ML-KEM.Encaps_internal(ek, m): the shared key and ciphertext; the server's side, so tests only. */
#[cfg(test)]
pub(super) fn encapsulate(ek: &[u8], m: &[u8; 32]) -> ([u8; 32], Vec<u8>) {
    let g = sha3_512(&[m, &sha3_256(&[ek])]);
    let r: [u8; 32] = g[32..].try_into().unwrap_or_default();
    (g[..32].try_into().unwrap_or_default(), encrypt(ek, m, &r))
}

/**
 * Algorithm 18, ML-KEM.Decaps_internal(dk, c): re-encrypts and, on any mismatch, answers the
 * implicit-rejection key J(z || c) instead, selected in constant time. None only when the
 * ciphertext has the wrong length, which the caller reports as an illegal parameter.
 */
pub(super) fn decapsulate(pair: &KeyPair, c: &[u8]) -> Option<[u8; 32]> {
    if c.len() != CT_BYTES || pair.secret.len() != DK_BYTES {
        return None;
    }
    let dk = &pair.secret;
    let ek = &dk[K * POLY_BYTES..K * POLY_BYTES + EK_BYTES];
    let h = &dk[K * POLY_BYTES + EK_BYTES..DK_BYTES - 32];
    let z = &dk[DK_BYTES - 32..];
    let m = decrypt(dk, c);
    let g = sha3_512(&[&m, h]);
    let r: [u8; 32] = g[32..].try_into().unwrap_or_default();
    let rejected = shake256_32(&[z, c]);
    let same = encrypt(ek, &m, &r)
        .iter()
        .zip(c)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b));
    let keep = (u16::from(same).wrapping_sub(1) >> 8) as u8;
    Some(std::array::from_fn(|i| {
        (g[i] & keep) | (rejected[i] & !keep)
    }))
}
