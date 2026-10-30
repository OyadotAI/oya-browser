/** One pre-HTTP recovery candidate preserves policy, verification and the original deadline. */
use super::{Stage, io_error, remaining, retry_with_rsa};
use crate::protocol::{EngineError, EngineResult};
use std::{io, net::SocketAddr, time::Instant};

/** Keep the actual handshake error typed until recovery is selected; other phases are terminal. */
pub(super) enum Failure {
    /** Only Rustls handshake I/O produces this variant, before any application request bytes. */
    Handshake(io::Error),
    /** Policy, configuration, connect and socket-option errors never trigger this recovery. */
    Terminal(EngineError),
}

impl From<EngineError> for Failure {
    /** Existing policy and deadline errors retain their original public category. */
    fn from(error: EngineError) -> Self {
        Self::Terminal(error)
    }
}

impl Failure {
    /** Translate the final typed outcome exactly once, after recovery is exhausted. */
    fn public(self) -> EngineError {
        match self {
            Self::Handshake(error) => io_error(Stage::Handshake, error),
            Self::Terminal(error) => error,
        }
    }
}

/** At most two TLS candidates total: modern then modern-on-reset OR the existing alert-only RSA
fallback. A recovery failure is terminal, including a second reset or a later RSA-eligible alert.
Both callbacks receive only previously admitted peers and run before HTTP transmission. */
pub(super) fn recover<T>(
    first: Result<T, Failure>,
    addresses: &[SocketAddr],
    peer: SocketAddr,
    deadline: Instant,
    modern: impl FnOnce(SocketAddr) -> Result<T, Failure>,
    legacy: impl FnOnce() -> Result<T, Failure>,
) -> EngineResult<T> {
    remaining(deadline)?;
    let result = match first {
        Err(Failure::Handshake(error)) if raw_reset(&error) => {
            modern(preferred_peer(addresses, peer))
        }
        Err(Failure::Handshake(error)) if retry_with_rsa(&error) => legacy(),
        result => result,
    };
    // Lease shutdown can surface EOF/reset: expiration remains a timeout, never another retry.
    remaining(deadline)?;
    result.map_err(Failure::public)
}

/** Only an actual reset of the transport retries; protocol, alert and certificate failures do not. */
fn raw_reset(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::ConnectionReset
        && !error
            .get_ref()
            .is_some_and(|inner| inner.is::<rustls::Error>())
}

/** Prefer the other family, then a distinct address; a single-peer origin gets one fresh socket. */
fn preferred_peer(addresses: &[SocketAddr], failed: SocketAddr) -> SocketAddr {
    addresses
        .iter()
        .find(|peer| peer.is_ipv6() != failed.is_ipv6())
        .or_else(|| addresses.iter().find(|peer| **peer != failed))
        .copied()
        .unwrap_or(failed)
}

#[cfg(test)]
mod tests;
