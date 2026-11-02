/** Async futex tickets carry inert local callback identities, never foreign VM objects or executable closures. */
use super::*;
use crate::v9::value::ObjectId;
use serde::{Deserialize, Serialize};

/** Native waiter queues remain bounded independently of byte storage and JS timer quotas. */
pub(in crate::v9) const MAX_WAITERS: usize = 4096;
/** Tickets fit exactly in the engine's binary64 captured-value representation. */
const MAX_TICKET: u64 = (1u64 << f64::MANTISSA_DIGITS) - 1;

/** A wire ticket belongs only to the agent whose wrapper is being serialized. */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AsyncSnapshot {
    ticket: u64,
    offset: usize,
    callback: ObjectId,
    notified: bool,
}
/** Other agents may notify this future but cannot execute its callback identity. */
pub(super) struct AsyncData {
    pub(super) ticket: u64,
    pub(super) owner: u64,
    pub(super) callback: ObjectId,
}
/** Registration can finish synchronously without ever exposing a pending promise. */
pub(in crate::v9) enum AsyncWait {
    Immediate(WaitResult),
    Pending(u64),
}

/** Deserialization reconstructs only inert local future state, with actual bounded callback references. */
pub(super) fn restore(
    rows: &[AsyncSnapshot],
    owner: u64,
    length: usize,
) -> EngineResult<(Vec<waits::Entry>, u64)> {
    if rows.len() > MAX_WAITERS {
        return Err(EngineError::limit("V9 shared waiters", MAX_WAITERS));
    }
    let mut tickets = std::collections::BTreeSet::new();
    let mut next = 1;
    let mut entries = Vec::new();
    for row in rows {
        if row.ticket == 0
            || row.ticket > MAX_TICKET
            || !tickets.insert(row.ticket)
            || row
                .offset
                .checked_add(std::mem::size_of::<i32>())
                .is_none_or(|end| end > length)
        {
            return Err(EngineError::invalid("Invalid async shared waiter."));
        }
        next = next.max(row.ticket + 1);
        entries.push(waits::Entry {
            offset: row.offset,
            signal: Arc::new(waits::Signal {
                notified: Mutex::new(row.notified),
                wake: std::sync::Condvar::new(),
            }),
            asynchronous: Some(AsyncData {
                ticket: row.ticket,
                owner,
                callback: row.callback,
            }),
        });
    }
    Ok((entries, next))
}
/** Consistent snapshot cuts include local future completion state along with the same revision's bytes. */
pub(super) fn snapshots(state: &State, owner: u64) -> Vec<AsyncSnapshot> {
    state
        .waiters
        .iter()
        .filter_map(|entry| {
            entry
                .asynchronous
                .as_ref()
                .filter(|data| data.owner == owner)
                .map(|data| AsyncSnapshot {
                    ticket: data.ticket,
                    offset: entry.offset,
                    callback: data.callback,
                    notified: *entry
                        .signal
                        .notified
                        .lock()
                        .unwrap_or_else(|error| error.into_inner()),
                })
        })
        .collect()
}

impl SharedBytes {
    /** A receiving VM chooses its own callback namespace without changing shared byte identity. */
    pub(in crate::v9) fn for_agent(&self, agent: u64) -> Self {
        Self {
            store: self.store.clone(),
            restored: self.restored,
            visible: self.visible,
            logical_maximum: self.logical_maximum,
            agent,
        }
    }
    /** A pending future retains its genuine promise callback as an ordinary edge in its own VM. */
    pub(in crate::v9) fn async_callbacks(&self) -> Vec<ObjectId> {
        self.lock()
            .waiters
            .iter()
            .filter_map(|entry| {
                entry
                    .asynchronous
                    .as_ref()
                    .filter(|data| data.owner == self.agent)
                    .map(|data| data.callback)
            })
            .collect()
    }
    /** State validation ties each ticket to the native callback's exact backing and captured ticket. */
    pub(in crate::v9) fn async_tickets(&self) -> Vec<(u64, ObjectId)> {
        self.lock()
            .waiters
            .iter()
            .filter_map(|entry| {
                entry
                    .asynchronous
                    .as_ref()
                    .filter(|data| data.owner == self.agent)
                    .map(|data| (data.ticket, data.callback))
            })
            .collect()
    }
    /** Compare and registration occur under one lock, just like blocking wait. */
    pub(in crate::v9) fn register_async(
        &self,
        offset: usize,
        expected: &[u8],
        zero_timeout: bool,
        callback: ObjectId,
    ) -> EngineResult<AsyncWait> {
        let mut state = self.lock();
        if state.read(offset, expected.len()).as_deref() != Some(expected) {
            return Ok(AsyncWait::Immediate(WaitResult::NotEqual));
        }
        if zero_timeout {
            return Ok(AsyncWait::Immediate(WaitResult::TimedOut));
        }
        if state.waiters.len() >= MAX_WAITERS || state.next_ticket >= MAX_TICKET {
            return Err(EngineError::limit("V9 shared waiters", MAX_WAITERS));
        }
        let ticket = state.next_ticket;
        state.next_ticket += 1;
        state.waiters.push(waits::Entry {
            offset,
            signal: Arc::new(waits::Signal {
                notified: Mutex::new(false),
                wake: std::sync::Condvar::new(),
            }),
            asynchronous: Some(AsyncData {
                ticket,
                owner: self.agent,
                callback,
            }),
        });
        state.revision = state.revision.wrapping_add(1);
        Ok(AsyncWait::Pending(ticket))
    }
    /** Poll atomically distinguishes an actual notification, elapsed deadline, pending state or consumed ticket. */
    pub(in crate::v9) fn poll_async(
        &self,
        ticket: u64,
        timed_out: bool,
    ) -> (bool, Option<WaitResult>) {
        let mut state = self.lock();
        let Some(index) = state.waiters.iter().position(|entry| {
            entry
                .asynchronous
                .as_ref()
                .is_some_and(|data| data.ticket == ticket && data.owner == self.agent)
        }) else {
            return (false, None);
        };
        let notified = *state.waiters[index]
            .signal
            .notified
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if !notified && !timed_out {
            return (true, None);
        }
        state.waiters.remove(index);
        state.revision = state.revision.wrapping_add(1);
        (
            true,
            Some(if notified {
                WaitResult::Ok
            } else {
                WaitResult::TimedOut
            }),
        )
    }
    /** Worker shutdown removes only that agent's asynchronous futures, preserving other agents' waiters. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn cancel_agent(&self, agent: u64) {
        let mut state = self.lock();
        let before = state.waiters.len();
        state.waiters.retain(|entry| {
            entry
                .asynchronous
                .as_ref()
                .is_none_or(|data| data.owner != agent)
        });
        if state.waiters.len() != before {
            state.revision = state.revision.wrapping_add(1);
        }
    }
}
