/** Validated exact-address connections preserve hostname verification without DNS rebinding. */
mod errors;
mod retry;
use crate::constants::DNS_ANSWERS;
use crate::network::WebRequest;
use crate::network::constants::{POOL_PROBE_RECORDS, READ_BYTES};
use crate::network::policy::{denied, public_address};
use crate::protocol::{EngineError, EngineResult};
use ::url::{Host, Url};
use errors::{Stage, io_error};
use rustls::{ClientConnection, StreamOwned};
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/** TLS uses the same owned HTTP codec and socket deadline as plaintext. */
pub(super) enum Connection {
    /** Exact policy-validated OS socket. */
    Plain(TcpStream),
    /** Rustls performs normal chain, hostname and signature validation. */
    Tls(Box<StreamOwned<ClientConnection, TcpStream>>),
    /** TLS 1.2 RSA key exchange for servers that refuse Rustls's suites; same verifier. */
    Legacy(Box<super::legacy::Stream>),
}

/**
 * Resolve once, reject every nonpublic answer, then connect to one validated address, racing
 * the address families as Chromium's Happy Eyeballs does (eyeballs.rs).
 */
pub(super) fn connect(url: &Url, fixture: bool, deadline: Instant) -> EngineResult<Connection> {
    let addresses = resolve(url, fixture, deadline)?;
    remaining(deadline)?;
    let (socket, peer) = race(&addresses, deadline)?;
    let first = modern(socket, url, deadline);
    retry::recover(
        first,
        &addresses,
        peer,
        deadline,
        |peer| {
            let socket = TcpStream::connect_timeout(&peer, remaining(deadline)?)
                .map_err(|error| io_error(Stage::Connect, error))?;
            modern(socket, url, deadline)
        },
        || legacy(&addresses, url, deadline).map_err(retry::Failure::Terminal),
    )
}

/** One raced connection to the admitted addresses. */
fn race(addresses: &[SocketAddr], deadline: Instant) -> EngineResult<(TcpStream, SocketAddr)> {
    super::eyeballs::race(addresses, deadline, |address, budget| {
        TcpStream::connect_timeout(address, budget)
    })
    .map_err(|error| io_error(Stage::Connect, error))
}

/** Every modern candidate keeps the identical TLS configuration, hostname and verifier.
The temporary lease interrupts Rustls's internal complete_io loop at the absolute deadline;
it is stopped and joined before a socket is handed off or a replacement is opened. */
fn modern(socket: TcpStream, url: &Url, deadline: Instant) -> Result<Connection, retry::Failure> {
    let mut connection = wrap(socket, url)?;
    let lease = if matches!(&connection, Connection::Tls(_)) {
        Some(connection.lease(deadline)?)
    } else {
        None
    };
    let result = connection.handshake(deadline);
    drop(lease);
    remaining(deadline)?;
    result?;
    Ok(connection)
}

/** The existing explicit handshake_failure alert alone selects RSA key exchange. This is
one recovery candidate, never an additional third attempt after a modern reset retry. */
fn legacy(addresses: &[SocketAddr], url: &Url, deadline: Instant) -> EngineResult<Connection> {
    let (socket, _) = race(addresses, deadline)?;
    let lease = super::lease::Lease::start(&socket, deadline)?;
    let budget = remaining(deadline)?;
    socket
        .set_read_timeout(Some(budget))
        .map_err(|error| io_error(Stage::SocketOptions, error))?;
    socket
        .set_write_timeout(Some(budget))
        .map_err(|error| io_error(Stage::SocketOptions, error))?;
    let name = url.host_str().ok_or_else(denied)?.trim_matches(['[', ']']);
    let result = super::legacy::connect(socket, name, &super::hello::verifier()?);
    drop(lease);
    remaining(deadline)?;
    result
        .map(|stream| Connection::Legacy(Box::new(stream)))
        .map_err(|error| io_error(Stage::Handshake, error))
}

