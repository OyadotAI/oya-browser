/** The MCP catalog is exercised through its pure public API without a browser or transport. */
use mars::mcp::catalog::{TranslationError, translate};
use serde_json::{Value, json};
mod discovery;
mod results;
mod validation;
/** Explicit native preconditions are deliberately distinct to catch replacement or silent refresh. */
fn target() -> Value {
    json!({"tab_id":"native:tab-7","control_epoch":41,"page_id":"native:page-9","node_id":"native:node-3","expected_revision":23})
}
/** Argument refusals keep their native machine code and never echo an offending supplied value. */
fn invalid(name: &str, arguments: &Value) {
    match translate(name, arguments, "mcp-1") {
        Err(TranslationError::InvalidArguments(error)) => assert_eq!(error.code, "invalid_request"),
        other => panic!("Expected a strict tool argument refusal, got {other:?}"),
    }
}
