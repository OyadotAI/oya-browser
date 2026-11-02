/**
 * The link's network: a TCP or TLS connection to the Oya server, a minimal RFC 6455 WebSocket
 * client for the control socket, and one bounded HTTP POST for pairing. The control socket is the
 * person's own account connection, not page traffic, so it does not go through Relay's page
 * policy; it does verify certificates against the bundled web roots, and plaintext is limited
 * to this machine by the callers (pairing refuses `ws://` anywhere else).
 */
use crate::constants::{
    HTTP_OK_END, HTTP_OK_MIN, HTTP_PORT, HTTPS_PORT, MASK_BYTES, MAX_FRAME_BYTES, MAX_HTTP_BYTES,
    NETWORK_TIMEOUT, SWITCHING_PROTOCOLS, WS_KEY_BYTES,
};
use ring::rand::{SecureRandom, SystemRandom};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use std::io::{self, ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;
use url::Url;

/// The GUID RFC 6455 appends to the handshake key.
const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// A plain or TLS connection to the server.
pub enum Conn {
    /// `ws://` or `http://`, loopback only by policy.
    Plain(TcpStream),
    /// `wss://` or `https://`, verified against the web roots.
    Tls(Box<StreamOwned<ClientConnection, TcpStream>>),
}

impl Read for Conn {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(buf),
            Self::Tls(s) => s.read(buf),
        }
    }
}

impl Write for Conn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(s) => s.write(buf),
            Self::Tls(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(s) => s.flush(),
            Self::Tls(s) => s.flush(),
        }
    }
}

impl Conn {
    /// Bounds blocking reads, so the socket loop wakes to beat and to stop.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        match self {
            Self::Plain(s) => s.set_read_timeout(timeout),
            Self::Tls(s) => s.sock.set_read_timeout(timeout),
        }
    }
}

/// An io error with `message`.
fn error(kind: ErrorKind, message: &str) -> io::Error {
    io::Error::new(kind, message.to_owned())
}

/// Connects to `url`'s host: TLS for `wss`/`https`, plain for `ws`/`http`.
pub fn dial(url: &Url) -> io::Result<Conn> {
    let tls = matches!(url.scheme(), "wss" | "https");
    let host = url
        .host_str()
        .ok_or_else(|| error(ErrorKind::InvalidInput, "URL has no host"))?;
    let port = url
        .port()
        .unwrap_or(if tls { HTTPS_PORT } else { HTTP_PORT });
    // Every resolved address in turn: `localhost` is ::1 first, and a server may bind only IPv4.
    let mut last = error(ErrorKind::NotFound, "Server address not found");
    let socket = (host.trim_matches(['[', ']']), port)
        .to_socket_addrs()?
        .find_map(|address| {
            TcpStream::connect_timeout(&address, NETWORK_TIMEOUT)
                .map_err(|e| last = e)
                .ok()
        })
        .ok_or(last)?;
    socket.set_read_timeout(Some(NETWORK_TIMEOUT))?;
    socket.set_write_timeout(Some(NETWORK_TIMEOUT))?;
    if !tls {
        return Ok(Conn::Plain(socket));
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let tls_error = |_| error(ErrorKind::InvalidData, "TLS setup failed");
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(tls_error)?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(host.trim_matches(['[', ']']).to_owned())
        .map_err(|_| error(ErrorKind::InvalidInput, "Invalid server name"))?;
    let connection = ClientConnection::new(Arc::new(config), name).map_err(tls_error)?;
    Ok(Conn::Tls(Box::new(StreamOwned::new(connection, socket))))
}

/// Random bytes from the system generator.
pub fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    SystemRandom::new()
        .fill(&mut bytes)
        .expect("system randomness");
    bytes
}

