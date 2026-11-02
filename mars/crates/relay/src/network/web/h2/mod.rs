/**
 * Owned HTTP/2 (RFC 9113) over TLS with ALPN: one multiplexed connection per origin shared by
 * every concurrent request. No thread is dedicated to a connection; whichever waiting caller
 * finds the socket free reads it and delivers frames to every stream (leader and followers).
 */
mod frame;
mod hpack;
mod registry;
mod state;
mod stream;
mod table;
use super::body;
use super::transport::{Connection, READ_CHUNK};
use super::{Failure, encoding, fields};
use crate::network::WebRequest;
use crate::network::constants::POOL_IDLE;
use crate::protocol::{EngineError, EngineResult};
use ::url::Url;
pub(super) use registry::{Acquired, REGISTRY};
use state::State;
use std::io::Read;
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Instant;
use stream::Ending;

/// A shared HTTP/2 connection.
pub(super) struct Session {
    state: Mutex<State>,
    /** Signalled whenever frames were processed or the reader stepped down. */
    progress: Condvar,
    /** A clone of the socket, read without holding the lock so writers are never blocked. */
    reader: TcpStream,
    peer: Option<SocketAddr>,
}

impl Session {
    /// Takes over a TLS connection that negotiated `h2` and sends the client preface.
    pub(super) fn start(connection: Connection, deadline: Instant) -> EngineResult<Arc<Self>> {
        let peer = connection.peer();
        let (tls, socket) = connection.into_tls().ok_or_else(state::reset_error)?;
        let reader = socket.try_clone().map_err(|_| state::reset_error())?;
        Ok(Arc::new(Self {
            state: Mutex::new(State::new(tls, socket, deadline)?),
            progress: Condvar::new(),
            reader,
            peer,
        }))
    }

    /// The exact admitted address the connection uses.
    pub(super) fn peer(&self) -> Option<SocketAddr> {
        self.peer
    }

    /// Whether a new request may use this connection: open, no GOAWAY, not idle too long.
    pub(super) fn usable(&self) -> bool {
        let state = self.lock();
        state.accepting() && state.used.elapsed() < POOL_IDLE
    }

    /// One request and its complete response. Retryable failures are those where the server
    /// provably did not process the request (GOAWAY, REFUSED_STREAM, a dead connection).
    pub(super) fn exchange(
        &self,
        url: &Url,
        request: &WebRequest,
        conditional: &[(&'static str, String)],
        limit: usize,
        deadline: Instant,
    ) -> Result<body::Response, Failure> {
        let fields = request_fields(url, request, conditional);
        let borrowed: Vec<(&str, &str)> = fields
            .iter()
            .map(|(n, v)| (n.as_str(), v.as_str()))
            .collect();
        let weight = fields::priority(request).1;
        let mut state = self.lock();
        while state.accepting() && !state.has_capacity() {
            state = self.wait(state, deadline).map_err(Failure::Final)?;
        }
        let head = request.method == crate::network::Method::Head;
        let body = request.body.as_slice();
        let id = match state.open(&borrowed, weight, body, limit, head, deadline) {
            Ok(Some(id)) => id,
            Ok(None) => return Err(Failure::Retryable(state::reset_error())),
            Err(error) => return Err(Failure::Retryable(error)),
        };
        loop {
            if let Some((stream, ending)) = state.take(id) {
                drop(state);
                self.progress.notify_all();
                return finish(stream, ending, limit);
            }
            state = match self.step(state, deadline) {
                Ok(state) => state,
                Err(error) => {
                    let mut state = self.lock();
                    state.cancel(id);
                    drop(state);
                    self.progress.notify_all();
                    return Err(Failure::Final(error));
                }
            };
        }
    }

    /** Reads once if nobody else is reading, otherwise waits for whoever is. */
    fn step<'a>(
        &'a self,
        mut state: MutexGuard<'a, State>,
        deadline: Instant,
    ) -> EngineResult<MutexGuard<'a, State>> {
        if state.reading {
            return self.wait(state, deadline);
        }
        state.reading = true;
        drop(state);
        let read = self.read(deadline);
        let mut state = self.lock();
        state.reading = false;
        match read {
            Ok(bytes) => state.feed(&bytes, deadline),
            Err(error) => {
                self.progress.notify_all();
                return Err(error);
            }
        }
        self.progress.notify_all();
        Ok(state)
    }