/** Addresses for a URL: literal and loopback hosts answer at once; a domain asks the system
resolver on a helper thread and waits only until `deadline`. The resolver has no timeout of its
own, and when it stalled on one third-party host the whole service stalled with it, which the
supervisor then killed and restarted ("Browser network service stopped" on every ad-heavy page);
now only that one request fails and the stalled lookup finishes on its own. */
fn resolve(url: &Url, fixture: bool, deadline: Instant) -> EngineResult<Vec<SocketAddr>> {
    let port = url.port_or_known_default().ok_or_else(denied)?;
    let host = url.host().ok_or_else(denied)?;
    let addresses: Vec<SocketAddr> = match host {
        Host::Ipv4(ip) => vec![SocketAddr::new(IpAddr::V4(ip), port)],
        Host::Ipv6(ip) => vec![SocketAddr::new(IpAddr::V6(ip), port)],
        // Chromium's ResolveLocalHostname: loopback without asking DNS.
        Host::Domain(host) if super::super::policy::local_hostname(host) => vec![
            SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), port),
            SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::LOCALHOST), port),
        ],
        Host::Domain(host) => resolve_domain(host, port, deadline)?,
    };
    if addresses.is_empty()
        || addresses.len() > DNS_ANSWERS
        || addresses
            .iter()
            .any(|address| !admitted(address.ip(), fixture))
    {
        return Err(denied());
    }
    Ok(addresses)
}

