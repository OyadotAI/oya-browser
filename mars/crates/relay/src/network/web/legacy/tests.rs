/** The legacy RSA key exchange against a scripted loopback TLS 1.2 server. */
use super::*;
use rustls::RootCertStore;
use std::net::TcpListener;
use std::thread::JoinHandle;

/// The RSA test CA and its leaves (valid until 2126 for localhost and 127.0.0.1).
const CA: &[u8] = include_bytes!("../fixtures/rsa-ca.der");
const LEAF: &[u8] = include_bytes!("../fixtures/rsa-enc.der");
const KEY: &[u8] = include_bytes!("../fixtures/rsa-enc.key.der");
/// The ECDSA test CA, which did not issue the RSA leaf.
const OTHER_CA: &[u8] = include_bytes!("../fixtures/ca.der");

/** A ServerHello or flight defect the strict parser must refuse. */
#[derive(Clone, Copy, Default, PartialEq)]
enum Fault {
    #[default]
    None,
    /// The RFC 8446 downgrade sentinel ending in this byte.
    Sentinel(u8),
    /// A 33-byte session id.
    LongSession,
    /// A byte after the ServerHello extensions vector.
    TrailingHello,
    /// renegotiation_info twice.
    DuplicateExtension,
    /// Bytes after the certificate list inside the Certificate message.
    TrailingCertificates,
    /// A HelloRequest in the same record after ServerHelloDone, kept out of the transcript.
    LeftoverHandshake,
}

/** How the scripted server misbehaves, if at all. */
#[derive(Clone, Copy, Default)]
struct Script {
    /// A parsing defect to inject.
    fault: Fault,
    /// Echo extended_master_secret.
    ems: bool,
    /// Corrupt the server Finished verify_data.
    tamper: bool,
    /// Refuse the hello with handshake_failure, as www.fsf.org refuses Rustls.
    refuse: bool,
}

/** A verifier trusting `root`, as hello.rs builds one over webpki-roots. */
fn verifier(root: &[u8]) -> Arc<WebPkiServerVerifier> {
    let mut roots = RootCertStore::empty();
    roots
        .add(CertificateDer::from(root.to_vec()))
        .expect("root");
    WebPkiServerVerifier::builder_with_provider(
        Arc::new(roots),
        Arc::new(rustls::crypto::ring::default_provider()),
    )
    .build()
    .expect("verifier")
}

/** Starts the one-connection server and connects a socket to it. */
fn serve(script: Script) -> (TcpStream, JoinHandle<io::Result<Vec<u8>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = std::thread::spawn(move || {
        let (sock, _) = listener.accept()?;
        server(sock, script)
    });
    (TcpStream::connect(address).expect("connect"), server)
}

