/** Supported declaration values update computed fields; Ok(false) means the property is unknown. */
use super::{
    ColorScheme, ComputedStyle, SizeKeyword, TableBox,
    constants::{WEIGHT_BOLD, WEIGHT_BOLD_FACE, WEIGHT_MAX, WEIGHT_MIN, WEIGHT_NORMAL},
    note, unsupported,
    values::{contextual_color, contextual_length, number},
};
use crate::{
    constants::{CSS_MAX_FONT_SIZE, CSS_MAX_SCALAR, CSS_MIN_FONT_SIZE},
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
    // Font tags, custom identifiers and image URLs keep their case; every other keyword is
    // matched lowercased.
    let value = if [
        "background-image",
        "mask-image",
        "border-image-source",
        "font-feature-settings",
        "font-variation-settings",
        "view-transition-name",
        "list-style-type",
    ]
    .contains(&key)
    {
        raw
    } else {
        lower.as_str()
    };
    let sized;
    let value = match super::calc_size::KEYS.iter().position(|name| *name == key) {
        Some(index) => {
            let font = style.font_size;
            sized = super::calc_size::split(style, index, value, (font, context))?;
            sized.as_str()
        }
        None => value,
    };
    // Animation longhands are retained by the cascade after atomic declaration validation.
    if super::animation::LONGHANDS.contains(&key) {
        return if super::animation::valid_value(key, value) {
            Ok(true)
        } else {
            Err(super::unsupported())
        };
    }
    if super::transition::apply(&mut style.transition, key, value)? {
        return Ok(true);
    }
    if let Some((_, part)) = super::border::longhand(key) {
        super::border::check(part, value)?;
        return Ok(true);
    }
    if key == "min-width" {
        let keyword = size_keyword(value, style.font_size, context)?;
        style.size_keywords[1] = keyword;
        style.min_width_percent = None;
        style.min_width = None;
        style.min_width_offset = 0.0;
        if let Some(percent) = value.strip_suffix('%') {
            style.min_width_percent = Some(number(percent)?);
        } else if value.starts_with("calc(") {
            let (main, percent, offset) = mixed_calc(value, style.font_size, context)?;
            match percent {
                true => (style.min_width_percent, style.min_width_offset) = (Some(main), offset),
                false => style.min_width = Some(main),
            }
        } else if keyword == SizeKeyword::None && value != "auto" {
            style.min_width = Some(contextual_length(value, style.font_size, context)?);
        }
        return Ok(true);
    }
    if key == "overflow-clip-margin" {
        (style.overflow_clip_box, style.overflow_clip_margin) =
            overflow_clip_margin(value, style.font_size, context)?;
        return Ok(true);
    }
    if key == "filter" {
        style.multicol.spanner_barrier = value != "none";
    }
    if key == "content-visibility" && matches!(value, "auto" | "visible") {
        // `content-visibility: auto` turns on layout, style and paint containment (css-contain-2),
        // so the box clips its contents, as a nested scroller's marker group shows. `hidden`
        // stays unsupported, so its rule keeps masking what it may hide.
        style.contain_paint = value == "auto";
        return Ok(true);
    }
    if key == "contain" {
        // Paint containment clips like `overflow: clip`; `contain` still groups the box below.
        style.contain_paint = value
            .split_whitespace()
            .any(|word| ["paint", "strict", "content"].contains(&word));
        style.multicol.spanner_barrier = value
            .split_whitespace()
            .any(|word| ["layout", "strict", "content"].contains(&word));
        style.multicol.contain_size = value
            .split_whitespace()
            .any(|word| ["size", "strict"].contains(&word));
    }
    if super::layout_props::apply(style, key, value, context)?
        || super::containment::apply(style, key, value, context)?
        || super::text_box::apply(style, key, value)?
        || super::multicol::apply(style, key, value, context)?
        || super::flex::apply(style, key, value, context)?
        || super::visual::apply(style, key, value, context, diagnostics)?
        || super::outline::apply(style, key, value, context)?
        || super::decoration::apply(style, key, value, context)?
        || super::border_image::apply(style, key, value, context)?
        || super::kept_values::apply(style, key, value, context)?
    {
        return Ok(true);
    }
    if box_side(style, key, value, context)? || text(style, key, value, context)? {
        return Ok(true);
    }
    layout(style, key, value, context, diagnostics)
}
/** `overflow-clip-margin: <visual-box> || <length>` as (reference box, px): 0 is the padding box
 * (the initial), 1 the border box, 2 the content box. Blink takes any length (ValueRange::kAll),
 * so a negative one pulls the clip edge inside its reference box. */
