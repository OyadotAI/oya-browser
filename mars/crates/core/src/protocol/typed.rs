/**
 * Typed private frames. They carry exactly the JSON a `Request` or `Response` would, field for
 * field, but are written straight from engine types and read straight into them, so a large
 * document never becomes a `serde_json::Value` tree on either side of a pipe. Agents still use
 * the `Value` envelopes; only coordinator and engine code use these.
 */
use super::{EngineError, EngineResult};
use crate::constants::{FRAME_BYTES, VERSION};
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

/** A request as written: the same fields and order as `Request`. */
#[derive(Serialize)]
struct Outgoing<'a, P> {
    version: u32,
    id: &'a str,
    op: &'a str,
    params: &'a P,
}

/** A success reply as written: the same fields and order as `Response`. */
#[derive(Serialize)]
struct Reply<'a, R> {
    version: u32,
    id: &'a str,
    result: &'a R,
}

/// One newline-terminated request frame, within FRAME_BYTES like every encoded message.
pub fn encode_request(id: &str, op: &str, params: &impl Serialize) -> EngineResult<Vec<u8>> {
    bounded(&Outgoing {
        version: VERSION,
        id,
        op,
        params,
    })
}

/// One newline-terminated success reply frame, within FRAME_BYTES.
pub fn encode_reply(id: &str, result: &impl Serialize) -> EngineResult<Vec<u8>> {
    bounded(&Reply {
        version: VERSION,
        id,
        result,
    })
}

/** Serializes once and checks the same bound `encode` does. */
fn bounded(value: &impl Serialize) -> EngineResult<Vec<u8>> {
    let mut bytes = serde_json::to_vec(value)
        .map_err(|_| EngineError::invalid("Cannot serialize the response."))?;
    bytes.push(b'\n');
    if bytes.len() > FRAME_BYTES {
        return Err(EngineError::limit("wire frame bytes", FRAME_BYTES));
    }
    Ok(bytes)
}

/// The envelope of a request whose parameters are only checked for well-formed JSON, so the
/// operation can be chosen before its exact parameter type is decoded.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestHead {
    /// Protocol revision, required on every message.
    pub version: u32,
    /// Correlation only.
    pub id: String,
    /// Private operation name.
    pub op: String,
    /** Skipped, but parsed, so malformed JSON anywhere fails here. */
    #[allow(dead_code)]
    params: IgnoredAny,
}

impl RequestHead {
    /// The same identity checks as `Request::validate`.
    pub fn validate(&self) -> EngineResult<()> {
        if self.version != VERSION || !super::valid_id(&self.id) {
            return Err(EngineError::invalid(format!(
                "Expected protocol version {VERSION} and a nonempty bounded request ID."
            )));
        }
        Ok(())
    }
}

/// A request decoded with its operation's exact parameter type.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedRequest<P> {
    /** Already checked by `RequestHead`. */
    #[allow(dead_code)]
    version: u32,
    /** Already checked by `RequestHead`. */
    #[allow(dead_code)]
    id: String,
    /** Already checked by `RequestHead`. */
    #[allow(dead_code)]
    op: String,
    /// The operation's parameters.
    pub params: P,
}

/// A reply decoded with the result type the caller expects.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedResponse<T> {
    /// Protocol revision for reply validation.
    pub version: u32,
    /// The request this answers.
    pub id: Option<String>,
    /// Present only on successful operations; absent reads as None.
    pub result: Option<T>,
    /// Present only on failed operations; absent reads as None.
    pub error: Option<EngineError>,
}
