/**
 * HTTP/2 connections by origin. Concurrent requests to an origin wait for the first one's
 * connection instead of each opening their own, so a page gets one multiplexed connection.
 */
use super::Session;
use crate::network::constants::{H2_HTTP1_ORIGINS, H2_SESSIONS};
use crate::network::web::pool::{Key, key};
use crate::network::web::transport::admitted;
use ::url::Url;
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

/// What a request should do for its origin.
pub(in crate::network::web) enum Acquired<'a> {
    /// Use this live HTTP/2 connection.
    Session(Arc<Session>),
    /// Connect; report the outcome through the claim so waiting requests can proceed.
    Claim(Claim<'a>),
    /// The origin speaks HTTP/1.1 (or another request's connect is too slow); connect normally.
    Http1,
}

/// Holds an origin's "connecting" mark; dropping it without `finish` releases waiters.
pub(in crate::network::web) struct Claim<'a> {
    registry: &'a Registry,
    key: Key,
}

/** Everything behind the registry lock. */
struct Inner {
    sessions: Vec<(Key, Arc<Session>)>,
    connecting: Vec<Key>,
    http1: VecDeque<Key>,
}

/// The origin table.
pub(in crate::network::web) struct Registry {
    inner: Mutex<Inner>,
    changed: Condvar,
}

/// The one registry the network process shares.
pub(in crate::network::web) static REGISTRY: Registry = Registry::new();

impl Registry {
    /** Empty; tests build their own. */
    pub(super) const fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                sessions: Vec::new(),
                connecting: Vec::new(),
                http1: VecDeque::new(),
            }),
            changed: Condvar::new(),
        }
    }

    /// A live session for the origin, a claim to connect, or HTTP/1.1. Waits for another
    /// request's connect only until `deadline`.
    pub(in crate::network::web) fn acquire(
        &self,
        url: &Url,
        fixture: bool,
        deadline: Instant,
    ) -> Acquired<'_> {
        let Some(key) = key(url, fixture) else {
            return Acquired::Http1;
        };
        let mut inner = self.lock();
        loop {
            inner.sessions.retain(|(_, session)| session.usable());
            if let Some((_, session)) = inner.sessions.iter().find(|(k, s)| {
                *k == key && s.peer().is_some_and(|peer| admitted(peer.ip(), fixture))
            }) {
                return Acquired::Session(session.clone());
            }
            if inner.http1.contains(&key) {
                return Acquired::Http1;
            }
            if !inner.connecting.contains(&key) {
                inner.connecting.push(key.clone());
                return Acquired::Claim(Claim {
                    registry: self,
                    key,
                });
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Acquired::Http1;
            }
            inner = self
                .changed
                .wait_timeout(inner, remaining)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /** Lock, recovering from a poisoned holder: the table is plain data. */
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /** Sessions currently held, for bound checks. */
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.lock().sessions.len()
    }
}

impl Claim<'_> {
    /// Records the connect's outcome: a session to share, or an HTTP/1.1-only origin.
    pub(in crate::network::web) fn finish(self, session: Option<Arc<Session>>) {
        let mut inner = self.registry.lock();
        match session {
            Some(session) => {
                inner.sessions.retain(|(key, _)| *key != self.key);
                if inner.sessions.len() >= H2_SESSIONS {
                    inner.sessions.remove(0);
                }
                inner.sessions.push((self.key.clone(), session));
            }
            None => {
                if inner.http1.len() >= H2_HTTP1_ORIGINS {
                    inner.http1.pop_front();
                }
                inner.http1.push_back(self.key.clone());
            }
        }
    }
}

impl Drop for Claim<'_> {
    /** Clears the mark whether or not the connect succeeded, then wakes every waiter. */
    fn drop(&mut self) {
        self.registry
            .lock()
            .connecting
            .retain(|key| *key != self.key);
        self.registry.changed.notify_all();
    }
}
