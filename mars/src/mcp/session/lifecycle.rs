/** MCP initialization negotiates only the documented pin and grants no native browser authority. */
use super::{
    envelopes::{object_params, only},
    *,
};
use crate::constants::{ID_BYTES, MCP_FRAME_BYTES, MCP_PROTOCOL_VERSION};
use serde_json::{Map, json};
impl Session {
    /** Ping remains prompt before startup; all native tools wait for the initialized notification. */
    pub(super) fn request(
        &mut self,
        id: &ExternalId,
        method: &str,
        params: Option<&Value>,
        now: Instant,
    ) -> Vec<Action> {
        let empty = json!({});
        let params = params.or(Some(&empty));
        match method {
            "initialize" => self.initialize(id, params),
            "ping" => {
                if object_params(params, &["_meta"]).is_some() {
                    self.send(wire::result(&id.value, json!({})), Some(&id.value))
                } else {
                    self.invalid_params(id)
                }
            }
            _ if self.phase != Phase::Ready => self.send(
                wire::error(
                    Some(&id.value),
                    wire::SESSION_NOT_READY,
                    "Initialize the MCP connection before calling tools.",
                ),
                Some(&id.value),
            ),
            "tools/list" => self.list_tools(id, params),
            "tools/call" => self.call_tool(id, params, now),
            _ => self.send(
                wire::error(
                    Some(&id.value),
                    wire::METHOD_NOT_FOUND,
                    "Unknown MCP method.",
                ),
                Some(&id.value),
            ),
        }
    }
    /** Unsupported version offers return the sole supported pin; the client decides whether to proceed. */
    fn initialize(&mut self, id: &ExternalId, params: Option<&Value>) -> Vec<Action> {
        if self.phase != Phase::Fresh {
            return self.send(
                wire::error(
                    Some(&id.value),
                    wire::INVALID_REQUEST,
                    "The MCP connection was already initialized.",
                ),
                Some(&id.value),
            );
        }
        let Some(params) = object_params(
            params,
            &["protocolVersion", "capabilities", "clientInfo", "_meta"],
        ) else {
            return self.invalid_params(id);
        };
        if !bounded(params.get("protocolVersion"), ID_BYTES)
            || !params.get("capabilities").is_some_and(Value::is_object)
            || !client_info(params.get("clientInfo"))
        {
            return self.invalid_params(id);
        }
        self.phase = Phase::AwaitingInitialized;
        self.send(wire::result(&id.value,json!({"protocolVersion":MCP_PROTOCOL_VERSION,"capabilities":{"tools":{}},"serverInfo":{"name":"oya-native","version":env!("CARGO_PKG_VERSION")}})),Some(&id.value))
    }
    /** Parameter failures remain protocol failures rather than pretending a tool was executed. */
    pub(super) fn invalid_params(&mut self, id: &ExternalId) -> Vec<Action> {
        self.send(
            wire::error(
                Some(&id.value),
                wire::INVALID_PARAMS,
                "Invalid MCP request parameters.",
            ),
            Some(&id.value),
        )
    }
}
/** Implementation metadata is accepted without reflecting it or granting client-requested capabilities. */
fn client_info(value: Option<&Value>) -> bool {
    let Some(info) = value.and_then(Value::as_object) else {
        return false;
    };
    only(
        info,
        &[
            "name",
            "version",
            "title",
            "description",
            "websiteUrl",
            "icons",
        ],
    ) && bounded(info.get("name"), ID_BYTES)
        && bounded(info.get("version"), ID_BYTES)
        && optional_text(info, "title", MCP_FRAME_BYTES)
        && optional_text(info, "description", MCP_FRAME_BYTES)
        && optional_text(info, "websiteUrl", MCP_FRAME_BYTES)
        && info.get("icons").is_none_or(icons)
}
/** Optional metadata text receives local byte limits, independent of the client's Unicode character count. */
fn optional_text(info: &Map<String, Value>, key: &str, limit: usize) -> bool {
    info.get(key)
        .is_none_or(|value| value.as_str().is_some_and(|text| text.len() <= limit))
}
/** Required implementation/version strings are bounded and nonempty, without diagnostic interpolation. */
fn bounded(value: Option<&Value>, limit: usize) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|text| !text.is_empty() && text.len() <= limit)
}
/** Standard icon metadata remains inert bounded JSON and cannot cause fetches or browser actions. */
fn icons(value: &Value) -> bool {
    let Some(icons) = value.as_array() else {
        return false;
    };
    icons.iter().all(|icon| {
        icon.as_object().is_some_and(|icon| {
            only(icon, &["src", "mimeType", "sizes", "theme"])
                && bounded(icon.get("src"), MCP_FRAME_BYTES)
                && optional_text(icon, "mimeType", MCP_FRAME_BYTES)
                && optional_text(icon, "theme", MCP_FRAME_BYTES)
                && icon.get("sizes").is_none_or(|sizes| {
                    sizes.as_array().is_some_and(|sizes| {
                        sizes.iter().all(|size| {
                            size.as_str()
                                .is_some_and(|size| size.len() <= MCP_FRAME_BYTES)
                        })
                    })
                })
        })
    })
}
