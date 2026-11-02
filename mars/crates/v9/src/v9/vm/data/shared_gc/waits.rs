/** Shared arena synchronization waits preserve registration, notification and cancellation order. */
use super::*;
use std::sync::Condvar;
use std::time::{Duration, Instant};

const MAX_WAITERS: usize = 4096;
const CANCELLATION_POLL: Duration = Duration::from_millis(5);

/** Distinguish cancellation, exhausted queues and invalid ownership without fabricating timeouts. */
#[derive(Clone, Copy)]
pub(in crate::v9) enum WaitFailure {
    Cancelled,
    Limit,
    Invalid,
}

#[derive(Clone)]
pub(super) struct Waiter {
    node: u64,
    signal: Arc<Signal>,
}
struct Signal {
    notified: Mutex<bool>,
    wake: Condvar,
}

/** Wake only the requested node's actual registered waiters. */
fn notify(state: &mut State, node: u64, count: usize) -> usize {
    let mut notified = 0;
    state.waiters.retain(|entry| {
        if entry.node != node || notified >= count {
            return true;
        }
        let mut signalled = entry
            .signal
            .notified
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if *signalled {
            return true;
        }
        *signalled = true;
        entry.signal.wake.notify_one();
        notified += 1;
        false
    });
    notified
}
/** OS wakeups remain distinct from notification, timeout and worker cancellation. */
fn wait_signal(
    signal: &Signal,
    timeout: Option<Duration>,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<bool, ()> {
    let started = Instant::now();
    let mut notified = signal
        .notified
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    loop {
        if *notified {
            return Ok(true);
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
            return Ok(true);
        }
        let remaining = timeout.map(|timeout| timeout.saturating_sub(started.elapsed()));
        if remaining == Some(Duration::ZERO) {
            return Ok(false);
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

impl SharedGc {
    /** Wait while a mutex still has the observed owner, with atomic comparison and registration. */
    pub(in crate::v9) fn wait_owner(
        &self,
        expected: i128,
        timeout: Option<Duration>,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<Option<bool>, WaitFailure> {
        let signal = Arc::new(Signal {
            notified: Mutex::new(false),
            wake: Condvar::new(),
        });
        let registered = super::super::SharedBytes::ordered(|| {
            let mut state = self.root.arena.lock();
            if state
                .records
                .get(&self.node())
                .and_then(|r| r.values.first())
                != Some(&SharedGcValue::BigInt(expected))
            {
                return Some(false);
            }
            if state.waiters.len() >= MAX_WAITERS {
                return None;
            }
            state.waiters.push(Waiter {
                node: self.node(),
                signal: signal.clone(),
            });
            Some(true)
        });
        match registered {
            None => return Err(WaitFailure::Limit),
            Some(false) => return Ok(None),
            Some(true) => (),
        }
        let result = wait_signal(&signal, timeout, &mut cancelled);
        self.root
            .arena
            .lock()
            .waiters
            .retain(|entry| !Arc::ptr_eq(&entry.signal, &signal));
        result.map(Some).map_err(|()| WaitFailure::Cancelled)
    }
    /** Register on the condition and release the mutex in one ordered native event. */
    pub(in crate::v9) fn condition_wait(
        &self,
        mutex: &SharedGc,
        owner: i128,
        timeout: Option<Duration>,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<bool, WaitFailure> {
        if !self.arena().same_store(&mutex.arena()) {
            return Err(WaitFailure::Invalid);
        }
        let signal = Arc::new(Signal {
            notified: Mutex::new(false),
            wake: Condvar::new(),
        });
        let registered = super::super::SharedBytes::ordered(|| {
            let mut state = self.root.arena.lock();
            if state.waiters.len() >= MAX_WAITERS {
                return Err(WaitFailure::Limit);
            }
            if state
                .records
                .get(&mutex.node())
                .and_then(|r| r.values.first())
                != Some(&SharedGcValue::BigInt(owner))
            {
                return Err(WaitFailure::Invalid);
            }
            state.waiters.push(Waiter {
                node: self.node(),
                signal: signal.clone(),
            });
            state.revision += 1;
            let revision = state.revision;
            let record = state
                .records
                .get_mut(&mutex.node())
                .expect("mutex validated");
            record.values[0] = SharedGcValue::BigInt(0);
            record.version = revision;
            notify(&mut state, mutex.node(), 1);
            Ok(())
        });
        registered?;
        let result = wait_signal(&signal, timeout, &mut cancelled);
        self.root
            .arena
            .lock()
            .waiters
            .retain(|entry| !Arc::ptr_eq(&entry.signal, &signal));
        result.map_err(|()| WaitFailure::Cancelled)
    }
    /** Compare a shared struct field and register on this queue under the same arena lock. */
    pub(in crate::v9) fn wait_field(
        &self,
        object: &SharedGc,
        index: usize,
        expected: i32,
        timeout: Option<Duration>,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<Option<bool>, WaitFailure> {
        if !self.arena().same_store(&object.arena()) {
            return Err(WaitFailure::Invalid);
        }
        let signal = Arc::new(Signal {
            notified: Mutex::new(false),
            wake: Condvar::new(),
        });
        let registered = super::super::SharedBytes::ordered(|| {
            let mut state = self.root.arena.lock();
            let Some(value) = state
                .records
                .get(&object.node())
                .and_then(|r| r.values.get(index))
            else {
                return Err(WaitFailure::Invalid);
            };
            if value != &SharedGcValue::Number((expected as f64).to_bits()) {
                return Ok(false);
            }
            if state.waiters.len() >= MAX_WAITERS {
                return Err(WaitFailure::Limit);
            }
            state.waiters.push(Waiter {
                node: self.node(),
                signal: signal.clone(),
            });
            Ok(true)
        })?;
        if !registered {
            return Ok(None);
        }
        let result = wait_signal(&signal, timeout, &mut cancelled);
        self.root
            .arena
            .lock()
            .waiters
            .retain(|entry| !Arc::ptr_eq(&entry.signal, &signal));
        result.map(Some).map_err(|()| WaitFailure::Cancelled)
    }
    /** Notification and queue counts read actual arena waiter entries. */
    pub(in crate::v9) fn notify_waiters(&self, count: usize) -> usize {
        notify(&mut self.root.arena.lock(), self.node(), count)
    }
    pub(in crate::v9) fn waiter_count(&self) -> usize {
        self.root
            .arena
            .lock()
            .waiters
            .iter()
            .filter(|entry| entry.node == self.node())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_wait_notifies_only_its_queue() {
        let arena = SharedGcArena::new().unwrap();
        let queue = arena.allocate(b"queue", 0, false, vec![]).unwrap();
        let other = arena.allocate(b"queue", 0, false, vec![]).unwrap();
        let object = arena
            .allocate(
                b"object",
                0,
                false,
                vec![SharedGcValue::Number((7.0f64).to_bits())],
            )
            .unwrap();
        let waiting = queue.clone();
        let worker = std::thread::spawn(move || {
            waiting
                .wait_field(&object, 0, 7, Some(Duration::from_secs(2)), || false)
                .ok()
        });
        let deadline = Instant::now() + Duration::from_secs(1);
        while queue.waiter_count() == 0 && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(other.notify_waiters(1), 0);
        assert_eq!(queue.notify_waiters(1), 1);
        assert!(matches!(worker.join().unwrap(), Some(Some(true))));
        assert_eq!(queue.waiter_count(), 0);
    }

    #[test]
    fn field_wait_compares_before_registering_and_times_out() {
        let arena = SharedGcArena::new().unwrap();
        let queue = arena.allocate(b"queue", 0, false, vec![]).unwrap();
        let object = arena
            .allocate(
                b"object",
                0,
                false,
                vec![SharedGcValue::Number((42.0f64).to_bits())],
            )
            .unwrap();
        assert!(matches!(
            queue.wait_field(&object, 0, 41, Some(Duration::ZERO), || false),
            Ok(None)
        ));
        assert!(matches!(
            queue.wait_field(&object, 0, 42, Some(Duration::ZERO), || false),
            Ok(Some(false))
        ));
        assert_eq!(queue.waiter_count(), 0);
        let other = SharedGcArena::new()
            .unwrap()
            .allocate(b"object", 0, false, vec![])
            .unwrap();
        assert!(matches!(
            queue.wait_field(&other, 0, 42, Some(Duration::ZERO), || false),
            Err(WaitFailure::Invalid)
        ));
    }
}
