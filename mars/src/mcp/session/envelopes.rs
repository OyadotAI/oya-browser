/** Strict envelope classification separates requests, notifications and unsolicited client responses. */
use super::*;
use serde_json::Map;
impl Session {
    /** Only unambiguous valid external IDs can appear in a protocol-error response. */
    pub(super) fn envelope(&mut self, decoded: wire::Decoded, now: Instant) -> Vec<Action> {
        let Some(object) = decoded.value.as_object() else {
            return self.send(
                wire::error(
                    None,
                    wire::INVALID_REQUEST,
                    "Expected one JSON-RPC request object.",
                ),
                None,
            );
        };
        if !object.contains_key("id") && object.contains_key("method") {
            return if decoded.duplicate {
                Vec::new()
            } else {
                self.notification(object, decoded.raw_cancel_id.as_deref())
            };
        }
        let id = if decoded.duplicate_id {
            None
        } else {
            object
                .get("id")
                .and_then(|id| ExternalId::parse(id, decoded.raw_id.as_deref()))
        };
        if valid_response(object, id.as_ref()) && !decoded.duplicate {
            return Vec::new();
        }
        if let Some(id) = &id
            && let Some(actions) = self.remember(id)
        {
            return actions;
        }
        if decoded.duplicate || !valid_request(object) || id.is_none() {
            let id = id.as_ref().map(|id| &id.value);
            return self.send(
                wire::error(
                    id,
                    wire::INVALID_REQUEST,
                    "Invalid JSON-RPC request envelope.",
                ),
                id,
            );
        }
        let id = id.expect("validated external ID");
        self.request(
            &id,
            object["method"].as_str().expect("validated method"),
            object.get("params"),
            now,
        )
    }
    /** Invalid and unknown notifications are always ignored rather than acknowledged. */
    fn notification(
        &mut self,
        object: &Map<String, Value>,
        raw_cancel_id: Option<&str>,
    ) -> Vec<Action> {
        if !only(object, &["jsonrpc", "method", "params"])
            || object.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        {
            return Vec::new();
        }
        match object.get("method").and_then(Value::as_str) {
            Some("notifications/initialized")
                if object.get("params").is_none()
                    || object_params(object.get("params"), &["_meta"]).is_some() =>
            {
                if self.phase == Phase::AwaitingInitialized {
                    self.phase = Phase::Ready;
                }
                Vec::new()
            }
            Some("notifications/cancelled") => self.cancel(object.get("params"), raw_cancel_id),
            _ => Vec::new(),
        }
    }
}
/** Unknown fields never become a second dispatch or hidden task augmentation path. */
pub(super) fn only(object: &Map<String, Value>, allowed: &[&str]) -> bool {
    object.keys().all(|key| allowed.contains(&key.as_str()))
}
/** Known request parameters must be strict objects with optional object-valued MCP metadata. */
pub(super) fn object_params<'a>(
    params: Option<&'a Value>,
    allowed: &[&str],
) -> Option<&'a Map<String, Value>> {
    let object = params?.as_object()?;
    (only(object, allowed) && object.get("_meta").is_none_or(Value::is_object)).then_some(object)
}
/** Requests always carry a method and ID; parameter schemas are validated by their method handlers. */
fn valid_request(object: &Map<String, Value>) -> bool {
    only(object, &["jsonrpc", "id", "method", "params"])
        && object.get("jsonrpc").and_then(Value::as_str) == Some("2.0")
        && object.get("method").is_some_and(Value::is_string)
        && object.contains_key("id")
}
/** No server-originated requests are issued, so valid client responses have nothing to complete. */
fn valid_response(object: &Map<String, Value>, id: Option<&ExternalId>) -> bool {
    if id.is_none()
        || object.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || !only(object, &["jsonrpc", "id", "result", "error"])
    {
        return false;
    }
    match (object.get("result"), object.get("error")) {
        (Some(_), None) => true,
        (None, Some(Value::Object(error))) => {
            only(error, &["code", "message", "data"])
                && error
                    .get("code")
                    .is_some_and(|code| code.as_i64().is_some())
                && error.get("message").is_some_and(Value::is_string)
        }
        _ => false,
    }
}
