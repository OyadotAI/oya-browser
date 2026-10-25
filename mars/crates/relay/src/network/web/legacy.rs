/**
 * TLS 1.2 with RSA key exchange (TLS_RSA_WITH_AES_128_GCM_SHA256 and _256_GCM_SHA384), for
 * servers that refuse every suite Rustls offers. Chromium's cipher string in
 * net/socket/ssl_client_socket_impl.cc (`ALL:!aPSK:!ECDSA+SHA1:!3DES`) keeps kRSA unless
 * `SSLConfig::require_ecdhe`, which defaults to false (checked against the pinned Chromium
 * source of 2026-09), so Chrome reaches such servers (www.fsf.org, linked from gnu.org,
 * negotiates only DHE or kRSA). Rustls never offers them, so the transport retries here only
 * after a handshake_failure alert. The retry is guarded against downgrade: it sends
 * TLS_FALLBACK_SCSV (RFC 7507) and refuses a ServerHello carrying the RFC 8446 downgrade
 * sentinel. Verification is not relaxed: the chain, name and validity go through the same
 * Rustls WebPKI verifier and roots, the leaf must allow keyEncipherment, both Finished messages
 * are checked, and Extended Master Secret (RFC 7627) is used whenever offered back. Parsing is
 * strict: no trailing bytes, duplicate extensions or handshake bytes left after
 * ServerHelloDone. Renegotiation is never performed; a server HelloRequest is ignored.
 */
mod rsa;

use ring::{aead, digest, hmac, rand::SecureRandom};
use rustls::client::WebPkiServerVerifier;
use rustls::client::danger::ServerCertVerifier;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

/// TLS 1.2's wire version.
const TLS12: [u8; 2] = [3, 3];
/// Record content types.
const CHANGE_CIPHER_SPEC: u8 = 20;
const ALERT: u8 = 21;
const HANDSHAKE: u8 = 22;
const APPLICATION_DATA: u8 = 23;
/// Handshake message types.
const CLIENT_HELLO: u8 = 1;
const SERVER_HELLO: u8 = 2;
const CERTIFICATE: u8 = 11;
const CERTIFICATE_REQUEST: u8 = 13;
const SERVER_HELLO_DONE: u8 = 14;
const CLIENT_KEY_EXCHANGE: u8 = 16;
const FINISHED: u8 = 20;
const HELLO_REQUEST: u8 = 0;
/// Extensions offered: server_name, signature_algorithms, ALPN, extended_master_secret,
/// renegotiation_info.
const SERVER_NAME: u16 = 0;
const SIGNATURE_ALGORITHMS: u16 = 13;
const ALPN: u16 = 16;
const EXTENDED_MASTER_SECRET: u16 = 23;
const RENEGOTIATION_INFO: u16 = 0xff01;
/// TLS_FALLBACK_SCSV (RFC 7507): tells a server that also speaks a newer version to refuse.
const FALLBACK_SCSV: u16 = 0x5600;
/// RFC 8446 4.1.3: the last 8 bytes of a ServerHello random from a TLS 1.3 server that was
/// downgraded to TLS 1.2 (final byte 01) or to TLS 1.1 and below (final byte 00).
const DOWNGRADE_SENTINEL: [u8; 7] = *b"DOWNGRD";
/// A session id is at most 32 bytes (RFC 5246 7.4.1.2).
const SESSION_ID_LIMIT: u8 = 32;
/// Chrome's signature_algorithms list, as hello.rs offers through Rustls.
const SCHEMES: [u16; 8] = [
    0x0403, 0x0804, 0x0401, 0x0503, 0x0805, 0x0501, 0x0806, 0x0601,
];
/// RFC 8446 section 5.1: a plaintext fragment is at most 2^14 bytes, a ciphertext 2^14 + 2048.
const PLAINTEXT_LIMIT: usize = 16_384;
const CIPHERTEXT_LIMIT: usize = PLAINTEXT_LIMIT + 2048;
/// A certificate chain larger than this is refused rather than buffered.
const HANDSHAKE_LIMIT: usize = 256 * 1024;
/// GCM's explicit per-record nonce, its implicit salt, and the tag (RFC 5288).
const EXPLICIT_NONCE: usize = 8;
const SALT: usize = 4;
const TAG: usize = 16;
/// Finished verify_data and master secret lengths (RFC 5246 sections 7.4.9 and 8.1).
const VERIFY_DATA: usize = 12;
const MASTER_SECRET: usize = 48;
/// A ClientHello's and a ServerHello's random.
const RANDOM: usize = 32;
/// Alert levels and descriptions read from the server.
const CLOSE_NOTIFY: u8 = 0;
const HANDSHAKE_FAILURE: u8 = 40;