/** Lookups still running past their request: a resolver stall storm may hold this many helper
threads at once; beyond it a lookup fails at once instead of adding another blocked thread. */
const DNS_IN_FLIGHT: usize = 16;
/** Helper threads currently inside the system resolver. The service process belongs to one
window and answers one request at a time, so this pool is per window: a page can only fill it
one stalled host per request, and slots return as the system resolver gives up. */
static DNS_RUNNING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/** One slot of DNS_RUNNING, given back when the lookup thread ends however it ends. */
struct DnsSlot;
impl Drop for DnsSlot {
    fn drop(&mut self) {
        DNS_RUNNING.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/** The system resolver's answers for a domain, bounded by the request deadline and by the
number of lookups still outstanding. */
fn resolve_domain(host: &str, port: u16, deadline: Instant) -> EngineResult<Vec<SocketAddr>> {
    use std::sync::atomic::Ordering;
    if DNS_RUNNING.load(Ordering::Relaxed) >= DNS_IN_FLIGHT {
        return Err(resolver_timeout());
    }
    let (sender, receiver) = std::sync::mpsc::channel();
    let query = (host.to_string(), port);
    DNS_RUNNING.fetch_add(1, Ordering::Relaxed);
    let slot = DnsSlot;
    std::thread::Builder::new()
        .name("mars-dns".into())
        .spawn(move || {
            let _slot = slot;
            let answer = query
                .to_socket_addrs()
                .map(|addresses| addresses.take(DNS_ANSWERS + 1).collect::<Vec<_>>());
            let _ = sender.send(answer);
        })
        .map_err(|error| io_error(Stage::ResolverStart, error))?;
    match receiver.recv_timeout(remaining(deadline)?) {
        Ok(answer) => answer.map_err(|error| io_error(Stage::Resolve, error)),
        Err(_) => Err(resolver_timeout()),
    }
}

/** A lookup that outlived the request: the destination is reported unreachable for now. */
fn resolver_timeout() -> EngineError {
    EngineError::new(
        "timeout",
        "The destination's address could not be resolved in time.",
        "Retry the destination.",
    )
}

/** The one destination predicate: public, or loopback under the exact fixture grant. */
pub(super) fn admitted(ip: IpAddr, fixture: bool) -> bool {
    public_address(ip) || (fixture && ip.is_loopback())
}

/** SNI and certificate verification use the URL hostname, never the resolved IP substitute. */
fn wrap(socket: TcpStream, url: &Url) -> EngineResult<Connection> {
    if url.scheme() == "http" {
        return Ok(Connection::Plain(socket));
    }
    let name = rustls::pki_types::ServerName::try_from(
        url.host_str()
            .ok_or_else(denied)?
            .trim_matches(['[', ']'])
            .to_owned(),
    )
    .map_err(|_| tls_error())?;
    let connection =
        ClientConnection::new(super::hello::config()?, name).map_err(|_| tls_error())?;
    Ok(Connection::Tls(Box::new(StreamOwned::new(
        connection, socket,
    ))))
}

impl Connection {
    /** Prepared socket lifetime is bounded even before a transmit operation arrives. */
    pub(super) fn lease(&self, deadline: Instant) -> EngineResult<super::lease::Lease> {
        super::lease::Lease::start(self.socket(), deadline)
    }
    /**
     * No application bytes leave until the coordinator authorizes transmission. `conditional`
     * holds the cache's validator fields, built from stored header values that header parsing
     * already proved free of control characters. GET asks to keep the connection open for
     * reuse; POST always closes, since a POST is never retried. Header order and spelling
     * follow Chrome's HTTP/1.1 requests.
     */
    pub(super) fn write_web_request(
        &mut self,
        url: &Url,
        request: &WebRequest,
        conditional: &[(&'static str, String)],
        deadline: Instant,
    ) -> EngineResult<()> {
        let wire = http1_request(url, request, conditional)?;
        let mut bytes = wire.as_bytes();
        while !bytes.is_empty() {
            self.deadline(deadline)?;
            let count = match self {
                Self::Plain(socket) => socket.write(bytes),
                Self::Tls(stream) => stream.write(bytes),
                Self::Legacy(stream) => stream.write(bytes),
            }
            .map_err(|error| io_error(Stage::Write, error))?;
            if count == 0 {
                return Err(io_error(Stage::Write, std::io::ErrorKind::WriteZero.into()));
            }
            bytes = &bytes[count..];
        }
        Ok(())
    }

    /** The ALPN result: `h2` when the server chose HTTP/2, otherwise `http/1.1`. */
    pub(super) fn protocol(&self) -> &'static str {
        match self {
            Self::Tls(stream) if stream.conn.alpn_protocol() == Some(b"h2") => "h2",
            _ => "http/1.1",
        }
    }

    /** Hands a negotiated TLS session to the HTTP/2 framing layer; None for plaintext. */
    pub(super) fn into_tls(self) -> Option<(ClientConnection, TcpStream)> {
        match self {
            Self::Tls(stream) => {
                let StreamOwned { conn, sock } = *stream;
                Some((conn, sock))
            }
            Self::Plain(_) | Self::Legacy(_) => None,
        }
    }

    /**
     * TLS negotiation finishes in preparation, while stale ownership can still prevent POST.
     * Preserve typed handshake I/O failures until the bounded recovery decision is made.
     */
    fn handshake(&mut self, deadline: Instant) -> Result<(), retry::Failure> {
        if let Self::Tls(stream) = self {
            while stream.conn.is_handshaking() {
                let remaining = remaining(deadline)?;
                stream
                    .sock
                    .set_read_timeout(Some(remaining))
                    .map_err(|error| io_error(Stage::SocketOptions, error))?;
                stream
                    .sock
                    .set_write_timeout(Some(remaining))
                    .map_err(|error| io_error(Stage::SocketOptions, error))?;
                stream
                    .conn
                    .complete_io(&mut stream.sock)
                    .map_err(retry::Failure::Handshake)?;
            }
        }
        Ok(())
    }

    /** The exact admitted address this socket was connected to. */
    pub(super) fn peer(&self) -> Option<SocketAddr> {
        self.socket().peer_addr().ok()
    }

    /**
     * An idle pooled connection is reusable only when the peer has neither closed it nor sent
     * anything unrequested. TLS records that arrived while idle (session tickets, a close
     * alert) are processed without blocking; any plaintext or a close disqualifies it.
     */
    pub(super) fn idle_and_open(&mut self) -> bool {
        if self.socket().set_nonblocking(true).is_err() {
            return false;
        }
        let open = match self {
            Self::Plain(socket) => matches!(
                socket.peek(&mut [0]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
            ),
            Self::Tls(stream) => tls_idle(stream),
            Self::Legacy(stream) => {
                stream.open()
                    && matches!(
                        stream.sock.peek(&mut [0]),
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
                    )
            }
        };
        self.socket().set_nonblocking(false).is_ok() && open
    }

    /** Both variants expose the one OS socket for timeouts, probes and leases. */
    fn socket(&self) -> &TcpStream {
        match self {
            Self::Plain(socket) => socket,
            Self::Tls(stream) => &stream.sock,
            Self::Legacy(stream) => &stream.sock,
        }
    }

    /**
     * Reads and TLS handshake I/O share the remaining absolute request budget. A TCP close
     * without TLS close_notify reads as EOF, as in every browser: servers that end a
     * connection-delimited body that way (craigslist.org's redirects) are common, and
     * Content-Length and chunked bodies still refuse a short read as truncated.
     */
    pub(super) fn read(&mut self, bytes: &mut [u8], deadline: Instant) -> EngineResult<usize> {
        self.deadline(deadline)?;
        match self {
            Self::Plain(socket) => socket.read(bytes),
            Self::Tls(stream) => match stream.read(bytes) {
                Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => Ok(0),
                result => result,
            },
            Self::Legacy(stream) => match stream.read(bytes) {
                Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => Ok(0),
                result => result,
            },
        }
        .map_err(|error| io_error(Stage::Read, error))
    }

    /** Every socket operation receives both timeouts because TLS reads may also write. */
    fn deadline(&self, deadline: Instant) -> EngineResult<()> {
        let socket = self.socket();
        let duration = remaining(deadline)?;
        socket
            .set_read_timeout(Some(duration))
            .map_err(|error| io_error(Stage::SocketOptions, error))?;
        socket
            .set_write_timeout(Some(duration))
            .map_err(|error| io_error(Stage::SocketOptions, error))
    }
}

/** One total budget covers redirects, handshake and body reads. */
fn remaining(deadline: Instant) -> EngineResult<Duration> {
    let duration = deadline.saturating_duration_since(Instant::now());
    if duration.is_zero() {
        return Err(EngineError::new(
            "timeout",
            "The web request exceeded its eight-second deadline.",
            "Retry the destination.",
        ));
    }
    Ok(duration)
}

/**
 * Only a handshake_failure alert retries over kRSA: it is how a server says no suite matched.
 * A protocol_version or insufficient_security refusal is not one a different TLS 1.2 suite
 * can answer, and retrying on it would widen the downgrade surface for nothing.
 */
pub(super) fn retry_with_rsa(error: &std::io::Error) -> bool {
    matches!(
        error
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<rustls::Error>()),
        Some(rustls::Error::AlertReceived(
            rustls::AlertDescription::HandshakeFailure
        ))
    )
}

/**
 * The server answered the hello with handshake_failure (or a version or security refusal):
 * no suite or version in common, Chromium's ERR_SSL_VERSION_OR_CIPHER_MISMATCH. A server that
 * picked one of the suites hello.rs advertises for Chrome's shape but Rustls cannot run lands
 * here too, so legacy.rs gets its turn at the kRSA ones.
 */
fn refused_suites(error: &std::io::Error) -> bool {
    use rustls::AlertDescription::{HandshakeFailure, InsufficientSecurity, ProtocolVersion};
    use rustls::PeerMisbehaved::SelectedUnofferedCipherSuite;
    matches!(
        error
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<rustls::Error>()),
        Some(
            rustls::Error::AlertReceived(HandshakeFailure | InsufficientSecurity | ProtocolVersion)
                | rustls::Error::PeerMisbehaved(SelectedUnofferedCipherSuite)
        )
    )
}

/** A certificate refusal keeps its text; a suite mismatch says so instead. */
fn tls_failure(stage: Stage, error: &std::io::Error) -> EngineError {
    let legacy = error
        .get_ref()
        .is_some_and(|inner| inner.is::<super::legacy::Mismatch>());
    if legacy || refused_suites(error) {
        return EngineError::new(
            "tls_error",
            "The server uses an unsupported TLS protocol or cipher suite.",
            "The site's TLS configuration is not one a browser accepts.",
        );
    }
    if matches!(
        error
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<rustls::Error>()),
        Some(rustls::Error::InvalidCertificate(_))
    ) {
        return tls_error();
    }
    EngineError::new(
        "tls_error",
        stage.tls_message(),
        "Check the destination's TLS configuration and retry.",
    )
}

/** Native TLS copy gives the user an explicit final refusal. */
fn tls_error() -> EngineError {
    EngineError::new(
        "tls_error",
        "The server's certificate could not be verified.",
        "Check the destination and its certificate; verification cannot be bypassed.",
    )
}
/** Drains idle TLS records on a nonblocking socket; open means no plaintext and no close. */
fn tls_idle(stream: &mut StreamOwned<ClientConnection, TcpStream>) -> bool {
    if !stream
        .conn
        .process_new_packets()
        .is_ok_and(|state| !state.peer_has_closed() && state.plaintext_bytes_to_read() == 0)
    {
        return false;
    }
    for _ in 0..POOL_PROBE_RECORDS {
        match stream.conn.read_tls(&mut stream.sock) {
            Ok(0) => return false,
            Ok(_) => match stream.conn.process_new_packets() {
                Ok(state) if !state.peer_has_closed() && state.plaintext_bytes_to_read() == 0 => {}
                _ => return false,
            },
            Err(error) => return error.kind() == std::io::ErrorKind::WouldBlock,
        }
    }
    false
}

/** Fixed read chunks keep framing allocation bounded independently of socket buffering. */
pub(super) const READ_CHUNK: usize = READ_BYTES;

/** The request line, Host, Connection, then Chrome's fields, a script's own, then the body. */
fn http1_request(
    url: &Url,
    request: &WebRequest,
    conditional: &[(&'static str, String)],
) -> EngineResult<String> {
    let host = url.host_str().ok_or_else(denied)?;
    let host = url
        .port()
        .map(|port| format!("{host}:{port}"))
        .unwrap_or_else(|| host.to_owned());
    let target = format!(
        "{}{}",
        url.path(),
        url.query()
            .map(|query| format!("?{query}"))
            .unwrap_or_default()
    );
    let method = super::fields::method(request);
    let connection = if !request.method.is_safe() {
        "close"
    } else {
        "keep-alive"
    };
    let mut wire =
        format!("{method} {target} HTTP/1.1\r\nHost: {host}\r\nConnection: {connection}\r\n");
    if let Some(length) = super::fields::content_length(request) {
        wire.push_str(&format!("Content-Length: {length}\r\n"));
    }
    for (name, value) in super::fields::fields(url, request, conditional) {
        wire.push_str(&format!("{}: {value}\r\n", super::fields::http1_name(name)));
    }
    for (name, value) in super::fields::page_fields(request) {
        wire.push_str(&format!("{}: {value}\r\n", super::fields::http1_name(name)));
    }
    wire.push_str("\r\n");
    wire.push_str(&request.body);
    Ok(wire)
}

#[cfg(test)]
mod tests {
    use super::*;

    /** A TLS peer that closes TCP without close_notify ends a body like a plain close. */
    #[test]
    fn a_tls_close_without_close_notify_reads_as_end_of_body() {
        let (client, server) = super::super::testing::tls(b"http/1.1", |mut tls| {
            tls.write_all(b"HTTP/1.1 302 Found\r\nLocation: /next\r\n\r\n")
                .expect("write");
            tls.flush().expect("flush");
            let _ = tls.sock.shutdown(std::net::Shutdown::Both);
        });
        let mut connection = Connection::Tls(Box::new(client));
        let deadline = Instant::now() + Duration::from_secs(5);
        let response =
            super::super::body::response(&mut connection, deadline, false, false, Vec::new())
                .expect("response");
        assert_eq!((response.status, response.reusable), (302, false));
        server.join().expect("server");
    }
}
