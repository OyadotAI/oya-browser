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
    let name = match (alias(&name), name.strip_prefix("-webkit-")) {
        (Some(standard), _) => standard.to_string(),
        (None, Some(standard))
            if standard.starts_with("animation") || standard.starts_with("mask") =>
        {
            standard.to_string()
        }
        _ => name,
    };
    let might_hide = hiding(&name, value);
    block.hiding |= might_hide;
    // `var()` substitutes at computed-value time, as does `attr()` outside `content`.
    let lower = value.to_ascii_lowercase();
    let pending = lower.contains("var(") || (lower.contains("attr(") && name != "content");
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
/**
 * Legacy and prefixed names the CSS parser treats as another property, as Blink's
 * `alias_for` entries do (sorted, for binary search): `grid-gap` is `gap`, `-webkit-transform`
 * is `transform`, and so on, so both spellings cascade as one property.
 */
const ALIASES: [(&str, &str); 110] = [
    ("-webkit-align-content", "align-content"),
    ("-webkit-align-items", "align-items"),
    ("-webkit-align-self", "align-self"),
    ("-webkit-animation", "animation"),
    ("-webkit-animation-delay", "animation-delay"),
    ("-webkit-animation-direction", "animation-direction"),
    ("-webkit-animation-duration", "animation-duration"),
    ("-webkit-animation-fill-mode", "animation-fill-mode"),
    (
        "-webkit-animation-iteration-count",
        "animation-iteration-count",
    ),
    ("-webkit-animation-name", "animation-name"),
    ("-webkit-animation-play-state", "animation-play-state"),
    (
        "-webkit-animation-timing-function",
        "animation-timing-function",
    ),
    ("-webkit-app-region", "app-region"),
    ("-webkit-appearance", "appearance"),
    ("-webkit-backface-visibility", "backface-visibility"),
    ("-webkit-background-clip", "background-clip"),
    ("-webkit-background-origin", "background-origin"),
    ("-webkit-background-size", "background-size"),
    ("-webkit-border-after", "border-block-end"),
    ("-webkit-border-after-color", "border-block-end-color"),
    ("-webkit-border-after-style", "border-block-end-style"),
    ("-webkit-border-after-width", "border-block-end-width"),
    ("-webkit-border-before", "border-block-start"),
    ("-webkit-border-before-color", "border-block-start-color"),
    ("-webkit-border-before-style", "border-block-start-style"),
    ("-webkit-border-before-width", "border-block-start-width"),
    (
        "-webkit-border-bottom-left-radius",
        "border-bottom-left-radius",
    ),
    (
        "-webkit-border-bottom-right-radius",
        "border-bottom-right-radius",
    ),
    ("-webkit-border-end", "border-inline-end"),
    ("-webkit-border-end-color", "border-inline-end-color"),
    ("-webkit-border-end-style", "border-inline-end-style"),
    ("-webkit-border-end-width", "border-inline-end-width"),
    ("-webkit-border-radius", "border-radius"),
    ("-webkit-border-start", "border-inline-start"),
    ("-webkit-border-start-color", "border-inline-start-color"),
    ("-webkit-border-start-style", "border-inline-start-style"),
    ("-webkit-border-start-width", "border-inline-start-width"),
    ("-webkit-border-top-left-radius", "border-top-left-radius"),
    ("-webkit-border-top-right-radius", "border-top-right-radius"),
    ("-webkit-box-shadow", "box-shadow"),
    ("-webkit-box-sizing", "box-sizing"),
    ("-webkit-clip-path", "clip-path"),
    ("-webkit-column-count", "column-count"),
    ("-webkit-column-gap", "column-gap"),
    ("-webkit-column-rule", "column-rule"),
    ("-webkit-column-rule-color", "column-rule-color"),
    ("-webkit-column-rule-style", "column-rule-style"),
    ("-webkit-column-rule-width", "column-rule-width"),
    ("-webkit-column-span", "column-span"),
    ("-webkit-column-width", "column-width"),
    ("-webkit-columns", "columns"),
    ("-webkit-filter", "filter"),
    ("-webkit-flex", "flex"),
    ("-webkit-flex-basis", "flex-basis"),
    ("-webkit-flex-direction", "flex-direction"),
    ("-webkit-flex-flow", "flex-flow"),
    ("-webkit-flex-grow", "flex-grow"),
    ("-webkit-flex-shrink", "flex-shrink"),
    ("-webkit-flex-wrap", "flex-wrap"),
    ("-webkit-font-feature-settings", "font-feature-settings"),
    ("-webkit-hyphenate-character", "hyphenate-character"),
    ("-webkit-justify-content", "justify-content"),
    ("-webkit-logical-height", "block-size"),
    ("-webkit-logical-width", "inline-size"),
    ("-webkit-margin-after", "margin-block-end"),
    ("-webkit-margin-before", "margin-block-start"),
    ("-webkit-margin-end", "margin-inline-end"),
    ("-webkit-margin-start", "margin-inline-start"),
    ("-webkit-mask", "mask"),
    ("-webkit-mask-clip", "mask-clip"),
    ("-webkit-mask-composite", "mask-composite"),
    ("-webkit-mask-image", "mask-image"),
    ("-webkit-mask-origin", "mask-origin"),
    ("-webkit-mask-position", "mask-position"),
    ("-webkit-mask-repeat", "mask-repeat"),
    ("-webkit-mask-size", "mask-size"),
    ("-webkit-max-logical-height", "max-block-size"),
    ("-webkit-max-logical-width", "max-inline-size"),
    ("-webkit-min-logical-height", "min-block-size"),
    ("-webkit-min-logical-width", "min-inline-size"),
    ("-webkit-opacity", "opacity"),
    ("-webkit-order", "order"),
    ("-webkit-padding-after", "padding-block-end"),
    ("-webkit-padding-before", "padding-block-start"),
    ("-webkit-padding-end", "padding-inline-end"),
    ("-webkit-padding-start", "padding-inline-start"),
    ("-webkit-perspective", "perspective"),
    ("-webkit-perspective-origin", "perspective-origin"),
    ("-webkit-print-color-adjust", "print-color-adjust"),
    ("-webkit-shape-image-threshold", "shape-image-threshold"),
    ("-webkit-shape-margin", "shape-margin"),
    ("-webkit-shape-outside", "shape-outside"),
    ("-webkit-text-emphasis", "text-emphasis"),
    ("-webkit-text-emphasis-color", "text-emphasis-color"),
    ("-webkit-text-emphasis-position", "text-emphasis-position"),
    ("-webkit-text-emphasis-style", "text-emphasis-style"),
    ("-webkit-text-size-adjust", "text-size-adjust"),
    ("-webkit-transform", "transform"),
    ("-webkit-transform-origin", "transform-origin"),
    ("-webkit-transform-style", "transform-style"),
    ("-webkit-transition", "transition"),
    ("-webkit-transition-delay", "transition-delay"),
    ("-webkit-transition-duration", "transition-duration"),
    ("-webkit-transition-property", "transition-property"),
    (
        "-webkit-transition-timing-function",
        "transition-timing-function",
    ),
    ("-webkit-user-select", "user-select"),
    ("grid-column-gap", "column-gap"),
    ("grid-gap", "gap"),
    ("grid-row-gap", "row-gap"),
    ("word-wrap", "overflow-wrap"),
];
/** The property an alias stands for. */
fn alias(name: &str) -> Option<&'static str> {
    ALIASES
        .binary_search_by(|(alias, _)| (*alias).cmp(name))
        .ok()
        .map(|index| ALIASES[index].1)
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
        let value = &super::values::integer_math(key, value);
        let physical = super::logical::physical(key, super::logical::Mode::default());
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

#[cfg(test)]
mod tests {
    /** The alias table is sorted, so binary search finds every entry. */
    #[test]
    fn aliases_are_sorted_and_resolve() {
        assert!(super::ALIASES.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert_eq!(super::alias("grid-gap"), Some("gap"));
        assert_eq!(super::alias("-webkit-box-shadow"), Some("box-shadow"));
        assert_eq!(super::alias("gap"), None);
    }
}
