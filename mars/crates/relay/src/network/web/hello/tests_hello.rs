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

/** The cipher suites and extensions of the first ClientHello the shared configuration writes. */
fn hello() -> (Vec<u16>, Vec<(u16, Vec<u8>)>) {
    let name = rustls::pki_types::ServerName::try_from("example.com").expect("name");
    let mut connection =
        rustls::ClientConnection::new(super::config().expect("config"), name).expect("conn");
    let mut wire = Vec::new();
    connection.write_tls(&mut wire).expect("write");
    if let Ok(path) = std::env::var("RELAY_HELLO_DUMP") {
        std::fs::write(path, &wire).expect("dump");
    }
    decode_hello(&wire[5 + 4..])
}

/** Decode one ClientHello body so retry assertions inspect the actual emitted shares. */
fn decode_hello(hello: &[u8]) -> (Vec<u16>, Vec<(u16, Vec<u8>)>) {
    let u16_at = |at: usize| u16::from_be_bytes([hello[at], hello[at + 1]]);
    let mut at = 2 + 32;
    at += 1 + usize::from(hello[at]);
    let suites = (0..usize::from(u16_at(at)) / 2)
        .map(|index| u16_at(at + 2 + 2 * index))
        .collect();
    at += 2 + usize::from(u16_at(at));
    at += 1 + usize::from(hello[at]);
    let end = at + 2 + usize::from(u16_at(at));
    at += 2;
    let mut extensions = Vec::new();
    while at < end {
        let (kind, length) = (u16_at(at), usize::from(u16_at(at + 2)));
        extensions.push((kind, hello[at + 4..at + 4 + length].to_vec()));
        at += 4 + length;
    }
    (suites, extensions)
}

/** The extensions of the first ClientHello the shared configuration writes. */
fn hello_extensions() -> Vec<(u16, Vec<u8>)> {
    hello().1
}

/** The body of extension `kind`, which must be present. */
fn extension(extensions: &[(u16, Vec<u8>)], kind: u16) -> &[u8] {
    &extensions.iter().find(|(k, _)| *k == kind).expect("ext").1
}

/** Chrome's suites, in Chrome's order, after its leading GREASE. */
const CHROME_SUITES: [u16; 15] = [
    0x1301, 0x1302, 0x1303, 0xc02b, 0xc02f, 0xc02c, 0xc030, 0xcca9, 0xcca8, 0xc013, 0xc014, 0x009c,
    0x009d, 0x002f, 0x0035,
];
/** Chrome's extension types, sorted; GREASE ones excluded. */
const CHROME_EXTENSIONS: [u16; 16] = [
    0, 5, 10, 11, 13, 16, 18, 23, 27, 35, 43, 45, 51, 17613, 65037, 65281,
];

/** The lowercase hex of a hash's first twelve characters, as JA4 abbreviates. */
fn ja4_hash(text: &str) -> String {
    hex(ring::digest::digest(&ring::digest::SHA256, text.as_bytes()).as_ref())[..12].to_owned()
}

/** The comma-joined four-digit hex of sorted codes, JA4's raw form. */
fn sorted_hex(codes: impl Iterator<Item = u16>) -> String {
    let mut codes: Vec<u16> = codes.collect();
    codes.sort_unstable();
    codes
        .iter()
        .map(|code| format!("{code:04x}"))
        .collect::<Vec<_>>()
        .join(",")
}

/** JA4 (FoxIO) of the first hello: version, SNI, counts, ALPN, then the two sorted-list hashes. */
fn ja4(suites: &[u16], extensions: &[(u16, Vec<u8>)]) -> (String, String, String) {
    let real_suites = suites
        .iter()
        .copied()
        .filter(|s| !rustls::client::is_grease(*s));
    let real_extensions = || {
        extensions
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| !rustls::client::is_grease(*k))
    };
    let alpn = extension(extensions, 16);
    let first = alpn[3] as char;
    let last = alpn[3 + usize::from(alpn[2]) - 1] as char;
    let a = format!(
        "t13d{:02}{:02}{first}{last}",
        real_suites.clone().count(),
        real_extensions().count()
    );
    let signature_algorithms = extension(extensions, 13);
    let schemes = signature_algorithms[2..]
        .chunks(2)
        .map(|pair| format!("{:02x}{:02x}", pair[0], pair[1]))
        .collect::<Vec<_>>()
        .join(",");
    let b = sorted_hex(real_suites);
    let c = format!(
        "{}_{schemes}",
        sorted_hex(real_extensions().filter(|k| *k != 0 && *k != 16))
    );
    (a, b, c)
}

