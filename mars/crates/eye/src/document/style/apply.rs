/** Supported declaration values update computed fields; Ok(false) means the property is unknown. */
use super::{
    ColorScheme, ComputedStyle, TableBox,
    constants::{WEIGHT_BOLD, WEIGHT_BOLD_FACE, WEIGHT_MAX, WEIGHT_MIN, WEIGHT_NORMAL},
    note, unsupported,
    values::{contextual_color, contextual_length, number},
};
use crate::{
    constants::{CSS_MAX_FONT_SIZE, CSS_MAX_SCALAR, CSS_MIN_FONT_SIZE, CSS_ROOT_FONT_SIZE},
    document::RenderContext,
    protocol::EngineResult,
};
/** Applies one longhand; values are lowercased except where URLs keep their case. */
pub(super) fn apply(
    style: &mut ComputedStyle,
    key: &str,
    value: &str,
    context: RenderContext,
    diagnostics: &mut Vec<String>,
) -> EngineResult<bool> {
    let raw = value.trim();
    let lower = raw.to_ascii_lowercase();
    let value = if key == "background-image" || key == "mask-image" {
        raw
    } else {
        lower.as_str()
    };
    // Animation longhands are consumed by the cascade's resting-frame step, never computed.
    if super::animation::LONGHANDS.contains(&key) {
        return Ok(true);
    }
    if let Some((_, part)) = super::border::longhand(key) {
        super::border::check(part, value)?;
        return Ok(true);
    }
    if key == "min-width" && !["auto", "min-content", "max-content", "fit-content"].contains(&value)
    {
        style.min_width = Some(contextual_length(value, style.font_size, context)?);
        return Ok(true);
    }
    if super::layout_props::apply(style, key, value, context)?
        || super::flex::apply(style, key, value)?
        || super::visual::apply(style, key, value, context, diagnostics)?
    {
        return Ok(true);
    }
    if box_side(style, key, value, context)? || text(style, key, value, context)? {
        return Ok(true);
    }
    layout(style, key, value, context, diagnostics)
}
/**
 * Margins (signed, `auto` allowed) and paddings (nonnegative). A percentage, alone or inside
 * `calc()`, is kept apart from the pixels: CSS resolves it against the containing block's width
 * on every side, which only layout knows.
 */
fn box_side(
    style: &mut ComputedStyle,
    key: &str,
    value: &str,
    context: RenderContext,
) -> EngineResult<bool> {
    let side = |prefix: &str| {
        super::shorthand::SIDES
            .iter()
            .position(|side| key.strip_prefix(prefix) == Some(side))
    };
    if let Some(side) = side("margin-") {
        style.margin_auto[side] = value == "auto";
        let (px, percent) = if value == "auto" {
            (0.0, 0.0)
        } else {
            side_length(value, style.font_size, context)?
        };
        // Extreme margins clamp, so a far off-screen shift stays off-screen and out of reach.
        style.margin[side] = px.clamp(-CSS_MAX_SCALAR, CSS_MAX_SCALAR);
        style.margin_percent[side] = percent.clamp(-CSS_MAX_SCALAR, CSS_MAX_SCALAR);
        return Ok(true);
    }
    if let Some(side) = side("padding-") {
        let (px, percent) = side_length(value, style.font_size, context)?;
        let bounded = |v: f64| (0.0..=CSS_MAX_SCALAR).contains(&v);
        if value == "auto" || !bounded(px) || !bounded(percent) {
            return Err(unsupported());
        }
        style.padding[side] = px;
        style.padding_percent[side] = percent;
        return Ok(true);
    }
    Ok(false)
}
/** A side length as finite pixels plus a percentage of the containing block's width. */
fn side_length(value: &str, font: f64, context: RenderContext) -> EngineResult<(f64, f64)> {
    let dimension = super::values::dimension(value, font, context)?;
    if !dimension.px.is_finite() || !dimension.percent.is_finite() {
        return Err(unsupported());
    }
    Ok((dimension.px, dimension.percent))
}
/** Font, color and text properties. */
fn text(
    style: &mut ComputedStyle,
    key: &str,
    value: &str,
    context: RenderContext,
) -> EngineResult<bool> {
    match key {
        "color-scheme" => style.color_scheme = color_scheme(value)?,
        "direction" => {
            style.rtl = match value {
                "rtl" => true,
                "ltr" => false,
                _ => return Err(unsupported()),
            }
        }
        "line-height" => style.line_height = line_height(value, style.font_size, context)?,
        "font-style" => {
            style.font_style = match value.split_whitespace().next().unwrap_or_default() {
                "normal" => super::FontStyle::Normal,
                "italic" => super::FontStyle::Italic,
                "oblique" => super::FontStyle::Oblique,
                _ => return Err(unsupported()),
            }
        }
        "font-size" => style.font_size = font_size(value, style.font_size, context)?,
        "font-weight" => set_weight(style, weight(value, style.font_weight)?),
        "color" => {
            if let Some(color) = contextual_color(value, style.used_scheme, style.color.as_deref())?
            {
                style.color = Some(color);
            }
        }
        "background-color" => {
            style.background = contextual_color(value, style.used_scheme, style.color.as_deref())?
        }
        "text-align" => {
            style.align = match value {
                "left" | "center" | "right" | "start" | "end" => value,
                "justify" | "justify-all" | "match-parent" => "start",
                "-webkit-center" => "webkit-center",
                _ => return Err(unsupported()),
            }
            .into()
        }
        _ => return Ok(false),
    }
    Ok(true)
}
/** Display, visibility, sizing, opacity and decorations Eye accepts without painting. */
fn layout(
    style: &mut ComputedStyle,
    key: &str,
    value: &str,
    context: RenderContext,
    diagnostics: &mut Vec<String>,
) -> EngineResult<bool> {
    match key {
        "display" => display(style, value, diagnostics)?,
        "visibility" => match value {
            "hidden" | "collapse" => style.hidden = true,
            "visible" => {}
            _ => return Err(unsupported()),
        },
        "pointer-events" => style.interactive = value != "none",
        "width" => width(style, value, context)?,
        "max-width" => {
            style.percent_max_width = value.ends_with('%');
            style.max_width = match value {
                "none" | "min-content" | "max-content" | "fit-content" => {
                    style.percent_max_width = false;
                    None
                }
                _ if style.percent_max_width => Some(number(value.trim_end_matches('%'))?),
                _ => Some(contextual_length(value, style.font_size, context)?),
            }
        }
        "opacity" => style.opacity = opacity(value)?,
        "outline" | "outline-color" | "outline-width" | "outline-style" | "outline-offset" => note(
            diagnostics,
            format!("Style decoration {key} is not painted."),
        ),
        _ => return Ok(false),
    }
    Ok(true)
}
/**
 * `width`: a keyword, a percentage, a length, or a `calc()` mixing both, whose px part waits in
 * `width_offset` until the percentage resolves against the containing block.
 */
