/**
 * Value grammars for the kept properties (`kept_values.rs`). Each check returns the canonical
 * text `getComputedStyle` reports, or drops the declaration, as Blink's parser does for values
 * outside its grammar. Keywords arrive lowercased; font tags and custom identifiers keep case.
 */
use super::{
    ComputedStyle,
    computed_css::css_color,
    constants::{FONT_TAG_BYTES, FONT_TAG_FIRST_BYTE, FONT_TAG_LAST_BYTE, SNAP_ALIGN_WORDS},
    syntax::{split_top, tokens},
    unsupported,
    values::{contextual_color, dimension, number, signed_number},
};
use crate::{document::RenderContext, protocol::EngineResult};

/** A single keyword that must be one of `allowed`; anything else drops the declaration. */
pub(super) fn one_of(value: &str, allowed: &[&str]) -> EngineResult<String> {
    if allowed.contains(&value) {
        Ok(value.to_string())
    } else {
        Err(unsupported())
    }
}

/** True when no word repeats, so `word word` never passes a set grammar. */
fn distinct(parts: &[&str]) -> bool {
    parts
        .iter()
        .enumerate()
        .all(|(i, part)| !parts[..i].contains(part))
}

/** Words drawn from `allowed`, each at most once and between `min` and `max` of them. */
fn word_set(value: &str, allowed: &[&str], (min, max): (usize, usize)) -> EngineResult<String> {
    let parts: Vec<&str> = value.split_whitespace().collect();
    let known = parts.iter().all(|part| allowed.contains(part));
    if (min..=max).contains(&parts.len()) && known && distinct(&parts) {
        Ok(parts.join(" "))
    } else {
        Err(unsupported())
    }
}

/** A comma list where every item is one of `allowed`, canonically joined with `, `. */
fn comma_list(value: &str, allowed: &[&str]) -> EngineResult<String> {
    let items = split_top(value, b',');
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        out.push(one_of(item.trim(), allowed)?);
    }
    Ok(out.join(", "))
}

/** `appearance`: the native control keywords Blink names, plus `none` and `auto`. */
pub(super) fn appearance(value: &str, _: &ComputedStyle, _: RenderContext) -> EngineResult<String> {
    one_of(
        value,
        &[
            "none",
            "auto",
            "button",
            "checkbox",
            "radio",
            "textfield",
            "menulist",
            "menulist-button",
            "searchfield",
            "textarea",
            "push-button",
            "square-button",
            "listbox",
            "meter",
            "progress-bar",
            "slider-horizontal",
            "slider-vertical",
            "inner-spin-button",
        ],
    )
}

/** `touch-action`: `auto`, `none`, `manipulation`, or pan axes and `pinch-zoom` in any order. */
pub(super) fn touch_action(
    value: &str,
    _: &ComputedStyle,
    _: RenderContext,
) -> EngineResult<String> {
    const PAN_X: [&str; 3] = ["pan-x", "pan-left", "pan-right"];
    const PAN_Y: [&str; 3] = ["pan-y", "pan-up", "pan-down"];
    if matches!(value, "auto" | "none" | "manipulation") {
        return Ok(value.into());
    }
    let parts: Vec<&str> = value.split_whitespace().collect();
    let axis = |group: &[&str]| parts.iter().filter(|part| group.contains(part)).count() <= 1;
    let known = parts
        .iter()
        .all(|part| PAN_X.contains(part) || PAN_Y.contains(part) || *part == "pinch-zoom");
    if !parts.is_empty() && known && distinct(&parts) && axis(&PAN_X) && axis(&PAN_Y) {
        Ok(parts.join(" "))
    } else {
        Err(unsupported())
    }
}

/** `user-select`. */
pub(super) fn user_select(
    value: &str,
    _: &ComputedStyle,
    _: RenderContext,
) -> EngineResult<String> {
    one_of(value, &["auto", "text", "none", "contain", "all"])
}

/** `overscroll-behavior` longhands: how a scroll chains past the box's edge on one axis. */
pub(super) fn overscroll(value: &str, _: &ComputedStyle, _: RenderContext) -> EngineResult<String> {
    one_of(value, &["auto", "contain", "none"])
}

