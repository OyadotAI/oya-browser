/**
 * Happy Eyeballs TCP connection racing, after Chromium's TcpConnectJob
 * (net/socket/tcp_connect_job.cc): a primary attempt walks the addresses of the first DNS
 * answer's family in order; if it has not connected within kIPv6FallbackTime (300 ms), a
 * fallback attempt starts walking the other family, and the first socket to connect wins.
 * Without it, one address whose SYNs are silently dropped (www.fsf.org drops about half its
 * connections per flow) holds the whole navigation budget, while Chrome connects over the
 * other family. Every address raced was already admitted by policy.
 */
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Chromium's TcpConnectJob::kIPv6FallbackTime.
pub(super) const FALLBACK: Duration = Duration::from_millis(300);

/** What one attempt reports: a connected socket and its address, or its last error. */
type Outcome = io::Result<(TcpStream, SocketAddr)>;

/**
 * Connects to the first address that answers. `open` performs one bounded connect; it is a
 * parameter so tests can stand in for a peer that never answers.
 */
pub(super) fn race<F>(
    addresses: &[SocketAddr],
    deadline: Instant,
    open: F,
) -> io::Result<(TcpStream, SocketAddr)>
where
    F: Fn(&SocketAddr, Duration) -> io::Result<TcpStream> + Clone + Send + 'static,
{
    let first = addresses.first().ok_or_else(no_address)?.is_ipv6();
    let (primary, fallback): (Vec<_>, Vec<_>) = addresses
        .iter()
        .copied()
        .partition(|address| address.is_ipv6() == first);
    let (sender, outcomes) = mpsc::channel();
    let mut running = start(primary, deadline, open.clone(), sender.clone());
    let mut pending = (!fallback.is_empty()).then_some(fallback);
    let mut last = timed_out();
    while running > 0 || pending.is_some() {
        let wait = match pending {
            Some(_) => FALLBACK.min(left(deadline)),
            None => left(deadline),
        };
        match outcomes.recv_timeout(wait) {
            Ok(Ok(connected)) => return Ok(connected),
            Ok(Err(error)) => {
                running -= 1;
                last = error;
                if running == 0 && pending.is_none() {
                    break;
                }
                if running > 0 {
                    continue;
                }
            }
            Err(_) if left(deadline).is_zero() => return Err(timed_out()),
            Err(_) => {}
        }
        if let Some(addresses) = pending.take() {
            running += start(addresses, deadline, open.clone(), sender.clone());
        }
    }
    Err(last)
}

/** Starts one attempt walking `addresses` in order on its own thread; returns 1, or 0 if empty. */
fn start<F>(
    addresses: Vec<SocketAddr>,
    deadline: Instant,
    open: F,
    sender: mpsc::Sender<Outcome>,
) -> usize
where
    F: Fn(&SocketAddr, Duration) -> io::Result<TcpStream> + Send + 'static,
{
    if addresses.is_empty() {
        return 0;
    }
    std::thread::spawn(move || {
        let mut last = no_address();
        for address in addresses {
            let budget = left(deadline);
            if budget.is_zero() {
                last = timed_out();
                break;
            }
            match open(&address, budget) {
                // A losing socket is dropped here when the receiver is already gone.
                Ok(socket) => {
                    let _ = sender.send(Ok((socket, address)));
                    return;
                }
                Err(error) => last = error,
            }
        }
        let _ = sender.send(Err(last));
    });
    1
}

/** Time left before the deadline, zero once it passed. */
fn left(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

/** The budget ran out with no connection. */
fn timed_out() -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, "connect timed out")
}

/** No address to try. */
fn no_address() -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, "no address")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /** A listener on IPv4 loopback, and an IPv6 address standing in for a dropped flow. */
    fn listener() -> (TcpListener, SocketAddr, SocketAddr) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let live = listener.local_addr().expect("address");
        let dead = SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], live.port()));
        (listener, live, dead)
    }

    /** Connects for real, except that `dead` never answers until its budget ends. */
    fn open(dead: SocketAddr) -> impl Fn(&SocketAddr, Duration) -> io::Result<TcpStream> + Clone {
        move |address, budget| {
            if *address == dead {
                std::thread::sleep(budget);
                return Err(timed_out());
            }
            TcpStream::connect_timeout(address, budget)
        }
    }

    #[test]
    fn a_silent_first_family_falls_back_to_the_other_after_300_ms() {
        let (_listener, live, dead) = listener();
        let started = Instant::now();
        let deadline = started + Duration::from_secs(5);
        let (_, address) = race(&[dead, live], deadline, open(dead)).expect("fallback wins");
        assert_eq!(address, live);
        let spent = started.elapsed();
        assert!(
            spent >= FALLBACK && spent < Duration::from_secs(2),
            "{spent:?}"
        );
    }

    #[test]
    fn a_refused_primary_starts_the_fallback_at_once() {
        let (_listener, live, dead) = listener();
        let refuse = move |address: &SocketAddr, budget| {
            if *address == dead {
                return Err(io::Error::from(io::ErrorKind::ConnectionRefused));
            }
            TcpStream::connect_timeout(address, budget)
        };
        let started = Instant::now();
        let deadline = started + Duration::from_secs(5);
        let (_, address) = race(&[dead, live], deadline, refuse).expect("fallback wins");
        assert_eq!(address, live);
        assert!(started.elapsed() < FALLBACK, "no wait after a refusal");
    }

    #[test]
    fn one_family_walks_its_addresses_in_order() {
        let (_listener, live, _) = listener();
        let dead = SocketAddr::from(([127, 0, 0, 2], live.port()));
        let deadline = Instant::now() + Duration::from_secs(2);
        let (_, address) = race(&[live, dead], deadline, open(dead)).expect("first answers");
        assert_eq!(address, live);
    }

    #[test]
    fn every_address_silent_times_out_at_the_deadline() {
        let (_listener, _, dead) = listener();
        let started = Instant::now();
        let error =
            race(&[dead], started + Duration::from_millis(400), open(dead)).expect_err("timeout");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
