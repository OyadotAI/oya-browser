/** MCP envelopes and encoding use their own frame bound without changing native service limits. */
mod fields;
pub(super) mod ids;
use crate::{
    constants::MCP_FRAME_BYTES,
    protocol::{EngineError, EngineResult},
};
pub(super) use ids::ExternalId;
use serde_json::{Value, json};
use std::io::{self, Write};
/// Standard JSON-RPC error codes are named once for every lifecycle and wrapper rejection.
pub(super) const PARSE_ERROR: i32 = -32700;
pub(super) const INVALID_REQUEST: i32 = -32600;
pub(super) const METHOD_NOT_FOUND: i32 = -32601;
pub(super) const INVALID_PARAMS: i32 = -32602;
pub(super) const INTERNAL_ERROR: i32 = -32603;
pub(super) const SESSION_NOT_READY: i32 = -32000;
/** Top-level field inspection preserves numeric ID spelling and detects ambiguous duplicate envelopes. */
pub(super) struct Decoded {
    /// Parsed data is never logged or retained as pending request metadata.
    pub value: Value,
    /// Original numeric tokens permit exact integer checks before float rounding.
    pub raw_id: Option<String>,
    /// Duplicate fields cannot silently change request intent.
    pub duplicate: bool,
    /// Duplicate ID members have no unambiguous response correlation.
    pub duplicate_id: bool,
    /// Cancellation correlation also retains exact numeric spelling.
    pub raw_cancel_id: Option<String>,
}
/** Parse one already-framed JSON value while preserving the exact top-level numeric ID token. */
pub(super) fn decode(frame: &[u8]) -> Result<Decoded, ()> {
    let value: Value = serde_json::from_slice(frame).map_err(|_| ())?;
    let (raw_id, duplicate, duplicate_id) = if value.is_object() {
        fields::inspect(frame)
    } else {
        (None, false, false)
    };
    let raw_cancel_id = fields::member(frame, "params")
        .and_then(|params| fields::member(params.as_bytes(), "requestId"));
    Ok(Decoded {
        value,
        raw_id,
        duplicate,
        duplicate_id,
        raw_cancel_id,
    })
}
/** Serialize incrementally so an oversized value never requires an unbounded output allocation. */
pub fn encode(value: &Value) -> EngineResult<Vec<u8>> {
    let mut output = Frame(Vec::new());
    serde_json::to_writer(&mut output, value)
        .map_err(|_| EngineError::limit("MCP frame bytes", MCP_FRAME_BYTES))?;
    output
        .write_all(b"\n")
        .map_err(|_| EngineError::limit("MCP frame bytes", MCP_FRAME_BYTES))?;
    Ok(output.0)
}
/** The final newline consumes the same bounded allocation as JSON bytes. */
struct Frame(Vec<u8>);
impl Write for Frame {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > MCP_FRAME_BYTES {
            return Err(io::Error::other("MCP frame limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
/** Error messages are fixed and never interpolate untrusted input or typed field values. */
pub(super) fn error(id: Option<&Value>, code: i32, message: &'static str) -> Value {
    let mut response = json!({"jsonrpc":"2.0","error":{"code":code,"message":message}});
    if let Some(id) = id {
        response["id"] = id.clone();
    }
    response
}
/** Native or protocol success preserves the original typed external correlation. */
pub(super) fn result(id: &Value, result: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":result})
}
