/** One bounded language policy feeds script-visible preferences and transport headers. */
use crate::constants::{LANGUAGE_TAG_BYTES, NAVIGATOR_LANGUAGES};
use crate::protocol::{EngineError, EngineResult};
/// A host with no configured preferences uses the same fallback in JavaScript and HTTP.
pub const DEFAULT: &str = "en-US";
/** Reject header delimiters and controls while retaining the existing ASCII tag policy. */
pub fn valid(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= LANGUAGE_TAG_BYTES
        && tag.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
/** Validate service and host inputs before preferences cross engine boundaries. */
pub fn validate(tags: &[String]) -> EngineResult<()> {
    if tags.len() > NAVIGATOR_LANGUAGES || tags.iter().any(|tag| !valid(tag)) {
        return Err(EngineError::invalid(
            "Invalid bounded language preferences.",
        ));
    }
    Ok(())
}
/** Preserve order with descending quality values; never insert languages absent from the policy. */
pub fn header(tags: &[String]) -> String {
    if tags.is_empty() {
        return DEFAULT.into();
    }
    tags.iter()
        .enumerate()
        .map(|(index, tag)| {
            if index == 0 {
                tag.clone()
            } else {
                format!("{tag};q=0.{}", 9usize.saturating_sub(index - 1).max(1))
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}
