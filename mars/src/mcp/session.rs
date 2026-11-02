/** A pure bounded session owns lifecycle and correlation while the desktop owns execution and cleanup. */
mod envelopes;
mod lifecycle;
mod native;
mod tools;
mod waits;
use super::wire::ids::IdKey;
use super::{
    Action,
    wire::{self, ExternalId},
};
use crate::{
    constants::{
        MCP_FRAME_BYTES, MCP_INITIALIZE_DEADLINE, MCP_REQUEST_DEADLINE, MCP_SESSION_REQUESTS,
    },
    protocol::Response,
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};
/** Initialization notification is distinct from a successful initialize response. */
#[derive(PartialEq, Eq)]
enum Phase {
    Fresh,
    AwaitingInitialized,
    Ready,
    Closed,
}
/** Only correlation and cancellation metadata survive dispatch; typed payloads/passwords do not. */
struct Pending {
    external: ExternalId,
    operation: String,
    tab_id: Option<String>,
    control_epoch: Option<u64>,
    admitted: Instant,
    cancelled: bool,
    /// A page_wait keeps its condition so the session can re-check it between human actions.
    wait: Option<waits::Plan>,
}
/// A connection's state transitions and deadlines are deterministic over the caller-supplied clock.
pub struct Session {
    phase: Phase,
    started: Instant,
    seen: BTreeSet<IdKey>,
    pending: BTreeMap<String, Pending>,
    /// Unsatisfied waits between checks; they hold no native slot and no worker time.
    waiting: Vec<waits::Waiting>,
    next_native: u64,
}
impl Session {
    /// Start the initialization deadline without creating browser state or acquiring control.
    pub fn new(now: Instant) -> Self {
        Self {
            phase: Phase::Fresh,
            started: now,
            seen: BTreeSet::new(),
            pending: BTreeMap::new(),
            waiting: Vec::new(),
            next_native: 0,
        }
    }
    /// Consume exactly one complete bounded message; notifications never fabricate response IDs.
    pub fn receive(&mut self, frame: &[u8], now: Instant) -> Vec<Action> {
        let expired = self.poll(now);
        if self.phase == Phase::Closed {
            return expired;
        }
        if frame.len() > MCP_FRAME_BYTES {
            return self.close("The MCP input frame exceeded its limit.");
        }
        match wire::decode(frame) {
            Ok(decoded) => self.envelope(decoded, now),
            Err(()) => self.send(
                wire::error(None, wire::PARSE_ERROR, "Invalid JSON message."),
                None,
            ),
        }
    }
    /// Complete one exact native correlation after its visible UI update has been acknowledged.
    pub fn complete(&mut self, response: Response, now: Instant) -> Vec<Action> {
        let expired = self.poll(now);
        if self.phase == Phase::Closed {
            return expired;
        }
        self.native_response(response, now)
    }
    /// Expiry ends the connection rather than inventing the outcome of admitted native work.
    pub fn poll(&mut self, now: Instant) -> Vec<Action> {
        if self.phase == Phase::Closed {
            return Vec::new();
        }
        if self.phase != Phase::Ready
            && now.saturating_duration_since(self.started) >= MCP_INITIALIZE_DEADLINE
        {
            return self.close("The MCP initialization deadline expired.");
        }
        if self
            .pending
            .values()
            .any(|pending| now.saturating_duration_since(pending.admitted) >= MCP_REQUEST_DEADLINE)
        {
            return self.close("The MCP native request deadline expired.");
        }
        self.recheck(now)
    }
    /** Duplicate or exhausted whole-session correlation retires the session without an ambiguous reply. */
    fn remember(&mut self, id: &ExternalId) -> Option<Vec<Action>> {
        if self.seen.contains(&id.key) {
            return Some(self.close("An MCP request ID was reused."));
        }
        if self.seen.len() >= MCP_SESSION_REQUESTS {
            return Some(self.close("The MCP session request limit was reached."));
        }
        self.seen.insert(id.key.clone());
        None
    }
    /** Every outgoing value fits completely or becomes a bounded correlated internal-error response. */
    fn send(&mut self, value: Value, id: Option<&Value>) -> Vec<Action> {
        if wire::encode(&value).is_ok() {
            return vec![Action::Send(value)];
        }
        let fallback = wire::error(
            id,
            wire::INTERNAL_ERROR,
            "The MCP response exceeded its output limit.",
        );
        if wire::encode(&fallback).is_ok() {
            vec![Action::Send(fallback)]
        } else {
            self.close("The MCP response could not be encoded.")
        }
    }
    /** A terminal session cannot emit later replies, dispatch or cancellation actions. */
    fn close(&mut self, reason: &'static str) -> Vec<Action> {
        if self.phase == Phase::Closed {
            return Vec::new();
        }
        self.phase = Phase::Closed;
        self.pending.clear();
        self.waiting.clear();
        self.seen.clear();
        vec![Action::Close(reason)]
    }
}
