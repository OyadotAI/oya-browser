/** `text-decoration-style`, `text-decoration-thickness` and `text-underline-offset`: how the underline, overline and line-through paint. */
use super::{
    ComputedStyle, DecorationStyle, unsupported,
    values::{contextual_length, signed_length},
};
use crate::{document::RenderContext, protocol::EngineResult};

/** Applies one decoration longhand, or reports that `key` belongs elsewhere. */
pub(super) fn apply(
    style: &mut ComputedStyle,
    key: &str,
    value: &str,
    context: RenderContext,
) -> EngineResult<bool> {
    match key {
        "text-decoration-style" => style.decoration_style = decoration_style(value)?,
        "text-decoration-thickness" => {
            style.decoration_thickness = thickness(value, style.font_size, context)?
        }
        "text-underline-offset" => {
            style.underline_offset = match value {
                "auto" => None,
                _ => Some(signed_length(value, style.font_size, context)?),
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/** The keyword `text-decoration-style` names; any other value is invalid and the declaration drops. */
fn decoration_style(value: &str) -> EngineResult<DecorationStyle> {
    Ok(match value {
        "solid" => DecorationStyle::Solid,
        "double" => DecorationStyle::Double,
        "dotted" => DecorationStyle::Dotted,
        "dashed" => DecorationStyle::Dashed,
        "wavy" => DecorationStyle::Wavy,
        _ => return Err(unsupported()),
    })
}

/** `text-decoration-thickness`: a length, or `auto`/`from-font` for the font's own (None). */
fn thickness(value: &str, font: f64, context: RenderContext) -> EngineResult<Option<f64>> {
    match value {
        "auto" | "from-font" => Ok(None),
        _ => contextual_length(value, font, context).map(Some),
    }
}