/** `resize`. */
pub(super) fn resize(value: &str, _: &ComputedStyle, _: RenderContext) -> EngineResult<String> {
    one_of(
        value,
        &["none", "both", "horizontal", "vertical", "block", "inline"],
    )
}

/** `scrollbar-width`. */
pub(super) fn scrollbar_width(
    value: &str,
    _: &ComputedStyle,
    _: RenderContext,
) -> EngineResult<String> {
    one_of(value, &["auto", "thin", "none"])
}

/** `scrollbar-gutter`: `auto`, or `stable` with optional `both-edges`. */
pub(super) fn scrollbar_gutter(
    value: &str,
    _: &ComputedStyle,
    _: RenderContext,
) -> EngineResult<String> {
    match value {
        "auto" => Ok(value.into()),
        "stable" | "stable both-edges" => Ok(value.into()),
        "both-edges stable" => Ok("stable both-edges".into()),
        _ => Err(unsupported()),
    }
}

/** `scrollbar-color`: `auto`, or a thumb color and an optional track color. */
pub(super) fn scrollbar_color(
    value: &str,
    style: &ComputedStyle,
    _: RenderContext,
) -> EngineResult<String> {
    if value == "auto" {
        return Ok(value.into());
    }
    let colors = tokens(value);
    if !(1..=2).contains(&colors.len()) {
        return Err(unsupported());
    }
    let mut out = Vec::with_capacity(colors.len());
    for color in colors {
        let literal = contextual_color(color, style.used_scheme, style.color.as_deref())?;
        out.push(css_color(&literal.ok_or_else(unsupported)?));
    }
    Ok(out.join(" "))
}

/** `forced-color-adjust`. */
pub(super) fn forced_color_adjust(
    value: &str,
    _: &ComputedStyle,
    _: RenderContext,
) -> EngineResult<String> {
    one_of(value, &["auto", "none", "preserve-parent-color"])
}

/** `text-size-adjust`: `auto`, `none`, or a non-negative percentage. */
pub(super) fn text_size_adjust(
    value: &str,
    _: &ComputedStyle,
    _: RenderContext,
) -> EngineResult<String> {
    match value {
        "auto" | "none" => Ok(value.into()),
        _ => Ok(format!(
            "{}%",
            number(value.strip_suffix('%').ok_or_else(unsupported)?)?
        )),
    }
}

/** `font-synthesis`: `none`, or the faces the browser may fake, each at most once. */
pub(super) fn font_synthesis(
    value: &str,
    _: &ComputedStyle,
    _: RenderContext,
) -> EngineResult<String> {
    if value == "none" {
        return Ok(value.into());
    }
    word_set(
        value,
        &["weight", "style", "small-caps", "position"],
        (1, 4),
    )
}

/** `list-style-position`. */
pub(super) fn list_style_position(
    value: &str,
    _: &ComputedStyle,
    _: RenderContext,
) -> EngineResult<String> {
    one_of(value, &["inside", "outside"])
}

/** `background-blend-mode`: one blend mode per background layer. */
pub(super) fn background_blend_mode(
    value: &str,
    _: &ComputedStyle,
    _: RenderContext,
) -> EngineResult<String> {
    comma_list(
        value,
        &[
            "normal",
            "multiply",
            "screen",
            "overlay",
            "darken",
            "lighten",
            "color-dodge",
            "color-burn",
            "hard-light",
            "soft-light",
            "difference",
            "exclusion",
            "hue",
            "saturation",
            "color",
            "luminosity",
        ],
    )
}

/** `mask-composite`: how each mask layer combines with the ones below it. */
pub(super) fn mask_composite(
    value: &str,
    _: &ComputedStyle,
    _: RenderContext,
) -> EngineResult<String> {
    comma_list(value, &["add", "subtract", "intersect", "exclude"])
}

