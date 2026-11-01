/** `outline` and its longhands: a ring painted outside the border box, which never changes layout. */
use super::{
    ComputedStyle,
    border::{BorderStyle, width_in},
    shorthand::BORDER_STYLES,
    unsupported,
    values::{contextual_color, signed_length},
};
use crate::{constants::CSS_MAX_BORDER_WIDTH, document::RenderContext, protocol::EngineResult};

/** Applies one outline longhand, or reports that `key` belongs elsewhere. */
pub(super) fn apply(
    style: &mut ComputedStyle,
    key: &str,
    value: &str,
    context: RenderContext,
) -> EngineResult<bool> {
    match key {
        "outline-style" => style.outline_style = outline_style(value)?,
        "outline-width" => {
            style.outline_width =
                width_in(value, style.font_size, context)?.min(CSS_MAX_BORDER_WIDTH)
        }
        "outline-color" => {
            style.outline_color =
                contextual_color(value, style.used_scheme, style.color.as_deref())?
        }
        "outline-offset" => style.outline_offset = signed_length(value, style.font_size, context)?,
        _ => return Ok(false),
    }
    Ok(true)
}

/**
 * The painted line an `outline-style` names: `none` and `hidden` paint nothing (None), `auto`
 * paints as a solid ring since Eye has no focus ring of its own, and the rest are border styles.
 */
fn outline_style(value: &str) -> EngineResult<Option<BorderStyle>> {
    match value {
        "none" | "hidden" => Ok(None),
        "auto" => Ok(Some(BorderStyle::Solid)),
        _ if BORDER_STYLES.contains(&value) => Ok(Some(BorderStyle::of(value))),
        _ => Err(unsupported()),
    }
}
