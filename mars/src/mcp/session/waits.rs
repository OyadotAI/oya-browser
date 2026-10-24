/** page_wait re-checks its condition between other work instead of holding the native worker. */
use super::*;
use crate::{constants::MCP_PENDING_REQUESTS, protocol::Request, runtime::WAIT_MAX_MS};
use serde_json::json;
use std::time::Duration;

/** Wait applied when the caller names no timeout. */
const WAIT_DEFAULT_MS: u64 = 5_000;
/** Pause between checks; small next to a person's action, large next to a native check. */
const RECHECK: Duration = Duration::from_millis(50);

/** The condition and its absolute deadline; the request is a small read-only query. */
pub(super) struct Plan {
    params: Value,
    deadline: Instant,
}

/** One unsatisfied wait parked until its next check. */
pub(super) struct Waiting {
    /// Correlation and the plan travel together into the next dispatch.
    pub pending: Pending,
    next: Instant,
    last: Value,
}

/** Only page.wait gets a plan; the timeout is capped by the same bound the native side checks. */
pub(super) fn plan(request: &Request, now: Instant) -> Option<Plan> {
    if request.op != "page.wait" {
        return None;
    }
    let timeout = request
        .params
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or(WAIT_DEFAULT_MS)
        .min(WAIT_MAX_MS);
    Some(Plan {
        params: request.params.clone(),
        deadline: now + Duration::from_millis(timeout),
    })
}

impl Session {
    /**
     * Park an unsatisfied wait that still has time left and return None. Anything else
     * (satisfied, timed out, not a wait) is handed back to be answered now.
     */
    pub(super) fn hold(
        &mut self,
        pending: Pending,
        result: Value,
        now: Instant,
    ) -> Option<(Pending, Value)> {
        let Some(plan) = &pending.wait else {
            return Some((pending, result));
        };
        if result["satisfied"] != json!(false) {
            return Some((pending, result));
        }
        if now >= plan.deadline {
            let mut result = result;
            result["timed_out"] = json!(true);
            return Some((pending, result));
        }
        self.waiting.push(Waiting {
            pending,
            next: now + RECHECK,
            last: result,
        });
        None
    }
    /** Due waits are dispatched again under fresh private IDs; expired ones are answered. */
    pub(super) fn recheck(&mut self, now: Instant) -> Vec<Action> {
        let mut actions = Vec::new();
        let mut index = 0;
        while index < self.waiting.len() {
            let waiting = &self.waiting[index];
            let expired = waiting
                .pending
                .wait
                .as_ref()
                .is_some_and(|plan| now >= plan.deadline);
            if !expired && (waiting.next > now || self.pending.len() >= MCP_PENDING_REQUESTS) {
                index += 1;
                continue;
            }
            let waiting = self.waiting.remove(index);
            if expired {
                let mut result = waiting.last;
                result["timed_out"] = json!(true);
                actions.extend(self.answer(&waiting.pending, result));
            } else {
                actions.push(self.redispatch(waiting.pending, now));
            }
        }
        actions
    }
    /** A re-check is a new native request; the external ID stays the caller's. */
    fn redispatch(&mut self, mut pending: Pending, now: Instant) -> Action {
        let native_id = format!("mcp-{}", self.next_native);
        self.next_native += 1;
        let params = pending
            .wait
            .as_ref()
            .map(|plan| plan.params.clone())
            .unwrap_or_default();
        let request = Request::new(&native_id, &pending.operation, params);
        pending.admitted = now;
        self.pending.insert(native_id, pending);
        Action::Dispatch(request)
    }
    /** Send one wait's final tool result. */
    fn answer(&mut self, pending: &Pending, result: Value) -> Vec<Action> {
        match crate::mcp::catalog::tool_result(Ok(result)) {
            Ok(result) => self.send(
                wire::result(&pending.external.value, result),
                Some(&pending.external.value),
            ),
            Err(_) => self.close("The MCP native result was invalid."),
        }
    }
}