/// Reads a response head (through the blank line) and returns it with any bytes after it.
fn read_head(stream: &mut impl Read) -> io::Result<(String, Vec<u8>)> {
    let mut buf = Vec::new();
    let mut chunk = [0; 1024];
    loop {
        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let rest = buf.split_off(end + 4);
            return Ok((String::from_utf8_lossy(&buf).into_owned(), rest));
        }
        if buf.len() > MAX_HTTP_BYTES {
            return Err(error(ErrorKind::InvalidData, "Response head too large"));
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Err(error(
                ErrorKind::UnexpectedEof,
                "Server closed during the handshake",
            ));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

/// The status code and a header's value from a response head.
fn status_and_header(head: &str, name: &str) -> (u16, Option<String>) {
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_default();
    let header = head.lines().skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().to_owned())
    });
    (status, header)
}

/// The path and query a request line names.
fn target(url: &Url) -> String {
    match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().to_owned(),
    }
}

/// The Host header: the port only when it is not the scheme's default.
fn host_header(url: &Url) -> String {
    let host = url.host_str().unwrap_or_default();
    url.port()
        .map_or_else(|| host.to_owned(), |port| format!("{host}:{port}"))
}

/// POSTs `body` as JSON and returns the status and body; no redirects, bounded size.
pub fn post_json(url: &Url, body: &str) -> io::Result<(u16, String)> {
    let mut conn = dial(url)?;
    let request = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        target(url),
        host_header(url),
        body.len()
    );
    conn.write_all(request.as_bytes())?;
    conn.flush()?;
    let (head, mut rest) = read_head(&mut conn)?;
    let limit = (MAX_HTTP_BYTES - rest.len().min(MAX_HTTP_BYTES)) as u64;
    match conn.take(limit).read_to_end(&mut rest) {
        Ok(_) => {}
        // A TLS peer that closes without close_notify still delivered the whole body.
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => {}
        Err(e) => return Err(e),
    }
    let (status, _) = status_and_header(&head, "content-length");
    Ok((status, String::from_utf8_lossy(&rest).into_owned()))
}

/// Whether `status` is a success.
pub fn success(status: u16) -> bool {
    (HTTP_OK_MIN..HTTP_OK_END).contains(&status)
}

/// What a read produced.
#[derive(Debug, PartialEq, Eq)]
pub enum Incoming {
    /// A whole text message.
    Text(String),
    /// The server closed, with its code (1005 when it gave none).
    Close(u16),
}

/// A client WebSocket over any byte stream.
pub struct WebSocket<S> {
    /// The connection.
    stream: S,
    /// Bytes read but not yet framed.
    buffer: Vec<u8>,
    /// A fragmented message being assembled.
    partial: Vec<u8>,
}