/** The server side; returns the application bytes it read before answering. */
fn server(mut sock: TcpStream, script: Script) -> io::Result<Vec<u8>> {
    let mut handshake = Handshake {
        sock: &mut sock,
        transcript: Vec::new(),
        pending: Vec::new(),
    };
    let (_, hello) = handshake.next()?;
    if script.refuse {
        send(handshake.sock, ALERT, &[2, HANDSHAKE_FAILURE])?;
        return Ok(Vec::new());
    }
    let client_random: [u8; RANDOM] = hello[2..2 + RANDOM].try_into().expect("random");
    let mut server_random = [7u8; RANDOM];
    if let Fault::Sentinel(last) = script.fault {
        server_random[RANDOM - 8..RANDOM - 1].copy_from_slice(b"DOWNGRD");
        server_random[RANDOM - 1] = last;
    }
    let mut body = TLS12.to_vec();
    body.extend_from_slice(&server_random);
    if script.fault == Fault::LongSession {
        body.push(33);
        body.extend_from_slice(&[9; 33]);
    } else {
        body.push(0);
    }
    body.extend_from_slice(&[0, 0x9c, 0]);
    let mut extensions = Vec::new();
    extension(&mut extensions, RENEGOTIATION_INFO, &[0]);
    if script.fault == Fault::DuplicateExtension {
        extension(&mut extensions, RENEGOTIATION_INFO, &[0]);
    }
    if script.ems {
        extension(&mut extensions, EXTENDED_MASTER_SECRET, &[]);
    }
    vector16(&mut body, &extensions);
    if script.fault == Fault::TrailingHello {
        body.push(0);
    }
    let mut flight = handshake.message(SERVER_HELLO, &body);
    let mut chain = Vec::new();
    chain.extend_from_slice(&(LEAF.len() as u32).to_be_bytes()[1..]);
    chain.extend_from_slice(LEAF);
    let mut certificates = (chain.len() as u32).to_be_bytes()[1..].to_vec();
    certificates.extend_from_slice(&chain);
    if script.fault == Fault::TrailingCertificates {
        certificates.extend_from_slice(&[0, 0]);
    }
    flight.extend(handshake.message(CERTIFICATE, &certificates));
    flight.extend(handshake.message(SERVER_HELLO_DONE, &[]));
    if script.fault == Fault::LeftoverHandshake {
        flight.extend_from_slice(&[HELLO_REQUEST, 0, 0, 0]);
    }
    send(handshake.sock, HANDSHAKE, &flight)?;
    let (_, exchange) = handshake.next()?;
    let (n, d) = rsa::tests::private(KEY);
    let block = rsa::modpow(&exchange[2..], &d, &n);
    let premaster = &block[block.len() - MASTER_SECRET..];
    let suite = SUITES[0];
    let master = if script.ems {
        prf(
            suite,
            premaster,
            b"extended master secret",
            handshake.hash(suite).as_ref(),
            MASTER_SECRET,
        )
    } else {
        prf(
            suite,
            premaster,
            b"master secret",
            &[client_random.as_slice(), &server_random].concat(),
            MASTER_SECRET,
        )
    };
    let hello = Hello {
        suite,
        random: server_random,
        extended_master_secret: script.ems,
    };
    let (mut read, mut write) = keys(&hello, &master, &client_random)?;
    let (kind, _) = receive(handshake.sock)?;
    assert_eq!(kind, CHANGE_CIPHER_SPEC);
    let expected = finished(&master, suite, b"client finished", &handshake.hash(suite));
    let (_, sealed) = receive(handshake.sock)?;
    let message = read.open(HANDSHAKE, &sealed)?;
    assert_eq!(&message[4..], expected.as_slice(), "client Finished");
    handshake.transcript.extend_from_slice(&message);
    let mut verify = finished(&master, suite, b"server finished", &handshake.hash(suite));
    if script.tamper {
        verify[0] ^= 1;
    }
    let reply = [vec![FINISHED, 0, 0, VERIFY_DATA as u8], verify].concat();
    send(handshake.sock, CHANGE_CIPHER_SPEC, &[1])?;
    let sealed = write.seal(HANDSHAKE, &reply)?;
    send(handshake.sock, HANDSHAKE, &sealed)?;
    if script.tamper {
        return Ok(Vec::new());
    }
    let (_, request) = receive(handshake.sock)?;
    let request = read.open(APPLICATION_DATA, &request)?;
    let sealed = write.seal(APPLICATION_DATA, b"HTTP/1.1 200 OK\r\n\r\n")?;
    send(handshake.sock, APPLICATION_DATA, &sealed)?;
    let sealed = write.seal(ALERT, &[1, CLOSE_NOTIFY])?;
    send(handshake.sock, ALERT, &sealed)?;
    Ok(request)
}

/** Handshakes, sends a request and reads the reply to close_notify. */
fn exchange(script: Script) -> (Vec<u8>, Vec<u8>) {
    let (sock, server) = serve(script);
    let mut stream = connect(sock, "localhost", &verifier(CA)).expect("handshake");
    stream.write_all(b"GET / HTTP/1.1\r\n\r\n").expect("write");
    let mut reply = Vec::new();
    stream.read_to_end(&mut reply).expect("read");
    (server.join().expect("server").expect("server io"), reply)
}

#[test]
fn rsa_key_exchange_carries_http_both_ways_with_extended_master_secret() {
    let (request, reply) = exchange(Script {
        ems: true,
        ..Script::default()
    });
    assert_eq!(request, b"GET / HTTP/1.1\r\n\r\n");
    assert_eq!(reply, b"HTTP/1.1 200 OK\r\n\r\n");
}

#[test]
fn rsa_key_exchange_falls_back_to_the_rfc_5246_master_secret() {
    let (_, reply) = exchange(Script::default());
    assert_eq!(reply, b"HTTP/1.1 200 OK\r\n\r\n");
}

