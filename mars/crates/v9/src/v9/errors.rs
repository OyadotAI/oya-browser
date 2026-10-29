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
/** Lexer reasons whose SyntaxError text V8 words differently. Pages compare the messages they
catch against V8's (engine checks do), so the script-visible text is V8's; the diagnostic the
host logs keeps the precise reason. */
const V8_REASONS: [(&str, &str); 12] = [
    ("unterminated string literal", "Invalid or unexpected token"),
    (
        "unescaped line break in string",
        "Invalid or unexpected token",
    ),
    ("unterminated escape", "Invalid or unexpected token"),
    ("invalid numeric suffix", "Invalid or unexpected token"),
    ("invalid numeric literal", "Invalid or unexpected token"),
    (
        "invalid legacy numeric literal",
        "Invalid or unexpected token",
    ),
    ("invalid BigInt literal", "Invalid or unexpected token"),
    ("unexpected character", "Invalid or unexpected token"),
    ("unterminated block comment", "Invalid or unexpected token"),
    ("unterminated template literal", "Unexpected end of input"),
    (
        "unterminated regular expression",
        "Invalid regular expression: missing /",
    ),
    ("unexpected end of input", "Unexpected end of input"),
];

/** JavaScript SyntaxError.message carries the parse reason; structured host errors keep their
location prefix. The reason is V8's text already, case included (`let is disallowed...`,
`await is only valid...`), so nothing here recases it. */
pub(super) fn syntax_reason(error: &EngineError) -> String {
    let reason = reason(error);
    match V8_REASONS.iter().find(|(ours, _)| *ours == reason) {
        Some((_, v8)) => (*v8).to_string(),
        None => reason.to_string(),
    }
}
/** The reason a syntax diagnostic was raised with, without its location prefix and the period
the diagnostic adds. */
pub(super) fn reason(error: &EngineError) -> &str {
    let reason = error
        .message
        .split_once(": ")
        .map_or(error.message.as_str(), |(_, reason)| reason);
    reason.strip_suffix('.').unwrap_or(reason)
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
