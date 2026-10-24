/** Static reasons and structured locations prevent source/secret leakage in VM errors. */
use crate::protocol::{EngineError, SourceLocation};
/** Malformed supported grammar is distinct from recognized unimplemented JavaScript. */
pub(super) fn syntax(location: SourceLocation, reason: &str) -> EngineError {
    located(
        EngineError::new(
            "script_syntax_error",
            format!(
                "Script syntax error at line {}, column {}: {reason}.",
                location.line, location.column
            ),
            "Correct the script; no script changes were applied.",
        ),
        location,
        "parse",
    )
}
/** Budget and DOM failures retain their stable error classification. */
pub(super) fn located(
    mut error: EngineError,
    location: SourceLocation,
    phase: &str,
) -> EngineError {
    if error.location.is_none() {
        error.location = Some(Box::new(location));
        error.phase = Some(phase.into());
        error.input_kind = Some("script".into());
    }
    error
}
