/** Indexed FIFO waiters share a backing mutex for comparison/registration and use individual wake signals. */
use super::*;
use std::sync::Condvar;
use std::time::{Duration, Instant};

/** Periodic wake checks enforce VM deadlines and worker termination even for infinite JS waits. */
const CANCELLATION_POLL: Duration = Duration::from_millis(5);
/** One waiter belongs to exactly one byte offset until notified, timed out or cancelled. */
pub(super) struct Entry {
    pub(super) offset: usize,
    pub(super) signal: Arc<Signal>,
    pub(super) asynchronous: Option<asynchronous::AsyncData>,
}
/** Per-waiter notification prevents notify at one offset from waking an unrelated address. */
pub(super) struct Signal {
    pub(super) notified: Mutex<bool>,
    pub(super) wake: Condvar,
}
/** A wait result comes from actual comparison, notification or elapsed time. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::v9) enum WaitResult {
    NotEqual,
    TimedOut,
    Ok,
}

impl SharedBytes {
    /** Report live pending waiters at an offset without inventing queue entries. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn waiter_count(&self, offset: usize) -> usize {
        self.lock()
            .waiters
            .iter()
            .filter(|entry| {
                entry.offset == offset
                    && !*entry
                        .signal
                        .notified
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
            })
            .count()
    }
    /** Notified async entries remain until their owning VM settles the corresponding promise. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn unresolved_async_count(&self, offset: usize) -> usize {
        self.lock()
            .waiters
            .iter()
            .filter(|entry| {
                entry.offset == offset
                    && entry.asynchronous.is_some()
                    && *entry
                        .signal
                        .notified
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
            })
            .count()
    }
    /** Value comparison and queue insertion are atomic with respect to every store and notify. */
    pub(in crate::v9) fn wait(
        &self,
        offset: usize,
        expected: &[u8],
        timeout: Option<Duration>,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<WaitResult, ()> {
        let signal = Arc::new(Signal {
            notified: Mutex::new(false),
            wake: Condvar::new(),
        });
        {
            let mut state = self.lock();
            if state.read(offset, expected.len()).as_deref() != Some(expected) {
                return Ok(WaitResult::NotEqual);
            }
            if timeout == Some(Duration::ZERO) {
                return Ok(WaitResult::TimedOut);
            }
            state.waiters.push(Entry {
                offset,
                signal: signal.clone(),
                asynchronous: None,
            });
        }
        let result = wait_signal(&signal, timeout, &mut cancelled);
        self.lock()
            .waiters
            .retain(|entry| !Arc::ptr_eq(&entry.signal, &signal));
        result
    }
    /** Local callback identities can be queued only by their owning VM; other agents merely set wake flags. */
    pub(in crate::v9) fn notify_callbacks(
        &self,
        offset: usize,
        count: usize,
    ) -> (usize, Vec<crate::v9::value::ObjectId>) {
        let mut state = self.lock();
        let mut notified = 0;
        let mut callbacks = Vec::new();
        state.waiters.retain(|entry| {
            let mut signalled = entry
                .signal
                .notified
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if *signalled || entry.offset != offset || notified >= count {
                return true;
            }
            *signalled = true;
            entry.signal.wake.notify_one();
            notified += 1;
            if let Some(data) = &entry.asynchronous {
                if data.owner == self.agent {
                    callbacks.push(data.callback)
                }
                true
            } else {
                false
            }
        });
        if notified > 0 {
            state.revision = state.revision.wrapping_add(1)
        }
        (notified, callbacks)
    }
}
/** Spurious OS wakeups never become JS notifications; elapsed time and cancellation remain distinct. */
fn wait_signal(
    signal: &Signal,
    timeout: Option<Duration>,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<WaitResult, ()> {
    let started = Instant::now();
    let mut notified = signal
        .notified
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    loop {
        if *notified {
            return Ok(WaitResult::Ok);
        }
        drop(notified);
        if cancelled() {
            return Err(());
        }
        notified = signal
            .notified
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if *notified {
            return Ok(WaitResult::Ok);
        }
        let remaining = timeout.map(|timeout| timeout.saturating_sub(started.elapsed()));
        if remaining == Some(Duration::ZERO) {
            return Ok(WaitResult::TimedOut);
        }
        let pause = remaining.map_or(CANCELLATION_POLL, |remaining| {
            remaining.min(CANCELLATION_POLL)
        });
        notified = signal
            .wake
            .wait_timeout(notified, pause)
            .unwrap_or_else(|error| error.into_inner())
            .0;
    }
}