impl<S: Read + Write> WebSocket<S> {
    /// Opens the WebSocket at `url` over `stream`, checking the server's accept key.
    pub fn client(mut stream: S, url: &Url) -> io::Result<Self> {
        let key = crate::base64::encode(&random::<WS_KEY_BYTES>());
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n",
            target(url),
            host_header(url)
        );
        stream.write_all(request.as_bytes())?;
        stream.flush()?;
        let (head, buffer) = read_head(&mut stream)?;
        let (status, accept) = status_and_header(&head, "sec-websocket-accept");
        if status != SWITCHING_PROTOCOLS || accept.as_deref() != Some(&accept_key(&key)) {
            return Err(error(
                ErrorKind::ConnectionRefused,
                "Server refused the WebSocket upgrade",
            ));
        }
        Ok(Self {
            stream,
            buffer,
            partial: Vec::new(),
        })
    }

    /// The connection, for timeouts.
    pub fn get_ref(&self) -> &S {
        &self.stream
    }

    /// Sends one text message.
    pub fn send_text(&mut self, text: &str) -> io::Result<()> {
        self.send_frame(0x1, text.as_bytes())
    }

    /// Sends a close frame with `code`; the peer's close follows.
    pub fn close(&mut self, code: u16) -> io::Result<()> {
        self.send_frame(0x8, &code.to_be_bytes())
    }

    /// One masked, final frame.
    fn send_frame(&mut self, opcode: u8, payload: &[u8]) -> io::Result<()> {
        let mut frame = vec![0x80 | opcode];
        let len = payload.len();
        match len {
            0..=125 => frame.push(0x80 | len as u8),
            126..=0xffff => {
                frame.push(0x80 | 126);
                frame.extend_from_slice(&(len as u16).to_be_bytes());
            }
            _ => {
                frame.push(0x80 | 127);
                frame.extend_from_slice(&(len as u64).to_be_bytes());
            }
        }
        let mask = random::<MASK_BYTES>();
        frame.extend_from_slice(&mask);
        frame.extend(
            payload
                .iter()
                .enumerate()
                .map(|(i, b)| b ^ mask[i % MASK_BYTES]),
        );
        self.stream.write_all(&frame)?;
        self.stream.flush()
    }

    /// The next message, or None when the read timed out with no whole message yet.
    /// Pings are answered here; pongs are absorbed.
    pub fn read(&mut self) -> io::Result<Option<Incoming>> {
        loop {
            if let Some(message) = self.take_frame()? {
                return Ok(Some(message));
            }
            let mut chunk = [0; 16 * 1024];
            match self.stream.read(&mut chunk) {
                Ok(0) => return Ok(Some(Incoming::Close(1006))),
                Ok(n) => self.buffer.extend_from_slice(&chunk[..n]),
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    return Ok(None);
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Frames one message out of the buffer when it holds a whole one.
    fn take_frame(&mut self) -> io::Result<Option<Incoming>> {
        while let Some((fin, opcode, payload)) = self.parse()? {
            match opcode {
                0x9 => self.send_frame(0xA, &payload)?,
                0xA => {}
                0x8 => {
                    return Ok(Some(Incoming::Close(
                        payload
                            .get(..2)
                            .map_or(1005, |c| u16::from_be_bytes([c[0], c[1]])),
                    )));
                }
                0x0..=0x2 => {
                    self.partial.extend_from_slice(&payload);
                    if self.partial.len() as u64 > MAX_FRAME_BYTES {
                        return Err(error(ErrorKind::InvalidData, "Server message too large"));
                    }
                    if fin {
                        let text =
                            String::from_utf8(std::mem::take(&mut self.partial)).map_err(|_| {
                                error(ErrorKind::InvalidData, "Server sent invalid UTF-8")
                            })?;
                        return Ok(Some(Incoming::Text(text)));
                    }
                }
                _ => return Err(error(ErrorKind::InvalidData, "Unknown WebSocket opcode")),
            }
        }
        Ok(None)
    }

    /// One frame (fin, opcode, unmasked payload) when the buffer holds it whole.
    fn parse(&mut self) -> io::Result<Option<(bool, u8, Vec<u8>)>> {
        let b = &self.buffer;
        if b.len() < 2 {
            return Ok(None);
        }
        let (masked, short) = (b[1] & 0x80 != 0, u64::from(b[1] & 0x7f));
        let (len, mut at) = match short {
            126 if b.len() >= 4 => (u64::from(u16::from_be_bytes([b[2], b[3]])), 4),
            127 if b.len() >= 10 => (
                u64::from_be_bytes(b[2..10].try_into().expect("eight bytes")),
                10,
            ),
            126 | 127 => return Ok(None),
            n => (n, 2),
        };
        if len > MAX_FRAME_BYTES {
            return Err(error(ErrorKind::InvalidData, "Server frame too large"));
        }
        let mask = masked
            .then(|| b.get(at..at + MASK_BYTES))
            .flatten()
            .map(|m| [m[0], m[1], m[2], m[3]]);
        at += if masked { MASK_BYTES } else { 0 };
        let end = at + len as usize;
        if b.len() < end || (masked && mask.is_none()) {
            return Ok(None);
        }
        let (fin, opcode) = (b[0] & 0x80 != 0, b[0] & 0x0f);
        let payload = b[at..end]
            .iter()
            .enumerate()
            .map(|(i, x)| mask.map_or(*x, |m| x ^ m[i % MASK_BYTES]))
            .collect();
        self.buffer.drain(..end);
        Ok(Some((fin, opcode, payload)))
    }
}

/// The accept key a server must answer `key` with.
pub fn accept_key(key: &str) -> String {
    let digest = ring::digest::digest(
        &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
        format!("{key}{WS_GUID}").as_bytes(),
    );
    crate::base64::encode(digest.as_ref())
}
