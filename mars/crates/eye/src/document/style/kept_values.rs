/**
 * Properties Eye parses and keeps for `getComputedStyle` and inheritance, but does not paint.
 * Blink accepts these values and answers scripts with them, so dropping them would make the
 * page's own feature checks disagree with the engine. Each entry names its initial value, whether
 * it inherits, and the grammar that canonicalizes what the author wrote. Shorthands are expanded
 * in `shorthand.rs`, so only longhands reach `apply`.
 */
use super::{ComputedStyle, kept_grammar as grammar};
use crate::{document::RenderContext, protocol::EngineResult};
use std::collections::BTreeMap;

/** A validator: the canonical value for `value`, or an unsupported-value error. */
type Check = fn(&str, &ComputedStyle, RenderContext) -> EngineResult<String>;

/** One kept property: its name, initial value, inheritance and grammar. */
pub(super) struct Kept {
    /// Lowercase property name.
    name: &'static str,
    /// The computed value when nothing was declared, as Blink serializes it.
    initial: &'static str,
    /// Whether the property inherits from the parent by default.
    inherited: bool,
    /// The grammar that validates and canonicalizes an authored value.
    check: Check,
}

/** Every kept property Eye answers for. Unlisted names fall through to the rest of the cascade. */
const KEPT: &[Kept] = &[
    kept("appearance", "auto", false, grammar::appearance),
    kept(
        "background-blend-mode",
        "normal",
        false,
        grammar::background_blend_mode,
    ),
    kept(
        "font-feature-settings",
        "normal",
        true,
        grammar::font_feature_settings,
    ),
    kept(
        "font-synthesis",
        "weight style small-caps position",
        true,
        grammar::font_synthesis,
    ),
    kept(
        "font-variation-settings",
        "normal",
        true,
        grammar::font_variation_settings,
    ),
    kept(
        "forced-color-adjust",
        "auto",
        true,
        grammar::forced_color_adjust,
    ),
    kept(
        "list-style-position",
        "outside",
        true,
        grammar::list_style_position,
    ),
    kept("mask-composite", "add", false, grammar::mask_composite),
    kept(
        "overscroll-behavior-block",
        "auto",
        false,
        grammar::overscroll,
    ),
    kept(
        "overscroll-behavior-inline",
        "auto",
        false,
        grammar::overscroll,
    ),
    kept("overscroll-behavior-x", "auto", false, grammar::overscroll),
    kept("overscroll-behavior-y", "auto", false, grammar::overscroll),
    kept("resize", "none", false, grammar::resize),
    kept(
        "scroll-padding-bottom",
        "auto",
        false,
        grammar::scroll_padding,
    ),
    kept(
        "scroll-padding-inline-end",
        "auto",
        false,
        grammar::scroll_padding,
    ),
    kept(
        "scroll-padding-inline-start",
        "auto",
        false,
        grammar::scroll_padding,
    ),
    kept(
        "scroll-padding-left",
        "auto",
        false,
        grammar::scroll_padding,
    ),
    kept(
        "scroll-padding-right",
        "auto",
        false,
        grammar::scroll_padding,
    ),
    kept("scroll-padding-top", "auto", false, grammar::scroll_padding),
    kept(
        "scroll-padding-block-end",
        "auto",
        false,
        grammar::scroll_padding,
    ),
    kept(
        "scroll-padding-block-start",
        "auto",
        false,
        grammar::scroll_padding,
    ),
    kept(
        "scroll-snap-align",
        "none",
        false,
        grammar::scroll_snap_align,
    ),
    kept(
        "scroll-snap-stop",
        "normal",
        false,
        grammar::scroll_snap_stop,
    ),
    kept("scrollbar-color", "auto", true, grammar::scrollbar_color),
    kept("scrollbar-gutter", "auto", false, grammar::scrollbar_gutter),
    kept("scrollbar-width", "auto", true, grammar::scrollbar_width),
    kept("text-size-adjust", "auto", true, grammar::text_size_adjust),
    kept("touch-action", "auto", false, grammar::touch_action),
    kept("user-select", "auto", false, grammar::user_select),
    kept(
        "view-transition-name",
        "none",
        false,
        grammar::view_transition_name,
    ),
];

/** Builds a table entry; a function so the registry reads as one row per property. */
const fn kept(name: &'static str, initial: &'static str, inherited: bool, check: Check) -> Kept {
    Kept {
        name,
        initial,
        inherited,
        check,
    }
}

/** The registry row for `name`, if Eye keeps that property. */
fn entry(name: &str) -> Option<&'static Kept> {
    KEPT.iter().find(|kept| kept.name == name)
}

/** Whether a kept property inherits by default, so `inherited()` carries it to children. */
pub(super) fn inherits(name: &str) -> bool {
    entry(name).is_some_and(|kept| kept.inherited)
}

/**
 * Validates one declaration and stores its canonical value. Ok(false) means the name is not
 * a kept property, so the caller keeps looking; an invalid value is an error and is dropped.
 */
pub(super) fn apply(
    style: &mut ComputedStyle,
    key: &str,
    value: &str,
    context: RenderContext,
) -> EngineResult<bool> {
    let Some(kept) = entry(key) else {
        return Ok(false);
    };
    let canonical = (kept.check)(value, style, context)?;
    style.kept_values.insert(key.to_string(), canonical);
    Ok(true)
}

/**
 * Copies a kept property for a CSS-wide keyword: the parent's stored value, or nothing, which
 * `computed()` reads as the initial value. Ok(false) when `key` is not kept.
 */
pub(super) fn copy(style: &mut ComputedStyle, source: &ComputedStyle, key: &str) -> bool {
    if entry(key).is_none() {
        return false;
    }
    match source.kept_values.get(key) {
        Some(value) => style.kept_values.insert(key.to_string(), value.clone()),
        None => style.kept_values.remove(key),
    };
    true
}

/** The values a child inherits from its parent: the inherited kept properties it stores. */
pub(super) fn inherited(parent: &ComputedStyle) -> BTreeMap<String, String> {
    parent
        .kept_values
        .iter()
        .filter(|(name, _)| inherits(name))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

/**
 * The computed value `getComputedStyle` reports for a kept property: the stored canonical value,
 * or the initial value. None when `name` is not kept.
 */
pub(super) fn computed(style: &ComputedStyle, name: &str) -> Option<String> {
    entry(name).map(|kept| {
        style
            .kept_values
            .get(name)
            .cloned()
            .unwrap_or_else(|| kept.initial.to_string())
    })
}