/** `view-transition-name`: `none`, or a custom identifier that keeps its case. */
pub(super) fn view_transition_name(
    value: &str,
    _: &ComputedStyle,
    _: RenderContext,
) -> EngineResult<String> {
    if value == "none" {
        return Ok(value.into());
    }
    let mut chars = value.chars();
    let starts_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '-');
    let rest_ok = chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if starts_ok && rest_ok && !matches!(value.to_ascii_lowercase().as_str(), "default" | "none") {
        Ok(value.into())
    } else {
        Err(unsupported())
    }
}

/** `scroll-snap-align`: `none`, or one or two of `start`, `end`, `center`. */
pub(super) fn scroll_snap_align(
    value: &str,
    _: &ComputedStyle,
    _: RenderContext,
) -> EngineResult<String> {
    if value == "none" {
        return Ok(value.into());
    }
    word_set(value, &["start", "end", "center"], (1, SNAP_ALIGN_WORDS))
}

/** `scroll-snap-stop`. */
pub(super) fn scroll_snap_stop(
    value: &str,
    _: &ComputedStyle,
    _: RenderContext,
) -> EngineResult<String> {
    one_of(value, &["normal", "always"])
}

/** A scroll-padding side: `auto`, or a non-negative length and percentage, canonically spelled. */
pub(super) fn scroll_padding(
    value: &str,
    style: &ComputedStyle,
    context: RenderContext,
) -> EngineResult<String> {
    if value == "auto" {
        return Ok(value.into());
    }
    let length = dimension(value, style.font_size, context)?;
    if !length.px.is_finite()
        || !length.percent.is_finite()
        || length.px < 0.0
        || length.percent < 0.0
    {
        return Err(unsupported());
    }
    Ok(match (length.px, length.percent) {
        (px, 0.0) => format!("{px}px"),
        (0.0, percent) => format!("{percent}%"),
        (px, percent) => format!("calc({px}px + {percent}%)"),
    })
}

/**
 * The quoted four-character tag and the rest of a font-settings item, as `(tag, rest)`. Blink
 * requires printable ASCII in the tag and no other shape.
 */
fn quoted_tag(item: &str) -> EngineResult<(&str, &str)> {
    let quote = item.chars().next().ok_or_else(unsupported)?;
    if quote != '\'' && quote != '"' {
        return Err(unsupported());
    }
    let close = item[1..].find(quote).ok_or_else(unsupported)? + 1;
    let tag = &item[1..close];
    let printable = tag
        .bytes()
        .all(|b| (FONT_TAG_FIRST_BYTE..=FONT_TAG_LAST_BYTE).contains(&b));
    if tag.len() != FONT_TAG_BYTES || !printable {
        return Err(unsupported());
    }
    Ok((tag, item[close + 1..].trim()))
}

/** `font-feature-settings`: `normal`, or tags with `on`, `off` or a non-negative integer. */
pub(super) fn font_feature_settings(
    value: &str,
    _: &ComputedStyle,
    _: RenderContext,
) -> EngineResult<String> {
    if value.eq_ignore_ascii_case("normal") {
        return Ok("normal".into());
    }
    let mut out = Vec::new();
    for item in split_top(value, b',') {
        let (tag, rest) = quoted_tag(item.trim())?;
        let setting = match rest.to_ascii_lowercase().as_str() {
            "" | "on" => 1.0,
            "off" => 0.0,
            integer => match integer.parse::<u32>() {
                Ok(n) => f64::from(n),
                Err(_) => return Err(unsupported()),
            },
        };
        out.push(format!("\"{tag}\" {setting}"));
    }
    Ok(out.join(", "))
}

/** `font-variation-settings`: `normal`, or tags with a signed number each. */
pub(super) fn font_variation_settings(
    value: &str,
    _: &ComputedStyle,
    _: RenderContext,
) -> EngineResult<String> {
    if value.eq_ignore_ascii_case("normal") {
        return Ok("normal".into());
    }
    let mut out = Vec::new();
    for item in split_top(value, b',') {
        let (tag, rest) = quoted_tag(item.trim())?;
        out.push(format!("\"{tag}\" {}", signed_number(rest)?));
    }
    Ok(out.join(", "))
}