fn width(style: &mut ComputedStyle, value: &str, context: RenderContext) -> EngineResult<()> {
    style.width_offset = 0.0;
    style.percent_width = value.ends_with('%');
    style.width = match value {
        "auto" | "min-content" | "max-content" | "fit-content" => {
            style.percent_width = false;
            None
        }
        _ if style.percent_width => Some(number(value.trim_end_matches('%'))?),
        _ if value.starts_with("calc(") => {
            let mixed = super::values::dimension(value, style.font_size, context)?;
            style.percent_width = mixed.percent != 0.0;
            style.width_offset = if style.percent_width { mixed.px } else { 0.0 };
            let main = if style.percent_width {
                mixed.percent
            } else {
                mixed.px
            };
            // A part past the computed-style bounds drops the declaration instead of failing
            // the whole cascade at validation.
            if !(0.0..=crate::constants::CSS_MAX_SCALAR).contains(&main) {
                return Err(unsupported());
            }
            Some(main)
        }
        _ => Some(contextual_length(value, style.font_size, context)?),
    };
    Ok(())
}
/** Display keywords; table boxes lay out as blocks for now. */
fn display(
    style: &mut ComputedStyle,
    value: &str,
    diagnostics: &mut Vec<String>,
) -> EngineResult<()> {
    let outer = value.split_whitespace().collect::<Vec<_>>().join(" ");
    match outer.as_str() {
        "none" => style.hidden = true,
        "inline" | "inline flow" => set_display(style, true, false, false, false),
        "inline-block" | "inline flow-root" | "inline-table" => {
            set_display(style, true, true, false, false)
        }
        "flex" | "block flex" | "-webkit-box" | "-webkit-flex" => {
            set_display(style, false, false, true, false)
        }
        "inline-flex" | "inline flex" | "-webkit-inline-box" | "-webkit-inline-flex" => {
            set_display(style, false, true, true, false)
        }
        "grid" | "block grid" => set_display(style, false, false, false, true),
        "inline-grid" | "inline grid" => set_display(style, false, true, false, true),
        "block" | "block flow" => set_display(style, false, false, false, false),
        "flow-root" | "block flow-root" => {
            set_display(style, false, false, false, false);
            style.flow_root = true;
        }
        "list-item" | "block list-item" | "block flow list-item" => {
            set_display(style, false, false, false, false);
            style.list_item = true;
        }
        "contents" => {
            set_display(style, true, false, false, false);
            style.display_contents = true;
        }
        "table" | "block table" => {
            set_display(style, false, false, false, false);
            style.table_box = TableBox::Table;
        }
        "table-row" => {
            set_display(style, false, false, false, false);
            style.table_box = TableBox::Row;
        }
        "table-cell" => {
            set_display(style, false, false, false, false);
            style.table_box = TableBox::Cell;
        }
        "table-caption" => {
            set_display(style, false, false, false, false);
            style.table_box = TableBox::Caption;
        }
        "table-row-group" | "table-header-group" | "table-footer-group" | "table-column"
        | "table-column-group" => {
            note(
                diagnostics,
                format!("CSS display {outer} lays out as block."),
            );
            set_display(style, false, false, false, false);
        }
        _ => return Err(unsupported()),
    }
    Ok(())
}
/** One display value sets every outer and inner display flag together, so none go stale. */
fn set_display(style: &mut ComputedStyle, inline: bool, atomic: bool, flex: bool, grid: bool) {
    style.inline = inline;
    style.inline_block = atomic;
    style.display_flex = flex;
    style.display_grid = grid;
    style.display_contents = false;
    style.list_item = false;
    style.table_box = TableBox::None;
    style.flow_root = false;
}
/** Offered color schemes. */
fn color_scheme(value: &str) -> EngineResult<ColorScheme> {
    Ok(
        match value
            .split_whitespace()
            .filter(|part| *part != "only")
            .collect::<Vec<_>>()
            .join(" ")
            .as_str()
        {
            "normal" => ColorScheme::Normal,
            "light" => ColorScheme::Light,
            "dark" => ColorScheme::Dark,
            "light dark" => ColorScheme::LightDark,
            "dark light" => ColorScheme::DarkLight,
            _ => return Err(unsupported()),
        },
    )
}
/** Keywords, lengths and percentages of the parent size, clamped into the supported range. */
fn font_size(value: &str, parent: f64, context: RenderContext) -> EngineResult<f64> {
    let keyword = [
        ("xx-small", 9.0),
        ("x-small", 10.0),
        ("small", 13.0),
        ("medium", CSS_ROOT_FONT_SIZE),
        ("large", 18.0),
        ("x-large", 24.0),
        ("xx-large", 32.0),
        ("xxx-large", 48.0),
        ("smaller", parent / 1.2),
        ("larger", parent * 1.2),
    ];
    let size = match keyword.iter().find(|(name, _)| *name == value) {
        Some((_, size)) => *size,
        None => super::values::dimension(value, parent, context)?.of(parent),
    };
    if !size.is_finite() || size < 0.0 {
        return Err(unsupported());
    }
    // `font-size: 0` hides text and the spaces between inline blocks, as in Chromium; only
    // sizes above it take the minimum.
    if size == 0.0 {
        return Ok(0.0);
    }
    Ok(size.clamp(CSS_MIN_FONT_SIZE, CSS_MAX_FONT_SIZE))
}
/** Numeric weights 1..1000, keywords, and bolder/lighter relative to the inherited weight. */
fn weight(value: &str, inherited: u16) -> EngineResult<u16> {
    Ok(match value {
        "normal" => WEIGHT_NORMAL,
        "bold" => WEIGHT_BOLD,
        "bolder" if inherited < 350 => WEIGHT_NORMAL,
        "bolder" if inherited < 550 => WEIGHT_BOLD,
        "bolder" => 900,
        "lighter" if inherited < 550 => 100,
        "lighter" if inherited < 750 => WEIGHT_NORMAL,
        "lighter" => WEIGHT_BOLD,
        _ => {
            let number: f64 = value.parse().map_err(|_| unsupported())?;
            if !(f64::from(WEIGHT_MIN)..=f64::from(WEIGHT_MAX)).contains(&number) {
                return Err(unsupported());
            }
            number.round() as u16
        }
    })
}
/** Keeps the legacy bold flag equal to the numeric weight's face. */
pub(super) fn set_weight(style: &mut ComputedStyle, weight: u16) {
    style.font_weight = weight;
    style.bold = weight >= WEIGHT_BOLD_FACE;
}
/** Numbers or percentages, clamped into 0..1 like CSS. */
fn opacity(value: &str) -> EngineResult<f64> {
    let opacity = match value.strip_suffix('%') {
        Some(percent) => number(percent)? / 100.0,
        None => number(value)?,
    };
    Ok(opacity.min(1.0))
}
/** Line heights in any unit or calc(); zero becomes the smallest positive advance the engine accepts. */
fn line_height(value: &str, font: f64, context: RenderContext) -> EngineResult<super::LineHeight> {
    if let Ok(height) = super::font::line_height(value, font) {
        return Ok(height);
    }
    let dimension = super::values::dimension(value, font, context)?;
    let advance = dimension.px + font * dimension.percent / 100.0;
    if !advance.is_finite() || !(0.0..=crate::constants::CSS_MAX_SCALAR).contains(&advance) {
        return Err(unsupported());
    }
    Ok(super::LineHeight::Length(
        advance.max(super::constants::LINE_HEIGHT_FLOOR),
    ))
}