fn overflow_clip_margin(value: &str, font: f64, context: RenderContext) -> EngineResult<(u8, f64)> {
    let (mut reference, mut margin) = (None, None);
    for word in value.split_whitespace() {
        let boxed = match word {
            "padding-box" => Some(0),
            "border-box" => Some(1),
            "content-box" => Some(2),
            _ => None,
        };
        match boxed {
            Some(_) if reference.is_some() => return Err(unsupported()),
            Some(boxed) => reference = Some(boxed),
            None if margin.is_some() => return Err(unsupported()),
            None => margin = Some(super::values::signed_length(word, font, context)?),
        }
    }
    if reference.is_none() && margin.is_none() {
        return Err(unsupported());
    }
    Ok((reference.unwrap_or(0), margin.unwrap_or(0.0)))
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
        "writing-mode" => {
            style.vertical_writing = match value {
                "horizontal-tb" | "lr" | "lr-tb" | "rl" | "rl-tb" => false,
                "vertical-rl" | "vertical-lr" | "sideways-rl" | "sideways-lr" | "tb" | "tb-rl" => {
                    true
                }
                _ => return Err(unsupported()),
            };
            style.writing_lr = matches!(value, "vertical-lr" | "sideways-lr");
            style.writing_sideways = value.starts_with("sideways");
        }
        "text-orientation" | "-webkit-text-orientation" => {
            style.text_orientation = match value {
                "mixed" | "vertical-right" => 0,
                "upright" => 1,
                "sideways" | "sideways-right" => 2,
                _ => return Err(unsupported()),
            }
        }
        "direction" => {
            style.rtl = match value {
                "rtl" => true,
                "ltr" => false,
                _ => return Err(unsupported()),
            }
        }
        "line-height" => style.line_height = line_height(value, style.font_size, context)?,
        "word-spacing" => {
            style.word_spacing = match value {
                "normal" => 0.0,
                _ => super::values::signed_length(value, style.font_size, context)?,
            }
        }
        "text-indent" => style.text_indent = side_length(value, style.font_size, context)?,
        "font-style" => {
            style.font_style = match value.split_whitespace().next().unwrap_or_default() {
                "normal" => super::FontStyle::Normal,
                "italic" => super::FontStyle::Italic,
                "oblique" => super::FontStyle::Oblique,
                _ => return Err(unsupported()),
            }
        }
        "font-size" => {
            let old = style.font_size;
            let parent = (style.font_size, style.font_size_absolute);
            (
                style.font_size,
                style.font_keyword,
                style.font_size_absolute,
            ) = font_size(value, parent, context)?;
            super::visual::respace(style, old);
        }
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
                "left" | "center" | "right" | "start" | "end" | "justify" => value,
                "match-parent" | "-webkit-match-parent" => {
                    super::text_align::matched(&style.align, style.parent_rtl)
                }
                "justify-all" => "start",
                "-webkit-center" => "webkit-center",
                _ => return Err(unsupported()),
            }
            .into()
        }
        "text-align-last" => style.align_last = super::text_align::last(value, style)?,
        "text-justify" => style.text_justify = super::text_align::justify(value)?,
        "text-autospace" => style.text_autospace = super::text_align::autospace(value)?,
        "hyphens" | "-webkit-hyphens" => style.hyphens_none = super::text_align::hyphens(value)?,
        "hyphenate-character" => {
            style.hyphenate_character = super::text_align::hyphenate_character(value)?;
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
            "hidden" => style.invisible = true,
            // ponytail: collapse removes the box everywhere, right for table rows and flex
            // items; elsewhere it should act as hidden.
            "collapse" => style.hidden = true,
            "visible" => style.invisible = false,
            _ => return Err(unsupported()),
        },
        "pointer-events" => style.interactive = value != "none",
        "width" => width(style, value, context)?,
        "max-width" => {
            let keyword = size_keyword(value, style.font_size, context)?;
            style.size_keywords[2] = keyword;
            style.percent_max_width = value.ends_with('%');
            style.max_width_offset = 0.0;
            style.max_width = match value {
                _ if keyword != SizeKeyword::None => {
                    style.percent_max_width = false;
                    None
                }
                "none" | "min-content" | "max-content" | "fit-content" => {
                    style.percent_max_width = false;
                    None
                }
                _ if style.percent_max_width => Some(number(value.trim_end_matches('%'))?),
                _ if value.starts_with("calc(") => {
                    let (main, percent, offset) = mixed_calc(value, style.font_size, context)?;
                    (style.percent_max_width, style.max_width_offset) = (percent, offset);
                    Some(main)
                }
                _ => Some(contextual_length(value, style.font_size, context)?),
            }
        }
        "opacity" => style.opacity = opacity(value)?,
        _ => return Ok(false),
    }
    Ok(true)
}
/** The content keyword a `width`, `min-width` or `max-width` value names, if any; a
 * `fit-content()` argument is a non-negative length or percentage. */