/** One of the two suites offered. */
#[derive(Clone, Copy)]
struct Suite {
    /// Wire id.
    id: u16,
    aead: &'static aead::Algorithm,
    prf: hmac::Algorithm,
    hash: &'static digest::Algorithm,
}

/// Chromium's kRSA AEAD suites in BoringSSL's preference order.
const SUITES: [Suite; 2] = [
    Suite {
        id: 0x009c,
        aead: &aead::AES_128_GCM,
        prf: hmac::HMAC_SHA256,
        hash: &digest::SHA256,
    },
    Suite {
        id: 0x009d,
        aead: &aead::AES_256_GCM,
        prf: hmac::HMAC_SHA384,
        hash: &digest::SHA384,
    },
];

/** The server refused these suites too: Chrome's ERR_SSL_VERSION_OR_CIPHER_MISMATCH. */
#[derive(Debug)]
pub(super) struct Mismatch;

impl std::fmt::Display for Mismatch {
    /** A short reason for logs. */
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("the server shares no TLS version or cipher suite")
    }
}

impl std::error::Error for Mismatch {}

/** One direction's AEAD key, implicit salt and record sequence number. */
struct Direction {
    key: aead::LessSafeKey,
    salt: [u8; SALT],
    sequence: u64,
}

impl Direction {
    /** Keys one direction from its slice of the key block. */
    fn new(suite: Suite, key: &[u8], salt: &[u8]) -> io::Result<Self> {
        let key = aead::UnboundKey::new(suite.aead, key).map_err(|_| invalid("key"))?;
        Ok(Self {
            key: aead::LessSafeKey::new(key),
            salt: salt.try_into().map_err(|_| invalid("salt"))?,
            sequence: 0,
        })
    }

    /** The record's nonce and additional data (RFC 5246 6.2.3.3, RFC 5288 3). */
    fn nonce(
        &self,
        explicit: &[u8; EXPLICIT_NONCE],
        kind: u8,
        length: usize,
    ) -> (aead::Nonce, aead::Aad<[u8; 13]>) {
        let mut nonce = [0u8; 12];
        nonce[..SALT].copy_from_slice(&self.salt);
        nonce[SALT..].copy_from_slice(explicit);
        let mut aad = [0u8; 13];
        aad[..8].copy_from_slice(&self.sequence.to_be_bytes());
        aad[8] = kind;
        aad[9..11].copy_from_slice(&TLS12);
        aad[11..].copy_from_slice(&(length as u16).to_be_bytes());
        (
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(aad),
        )
    }

    /** A sealed record body: the explicit nonce (the sequence number), ciphertext and tag. */
    fn seal(&mut self, kind: u8, plaintext: &[u8]) -> io::Result<Vec<u8>> {
        let explicit = self.sequence.to_be_bytes();
        let (nonce, aad) = self.nonce(&explicit, kind, plaintext.len());
        let mut body = plaintext.to_vec();
        self.key
            .seal_in_place_append_tag(nonce, aad, &mut body)
            .map_err(|_| invalid("seal"))?;
        self.sequence += 1;
        Ok([explicit.as_slice(), &body].concat())
    }

