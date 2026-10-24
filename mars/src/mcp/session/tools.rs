/** The protocol wrapper admits only the fixed catalog and retains no tool argument payloads. */
use super::{envelopes::object_params, *};
use crate::{
    constants::{ID_BYTES, MCP_PENDING_REQUESTS},
    mcp::catalog::{self, TranslationError},
};
use serde_json::json;
impl Session {
    /** The first catalog is fixed and unpaginated; a nonempty cursor cannot imply missing tools. */
    pub(super) fn list_tools(&mut self, id: &ExternalId, params: Option<&Value>) -> Vec<Action> {
        let Some(params) = object_params(params, &["cursor", "_meta"]) else {
            return self.invalid_params(id);
        };
        if params
            .get("cursor")
            .is_some_and(|cursor| cursor.as_str() != Some(""))
        {
            return self.invalid_params(id);
        }
        self.send(
            wire::result(&id.value, json!({"tools":catalog::tools()})),
            Some(&id.value),
        )
    }
    /** Translation preserves supplied preconditions; neither readiness nor discovery grants control. */
    pub(super) fn call_tool(
        &mut self,
        id: &ExternalId,
        params: Option<&Value>,
        now: Instant,
    ) -> Vec<Action> {
        let Some(params) = object_params(params, &["name", "arguments", "_meta"]) else {
            return self.invalid_params(id);
        };
        let Some(name) = params
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty() && name.len() <= ID_BYTES)
        else {
            return self.invalid_params(id);
        };
        let empty = json!({});
        let arguments = params.get("arguments").unwrap_or(&empty);
        if !arguments.is_object() {
            return self.invalid_params(id);
        }
        let native_id = format!("mcp-{}", self.next_native);
        match catalog::translate(name, arguments, &native_id) {
            Ok(request) => self.admit(id, request, now),
            Err(TranslationError::UnknownTool) => self.send(
                wire::error(
                    Some(&id.value),
                    wire::INVALID_PARAMS,
                    "Unknown tool. Call tools/list for supported native tools.",
                ),
                Some(&id.value),
            ),
            Err(TranslationError::InvalidArguments(error)) => {
                match catalog::tool_result(Err(error)) {
                    Ok(result) => self.send(wire::result(&id.value, result), Some(&id.value)),
                    Err(_) => self.close("The MCP tool error could not be encoded."),
                }
            }
        }
    }
    /** Cancelled calls continue to occupy pending slots until the corresponding native reply is drained. */
    fn admit(
        &mut self,
        id: &ExternalId,
        request: crate::protocol::Request,
        now: Instant,
    ) -> Vec<Action> {
        if self.pending.len() + self.waiting.len() >= MCP_PENDING_REQUESTS {
            return self.send(
                wire::error(
                    Some(&id.value),
                    wire::SESSION_NOT_READY,
                    "The MCP native request queue is full.",
                ),
                Some(&id.value),
            );
        }
        self.next_native += 1;
        self.pending.insert(
            request.id.clone(),
            Pending {
                external: id.clone(),
                operation: request.op.clone(),
                tab_id: request
                    .params
                    .get("tab_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                control_epoch: request.params.get("control_epoch").and_then(Value::as_u64),
                admitted: now,
                cancelled: false,
                wait: super::waits::plan(&request, now),
            },
        );
        vec![Action::Dispatch(request)]
    }
}