fn size_keyword(value: &str, font: f64, context: RenderContext) -> EngineResult<SizeKeyword> {
    Ok(match value {
        "min-content" | "-webkit-min-content" => SizeKeyword::MinContent,
        "max-content" | "-webkit-max-content" => SizeKeyword::MaxContent,
        "fit-content" | "-webkit-fit-content" => SizeKeyword::FitContent,
        _ => match value
            .strip_prefix("fit-content(")
            .and_then(|rest| rest.strip_suffix(')'))
        {
            Some(limit) => {
                let limit = super::values::dimension(limit, font, context)?;
                if limit.px < 0.0 || limit.percent < 0.0 {
                    return Err(unsupported());
                }
                SizeKeyword::FitContentLimit(super::Extent {
                    px: limit.px.min(CSS_MAX_SCALAR),
                    percent: limit.percent.min(CSS_MAX_SCALAR),
                })
            }
            None => SizeKeyword::None,
        },
    })
}
/**
 * A `calc()` of a percentage and a length, split into its main part (the percentage when there
 * is one, else the pixels), whether that part is a percentage, and the pixel offset that rides
 * on the percentage. A math function out of range is clamped, as CSS Values 4 requires.
 */
fn mixed_calc(value: &str, font: f64, context: RenderContext) -> EngineResult<(f64, bool, f64)> {
    let mixed = super::values::dimension(value, font, context)?;
    let percent = mixed.percent != 0.0;
    let (main, offset) = match percent {
        true => (mixed.percent, mixed.px),
        false => (mixed.px, 0.0),
    };
    Ok((main.clamp(0.0, CSS_MAX_SCALAR), percent, offset))
}
/**
 * `width`: a keyword, a percentage, a length, or a `calc()` mixing both, whose px part waits in
 * `width_offset` until the percentage resolves against the containing block.
 */
