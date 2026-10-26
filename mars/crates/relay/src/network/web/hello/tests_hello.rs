/** Keccak, ML-KEM-768, the hybrid group and the ClientHello they put on the wire. */
use super::{hybrid, mlkem};
use rustls::crypto::SupportedKxGroup;

/** Lowercase hex for comparing against published vectors. */
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn keccak_matches_the_fips_202_vectors() {
    assert_eq!(
        hex(&mlkem::sha3_256(&[b""])),
        "a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a"
    );
    assert_eq!(
        hex(&mlkem::sha3_256(&[b"abc"])),
        "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532"
    );
    // NIST's 1600-bit message spans more than one 136-byte block.
    assert_eq!(
        hex(&mlkem::sha3_256(&[&[0xa3; 200]])),
        "79f38adec5c20307a98ef76e8324afbfd46cfd81b22e3973c65fa1bd9de31787"
    );
    let mut shake = [0u8; 32];
    mlkem::Sponge::new(168, 0x1f).finish().squeeze(&mut shake);
    assert_eq!(
        hex(&shake),
        "7f9c2ba4e88f827d616045507605853ed73b8093f6efbc88eb1a6eacfa66ef26"
    );
}

#[test]
fn ml_kem_decapsulates_what_was_encapsulated_and_sizes_match_fips_203() {
    let pair = mlkem::generate(&[7; 32], &[9; 32]);
    assert_eq!(pair.public.len(), 1184);
    let (key, ciphertext) = mlkem::encapsulate(&pair.public, &[3; 32]);
    assert_eq!(ciphertext.len(), 1088);
    assert_eq!(mlkem::decapsulate(&pair, &ciphertext), Some(key));
}

#[test]
fn a_tampered_ciphertext_gets_the_implicit_rejection_key_not_the_real_one() {
    let pair = mlkem::generate(&[1; 32], &[2; 32]);
    let (key, mut ciphertext) = mlkem::encapsulate(&pair.public, &[5; 32]);
    ciphertext[10] ^= 1;
    let rejected = mlkem::decapsulate(&pair, &ciphertext).expect("length is right");
    assert_ne!(rejected, key);
    assert_eq!(mlkem::decapsulate(&pair, &ciphertext[1..]), None);
}

#[test]
fn the_hybrid_secret_is_the_ml_kem_key_then_the_x25519_secret() {
    use ring::agreement::{EphemeralPrivateKey, UnparsedPublicKey, X25519, agree_ephemeral};
    let client = hybrid::X25519MlKem768.start().expect("start");
    let share = client.pub_key().to_vec();
    assert_eq!(share.len(), 1184 + 32);
    let (_, component) = client.hybrid_component().expect("component");
    assert_eq!(component, &share[1184..]);
    let (kem, ciphertext) = mlkem::encapsulate(&share[..1184], &[8; 32]);
    let rng = ring::rand::SystemRandom::new();
    let server = EphemeralPrivateKey::generate(&X25519, &rng).expect("key");
    let server_public = server.compute_public_key().expect("public");
    let classic = agree_ephemeral(
        server,
        &UnparsedPublicKey::new(&X25519, &share[1184..]),
        |s| s.to_vec(),
    )
    .expect("agree");
    let answer = [ciphertext.as_slice(), server_public.as_ref()].concat();
    let secret = client.complete(&answer).expect("complete");
    assert_eq!(secret.secret_bytes(), [&kem[..], &classic[..]].concat());
}

#[test]
fn a_hybrid_answer_of_the_wrong_length_is_refused() {
    let client = hybrid::X25519MlKem768.start().expect("start");
    assert!(client.complete(&[0; 1088]).is_err());
}

/** The extensions of the first ClientHello the shared configuration writes. */
fn hello_extensions() -> Vec<(u16, Vec<u8>)> {
    let name = rustls::pki_types::ServerName::try_from("example.com").expect("name");
    let mut connection =
        rustls::ClientConnection::new(super::config().expect("config"), name).expect("conn");
    let mut wire = Vec::new();
    connection.write_tls(&mut wire).expect("write");
    if let Ok(path) = std::env::var("RELAY_HELLO_DUMP") {
        std::fs::write(path, &wire).expect("dump");
    }
    let hello = &wire[5 + 4..];
    let mut at = 2 + 32;
    at += 1 + usize::from(hello[at]);
    at += 2 + usize::from(u16::from_be_bytes([hello[at], hello[at + 1]]));
    at += 1 + usize::from(hello[at]);
    let end = at + 2 + usize::from(u16::from_be_bytes([hello[at], hello[at + 1]]));
    at += 2;
    let mut extensions = Vec::new();
    while at < end {
        let kind = u16::from_be_bytes([hello[at], hello[at + 1]]);
        let length = usize::from(u16::from_be_bytes([hello[at + 2], hello[at + 3]]));
        extensions.push((kind, hello[at + 4..at + 4 + length].to_vec()));
        at += 4 + length;
    }
    extensions
}

#[test]
fn the_hello_offers_chromes_groups_and_both_of_chromes_key_shares() {
    let extensions = hello_extensions();
    let find = |kind| &extensions.iter().find(|(k, _)| *k == kind).expect("ext").1;
    assert_eq!(
        find(10),
        &[0, 8, 0x11, 0xec, 0x00, 0x1d, 0x00, 0x17, 0x00, 0x18]
    );
    let shares = find(51);
    assert_eq!(&shares[2..6], &[0x11, 0xec, 0x04, 0xc0]);
    assert_eq!(&shares[6 + 1216..6 + 1216 + 4], &[0x00, 0x1d, 0x00, 0x20]);
}

/**
 * Interop with an independent ML-KEM: OpenSSL 3.6 generated the key from seed d = 07..07,
 * z = 09..09 (`genpkey -algorithm ML-KEM-768 -pkeyopt hexseed:`) and encapsulated to it
 * (`pkeyutl -encap`); our key must be byte-identical and our decapsulation must agree.
 */
#[test]
fn ml_kem_matches_openssl_for_the_same_seed_and_ciphertext() {
    let pair = mlkem::generate(&[7; 32], &[9; 32]);
    assert_eq!(pair.public, include_bytes!("kat/ek.bin"));
    assert_eq!(
        mlkem::decapsulate(&pair, include_bytes!("kat/ct.bin")),
        Some(*include_bytes!("kat/ss.bin"))
    );
}