    /** Opens one record body; any tampering fails the tag. */
    fn open(&mut self, kind: u8, record: &[u8]) -> io::Result<Vec<u8>> {
        if record.len() < EXPLICIT_NONCE + TAG {
            return Err(invalid("short record"));
        }
        let explicit: [u8; EXPLICIT_NONCE] = record[..EXPLICIT_NONCE]
            .try_into()
            .map_err(|_| invalid("nonce"))?;
        let (nonce, aad) = self.nonce(&explicit, kind, record.len() - EXPLICIT_NONCE - TAG);
        let mut body = record[EXPLICIT_NONCE..].to_vec();
        let plain = self
            .key
            .open_in_place(nonce, aad, &mut body)
            .map_err(|_| invalid("bad record tag"))?
            .len();
        body.truncate(plain);
        self.sequence += 1;
        Ok(body)
    }
}

/** An established legacy session: HTTP/1.1 bytes over sealed records on the OS socket. */
pub(super) struct Stream {
    /// The one validated socket.
    pub(super) sock: TcpStream,
    read: Direction,
    write: Direction,
    /// Decrypted application data not yet returned.
    incoming: Vec<u8>,
    /// How much of `incoming` was returned.
    at: usize,
    /// The server sent close_notify.
    closed: bool,
}

impl Read for Stream {
    /** Returns buffered plaintext, else opens records until application data or a close. */
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        while self.at == self.incoming.len() && !self.closed {
            self.next_record()?;
        }
        let count = bytes.len().min(self.incoming.len() - self.at);
        bytes[..count].copy_from_slice(&self.incoming[self.at..self.at + count]);
        self.at += count;
        Ok(count)
    }
}

impl Write for Stream {
    /** Seals at most one full-size record per call. */
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let chunk = &bytes[..bytes.len().min(PLAINTEXT_LIMIT)];
        let body = self.write.seal(APPLICATION_DATA, chunk)?;
        send(&mut self.sock, APPLICATION_DATA, &body)?;
        Ok(chunk.len())
    }

    /** The socket is unbuffered. */
    fn flush(&mut self) -> io::Result<()> {
        self.sock.flush()
    }
}

impl Stream {
    /** Opens one record after the handshake: data, a close, an ignored HelloRequest. */
    fn next_record(&mut self) -> io::Result<()> {
        let (kind, body) = receive(&mut self.sock)?;
        let plain = self.read.open(kind, &body)?;
        match kind {
            APPLICATION_DATA => {
                self.incoming = plain;
                self.at = 0;
                Ok(())
            }
            ALERT if plain.get(1) == Some(&CLOSE_NOTIFY) => {
                self.closed = true;
                Ok(())
            }
            HANDSHAKE if plain == [HELLO_REQUEST, 0, 0, 0] => Ok(()),
            _ => Err(invalid("unexpected record after the handshake")),
        }
    }

    /** Whether the server may still send data: no close_notify was read. */
    pub(super) fn open(&self) -> bool {
        !self.closed && self.at == self.incoming.len()
    }
}

/** The handshake's state as messages arrive. */
struct Handshake<'a> {
    sock: &'a mut TcpStream,
    /// Every handshake message so far, for the Finished and session hashes.
    transcript: Vec<u8>,
    /// Handshake bytes read but not yet split into messages.
    pending: Vec<u8>,
}

impl Handshake<'_> {
    /** Appends one outgoing message to the transcript and returns its framing. */
    fn message(&mut self, kind: u8, body: &[u8]) -> Vec<u8> {
        let mut message = vec![kind];
        message.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
        message.extend_from_slice(body);
        self.transcript.extend_from_slice(&message);
        message
    }

    /** The next whole server handshake message, read from plaintext handshake records. */
    fn next(&mut self) -> io::Result<(u8, Vec<u8>)> {
        loop {
            if self.pending.len() >= 4 {
                let length =
                    u32::from_be_bytes([0, self.pending[1], self.pending[2], self.pending[3]])
                        as usize;
                if length > HANDSHAKE_LIMIT {
                    return Err(invalid("handshake message size"));
                }
                if self.pending.len() >= 4 + length {
                    let message: Vec<u8> = self.pending.drain(..4 + length).collect();
                    self.transcript.extend_from_slice(&message);
                    return Ok((message[0], message[4..].to_vec()));
                }
            }
            let (kind, body) = receive(self.sock)?;
            match kind {
                HANDSHAKE => self.pending.extend_from_slice(&body),
                ALERT => return Err(alert(&body)),
                _ => return Err(invalid("unexpected record")),
            }
        }
    }

    /** The transcript hash under the suite's hash. */
    fn hash(&self, suite: Suite) -> digest::Digest {
        digest::digest(suite.hash, &self.transcript)
    }
}

