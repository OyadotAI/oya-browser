/** Native completion and cancellation correlate through private IDs and exact immutable authority metadata. */
use super::{envelopes::object_params, *};
use crate::{constants::VERSION, mcp::catalog, runtime::mutating};
impl Session {
    /** Correlation faults terminate rather than misreporting a different request's native outcome. */
    pub(super) fn native_response(&mut self, response: Response, now: Instant) -> Vec<Action> {
        if response.version != VERSION || response.result.is_some() == response.error.is_some() {
            return self.close("The MCP native response envelope was invalid.");
        }
        let Some(native_id) = response.id else {
            return self.close("The MCP native response had no correlation.");
        };
        let Some(mut pending) = self.pending.remove(&native_id) else {
            return self.close("The MCP native response correlation was invalid.");
        };
        let fatal = response
            .error
            .as_ref()
            .is_some_and(|error| error.service.is_some());
        if pending.cancelled {
            return if fatal {
                self.close("The MCP native engine disconnected.")
            } else {
                Vec::new()
            };
        }
        let result = match (response.result, response.error) {
            (Some(result), None) => match self.hold(pending, result, now) {
                None => return Vec::new(),
                Some((held, result)) => {
                    pending = held;
                    Ok(result)
                }
            },
            (None, Some(error)) => Err(error),
            _ => unreachable!("validated response exclusivity"),
        };
        let mut actions = match catalog::tool_result(result) {
            Ok(result) => self.send(
                wire::result(&pending.external.value, result),
                Some(&pending.external.value),
            ),
            Err(_) => {
                let mut actions = self.send(
                    wire::error(
                        Some(&pending.external.value),
                        wire::INTERNAL_ERROR,
                        "The MCP native result was invalid.",
                    ),
                    Some(&pending.external.value),
                );
                actions.extend(self.close("The MCP native result was invalid."));
                return actions;
            }
        };
        if fatal {
            actions.extend(self.close("The MCP native engine disconnected."));
        }
        actions
    }
    /** Notifications acknowledge nothing; cancellation never claims to undo committed effects. */
    pub(super) fn cancel(&mut self, params: Option<&Value>, raw_id: Option<&str>) -> Vec<Action> {
        let Some(params) = object_params(params, &["requestId", "reason", "_meta"]) else {
            return Vec::new();
        };
        if params
            .get("reason")
            .is_some_and(|reason| !reason.is_string())
        {
            return Vec::new();
        }
        let Some(id) = params
            .get("requestId")
            .and_then(|id| ExternalId::parse(id, raw_id))
        else {
            return Vec::new();
        };
        let before = self.waiting.len();
        self.waiting
            .retain(|waiting| waiting.pending.external.key != id.key);
        if self.waiting.len() != before {
            return Vec::new();
        }
        let Some((native_id, pending)) = self
            .pending
            .iter_mut()
            .find(|(_, pending)| pending.external.key == id.key && !pending.cancelled)
        else {
            return Vec::new();
        };
        pending.cancelled = true;
        vec![Action::Cancel {
            native_id: native_id.clone(),
            tab_id: pending.tab_id.clone(),
            control_epoch: pending.control_epoch,
            mutating: mutating(&pending.operation),
        }]
    }
}