#[test]
fn the_hello_offers_chromes_groups_and_both_of_chromes_key_shares() {
    let extensions = hello_extensions();
    let groups = extension(&extensions, 10);
    assert!(rustls::client::is_grease(u16::from_be_bytes([
        groups[2], groups[3]
    ])));
    assert_eq!(
        &groups[4..],
        &[0x11, 0xec, 0x00, 0x1d, 0x00, 0x17, 0x00, 0x18]
    );
    let shares = extension(&extensions, 51);
    let grease_share = 2 + 2 + 2 + 1;
    assert_eq!(&shares[4..7], &[0, 1, 0]);
    assert_eq!(
        &shares[grease_share..grease_share + 4],
        &[0x11, 0xec, 0x04, 0xc0]
    );
    let x25519 = grease_share + 4 + 1216;
    assert_eq!(&shares[x25519..x25519 + 4], &[0x00, 0x1d, 0x00, 0x20]);
}

#[test]
fn the_hello_offers_chromes_suites_behind_a_grease_one_and_no_scsv() {
    let (suites, _) = hello();
    assert!(rustls::client::is_grease(suites[0]));
    assert_eq!(&suites[1..], &CHROME_SUITES);
}

#[test]
fn the_hello_carries_chromes_extensions_wrapped_in_grease() {
    let extensions = hello_extensions();
    let (first, first_body) = &extensions[0];
    let (last, last_body) = extensions.last().expect("last");
    assert!(rustls::client::is_grease(*first) && first_body.is_empty());
    assert!(rustls::client::is_grease(*last) && last_body == &[0]);
    assert_ne!(first, last);
    let mut kinds: Vec<u16> = extensions
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !rustls::client::is_grease(*k))
        .collect();
    kinds.sort_unstable();
    assert_eq!(kinds, CHROME_EXTENSIONS);
    assert_eq!(extension(&extensions, 65281), &[0]);
    assert!(extension(&extensions, 18).is_empty());
    let versions = extension(&extensions, 43);
    assert!(rustls::client::is_grease(u16::from_be_bytes([
        versions[1],
        versions[2]
    ])));
    assert_eq!(&versions[3..], &[3, 4, 3, 3]);
}

#[test]
fn the_grease_ech_outer_is_shaped_like_chromes() {
    let extensions = hello_extensions();
    let ech = extension(&extensions, 65037);
    assert_eq!(&ech[..5], &[0, 0, 1, 0, 1]);
    assert_eq!(&ech[6..8], &[0, 32]);
    assert_eq!(&ech[40..42], &[0, 208]);
    assert_eq!(ech.len(), 1 + 4 + 1 + 2 + 32 + 2 + 208);
}

#[test]
fn grease_values_change_between_connections() {
    let grease = |extensions: Vec<(u16, Vec<u8>)>| {
        extensions
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| rustls::client::is_grease(*k))
            .chain(
                extension(&extensions, 10)[2..4]
                    .iter()
                    .map(|b| u16::from(*b)),
            )
            .collect::<Vec<_>>()
    };
    assert_ne!(grease(hello_extensions()), grease(hello_extensions()));
}

/** The ALPS offer names h2 alone, as Chrome's does. */
#[test]
fn the_hello_offers_alps_for_h2() {
    assert_eq!(
        extension(&hello_extensions(), 17613),
        &[0, 3, 2, b'h', b'2']
    );
}

/** JA4 is Chrome's, hash for hash: FoxIO's published `t13d1516h2_8daaf6152771_d8a2da3f94cd`. */
#[test]
fn the_ja4_is_chromes() {
    let (suites, extensions) = hello();
    let (a, b, c) = ja4(&suites, &extensions);
    assert_eq!(a, "t13d1516h2");
    assert_eq!(
        b,
        "002f,0035,009c,009d,1301,1302,1303,c013,c014,c02b,c02c,c02f,c030,cca8,cca9"
    );
    assert_eq!(ja4_hash(&b), "8daaf6152771");
    assert_eq!(
        c,
        "0005,000a,000b,000d,0012,0017,001b,0023,002b,002d,0033,44cd,fe0d,ff01_\
         0403,0804,0401,0503,0805,0501,0806,0601"
    );
    assert_eq!(ja4_hash(&c), "d8a2da3f94cd");
}

/** Protocol version lists, `'static` as Rustls's server builder wants them. */
const TLS13_ONLY: &[&rustls::SupportedProtocolVersion] = &[&rustls::version::TLS13];
const TLS12_ONLY: &[&rustls::SupportedProtocolVersion] = &[&rustls::version::TLS12];

/** The shaped client completes a handshake with a server of the given versions and groups. */
fn handshakes(
    versions: &'static [&'static rustls::SupportedProtocolVersion],
    groups: Vec<&'static dyn rustls::crypto::SupportedKxGroup>,
) {
    let (address, server) = super::super::testing::handshake_server(versions, groups);
    let config = super::build_with(super::super::testing::test_roots()).expect("config");
    let name = rustls::pki_types::ServerName::try_from("localhost").expect("name");
    let mut client = rustls::ClientConnection::new(config, name).expect("client");
    let mut socket = std::net::TcpStream::connect(address).expect("connect");
    while client.is_handshaking() {
        client.complete_io(&mut socket).expect("handshake");
    }
    server.join().expect("server");
}