/** What the server chose in its ServerHello. */
struct Hello {
    suite: Suite,
    random: [u8; RANDOM],
    extended_master_secret: bool,
}

/**
 * Runs the handshake on a fresh socket to `name` and returns the session. Certificates are
 * checked by `verifier`, the same WebPKI verifier Rustls uses.
 */
pub(super) fn connect(
    mut sock: TcpStream,
    name: &str,
    verifier: &Arc<WebPkiServerVerifier>,
) -> io::Result<Stream> {
    let random = ring::rand::SystemRandom::new();
    let mut client_random = [0u8; RANDOM];
    random
        .fill(&mut client_random)
        .map_err(|_| invalid("random"))?;
    let mut handshake = Handshake {
        sock: &mut sock,
        transcript: Vec::new(),
        pending: Vec::new(),
    };
    let body = client_hello(&client_random, name);
    let hello = handshake.message(CLIENT_HELLO, &body);
    send(handshake.sock, HANDSHAKE, &hello)?;
    let server = server_hello(&mut handshake)?;
    let (key, requested) = certificates(&mut handshake, name, verifier)?;
    let master = key_exchange(
        &mut handshake,
        &server,
        &client_random,
        &key,
        requested,
        &random,
    )?;
    let (mut write, mut read) = keys(&server, &master, &client_random)?;
    let verify = finished(
        &master,
        server.suite,
        b"client finished",
        &handshake.hash(server.suite),
    );
    let finished_message = handshake.message(FINISHED, &verify);
    send(handshake.sock, CHANGE_CIPHER_SPEC, &[1])?;
    let sealed = write.seal(HANDSHAKE, &finished_message)?;
    send(handshake.sock, HANDSHAKE, &sealed)?;
    server_finished(&mut handshake, &server, &master, &mut read)?;
    Ok(Stream {
        sock,
        read,
        write,
        incoming: Vec::new(),
        at: 0,
        closed: false,
    })
}

/**
 * The ClientHello body: TLS 1.2, both suites plus TLS_FALLBACK_SCSV, null compression and five
 * extensions.
 */
fn client_hello(random: &[u8; RANDOM], name: &str) -> Vec<u8> {
    let mut body = TLS12.to_vec();
    body.extend_from_slice(random);
    body.push(0);
    vector16(
        &mut body,
        &SUITES
            .iter()
            .map(|suite| suite.id)
            .chain([FALLBACK_SCSV])
            .flat_map(u16::to_be_bytes)
            .collect::<Vec<_>>(),
    );
    body.extend_from_slice(&[1, 0]);
    let mut extensions = Vec::new();
    if name.parse::<std::net::IpAddr>().is_err() {
        let mut entry = vec![0];
        vector16(&mut entry, name.as_bytes());
        let mut list = Vec::new();
        vector16(&mut list, &entry);
        extension(&mut extensions, SERVER_NAME, &list);
    }
    let mut schemes = Vec::new();
    vector16(
        &mut schemes,
        &SCHEMES
            .iter()
            .flat_map(|scheme| scheme.to_be_bytes())
            .collect::<Vec<_>>(),
    );
    extension(&mut extensions, SIGNATURE_ALGORITHMS, &schemes);
    extension(&mut extensions, ALPN, &alpn());
    extension(&mut extensions, EXTENDED_MASTER_SECRET, &[]);
    extension(&mut extensions, RENEGOTIATION_INFO, &[0]);
    vector16(&mut body, &extensions);
    body
}