    /** Waits for progress until the deadline. */
    fn wait<'a>(
        &'a self,
        state: MutexGuard<'a, State>,
        deadline: Instant,
    ) -> EngineResult<MutexGuard<'a, State>> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(timeout());
        }
        let (state, _) = self
            .progress
            .wait_timeout(state, remaining)
            .unwrap_or_else(PoisonError::into_inner);
        Ok(state)
    }

    /** One raw socket read within the deadline; a non-timeout error ends the connection. */
    fn read(&self, deadline: Instant) -> EngineResult<Vec<u8>> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(timeout());
        }
        let mut bytes = vec![0; READ_CHUNK];
        let stall = remaining.min(crate::constants::WEB_STALL_TIMEOUT);
        let _ = self.reader.set_read_timeout(Some(stall));
        match (&self.reader).read(&mut bytes) {
            Ok(count) => {
                bytes.truncate(count);
                Ok(bytes)
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                Err(timeout())
            }
            Err(_) => {
                self.lock().abandon();
                Err(state::reset_error())
            }
        }
    }

    /** A panicking holder cannot leave the state unusable for others; it is plain data. */
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/** Pseudo-headers in Chrome's order (method, authority, scheme, path), then Chrome's fields. */
fn request_fields(
    url: &Url,
    request: &WebRequest,
    conditional: &[(&'static str, String)],
) -> Vec<(String, String)> {
    let authority = match url.port() {
        Some(port) => format!("{}:{port}", url.host_str().unwrap_or_default()),
        None => url.host_str().unwrap_or_default().to_owned(),
    };
    let path = match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().to_owned(),
    };
    let method = fields::method(request);
    let mut all = vec![
        (":method".to_owned(), method.to_owned()),
        (":authority".to_owned(), authority),
        (":scheme".to_owned(), url.scheme().to_owned()),
        (":path".to_owned(), path),
    ];
    if let Some(length) = fields::content_length(request) {
        all.push(("content-length".to_owned(), length.to_string()));
    }
    all.extend(
        fields::fields(url, request, conditional)
            .into_iter()
            .map(|(n, v)| (n.to_owned(), v)),
    );
    all.extend(
        fields::page_fields(request).map(|(name, value)| (name.to_owned(), value.to_owned())),
    );
    all.push((
        "priority".to_owned(),
        fields::priority(request).0.to_owned(),
    ));
    all
}

/** A finished stream becomes the same decoded response HTTP/1.1 produces. */
fn finish(stream: stream::Stream, ending: Ending, limit: usize) -> Result<body::Response, Failure> {
    match ending {
        Ending::Failed { retry: true, error } => return Err(Failure::Retryable(error)),
        Ending::Failed {
            retry: false,
            error,
        } => return Err(Failure::Final(error)),
        Ending::Complete => {}
    }
    let status = stream
        .status
        .ok_or_else(|| Failure::Final(state::reset_error()))?;
    let body = encoding::decode(
        stream.body,
        stream.headers.get("content-encoding").map(String::as_str),
        limit,
    )
    .map_err(Failure::Final)?;
    Ok(body::Response {
        status,
        headers: stream.headers,
        set_cookies: stream.set_cookies,
        body,
        reusable: false,
    })
}

/** The request's own deadline passed; other streams continue. */
fn timeout() -> EngineError {
    EngineError::new(
        "timeout",
        "The web request timed out.",
        "Retry the destination.",
    )
}

#[cfg(test)]
mod tests;