#[test]
fn a_certificate_from_an_untrusted_root_is_refused() {
    let (sock, _server) = serve(Script::default());
    assert!(connect(sock, "localhost", &verifier(OTHER_CA)).is_err());
}

#[test]
fn a_certificate_for_another_name_is_refused() {
    let (sock, _server) = serve(Script::default());
    assert!(connect(sock, "example.com", &verifier(CA)).is_err());
}

#[test]
fn a_wrong_server_finished_is_refused() {
    let (sock, _server) = serve(Script {
        tamper: true,
        ..Script::default()
    });
    assert!(connect(sock, "localhost", &verifier(CA)).is_err());
}

#[test]
fn a_handshake_failure_alert_reports_a_cipher_mismatch() {
    let (sock, _server) = serve(Script {
        refuse: true,
        ..Script::default()
    });
    let error = connect(sock, "localhost", &verifier(CA))
        .err()
        .expect("refused");
    assert!(error.get_ref().is_some_and(|inner| inner.is::<Mismatch>()));
}

/** The widely published TLS 1.2 PRF (P_SHA256) vector. */
#[test]
fn prf_matches_the_tls12_sha256_vector() {
    let secret = [
        0x9b, 0xbe, 0x43, 0x6b, 0xa9, 0x40, 0xf0, 0x17, 0xb1, 0x76, 0x52, 0x84, 0x9a, 0x71, 0xdb,
        0x35,
    ];
    let seed = [
        0xa0, 0xba, 0x9f, 0x93, 0x6c, 0xda, 0x31, 0x18, 0x27, 0xa6, 0xf7, 0x96, 0xff, 0xd5, 0x19,
        0x8c,
    ];
    let out = prf(SUITES[0], &secret, b"test label", &seed, 100);
    assert_eq!(&out[..8], &[0xe3, 0xf2, 0x29, 0xba, 0x72, 0x7b, 0xe1, 0x7b]);
    assert_eq!(
        &out[92..],
        &[0x10, 0xff, 0xf7, 0x01, 0x87, 0x34, 0x7b, 0x66]
    );
}

/** The client's error text against a server injecting `fault`. */
fn refusal(fault: Fault) -> String {
    let (sock, _server) = serve(Script {
        fault,
        ..Script::default()
    });
    connect(sock, "localhost", &verifier(CA))
        .err()
        .expect("refused")
        .to_string()
}

#[test]
fn a_tls13_downgrade_sentinel_for_tls12_is_refused() {
    assert!(refusal(Fault::Sentinel(1)).contains("downgrade sentinel"));
}

#[test]
fn a_tls13_downgrade_sentinel_for_tls11_is_refused() {
    assert!(refusal(Fault::Sentinel(0)).contains("downgrade sentinel"));
}

#[test]
fn a_random_merely_resembling_the_sentinel_is_accepted() {
    let (_, reply) = exchange(Script {
        fault: Fault::Sentinel(2),
        ..Script::default()
    });
    assert_eq!(reply, b"HTTP/1.1 200 OK\r\n\r\n");
}

#[test]
fn the_fallback_client_hello_offers_tls_fallback_scsv() {
    let body = client_hello(&[0; RANDOM], "localhost");
    let mut reader = Reader(&body[2 + RANDOM + 1..]);
    let suites = reader.vector16().expect("suites");
    let ids: Vec<u16> = suites
        .chunks(2)
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
        .collect();
    assert_eq!(ids, [0x009c, 0x009d, 0x5600]);
}

#[test]
fn only_a_handshake_failure_alert_retries_over_rsa() {
    use rustls::AlertDescription::{HandshakeFailure, InsufficientSecurity, ProtocolVersion};
    let alert = |description| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            rustls::Error::AlertReceived(description),
        )
    };
    let retry = super::super::transport::retry_with_rsa;
    assert!(retry(&alert(HandshakeFailure)));
    assert!(!retry(&alert(ProtocolVersion)));
    assert!(!retry(&alert(InsufficientSecurity)));
}

#[test]
fn handshake_bytes_left_after_server_hello_done_are_refused() {
    assert!(refusal(Fault::LeftoverHandshake).contains("after ServerHelloDone"));
}

