/** Visual properties the painter consumes; each returns Ok(false) for names it does not own. */
use super::{
    ComputedStyle, FontFamily, Overflow, Position, TextTransform, WhiteSpace,
    constants::{LETTER_SPACING_MAX, PERCENT, RADIUS_HALF, RADIUS_PERCENT_BASIS},
    unsupported,
    values::{dimension, signed_length},
};
use crate::{constants::CSS_MAX_SCALAR, document::RenderContext, protocol::EngineResult};
/** Applies one visual longhand, or reports that `key` belongs elsewhere. */
pub(super) fn apply(
    style: &mut ComputedStyle,
    key: &str,
    value: &str,
    context: RenderContext,
    diagnostics: &mut Vec<String>,
) -> EngineResult<bool> {
    match key {
        "position" => style.position = position(value)?,
        "top" | "right" | "bottom" | "left" => {
            let side = super::shorthand::SIDES
                .iter()
                .position(|side| *side == key)
                .ok_or_else(unsupported)?;
            (style.inset[side], style.inset_percent[side]) =
                inset(value, style.font_size, context)?;
        }
        "z-index" => style.z_index = z_index(value)?,
        "overflow-x" | "overflow-y" => style.overflow = style.overflow.max(overflow(value)?),
        "box-shadow" => style.shadows = super::paint::shadows(value, style, context)?,
        "background-image" => {
            style.backgrounds = super::paint::backgrounds(value, style, context, diagnostics)?
        }
        "transform" => {
            (style.transform, style.translate_percent) =
                super::paint::transform(value, style.font_size, context)?
        }
        "text-decoration" | "text-decoration-line" => {
            let none = value.split_whitespace().any(|part| part == "none");
            style.text_decoration_none = none;
            if none {
                style.underline = false;
                style.strikethrough = false;
            } else {
                style.underline |= value.split_whitespace().any(|part| part == "underline");
                style.strikethrough |= value.split_whitespace().any(|part| part == "line-through");
            }
        }
        "letter-spacing" => style.letter_spacing = letter_spacing(value, style.font_size, context)?,
        "text-transform" => style.text_transform = text_transform(value)?,
        "white-space" => style.white_space = white_space(value)?,
        "cursor" => style.cursor_pointer = value.split(',').any(|part| part.trim() == "pointer"),
        "font-family" => {
            style.font_family = font_family(value);
            style.font_families = font_families(value);
        }
        // Paint keeps only the clipped region. The box stays out of agent reach, as before: the
        // pattern is a visually hidden label whose text still names its visible link, which stays
        // actionable, and a sliver left by a percentage inset is nothing a person could click.
        "clip" if value.starts_with("rect(") => {
            style.clip_rect = Some(super::clip::rect(value, style.font_size, context)?);
            style.interactive = false;
        }
        "clip-path" if value.starts_with("inset(") => {
            style.clip_path = Some(super::clip::inset(value, style.font_size, context)?);
            style.interactive = false;
        }
        "clip" if value == "auto" => style.clip_rect = None,
        "clip-path" if value == "none" => style.clip_path = None,
        // Mask icons would paint as solid boxes without the mask; like a dropped mask the
        // box stays out of agent reach, as before.
        "mask-image" => {
            let none = value.eq_ignore_ascii_case("none");
            style.masked = !none;
            style.interactive &= none;
            style.mask = super::paint::mask_url(value).map(|url| super::MaskLayer {
                url,
                ..Default::default()
            });
        }
        // Geometry follows the image: the cascade applies `mask-image` first in name order.
        "mask-size" | "mask-position" | "mask-repeat" => {
            let mut layer = style.mask.take().unwrap_or_default();
            super::layout_props::mask_geometry(&mut layer, key, value, style.font_size, context)?;
            style.mask = (!layer.url.is_empty()).then_some(layer);
        }
        "fill" | "stroke" | "stroke-width" | "stroke-linejoin" | "stroke-linecap" | "fill-rule"
        | "fill-opacity" | "stroke-opacity" => {}
        _ => return radius(style, key, value, context),
    }
    Ok(true)
}
/** Positioning keywords. */
fn position(value: &str) -> EngineResult<Position> {
    Ok(match value {
        "static" => Position::Static,
        "relative" => Position::Relative,
        "absolute" => Position::Absolute,
        "fixed" => Position::Fixed,
        "sticky" | "-webkit-sticky" => Position::Sticky,
        _ => return Err(unsupported()),
    })
}
/** `auto` is None; percentages store in `inset_percent` and lengths clamp so far offsets stay off-screen. */
fn inset(value: &str, font: f64, context: RenderContext) -> EngineResult<(Option<f64>, f64)> {
    if value == "auto" {
        return Ok((None, 0.0));
    }
    let length = dimension(value, font, context)?;
    let clamp = |v: f64| {
        if v.is_finite() {
            Ok(v.clamp(-CSS_MAX_SCALAR, CSS_MAX_SCALAR))
        } else {
            Err(unsupported())
        }
    };
    let percent = clamp(length.percent)?;
    if percent != 0.0 && length.px == 0.0 {
        return Ok((None, percent));
    }
    Ok((Some(clamp(length.px)?), percent))
}
/** Integers clamp into i32; `auto` is None. */
fn z_index(value: &str) -> EngineResult<Option<i32>> {
    if value == "auto" {
        return Ok(None);
    }
    let number: i64 = value.parse().map_err(|_| unsupported())?;
    Ok(Some(
        number.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    ))
}
/** Overflow keywords; `overlay` is the legacy spelling of auto. */
fn overflow(value: &str) -> EngineResult<Overflow> {
    Ok(match value {
        "visible" => Overflow::Visible,
        "hidden" => Overflow::Hidden,
        "clip" => Overflow::Clip,
        "scroll" => Overflow::Scroll,
        "auto" | "overlay" => Overflow::Auto,
        _ => return Err(unsupported()),
    })
}
/** `normal` is zero; lengths are signed and bounded by the largest font size. */
fn letter_spacing(value: &str, font: f64, context: RenderContext) -> EngineResult<f64> {
    if value == "normal" {
        return Ok(0.0);
    }
    let spacing = signed_length(value, font, context)?;
    if spacing.abs() <= LETTER_SPACING_MAX {
        Ok(spacing)
    } else {
        Err(unsupported())
    }
}
/** Case mapping keywords. */
fn text_transform(value: &str) -> EngineResult<TextTransform> {
    Ok(match value {
        "none" => TextTransform::None,
        "uppercase" => TextTransform::Uppercase,
        "lowercase" => TextTransform::Lowercase,
        "capitalize" => TextTransform::Capitalize,
        _ => return Err(unsupported()),
    })
}
/** Whitespace keywords; `break-spaces` behaves as pre-wrap here. */
fn white_space(value: &str) -> EngineResult<WhiteSpace> {
    Ok(match value {
        "normal" => WhiteSpace::Normal,
        "pre" => WhiteSpace::Pre,
        "nowrap" => WhiteSpace::Nowrap,
        "pre-wrap" | "break-spaces" => WhiteSpace::PreWrap,
        "pre-line" => WhiteSpace::PreLine,
        _ => return Err(unsupported()),
    })
}
/** The authored names, lowercased and unquoted, for `@font-face` matching; bounded in count and length. */
pub(super) fn font_families(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|name| name.trim().trim_matches(['"', '\'']).to_ascii_lowercase())
        .filter(|name| !name.is_empty() && name.len() <= super::font_face::FONT_NAME_BYTES)
        .take(super::font_face::FONT_FAMILY_NAMES)
        .collect()
}
/** The first family that names a known generic or common face picks the generic; else sans. */
pub(super) fn font_family(value: &str) -> FontFamily {
    for name in value.split(',') {
        let name = name.trim().trim_matches(['"', '\'']).to_ascii_lowercase();
        let family = match name.as_str() {
            "serif" | "times" | "times new roman" | "georgia" | "garamond" | "cambria" => {
                FontFamily::Serif
            }
            "monospace" | "courier" | "courier new" | "menlo" | "monaco" | "consolas"
            | "ui-monospace" | "sfmono-regular" | "sf mono" | "dejavu sans mono" => {
                FontFamily::Monospace
            }
            "system-ui" | "-apple-system" | "blinkmacsystemfont" | "ui-sans-serif" => {
                FontFamily::SystemUi
            }
            "sans-serif" | "helvetica" | "helvetica neue" | "arial" | "verdana" | "roboto"
            | "inter" | "segoe ui" | "tahoma" => FontFamily::SansSerif,
            _ => continue,
        };
        return family;
    }
    FontFamily::SansSerif
}
/** Corner radii; percentages use the authored px width when known. */
fn radius(
    style: &mut ComputedStyle,
    key: &str,
    value: &str,
    context: RenderContext,
) -> EngineResult<bool> {
    let corners = [
        "border-top-left-radius",
        "border-top-right-radius",
        "border-bottom-right-radius",
        "border-bottom-left-radius",
    ];
    let Some(corner) = corners.iter().position(|corner| *corner == key) else {
        return Ok(false);
    };
    let first = value.split_whitespace().next().ok_or_else(unsupported)?;
    let length = dimension(first, style.font_size, context)?;
    let basis = match style.width {
        Some(width) if !style.percent_width => width,
        _ if length.percent >= RADIUS_HALF => CSS_MAX_SCALAR * PERCENT / RADIUS_HALF,
        _ => RADIUS_PERCENT_BASIS,
    };
    let px = length.of(basis);
    if !px.is_finite() || px < 0.0 {
        return Err(unsupported());
    }
    style.radius[corner] = px.min(CSS_MAX_SCALAR);
    style.radius_percent[corner] = if style.width.is_some() && !style.percent_width {
        0.0
    } else {
        length.percent.clamp(0.0, CSS_MAX_SCALAR)
    };
    Ok(true)
}