/** ALPN offering only http/1.1: the HTTP/2 layer runs over Rustls sessions only. */
fn alpn() -> Vec<u8> {
    let mut list = vec![8];
    list.extend_from_slice(b"http/1.1");
    let mut value = Vec::new();
    vector16(&mut value, &list);
    value
}

/**
 * Reads the ServerHello: TLS 1.2, an offered suite, null compression, and only extensions
 * that were offered, each once, with renegotiation_info empty (RFC 5746) and ALPN http/1.1.
 * A downgrade sentinel in the random is refused, as are trailing bytes.
 */
fn server_hello(handshake: &mut Handshake<'_>) -> io::Result<Hello> {
    let (kind, body) = handshake.next()?;
    if kind != SERVER_HELLO {
        return Err(invalid("expected ServerHello"));
    }
    let mut reader = Reader(&body);
    if reader.take(2)? != TLS12 {
        return Err(Mismatch.into_io());
    }
    let random: [u8; RANDOM] = reader
        .take(RANDOM)?
        .try_into()
        .map_err(|_| invalid("random"))?;
    if random[RANDOM - 8..RANDOM - 1] == DOWNGRADE_SENTINEL && random[RANDOM - 1] <= 1 {
        return Err(invalid("downgrade sentinel in ServerHello random"));
    }
    let session = reader.take(1)?[0];
    if session > SESSION_ID_LIMIT {
        return Err(invalid("session id too long"));
    }
    reader.take(usize::from(session))?;
    let id = u16::from_be_bytes(reader.take(2)?.try_into().map_err(|_| invalid("suite"))?);
    let suite = *SUITES
        .iter()
        .find(|suite| suite.id == id)
        .ok_or_else(|| invalid("suite not offered"))?;
    if reader.take(1)? != [0] {
        return Err(invalid("compression"));
    }
    let mut extended_master_secret = false;
    if !reader.0.is_empty() {
        let mut extensions = Reader(reader.vector16()?);
        let mut seen = Vec::new();
        while !extensions.0.is_empty() {
            let id = u16::from_be_bytes(
                extensions
                    .take(2)?
                    .try_into()
                    .map_err(|_| invalid("extension"))?,
            );
            let value = extensions.vector16()?;
            if seen.contains(&id) {
                return Err(invalid("duplicate ServerHello extension"));
            }
            seen.push(id);
            match id {
                EXTENDED_MASTER_SECRET if value.is_empty() => extended_master_secret = true,
                RENEGOTIATION_INFO if value == [0] => {}
                ALPN if value == alpn() => {}
                SERVER_NAME if value.is_empty() => {}
                _ => return Err(invalid("unexpected ServerHello extension")),
            }
        }
    }
    finish(&reader, "ServerHello")?;
    Ok(Hello {
        suite,
        random,
        extended_master_secret,
    })
}

/**
 * Reads Certificate, an optional CertificateRequest, and ServerHelloDone; verifies the chain
 * for `name` now and returns the leaf's RSA key and whether a client certificate was asked for.
 * A ServerKeyExchange is refused: these suites never send one.
 */
fn certificates(
    handshake: &mut Handshake<'_>,
    name: &str,
    verifier: &Arc<WebPkiServerVerifier>,
) -> io::Result<(rsa::PublicKey, bool)> {
    let (kind, body) = handshake.next()?;
    if kind != CERTIFICATE {
        return Err(invalid("expected Certificate"));
    }
    let mut message = Reader(&body);
    let mut list = Reader(message.vector24()?);
    finish(&message, "Certificate")?;
    let mut chain = Vec::new();
    while !list.0.is_empty() {
        chain.push(CertificateDer::from(list.vector24()?.to_vec()));
    }
    let (leaf, intermediates) = chain.split_first().ok_or_else(|| invalid("empty chain"))?;
    let server = ServerName::try_from(name.to_owned()).map_err(|_| invalid("server name"))?;
    verifier
        .verify_server_cert(leaf, intermediates, &server, &[], UnixTime::now())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let key = rsa::leaf_key(leaf)?;
    let (mut kind, _) = handshake.next()?;
    let requested = kind == CERTIFICATE_REQUEST;
    if requested {
        kind = handshake.next()?.0;
    }
    if kind != SERVER_HELLO_DONE {
        return Err(invalid("expected ServerHelloDone"));
    }
    if !handshake.pending.is_empty() {
        return Err(invalid("handshake bytes after ServerHelloDone"));
    }
    Ok((key, requested))
}