#[test]
fn the_shaped_hello_handshakes_tls13() {
    handshakes(TLS13_ONLY, vec![rustls::crypto::ring::kx_group::X25519]);
}

#[test]
fn the_shaped_hello_handshakes_a_tls12_only_server() {
    handshakes(TLS12_ONLY, vec![rustls::crypto::ring::kx_group::X25519]);
}

#[test]
fn the_shaped_hello_survives_a_hello_retry_request() {
    handshakes(TLS13_ONLY, vec![rustls::crypto::ring::kx_group::SECP384R1]);
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

/** RFC8446's fixed HelloRetryRequest random distinguishes it from ServerHello. */
const RETRY_RANDOM: [u8; 32] = [
    0xcf, 0x21, 0xad, 0x74, 0xe5, 0x9a, 0x61, 0x11, 0xbe, 0x1d, 0x8c, 0x02, 0x1e, 0x65, 0xb8, 0x91,
    0xc2, 0xa2, 0x11, 0x16, 0x7a, 0xbb, 0x8c, 0x5e, 0x07, 0x9e, 0x09, 0xe2, 0xc8, 0xa8, 0x33, 0x9c,
];

/** Frame a synthetic HRR; no socket or permissive test server can hide malformed client output. */
fn retry_record(session: &[u8], group: Option<u16>) -> Vec<u8> {
    let mut extensions = vec![0, 43, 0, 2, 3, 4]; // supported_versions: TLS1.3
    if let Some(group) = group {
        extensions.extend_from_slice(&[0, 51, 0, 2]);
        extensions.extend_from_slice(&group.to_be_bytes());
    } else {
        extensions.extend_from_slice(&[0, 44, 0, 3, 0, 1, 42]); // nonempty cookie
    }
    let mut body = vec![3, 3];
    body.extend_from_slice(&RETRY_RANDOM);
    body.push(session.len() as u8);
    body.extend_from_slice(session);
    body.extend_from_slice(&[0x13, 0x01, 0]); // AES128-GCM-SHA256, null compression
    body.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
    body.extend(extensions);
    let mut handshake = vec![2];
    handshake.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
    handshake.extend(body);
    let mut record = vec![22, 3, 3];
    record.extend_from_slice(&(handshake.len() as u16).to_be_bytes());
    record.extend(handshake);
    record
}

/** Skip compatibility CCS records and return the unencrypted ClientHello body. */
fn client_hello_body(mut wire: &[u8]) -> &[u8] {
    while !wire.is_empty() {
        let length = usize::from(u16::from_be_bytes([wire[3], wire[4]]));
        if wire[0] == 22 {
            assert_eq!(wire[5], 1);
            return &wire[9..5 + length];
        }
        wire = &wire[5 + length..];
    }
    panic!("missing ClientHello");
}

/** Collect first and second key-share extensions around an in-memory retry. */
fn retry_shares(group: Option<u16>, classical_first: bool) -> (Vec<u8>, Vec<u8>) {
    let mut config = (*super::build_with(super::super::testing::test_roots()).unwrap()).clone();
    if classical_first {
        let mut provider = super::provider();
        provider.kx_groups.swap(0, 1);
        let shaping = config.hello_shaping.clone();
        config = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(provider))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates((*super::super::testing::test_roots()).clone())
            .with_no_client_auth();
        config.hello_shaping = shaping;
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    }
    let name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
    let mut client = rustls::ClientConnection::new(std::sync::Arc::new(config), name).unwrap();
    let mut first = Vec::new();
    client.write_tls(&mut first).unwrap();
    let hello = client_hello_body(&first);
    let session = &hello[35..35 + usize::from(hello[34])];
    let retry = retry_record(session, group);
    client.read_tls(&mut std::io::Cursor::new(retry)).unwrap();
    client.process_new_packets().unwrap();
    let mut second = Vec::new();
    client.write_tls(&mut second).unwrap();
    (
        extension(&decode_hello(hello).1, 51).to_vec(),
        extension(&decode_hello(client_hello_body(&second)).1, 51).to_vec(),
    )
}

#[test]
fn requested_group_retry_contains_only_the_requested_share() {
    let (_, shares) = retry_shares(Some(0x0018), false);
    assert_eq!(&shares[..6], &[0, 101, 0, 24, 0, 97]);
    assert_eq!(shares.len(), 103);
}

#[test]
fn requested_hybrid_retry_omits_grease_and_the_classical_component_share() {
    let (_, shares) = retry_shares(Some(0x11ec), true);
    assert_eq!(&shares[..6], &[0x04, 0xc4, 0x11, 0xec, 0x04, 0xc0]);
    assert_eq!(shares.len(), 1222);
}

#[test]
fn cookie_only_retry_preserves_initial_grease_and_both_real_shares() {
    let (first, second) = retry_shares(None, false);
    assert!(rustls::client::is_grease(u16::from_be_bytes([
        first[2], first[3]
    ])));
    assert_eq!(second, first);
}
