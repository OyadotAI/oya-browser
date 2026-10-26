/** Attribute semantics and character references stay owned and bounded. */
use super::Node;
use crate::constants::{ID_BYTES, SCRIPT_BYTES, TEXT_BYTES};
use crate::protocol::{EngineError, EngineResult};
use std::collections::BTreeMap;
pub(super) type Attributes = BTreeMap<String, String>;

/** Input types Eye implements with their own semantics. */
const INPUT_TYPES: [&str; 11] = [
    "text", "password", "hidden", "checkbox", "radio", "submit", "button", "email", "search",
    "url", "tel",
];
/** Typed fields Eye has no picker for; HTML falls back to a text field, so Eye edits them as text. */
const TEXT_INPUT_TYPES: [&str; 6] = ["number", "date", "datetime-local", "month", "week", "time"];
/** Controls without an implementation stay visible but static rather than refusing the page. */
const STATIC_INPUT_TYPES: [&str; 3] = ["file", "color", "range"];

/** Inert attributes survive; unsupported executable attributes refuse loading. */
pub(super) fn node(
    tag: &str,
    local_id: usize,
    parent: Option<usize>,
    mut attributes: Attributes,
) -> EngineResult<Node> {
    if attributes
        .keys()
        .any(|key| key.starts_with("on") && !(tag == "button" && key == "onclick"))
    {
        return Err(EngineError::unsupported(
            "This page uses an unsupported event handler.",
        ));
    }
    if let Some(kind) = attributes.get_mut("type") {
        *kind = kind.to_ascii_lowercase();
    }
    // An empty or overlong id keeps its attribute and gets the internal key, as in mutation.rs.
    let id = attributes
        .get("id")
        .filter(|id| !id.is_empty() && id.len() <= mars_core::constants::HTML_ID_BYTES)
        .cloned()
        .unwrap_or_else(|| format!("@{local_id}"));
    if attributes
        .get("onclick")
        .is_some_and(|s| s.len() > SCRIPT_BYTES)
    {
        return Err(syntax());
    }
    Ok(Node {
        local_id,
        html_id: id,
        tag: tag.into(),
        text: String::new(),
        parent,
        value: attributes.get("value").cloned().unwrap_or_default(),
        href: attributes
            .get("href")
            .cloned()
            .filter(|href| tag == "a" && !script_url(href)),
        handler: attributes.get("onclick").cloned(),
        placeholder: attributes.get("placeholder").cloned().unwrap_or_default(),
        disabled: attributes.contains_key("disabled"),
        checked: attributes.contains_key("checked"),
        selected: attributes.contains_key("selected"),
        attributes: attributes.clone(),
        style: Default::default(),
        validation: None,
        image_src: attributes
            .get("src")
            .cloned()
            .filter(|src| tag == "img" && !script_url(src)),
        image_alt: attributes.get("alt").cloned().unwrap_or_default(),
        image_size: None,
        image_ratio_free: false,
        table_cell: None,
        defined: false,
        space_after: false,
        space_leading: false,
        leading_space: String::new(),
        focused: false,
        embedded: false,
    })
}

/// Page-mode recovery applied before `node`: other inline handlers are stripped rather than run or
/// refused, oversized attribute values and identifiers are dropped, and input types without an
/// implementation fall back the way HTML's do. Returns the value-free diagnostics to record.
pub(super) fn recover(tag: &str, attributes: &mut Attributes) -> Vec<String> {
    let mut diagnostics = Vec::new();
    let before = attributes.len();
    // A button's onclick and the body's handlers, which HTML forwards to the window (V9 runs
    // only the window-event ones, so `<body onload>` starts a page as it does in Chromium).
    attributes.retain(|key, value| {
        !(key.starts_with("on")
            && !((tag == "button" && key == "onclick" || tag == "body")
                && value.len() <= SCRIPT_BYTES))
    });
    if attributes.len() < before {
        diagnostics.push("Inline event handler attributes were removed and never run.".into());
    }
    let before = attributes.len();
    attributes.retain(|key, value| {
        value.len() <= TEXT_BYTES && (key != "id" || (!value.is_empty() && value.len() <= ID_BYTES))
    });
    if attributes.len() < before {
        diagnostics.push("Oversized or empty attribute values were dropped.".into());
    }
    if tag == "input" {
        diagnostics.extend(input_type(attributes));
    }
    diagnostics
}

/** Unknown and picker types edit as text, image submits and reset is an inert button. */
fn input_type(attributes: &mut Attributes) -> Option<String> {
    let kind = attributes.get("type")?.trim().to_ascii_lowercase();
    if INPUT_TYPES.contains(&kind.as_str()) || STATIC_INPUT_TYPES.contains(&kind.as_str()) {
        attributes.insert("type".into(), kind);
        return None;
    }
    let fallback = match kind.as_str() {
        "image" => "submit",
        "reset" => "button",
        _ => "text",
    };
    attributes.insert("type".into(), fallback.into());
    Some(if TEXT_INPUT_TYPES.contains(&kind.as_str()) {
        format!("Input type {kind} is edited as plain text.")
    } else {
        "An unrecognized input type behaves as a text field.".into()
    })
}

/// `javascript:` addresses stay inert: links and images never receive them as targets.
/// Leading whitespace and controls are ignored and the scheme compares case-insensitively, as
/// URL parsing does, so `" JaVa\tscript:"` is caught too.
pub(super) fn script_url(url: &str) -> bool {
    let scheme = url
        .trim_start_matches(|c: char| c <= ' ')
        .chars()
        .filter(|c| !['\t', '\n', '\r'].contains(c))
        .take("javascript:".len())
        .collect::<String>();
    scheme.eq_ignore_ascii_case("javascript:")
}

/** Character references decode with the owned named table; decoding never grows text. */
pub(super) fn entities(source: &str) -> String {
    super::entities::decode(source, false)
}

/** Unsupported syntax cannot create partially trusted native targets. */
pub(super) fn syntax() -> EngineError {
    EngineError::unsupported("This page uses unsupported or malformed HTML markup.")
}
