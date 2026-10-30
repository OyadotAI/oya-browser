/** Native close decisions await worker acknowledgements while AppKit keeps processing events. */
use super::*;
use crate::desktop::broker::SaveTicket;
use objc2_app_kit::{NSAlert, NSAlertSecondButtonReturn, NSAlertThirdButtonReturn};
use std::time::{Duration, Instant};

/** A stalled save becomes an explicit recovery choice, never an assumed success. */
const SAVE_DEADLINE: Duration = Duration::from_secs(10);

/** Each close attempt owns its receiver so late results cannot close a reopened window. */
pub(super) enum CloseState {
    /** Ordinary native input is admitted. */
    Idle,
    /** Wait for authority invalidation without blocking the main thread on its mutex. */
    Admitting(Instant),
    /** The runtime owns persistence; the UI only polls this attempt's acknowledgement. */
    Saving(SaveTicket, Instant),
}
impl Window {
    /** Request closure once; repeated close events cannot enqueue duplicate persistence. */
    pub(super) fn request_close(&mut self) {
        if self.closed || !matches!(self.closing, CloseState::Idle) {
            return;
        }
        self.pending.clear();
        self.canvas.clear_actions();
        if self.broker.transport == Transport::Mcp {
            let _ = self.broker.begin_close();
            self.broker.cancel.store(true, Ordering::Release);
            self.closed = true;
            return;
        }
        self.closing = CloseState::Admitting(Instant::now());
    }
    /** Drain worker events before polling, then suppress new page actions during closure. */
    pub(super) fn poll_close(&mut self) -> bool {
        let state = std::mem::replace(&mut self.closing, CloseState::Idle);
        match state {
            CloseState::Idle => return false,
            CloseState::Admitting(started) => self.admit_close(started),
            CloseState::Saving(ticket, started) => match ticket.poll() {
                Some(true) => self.closed = true,
                Some(false) => self.close_failed(),
                None if started.elapsed() >= SAVE_DEADLINE => {
                    drop(ticket);
                    self.close_failed();
                }
                None => self.closing = CloseState::Saving(ticket, started),
            },
        }
        true
    }
    /** Revoke stale permits before saving; an unavailable lock has the same bounded recovery. */
    fn admit_close(&mut self, started: Instant) {
        if !self.broker.begin_close() {
            if started.elapsed() >= SAVE_DEADLINE {
                self.close_failed();
            } else {
                self.closing = CloseState::Admitting(started);
            }
            return;
        }
        if let Some(ticket) = self.broker.save_session() {
            self.closing = CloseState::Saving(ticket, started);
        } else {
            self.close_failed();
        }
    }
    /** Only an acknowledged save or an explicit discard commits the close decision. */
    fn close_failed(&mut self) {
        let alert = NSAlert::new(self.canvas.mtm());
        alert.setMessageText(&NSString::from_str("Your open tabs couldn’t be saved"));
        alert.setInformativeText(&NSString::from_str(
            "Saving failed or did not finish in time. Closing without saving may leave an older tab session for next time.",
        ));
        alert.addButtonWithTitle(&NSString::from_str("Keep Mars Open"));
        alert.addButtonWithTitle(&NSString::from_str("Try Again"));
        alert.addButtonWithTitle(&NSString::from_str("Close Without Saving"));
        let response = alert.runModal();
        if response == NSAlertSecondButtonReturn {
            self.closing = CloseState::Admitting(Instant::now());
        } else if response == NSAlertThirdButtonReturn {
            self.broker.discard_session();
            self.closed = true;
        } else {
            self.window.makeFirstResponder(Some(&self.mars.address));
        }
    }
}
