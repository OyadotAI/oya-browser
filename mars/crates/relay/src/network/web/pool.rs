/** A bounded keep-alive pool of idle, policy-admitted connections, reused for later GETs. */
use super::transport::{Connection, admitted};
use crate::network::constants::{POOL_IDLE, POOL_PER_ORIGIN, POOL_TOTAL};
use ::url::Url;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::Instant;

/**
 * Connections are shared only between requests with the same scheme, host, port and fixture
 * grant. TLS was verified for that exact host name, so the key never mixes names that happen
 * to resolve to one address, and a request whose grant differs never sees the other's socket.
 */
#[derive(Clone, PartialEq)]
pub(super) struct Key {
    /** `http` or `https`. */
    scheme: String,
    /** Normalized URL host, the TLS server name. */
    host: String,
    /** Explicit or default port. */
    port: u16,
    /** Whether the loopback fixture grant admitted the original connection. */
    fixture: bool,
}

/** One parked connection and the exact address its DNS answer was pinned to. */
struct Idle {
    /** Which requests may take it. */
    key: Key,
    /** Rechecked against the destination policy at checkout. */
    address: SocketAddr,
    /** Fully read, framed and idle. */
    connection: Connection,
    /** Idle connections past `POOL_IDLE` are closed rather than reused. */
    since: Instant,
}

/// Idle connections across every origin, most recently parked last.
pub(super) struct Pool {
    idle: Mutex<Vec<Idle>>,
}

/// The one pool the network process shares across navigations and subresources.
pub(super) static POOL: Pool = Pool::new();

impl Pool {
    /** An empty pool; tests build their own so bounds are observed in isolation. */
    pub(super) const fn new() -> Self {
        Self {
            idle: Mutex::new(Vec::new()),
        }
    }

    /**
     * Takes the most recently parked live connection for this request, if any. Its pinned
     * address must still pass the same policy a fresh DNS answer would, and the liveness
     * probe runs outside the lock so a slow peer cannot stall other requests.
     */
    pub(super) fn checkout(&self, url: &Url, fixture: bool) -> Option<Connection> {
        let key = key(url, fixture)?;
        loop {
            let mut idle = self.take(&key)?;
            if idle.since.elapsed() < POOL_IDLE
                && admitted(idle.address.ip(), fixture)
                && idle.connection.idle_and_open()
            {
                return Some(idle.connection);
            }
        }
    }

    /** Parks a reusable connection, evicting expired ones, then the oldest beyond each bound. */
    pub(super) fn checkin(&self, url: &Url, fixture: bool, connection: Connection) {
        let (Some(key), Some(address)) = (key(url, fixture), connection.peer()) else {
            return;
        };
        let Ok(mut idle) = self.idle.lock() else {
            return;
        };
        idle.retain(|entry| entry.since.elapsed() < POOL_IDLE);
        if let Some(oldest) = idle.iter().position(|entry| entry.key == key)
            && idle.iter().filter(|entry| entry.key == key).count() >= POOL_PER_ORIGIN
        {
            idle.remove(oldest);
        }
        if idle.len() >= POOL_TOTAL {
            idle.remove(0);
        }
        idle.push(Idle {
            key,
            address,
            connection,
            since: Instant::now(),
        });
    }

    /** Idle connections currently parked, for bound checks. */
    #[cfg(test)]
    fn len(&self) -> usize {
        self.idle.lock().map_or(0, |idle| idle.len())
    }

    /** Removes the newest matching entry under the lock. */
    fn take(&self, key: &Key) -> Option<Idle> {
        let mut idle = self.idle.lock().ok()?;
        let index = idle.iter().rposition(|entry| entry.key == *key)?;
        Some(idle.remove(index))
    }
}

/** URLs without a host or port have already been refused by policy; they never pool. */
pub(super) fn key(url: &Url, fixture: bool) -> Option<Key> {
    Some(Key {
        scheme: url.scheme().to_owned(),
        host: url.host_str()?.to_owned(),
        port: url.port_or_known_default()?,
        fixture,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};

    /** A connected loopback socket pair; the listener side is returned to keep it open. */
    fn pair(listener: &TcpListener) -> (Connection, TcpStream) {
        let client = TcpStream::connect(listener.local_addr().expect("address")).expect("connect");
        let (server, _) = listener.accept().expect("accept");
        (Connection::Plain(client), server)
    }

    /** Per-origin and total bounds both evict the oldest parked connection. */
    #[test]
    fn pool_keeps_at_most_six_per_origin_and_twenty_four_in_total() {
        let pool = Pool::new();
        let mut servers = Vec::new();
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let url =
            Url::parse(&format!("http://{}/", listener.local_addr().expect("a"))).expect("url");
        for _ in 0..POOL_PER_ORIGIN + 2 {
            let (connection, server) = pair(&listener);
            servers.push(server);
            pool.checkin(&url, true, connection);
        }
        assert_eq!(pool.len(), POOL_PER_ORIGIN);
        for _ in 0..POOL_TOTAL + 2 {
            let other = TcpListener::bind("127.0.0.1:0").expect("listener");
            let url =
                Url::parse(&format!("http://{}/", other.local_addr().expect("a"))).expect("url");
            let (connection, server) = pair(&other);
            servers.push(server);
            pool.checkin(&url, true, connection);
        }
        assert_eq!(pool.len(), POOL_TOTAL);
    }

    /** A grant change, a peer close or unrequested bytes all prevent reuse. */
    #[test]
    fn pool_refuses_other_grants_closed_peers_and_unread_bytes() {
        let pool = Pool::new();
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let url =
            Url::parse(&format!("http://{}/", listener.local_addr().expect("a"))).expect("url");
        let (connection, server) = pair(&listener);
        pool.checkin(&url, true, connection);
        assert!(pool.checkout(&url, false).is_none(), "other grant");
        let reused = pool.checkout(&url, true).expect("same grant reuses");
        pool.checkin(&url, true, reused);
        drop(server);
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert!(pool.checkout(&url, true).is_none(), "closed peer");
        let (connection, mut server) = pair(&listener);
        std::io::Write::write_all(&mut server, b"HTTP/1.1 200 OK\r\n").expect("stray");
        std::thread::sleep(std::time::Duration::from_millis(20));
        pool.checkin(&url, true, connection);
        assert!(pool.checkout(&url, true).is_none(), "unrequested bytes");
        assert_eq!(pool.len(), 0);
    }

    /** Without the fixture grant, a parked loopback connection fails the address policy. */
    #[test]
    fn pooled_loopback_connections_need_the_fixture_grant() {
        let pool = Pool::new();
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let url =
            Url::parse(&format!("http://{}/", listener.local_addr().expect("a"))).expect("url");
        let (connection, _server) = pair(&listener);
        pool.checkin(&url, false, connection);
        assert!(pool.checkout(&url, false).is_none());
    }
}
