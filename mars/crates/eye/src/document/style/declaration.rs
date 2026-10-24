/** Declarations are validated once at parse time; unsupported ones are dropped, never fatal. */
use super::{ComputedStyle, constants::CUSTOM_PROPERTY_BYTES, hiding::hiding, note, shorthand};
use crate::document::RenderContext;
/** One validated (or `var()`-deferred) longhand, custom property or pending shorthand. */
#[derive(Clone, Debug)]
pub(super) struct Declaration {
    /// Lowercase property name; custom properties keep their case.
    pub name: String,
    /// Authored value text, trimmed and without `!important`; shared by every match.
    pub value: std::rc::Rc<str>,
    /// Declared `!important`.
    pub important: bool,
}
/** A parsed declaration list with what agent safety needs to know about its drops. */
#[derive(Clone, Debug, Default)]
pub(super) struct Block {
    /// Kept declarations in source order.
    pub declarations: Vec<Declaration>,
    /// A dropped declaration could have hidden its targets.
    pub hides: bool,
    /// Any declaration, kept or dropped, could hide its targets.
    pub hiding: bool,
}
/** Why a declaration was dropped, for value-free diagnostics. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Drop {
    /// The property is unknown to Eye.
    Property,
    /// The property is known but this value is not supported.
    Value,
}
/** A `;`-separated list without nested rules, as in a style attribute. */
pub(super) fn block(source: &str, diagnostics: &mut Vec<String>) -> Block {
    let mut block = Block::default();
    for item in super::syntax::split_top(source, b';') {
        declaration(item, &mut block, diagnostics);
    }
    block
}
/** Parses one `name: value [!important]` item into the block. */
pub(super) fn declaration(item: &str, block: &mut Block, diagnostics: &mut Vec<String>) {
    let Some((name, value)) = item.split_once(':') else {
        return note(
            diagnostics,
            "A malformed CSS declaration was ignored.".into(),
        );
    };
    let name = name.trim();
    let (value, important) = importance(value);
    if name.is_empty() || name.contains(char::is_whitespace) {
        return note(
            diagnostics,
            "A malformed CSS declaration was ignored.".into(),
        );
    }
    if name.starts_with("--") {
        if value.len() <= CUSTOM_PROPERTY_BYTES {
            block.declarations.push(Declaration {
                name: name.into(),
                value: value.into(),
                important,
            });
        }
        return;
    }
    let name = name.to_ascii_lowercase();
    // A static frame never shows a transition in progress, so transitions are dropped silently.
    if name.starts_with("transition") || name.starts_with("-webkit-transition") {
        return;
    }
    let name = match name.strip_prefix("-webkit-") {
        Some(standard) if standard.starts_with("animation") || standard.starts_with("mask") => {
            standard.to_string()
        }
        _ => name,
    };
    let might_hide = hiding(&name, value);
    block.hiding |= might_hide;
    let pending = value.to_ascii_lowercase().contains("var(");
    let longhands = if pending {
        Ok(vec![(name.clone(), value.to_owned())])
    } else {
        validate(&name, value)
    };
    match longhands {
        Ok(longhands) => block
            .declarations
            .extend(longhands.into_iter().map(|(name, value)| Declaration {
                name,
                value: value.into(),
                important,
            })),
        Err(drop) => {
            block.hides |= might_hide;
            dropped(&name, drop, diagnostics);
        }
    }
}
/** Records a dropped declaration by property name only; vendor-prefixed names stay quiet. */
pub(super) fn dropped(name: &str, drop: Drop, diagnostics: &mut Vec<String>) {
    if name.starts_with('-') {
        return;
    }
    let name = super::diagnostic_name(name);
    note(
        diagnostics,
        match drop {
            Drop::Property => format!("CSS property {name} is not supported and was ignored."),
            Drop::Value => format!("A CSS value for {name} is not supported and was ignored."),
        },
    );
}
/** Expands shorthands and trial-applies each longhand to a scratch style. */
pub(super) fn validate(name: &str, value: &str) -> Result<Vec<(String, String)>, Drop> {
    let expanded = shorthand::expand(name, value)
        .map_err(|known| if known { Drop::Value } else { Drop::Property })?;
    for (key, value) in &expanded {
        if wide(value) {
            continue;
        }
        let mut scratch = ComputedStyle::default();
        let physical = super::logical::physical(key, false);
        match super::apply::apply(
            &mut scratch,
            physical.as_deref().unwrap_or(key),
            value,
            RenderContext::default(),
            &mut Vec::new(),
        ) {
            Ok(true) => {}
            Ok(false) => return Err(Drop::Property),
            Err(_) => return Err(Drop::Value),
        }
    }
    Ok(expanded)
}
/** CSS-wide keywords are valid for every property and resolve during the cascade. */
pub(super) fn wide(value: &str) -> bool {
    ["inherit", "initial", "unset", "revert", "revert-layer"]
        .contains(&value.trim().to_ascii_lowercase().as_str())
}
/** Strips a trailing `!important`, allowing whitespace after the bang. */
fn importance(value: &str) -> (&str, bool) {
    let trimmed = value.trim();
    let lower = trimmed.to_ascii_lowercase();
    if let Some(head) = lower.strip_suffix("important")
        && let Some(head) = head.trim_end().strip_suffix('!')
    {
        return (trimmed[..head.len()].trim(), true);
    }
    (trimmed, false)
}