#[test]
fn trailing_bytes_after_the_server_hello_extensions_are_refused() {
    assert!(refusal(Fault::TrailingHello).contains("trailing bytes after ServerHello"));
}

#[test]
fn trailing_bytes_after_the_certificate_list_are_refused() {
    assert!(refusal(Fault::TrailingCertificates).contains("trailing bytes after Certificate"));
}

#[test]
fn a_duplicate_server_hello_extension_is_refused() {
    assert!(refusal(Fault::DuplicateExtension).contains("duplicate ServerHello extension"));
}

#[test]
fn a_session_id_longer_than_32_bytes_is_refused() {
    assert!(refusal(Fault::LongSession).contains("session id too long"));
}

/** One DER element with a definite length, short or long form. */
fn der(tag: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let length = body.len().to_be_bytes();
    let start = length.iter().position(|&byte| byte != 0).unwrap_or(7);
    if body.len() >= 0x80 {
        out.push(0x80 | (8 - start) as u8);
    }
    out.extend_from_slice(&length[start..]);
    out.extend_from_slice(body);
    out
}

/** An rsaEncryption SubjectPublicKeyInfo body with a valid 2048-bit modulus and exponent `e`. */
fn spki_with_exponent(e: &[u8]) -> Vec<u8> {
    let mut n = vec![0u8];
    n.extend_from_slice(&[0xc5; 256]);
    let key = [der(0x02, &n), der(0x02, e)].concat();
    let algorithm = [
        der(
            0x06,
            &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01],
        ),
        der(0x05, &[]),
    ]
    .concat();
    let bits = [&[0u8][..], &der(0x30, &key)].concat();
    [der(0x30, &algorithm), der(0x03, &bits)].concat()
}

#[test]
fn an_odd_public_exponent_of_at_least_3_in_4_bytes_is_accepted() {
    assert!(rsa::spki(&spki_with_exponent(&[1, 0, 1])).is_some());
    assert!(rsa::spki(&spki_with_exponent(&[3])).is_some());
    assert!(rsa::spki(&spki_with_exponent(&[0x7f, 0xff, 0xff, 0xff])).is_some());
}

#[test]
fn a_public_exponent_of_1_is_refused() {
    assert!(rsa::spki(&spki_with_exponent(&[1])).is_none());
}

#[test]
fn an_even_public_exponent_is_refused() {
    assert!(rsa::spki(&spki_with_exponent(&[1, 0, 0])).is_none());
}

#[test]
fn a_public_exponent_longer_than_4_bytes_is_refused() {
    assert!(rsa::spki(&spki_with_exponent(&[1, 0, 0, 0, 1])).is_none());
}

/**
 * One AES-128-GCM record sealed by the module equals one sealed independently. The key block
 * came from `openssl kdf TLS1-PRF` (SHA-256, label "key expansion", seed server || client
 * random) and the record from Python's cryptography AESGCM, with the nonce built by hand as the
 * 4-byte client salt (key block bytes 32..36) plus the 8-byte explicit nonce, and the AAD as
 * seq || 23 || 03 03 || length.
 */
#[test]
fn an_aes_128_gcm_record_matches_an_independent_seal() {
    let master: Vec<u8> = (0..48).collect();
    let client_random: [u8; RANDOM] = std::array::from_fn(|i| 0x40 + i as u8);
    let server_random: [u8; RANDOM] = std::array::from_fn(|i| 0x80 + i as u8);
    let hello = Hello {
        suite: SUITES[0],
        random: server_random,
        extended_master_secret: false,
    };
    let (mut write, _) = keys(&hello, &master, &client_random).expect("keys");
    write.sequence = 0x0102_0304_0506_0708;
    let sealed = write
        .seal(APPLICATION_DATA, b"GET / HTTP/1.1\r\n\r\n")
        .expect("seal");
    let expected = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0xa1, 0x82, 0x35, 0x50, 0x2d, 0xef, 0x9c,
        0x23, 0x36, 0x22, 0x1f, 0x26, 0x0f, 0x2f, 0x63, 0x43, 0xbf, 0xeb, 0xdd, 0x28, 0xa8, 0xd6,
        0x6c, 0x6a, 0x54, 0x18, 0x77, 0xa4, 0xf5, 0xff, 0x5b, 0xcf, 0xab, 0x50,
    ];
    assert_eq!(sealed, expected);
}
