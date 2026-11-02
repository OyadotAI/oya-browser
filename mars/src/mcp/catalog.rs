/** The fixed MCP catalog translates validated values without observing pages or granting native authority. */
mod definition;
mod field;
mod result;
use crate::protocol::{EngineError, Request};
pub use result::tool_result;
use serde_json::Value;
/** Unknown discovery names are protocol mistakes; invalid tool arguments retain native error shape. */
#[derive(Debug)]
pub enum TranslationError {
    /// No arbitrary native operation can be selected through an unlisted tool name.
    UnknownTool,
    /// Exact schema and byte-bound failures never reach the native broker.
    InvalidArguments(EngineError),
}
/** Discovery and execution share one table so required fields cannot drift between their surfaces. */
pub fn tools() -> Vec<Value> {
    definition::TOOLS
        .iter()
        .map(definition::Definition::schema)
        .collect()
}
/** Translation copies explicit target preconditions unchanged and never refreshes, retries or grants them. */
pub fn translate(
    name: &str,
    arguments: &Value,
    native_id: &str,
) -> Result<Request, TranslationError> {
    let definition = definition::TOOLS
        .iter()
        .find(|definition| definition.name == name)
        .ok_or(TranslationError::UnknownTool)?;
    definition
        .validate(arguments)
        .map_err(TranslationError::InvalidArguments)?;
    let request = Request::new(native_id, definition.operation, arguments.clone());
    request
        .validate()
        .map_err(TranslationError::InvalidArguments)?;
    Ok(request)
}