/**
 * Sends an empty Certificate when one was requested (no client certificate is ever offered)
 * and the ClientKeyExchange; returns the master secret, extended per RFC 7627 when agreed.
 */
fn key_exchange(
    handshake: &mut Handshake<'_>,
    server: &Hello,
    client_random: &[u8; RANDOM],
    key: &rsa::PublicKey,
    requested: bool,
    random: &dyn SecureRandom,
) -> io::Result<Vec<u8>> {
    if requested {
        let empty = handshake.message(CERTIFICATE, &[0, 0, 0]);
        send(handshake.sock, HANDSHAKE, &empty)?;
    }
    let mut premaster = [0u8; MASTER_SECRET];
    premaster[..2].copy_from_slice(&TLS12);
    random
        .fill(&mut premaster[2..])
        .map_err(|_| invalid("random"))?;
    let mut exchange = Vec::new();
    vector16(&mut exchange, &key.encrypt(&premaster, random)?);
    let message = handshake.message(CLIENT_KEY_EXCHANGE, &exchange);
    send(handshake.sock, HANDSHAKE, &message)?;
    let suite = server.suite;
    Ok(if server.extended_master_secret {
        prf(
            suite,
            &premaster,
            b"extended master secret",
            handshake.hash(suite).as_ref(),
            MASTER_SECRET,
        )
    } else {
        let seed = [client_random.as_slice(), &server.random].concat();
        prf(suite, &premaster, b"master secret", &seed, MASTER_SECRET)
    })
}

/** The client-write and server-write directions from the key block (RFC 5246 6.3). */
fn keys(
    server: &Hello,
    master: &[u8],
    client_random: &[u8; RANDOM],
) -> io::Result<(Direction, Direction)> {
    let suite = server.suite;
    let size = suite.aead.key_len();
    let seed = [server.random.as_slice(), client_random].concat();
    let block = prf(suite, master, b"key expansion", &seed, 2 * size + 2 * SALT);
    let (client_key, rest) = block.split_at(size);
    let (server_key, salts) = rest.split_at(size);
    Ok((
        Direction::new(suite, client_key, &salts[..SALT])?,
        Direction::new(suite, server_key, &salts[SALT..])?,
    ))
}

/** Reads the server's ChangeCipherSpec and checks its sealed Finished against the transcript. */
fn server_finished(
    handshake: &mut Handshake<'_>,
    server: &Hello,
    master: &[u8],
    read: &mut Direction,
) -> io::Result<()> {
    let (kind, body) = receive(handshake.sock)?;
    match kind {
        CHANGE_CIPHER_SPEC if body == [1] => {}
        ALERT => return Err(alert(&body)),
        _ => return Err(invalid("expected ChangeCipherSpec")),
    }
    let expected = finished(
        master,
        server.suite,
        b"server finished",
        &handshake.hash(server.suite),
    );
    let (kind, body) = receive(handshake.sock)?;
    if kind != HANDSHAKE {
        return Err(invalid("expected Finished"));
    }
    let message = read.open(HANDSHAKE, &body)?;
    let mut wanted = vec![FINISHED, 0, 0, VERIFY_DATA as u8];
    wanted.extend_from_slice(&expected);
    let differs = message.len() != wanted.len()
        || message
            .iter()
            .zip(&wanted)
            .fold(0u8, |sum, (a, b)| sum | (a ^ b))
            != 0;
    if differs {
        return Err(invalid("server Finished does not match"));
    }
    Ok(())
}

