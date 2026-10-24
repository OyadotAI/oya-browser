/** Versioned bounded messages shared by agents and private engine pipes. */
mod codec;
mod error;
mod typed;

use crate::constants::{ID_BYTES, VERSION};
pub use codec::{decode, encode, read_frame, write_message};
pub use error::{EngineError, EngineResult, SourceLocation};
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub use typed::{RequestHead, TypedRequest, TypedResponse, encode_reply, encode_request};

/// One sequential operation; unknown envelope fields are rejected.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Protocol revision, required on every message.
    pub version: u32,
    /// Correlation only; repeating an ID does not replay a cached operation.
    pub id: String,
    /// Native operation name, never page-supplied executable code.
    pub op: String,
    /// Each operation validates its own exact parameter schema.
    pub params: Value,
}

impl Request {
    /// Builds an internal request using the same version as the public protocol.
    pub fn new(id: &str, op: &str, params: Value) -> Self {
        Self {
            version: VERSION,
            id: id.into(),
            op: op.into(),
            params,
        }
    }

    /// Validates identities before dispatch; control characters are not valid IDs.
    pub fn validate(&self) -> EngineResult<()> {
        if self.version != VERSION || !valid_id(&self.id) {
            return Err(EngineError::invalid(format!(
                "Expected protocol version {VERSION} and a nonempty bounded request ID."
            )));
        }
        Ok(())
    }
}

/// Machine output always contains exactly one result or error.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    /// Protocol revision for reply validation.
    pub version: u32,
    /// Null only when no valid correlation ID could be recovered.
    pub id: Option<String>,
    /// Present only on successful operations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Present only on failed operations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<EngineError>,
}

impl Response {
    /// Keeps success and failure shapes mutually exclusive.
    pub fn new(id: Option<String>, result: EngineResult<Value>) -> Self {
        let (result, error) = match result {
            Ok(value) => (Some(value), None),
            Err(error) => (None, Some(error)),
        };
        Self {
            version: VERSION,
            id,
            result,
            error,
        }
    }
}

/// Decodes exact parameter types instead of accepting silently ignored fields.
pub fn parameters<T: serde::de::DeserializeOwned>(value: Value) -> EngineResult<T> {
    serde_json::from_value(value)
        .map_err(|_| EngineError::invalid("Parameters do not match this operation's schema."))
}

/// IDs remain safe for correlation and human diagnostics.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= ID_BYTES && !id.chars().any(char::is_control)
}
