/** `initial-letter` (CSS Inline 3): a `::first-letter` sized to span several lines and sunk into them. */
use super::unsupported;
use crate::protocol::EngineResult;
use serde::{Deserialize, Serialize};

/// A non-`normal` `initial-letter`: the letter's size in lines and how many lines it sinks.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct InitialLetter {
    /// Lines the letter's cap height spans (at least 1, may be fractional).
    pub size: f64,
    /// Lines it sinks, resolved as Blink's `StyleInitialLetter`: `raise` is 1, an integer is
    /// itself, and `drop` or an omitted sink is the size rounded up.
    pub sink: f64,
}

/** `normal` (None), `<size>`, `<size> drop|raise` or `<size> <integer>`; a size below 1 or a
 * sink below 1 is invalid, so the declaration drops. */
pub(super) fn parse(value: &str) -> EngineResult<Option<InitialLetter>> {
    let parts: Vec<&str> = value.split_ascii_whitespace().collect();
    if parts.len() == 1 && parts[0].eq_ignore_ascii_case("normal") {
        return Ok(None);
    }
    let number = |text: &str| {
        text.parse::<f64>()
            .ok()
            .filter(|v| v.is_finite() && *v >= 1.0)
    };
    let (size, sink) = match parts.as_slice() {
        [size] => (number(size), Some("drop")),
        [size, sink] => (number(size), Some(*sink)),
        _ => (None, None),
    };
    let (Some(size), Some(sink)) = (size, sink) else {
        return Err(unsupported());
    };
    let sink = match sink.to_ascii_lowercase().as_str() {
        "drop" => size.ceil(),
        "raise" => 1.0,
        other => integer(other).ok_or_else(unsupported)?,
    };
    Ok(Some(InitialLetter { size, sink }))
}

/** A positive integer sink. */
fn integer(text: &str) -> Option<f64> {
    text.parse::<u32>().ok().filter(|v| *v >= 1).map(f64::from)
}
