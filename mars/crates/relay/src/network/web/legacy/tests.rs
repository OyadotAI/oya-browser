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

/** How the scripted server misbehaves, if at all. */
#[derive(Clone, Copy, Default)]
struct Script {
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
    let server_random = [7u8; RANDOM];
    let mut body = TLS12.to_vec();
    body.extend_from_slice(&server_random);
    body.extend_from_slice(&[0, 0, 0x9c, 0]);
    let mut extensions = Vec::new();
    extension(&mut extensions, RENEGOTIATION_INFO, &[0]);
    if script.ems {
        extension(&mut extensions, EXTENDED_MASTER_SECRET, &[]);
    }
    vector16(&mut body, &extensions);
    let mut flight = handshake.message(SERVER_HELLO, &body);
    let mut chain = Vec::new();
    chain.extend_from_slice(&(LEAF.len() as u32).to_be_bytes()[1..]);
    chain.extend_from_slice(LEAF);
    let mut certificates = (chain.len() as u32).to_be_bytes()[1..].to_vec();
    certificates.extend_from_slice(&chain);
    flight.extend(handshake.message(CERTIFICATE, &certificates));
    flight.extend(handshake.message(SERVER_HELLO_DONE, &[]));
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
