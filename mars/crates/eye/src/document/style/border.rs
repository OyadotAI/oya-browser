/** Per-side borders resolve width, style and color together, since a `none` style zeroes the width. */
use super::{
    ComputedStyle,
    constants::{BORDER_MEDIUM, BORDER_THICK, BORDER_THIN},
    shorthand::{BORDER_STYLES, SIDES},
    unsupported,
    values::{contextual_color, contextual_length},
};
use crate::{
    constants::{CSS_MAX_BORDER_WIDTH, CSS_ROOT_FONT_SIZE},
    document::RenderContext,
    protocol::EngineResult,
};
use std::collections::BTreeMap;
/** Width keywords or a nonnegative length, before the style is known. */
pub(super) fn width(value: &str) -> EngineResult<f64> {
    width_in(value, CSS_ROOT_FONT_SIZE, RenderContext::default())
}
/** Width keywords or a nonnegative length against the element font. */
fn width_in(value: &str, font: f64, context: RenderContext) -> EngineResult<f64> {
    match value {
        "thin" => Ok(BORDER_THIN),
        "medium" => Ok(BORDER_MEDIUM),
        "thick" => Ok(BORDER_THICK),
        "auto" => Err(unsupported()),
        _ => contextual_length(value, font, context),
    }
}
/** Whether `key` is a per-side border longhand, and which side and part. */
pub(super) fn longhand(key: &str) -> Option<(usize, &str)> {
    let rest = key.strip_prefix("border-")?;
    let (side, part) = rest.split_once('-')?;
    let side = SIDES.iter().position(|s| *s == side)?;
    ["width", "style", "color"]
        .contains(&part)
        .then_some((side, part))
}
/** Parse-time validation of one longhand. */
pub(super) fn check(part: &str, value: &str) -> EngineResult<()> {
    match part {
        "width" => width(value).map(|_| ()),
        "style" if BORDER_STYLES.contains(&value) => Ok(()),
        "color" => contextual_color(value, crate::document::Appearance::Light, None).map(|_| ()),
        _ => Err(unsupported()),
    }
}
/** Authored per-side values for one element. */
#[derive(Default)]
pub(super) struct Sides {
    /// Width text per side.
    width: [Option<String>; 4],
    /// Style keyword per side.
    style: [Option<String>; 4],
    /// Color text per side.
    color: [Option<String>; 4],
}
/** Moves every border longhand out of the cascade winners. */
pub(super) fn take(chosen: &mut BTreeMap<String, String>) -> Sides {
    let mut sides = Sides::default();
    let keys: Vec<String> = chosen
        .keys()
        .filter(|key| longhand(key).is_some())
        .cloned()
        .collect();
    for key in keys {
        let Some((side, part)) = longhand(&key) else {
            continue;
        };
        let value = chosen.remove(&key);
        let slot = match part {
            "width" => &mut sides.width,
            "style" => &mut sides.style,
            _ => &mut sides.color,
        };
        slot[side] = value;
    }
    sides
}
/**
 * Computes painted widths and colors; form controls default to a visible style as their UA
 * sheet does. `border` stays the uniform width (0 when sides differ) for the existing painter.
 */
pub(super) fn resolve(
    style: &mut ComputedStyle,
    sides: &Sides,
    context: RenderContext,
    control: bool,
) {
    let authored = sides.style.iter().any(Option::is_some);
    let mut none = 0;
    for side in 0..4 {
        let kind = sides.style[side]
            .as_deref()
            .unwrap_or(if control { "solid" } else { "none" });
        let invisible = ["none", "hidden"].contains(&kind);
        none += usize::from(invisible);
        let width = sides.width[side]
            .as_deref()
            .map_or(Ok(BORDER_MEDIUM), |w| width_in(w, style.font_size, context))
            .unwrap_or(0.0)
            .min(CSS_MAX_BORDER_WIDTH);
        let declared = sides.width[side].is_some() || sides.style[side].is_some();
        style.border_widths[side] = if invisible || !declared { 0.0 } else { width };
        let current = style.color.clone();
        style.border_colors[side] = sides.color[side]
            .as_deref()
            .and_then(|c| contextual_color(c, style.used_scheme, current.as_deref()).ok())
            .flatten();
    }
    style.border_style_none = authored && none == 4;
    aggregate(style);
}
/**
 * A CSS-wide keyword on one border longhand copies that side's computed part from `source`.
 * It runs after `resolve`, which would otherwise reset the side, then refreshes the summaries.
 */
pub(super) fn copy(style: &mut ComputedStyle, source: &ComputedStyle, side: usize, part: &str) {
    match part {
        "width" => style.border_widths[side] = source.border_widths[side],
        "color" => style.border_colors[side] = source.border_colors[side].clone(),
        _ => {
            style.border_widths[side] = source.border_widths[side];
            style.border_style_none = source.border_style_none;
        }
    }
    aggregate(style);
}
/** The uniform `border` width (0 when sides differ) and the first painted color the painter reads. */
fn aggregate(style: &mut ComputedStyle) {
    let first = style.border_widths[0];
    style.border = if style.border_widths.iter().all(|w| *w == first) {
        first
    } else {
        0.0
    };
    style.border_color = style
        .border_colors
        .iter()
        .zip(style.border_widths)
        .find(|(color, width)| color.is_some() && *width > 0.0)
        .or_else(|| {
            style
                .border_colors
                .iter()
                .zip(style.border_widths)
                .find(|(c, _)| c.is_some())
        })
        .and_then(|(color, _)| color.clone());
}
