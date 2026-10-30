/** Transition shorthand and longhands preserve list matching while rejecting partial declarations. */
use super::{TransitionEasing, TransitionStyle};
use crate::document::style::{
    constants::{MILLISECONDS_PER_SECOND, TRANSITION_TIME_MS, TRANSITIONS},
    syntax::{find_top, tokens},
    unsupported,
};
use crate::protocol::EngineResult;

/** The shorthand resets every transition longhand, including unsupported discrete behavior. */
pub(in crate::document::style) const LONGHANDS: [&str; 5] = [
    "transition-property",
    "transition-duration",
    "transition-timing-function",
    "transition-delay",
    "transition-behavior",
];

/** Expand all layers atomically so one invalid layer does not partially overwrite the cascade. */
pub(in crate::document::style) fn shorthand(raw: &str) -> Option<Vec<(String, String)>> {
    let layers = parts(raw)?;
    let mut lists: [Vec<String>; 5] = Default::default();
    for layer in layers {
        for (list, value) in lists.iter_mut().zip(layer_values(layer)?) {
            list.push(value);
        }
    }
    Some(
        LONGHANDS
            .iter()
            .zip(lists)
            .map(|(name, values)| (name.to_string(), values.join(", ")))
            .collect(),
    )
}

/** A layer has at most one property, easing and behavior, plus duration followed by delay. */
fn layer_values(layer: &str) -> Option<[String; 5]> {
    let mut slots: [Option<String>; 5] = Default::default();
    for token in tokens(layer) {
        let token = token.to_ascii_lowercase();
        let slot = if time(&token).is_some() {
            if slots[1].is_none() { 1 } else { 3 }
        } else if TransitionEasing::parse(&token).is_some() {
            2
        } else if token == "normal" || token == "allow-discrete" {
            4
        } else if property(&token) {
            0
        } else {
            return None;
        };
        if slots[slot].is_some() {
            return None;
        }
        slots[slot] = Some(token);
    }
    if slots.iter().all(Option::is_none) {
        return None;
    }
    let defaults = ["all", "0s", "ease", "0s", "normal"];
    Some(std::array::from_fn(|index| {
        slots[index]
            .take()
            .unwrap_or_else(|| defaults[index].into())
    }))
}

/** Preserve well-formed property identifiers; only the opacity match is animated by this slice. */
pub(super) fn property(value: &str) -> bool {
    let Some(first) = value.bytes().next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == b'-' || first == b'_')
        && value.len() <= crate::document::style::constants::CUSTOM_PROPERTY_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
        && !["initial", "inherit", "unset", "revert", "revert-layer"].contains(&value)
}

/** Parse finite CSS times in milliseconds; unitless zero is not a CSS time. */
pub(in crate::document::style) fn time(value: &str) -> Option<f64> {
    let (number, scale) = value
        .strip_suffix("ms")
        .map(|number| (number, 1.0))
        .or_else(|| {
            value
                .strip_suffix('s')
                .map(|number| (number, MILLISECONDS_PER_SECOND))
        })?;
    let result = number.parse::<f64>().ok()? * scale;
    (result.is_finite() && result.abs() <= TRANSITION_TIME_MS).then_some(result)
}

/** Parse one list within the shared item cap, returning no partial accepted value. */
fn list<T>(value: &str, parse: impl Fn(&str) -> Option<T>) -> EngineResult<Vec<T>> {
    let parts = parts(value).ok_or_else(unsupported)?;
    parts
        .into_iter()
        .map(|part| parse(part).ok_or_else(unsupported))
        .collect()
}

/** Comma lists reject empty items instead of accidentally accepting trailing or doubled separators. */
pub(in crate::document::style) fn parts(value: &str) -> Option<Vec<&str>> {
    let mut parts = Vec::new();
    let mut start = 0;
    loop {
        let end = find_top(value, start, b",");
        let part = value[start..end].trim();
        if part.is_empty() || parts.len() >= TRANSITIONS {
            return None;
        }
        parts.push(part);
        if end == value.len() {
            return Some(parts);
        }
        start = end + 1;
    }
}

/** Computed longhands are consumed by the timeline only after the entire cascade resolves. */
pub(in crate::document::style) fn apply(
    style: &mut TransitionStyle,
    key: &str,
    value: &str,
) -> EngineResult<bool> {
    match key {
        "transition-property" => {
            let names = list(value, |name| property(name).then(|| name.to_string()))?;
            if names.len() > 1 && names.iter().any(|name| name == "none") {
                return Err(unsupported());
            }
            style.properties = names;
        }
        "transition-duration" => {
            style.durations = list(value, |value| time(value).filter(|time| *time >= 0.0))?
        }
        "transition-delay" => style.delays = list(value, time)?,
        "transition-timing-function" => style.easings = list(value, TransitionEasing::parse)?,
        "transition-behavior" => {
            list(value, |value| (value == "normal").then_some(()))?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/** CSS-wide keywords copy only the selected computed list and never inherit unrelated properties. */
pub(in crate::document::style) fn copy(
    style: &mut TransitionStyle,
    source: &TransitionStyle,
    key: &str,
) -> bool {
    match key {
        "transition-property" => style.properties.clone_from(&source.properties),
        "transition-duration" => style.durations.clone_from(&source.durations),
        "transition-delay" => style.delays.clone_from(&source.delays),
        "transition-timing-function" => style.easings.clone_from(&source.easings),
        "transition-behavior" => {}
        _ => return false,
    }
    true
}