fn width(style: &mut ComputedStyle, value: &str, context: RenderContext) -> EngineResult<()> {
    let keyword = size_keyword(value, style.font_size, context)?;
    style.size_keywords[0] = keyword;
    style.width_offset = 0.0;
    style.percent_width = value.ends_with('%');
    style.width_stretch = matches!(value, "stretch" | "-webkit-fill-available");
    style.width_fit = keyword != SizeKeyword::None;
    if style.width_fit {
        style.percent_width = false;
        style.width = None;
        return Ok(());
    }
    if style.width_stretch {
        // A definite percentage-like size, so flex bases and blocks resolve it against the
        // containing block; sizing subtracts the margins and edges.
        style.percent_width = true;
        style.width = Some(100.0);
        return Ok(());
    }
    style.width = match value {
        "auto" | "min-content" | "max-content" | "fit-content" => {
            style.percent_width = false;
            None
        }
        _ if style.percent_width => Some(number(value.trim_end_matches('%'))?),
        _ if value.starts_with("calc(") => {
            let (main, percent, offset) = mixed_calc(value, style.font_size, context)?;
            (style.percent_width, style.width_offset) = (percent, offset);
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
        "none" => (style.hidden, style.display_none) = (true, true),
        "inline" | "inline flow" => set_display(style, true, false, false, false),
        "inline-block" | "inline flow-root" => set_display(style, true, true, false, false),
        // An inline-level table: atomic on its line, a table inside.
        // An inline box whose principal box is a list item, so it gets a marker.
        "inline-list-item" | "inline list-item" => {
            set_display(style, true, false, false, false);
            style.list_item = true;
        }
        "inline-table" | "inline table" => {
            set_display(style, true, true, false, false);
            style.table_box = TableBox::Table;
        }
        "flex" | "block flex" | "-webkit-box" | "-webkit-flex" => {
            set_display(style, false, false, true, false);
            style.webkit_box = outer == "-webkit-box";
        }
        "inline-flex" | "inline flex" | "-webkit-inline-box" | "-webkit-inline-flex" => {
            set_display(style, false, true, true, false);
            style.webkit_box = outer == "-webkit-inline-box";
        }
        "grid" | "block grid" => set_display(style, false, false, false, true),
        "inline-grid" | "inline grid" => set_display(style, false, true, false, true),
        "grid-lanes" | "block grid-lanes" | "inline-grid-lanes" | "inline grid-lanes" => {
            set_display(style, false, outer.starts_with("inline"), false, true);
            style.grid_lanes = true;
        }
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
                format!("CSS display {outer} lays out as block outside a table."),
            );
            set_display(style, false, false, false, false);
            style.table_box = match outer.as_str() {
                "table-row-group" => TableBox::RowGroup,
                "table-header-group" => TableBox::HeaderGroup,
                "table-footer-group" => TableBox::FooterGroup,
                _ => TableBox::Column,
            };
        }
        _ => return Err(unsupported()),
    }
    Ok(())
}
/** Blink's StyleAdjuster: a `-webkit-box` with a vertical `-webkit-box-orient` and a clamp
(`-webkit-line-clamp`'s legacy `continue`, or a collapsing one) is a flow-root (a
`-webkit-inline-box` an inline-block) whose block layout clamps its lines, not a flex container.
The legacy `continue` clamps nothing else, and only a block container clamps at all. */
pub(super) fn line_clamp_box(style: &mut ComputedStyle) {
    let clamps = style.continue_legacy || style.continue_collapse;
    if style.webkit_box && style.box_orient_vertical && clamps {
        let inline = style.inline_block;
        set_display(style, inline, inline, false, false);
        style.flow_root = !inline;
        style.continue_collapse = true;
        return;
    }
    if style.continue_legacy
        || style.display_flex
        || style.display_grid
        || style.table_box != super::TableBox::None
    {
        style.continue_collapse = false;
    }
}
/** One display value sets every outer and inner display flag together, so none go stale. */
fn set_display(style: &mut ComputedStyle, inline: bool, atomic: bool, flex: bool, grid: bool) {
    style.inline = inline;
    style.inline_block = atomic;
    style.display_flex = flex;
    style.display_grid = grid;
    style.grid_lanes = false;
    style.display_contents = false;
    style.list_item = false;
    style.table_box = TableBox::None;
    style.flow_root = false;
    style.webkit_box = false;
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
/** Keywords, lengths and percentages of the parent size, clamped into the supported range, with
 * the keyword (0 for none) and whether the size is absolute, as Blink's ConvertFontSize tracks. */
fn font_size(
    value: &str,
    (parent, parent_absolute): (f64, bool),
    context: RenderContext,
) -> EngineResult<(f64, u8, bool)> {
    let (size, keyword, absolute) = match value {
        "smaller" => (parent / 1.2, 0, parent_absolute),
        "larger" => (parent * 1.2, 0, parent_absolute),
        _ => match super::font::keyword(value) {
            Some(keyword) => (super::font::keyword_size(keyword, false), keyword, false),
            None => {
                let dimension = super::values::dimension(value, parent, context)?;
                let relative = dimension.percent != 0.0 || font_relative(value);
                let absolute = parent_absolute || !relative || value.contains('(');
                (dimension.of(parent), 0, absolute)
            }
        },
    };
    if !size.is_finite() || size < 0.0 {
        return Err(unsupported());
    }
    // `font-size: 0` hides text and the spaces between inline blocks, as in Chromium; only
    // sizes above it take the minimum.
    if size == 0.0 {
        return Ok((0.0, keyword, absolute));
    }
    Ok((
        size.clamp(CSS_MIN_FONT_SIZE, CSS_MAX_FONT_SIZE),
        keyword,
        absolute,
    ))
}
/** `em`, `ex`, `ch`, `ic`, `cap` and `lh` follow the parent font; `rem` and the rest do not. */
fn font_relative(value: &str) -> bool {
    let unit = value.trim_start_matches(|c: char| c.is_ascii_digit() || "+-.".contains(c));
    ["em", "ex", "ch", "ic", "cap", "lh"].contains(&unit)
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