/** verify_data: PRF(master, label, transcript hash), first 12 bytes. */
fn finished(master: &[u8], suite: Suite, label: &[u8], hash: &digest::Digest) -> Vec<u8> {
    prf(suite, master, label, hash.as_ref(), VERIFY_DATA)
}

/** TLS 1.2's PRF: P_hash(secret, label + seed) truncated to `length` (RFC 5246 section 5). */
fn prf(suite: Suite, secret: &[u8], label: &[u8], seed: &[u8], length: usize) -> Vec<u8> {
    let key = hmac::Key::new(suite.prf, secret);
    let seed = [label, seed].concat();
    let mut a = hmac::sign(&key, &seed);
    let mut out = Vec::with_capacity(length);
    while out.len() < length {
        let mut context = hmac::Context::with_key(&key);
        context.update(a.as_ref());
        context.update(&seed);
        out.extend_from_slice(context.sign().as_ref());
        a = hmac::sign(&key, a.as_ref());
    }
    out.truncate(length);
    out
}

/** Writes one record. */
fn send(sock: &mut TcpStream, kind: u8, body: &[u8]) -> io::Result<()> {
    let mut record = vec![kind, TLS12[0], TLS12[1]];
    record.extend_from_slice(&(body.len() as u16).to_be_bytes());
    record.extend_from_slice(body);
    sock.write_all(&record)
}

/** Reads one record within the ciphertext bound; a short read is EOF to the caller. */
fn receive(sock: &mut TcpStream) -> io::Result<(u8, Vec<u8>)> {
    let mut header = [0u8; 5];
    sock.read_exact(&mut header)?;
    let length = usize::from(u16::from_be_bytes([header[3], header[4]]));
    if header[1] != 3 || length > CIPHERTEXT_LIMIT {
        return Err(invalid("record header"));
    }
    let mut body = vec![0u8; length];
    sock.read_exact(&mut body)?;
    Ok((header[0], body))
}

/** A plaintext alert during the handshake; handshake_failure means no shared suite. */
fn alert(body: &[u8]) -> io::Error {
    if body.get(1) == Some(&HANDSHAKE_FAILURE) {
        Mismatch.into_io()
    } else {
        invalid("alert")
    }
}

impl Mismatch {
    /** As the transport's I/O error, recognizable by downcast. */
    fn into_io(self) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, self)
    }
}

/** Appends a 16-bit length-prefixed vector. */
fn vector16(out: &mut Vec<u8>, body: &[u8]) {
    out.extend_from_slice(&(body.len() as u16).to_be_bytes());
    out.extend_from_slice(body);
}

/** Appends one extension. */
fn extension(out: &mut Vec<u8>, id: u16, value: &[u8]) {
    out.extend_from_slice(&id.to_be_bytes());
    vector16(out, value);
}

/** A bounds-checked cursor over a received message. */
struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    /** The next `count` bytes. */
    fn take(&mut self, count: usize) -> io::Result<&'a [u8]> {
        if self.0.len() < count {
            return Err(invalid("truncated message"));
        }
        let (head, rest) = self.0.split_at(count);
        self.0 = rest;
        Ok(head)
    }

    /** A 16-bit length-prefixed vector. */
    fn vector16(&mut self) -> io::Result<&'a [u8]> {
        let length = self.take(2)?;
        self.take(usize::from(u16::from_be_bytes([length[0], length[1]])))
    }

    /** A 24-bit length-prefixed vector. */
    fn vector24(&mut self) -> io::Result<&'a [u8]> {
        let length = self.take(3)?;
        self.take(u32::from_be_bytes([0, length[0], length[1], length[2]]) as usize)
    }
}

/** Refuses bytes left over after a message's last field. */
fn finish(reader: &Reader<'_>, what: &str) -> io::Result<()> {
    if reader.0.is_empty() {
        Ok(())
    } else {
        Err(invalid(&format!("trailing bytes after {what}")))
    }
}

/** A protocol violation, surfaced as a TLS failure. */
fn invalid(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("legacy TLS: {what}"))
}

#[cfg(test)]
mod tests;
