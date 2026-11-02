/**
 * Tool results duplicate complete native JSON in text and structured form without adapter success
 * prose. A screenshot's PNG moves into an image content block instead of the JSON.
 */
use crate::protocol::{EngineError, EngineResult};
use serde_json::{Value, json};
/** Native failures remain tool failures; malformed native success values are internal protocol faults. */
pub fn tool_result(result: EngineResult<Value>) -> EngineResult<Value> {
    let (mut value, is_error) = match result {
        Ok(value) if value.is_object() => (value, false),
        Ok(_) => return Err(internal()),
        Err(error) => (
            json!({"error":serde_json::to_value(error).map_err(|_|internal())?}),
            true,
        ),
    };
    let image = value
        .as_object_mut()
        .and_then(|value| value.remove("png_base64"));
    let text = serde_json::to_string(&value).map_err(|_| internal())?;
    let mut content = vec![json!({"type":"text","text":text})];
    if let Some(data) = image {
        content.insert(
            0,
            json!({"type":"image","data":data,"mimeType":"image/png"}),
        );
    }
    Ok(json!({"content":content,"structuredContent":value,"isError":is_error}))
}
/** A fixed diagnostic avoids leaking malformed service payloads into adapter-generated errors. */
fn internal() -> EngineError {
    EngineError::new(
        "internal_error",
        "The native tool response is invalid.",
        "Start a new connection; no operation was replayed.",
    )
}
