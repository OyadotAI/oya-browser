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
        "overflow-x" | "overflow-y" => {
            let axis = overflow(value)?;
            style.overflow = style.overflow.max(axis);
            match key {
                "overflow-x" => style.overflow_x = axis,
                _ => style.overflow_y = axis,
            }
        }
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
        "letter-spacing" => {
            (style.letter_spacing, style.spacing_percent[0]) =
                spacing(value, style.font_size, context)?
        }
        "word-spacing" => {
            (style.word_spacing, style.spacing_percent[1]) =
                spacing(value, style.font_size, context)?
        }
        "text-transform" => {
            let (case, full_width, full_size_kana) = text_transform(value)?;
            style.text_transform = case;
            style.text_transform_full_width = full_width;
            style.text_transform_full_size_kana = full_size_kana;
        }
        "white-space" => {
            style.white_space = white_space(value)?;
            style.break_spaces = value.split_ascii_whitespace().any(|v| v == "break-spaces");
        }
        "white-space-collapse" => {
            style.white_space = longhands(Some(collapse(value)?), None, style.white_space);
            style.break_spaces = value == "break-spaces";
        }
        "tab-size" | "-moz-tab-size" => {
            style.tab_size = Some(tab_size(value, style.font_size, context)?)
        }
        "text-wrap-mode" | "text-wrap" => {
            style.white_space = longhands(None, Some(wrap_mode(value)?), style.white_space)
        }
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
        "filter"
        | "backdrop-filter"
        | "-webkit-backdrop-filter"
        | "mix-blend-mode"
        | "isolation"
        | "will-change"
        | "contain"
        | "clip-path"
        | "-webkit-clip-path" => {
            // A shape Eye cannot clip by may hide most of the box, so it leaves agent reach.
            if key.ends_with("clip-path") && value != "none" {
                style.interactive = false;
            }
            if key == "filter" {
                style.filter_blur = filter_blur(value, style.font_size, context);
            }
            stacking(style, key, value)
        }
        "fill" | "stroke" | "stroke-width" | "stroke-linejoin" | "stroke-linecap" | "fill-rule"
        | "fill-opacity" | "stroke-opacity" => svg_paint(style, key, value),
        _ => return radius(style, key, value, context),
    }
    Ok(true)
}
/**
 * Keeps an SVG paint property for the painter. A `fill` or `stroke` it cannot paint (an
 * external `url()`, a malformed color) is left out so the shape keeps its attribute paint
 * rather than vanishing, as a browser keeps drawing with an invalid declaration dropped.
 */
fn svg_paint(style: &mut ComputedStyle, key: &str, value: &str) {
    let value = value.trim();
    let paintable = value.eq_ignore_ascii_case("none")
        || value.eq_ignore_ascii_case("currentcolor")
        || value
            .get(..5)
            .is_some_and(|head| head.eq_ignore_ascii_case("url(#"))
        || super::literal_color(&value.to_ascii_lowercase()).is_ok();
    if matches!(key, "fill" | "stroke") && !paintable {
        return;
    }
    style.svg_paint.insert(key.to_owned(), value.to_owned());
}
/** `filter` or `backdrop-filter` other than `none`. */
pub const STACKING_FILTER: u8 = 1;
/** `mix-blend-mode` other than `normal`. */
pub const STACKING_BLEND: u8 = 2;
/** `isolation: isolate`. */
pub const STACKING_ISOLATION: u8 = 4;
/** `will-change` naming a property that would itself create a stacking context. */
pub const STACKING_WILL_CHANGE: u8 = 8;
/** `contain` with paint or layout containment (`paint`, `layout`, `strict`, `content`). */
pub const STACKING_CONTAIN: u8 = 16;
/** A `clip-path` shape Eye does not clip by (`circle()`, `url()`, ...), still a grouping property. */
pub const STACKING_CLIP_PATH: u8 = 32;
/** `backdrop-filter`, kept apart from `filter` so either one's `none` clears only itself. */
pub const STACKING_BACKDROP: u8 = 64;
/** Properties `will-change` can name that make Blink create a stacking context
 * (`HasPropertyThatCreatesStackingContext`). */
const WILL_CHANGE_STACKING: [&str; 12] = [
    "opacity",
    "transform",
    "translate",
    "rotate",
    "scale",
    "filter",
    "backdrop-filter",
    "mix-blend-mode",
    "isolation",
    "clip-path",
    "mask",
    "perspective",
];
/**
 * Records one grouping property that makes the box a stacking context, as Blink's
 * `CalculateIsStackingContextWithoutContainment` and `HasStackingGroupingProperty` read them.
 * Without it a `filter` or `isolation: isolate` card painted interleaved with its siblings'
 * positioned children instead of as one unit.
 */
fn stacking(style: &mut ComputedStyle, key: &str, value: &str) {
    let value = value.trim();
    let (bit, on) = match key {
        "filter" => (STACKING_FILTER, value != "none"),
        "backdrop-filter" | "-webkit-backdrop-filter" => (STACKING_BACKDROP, value != "none"),
        "mix-blend-mode" => (STACKING_BLEND, value != "normal"),
        "isolation" => (STACKING_ISOLATION, value == "isolate"),
        "will-change" => (
            STACKING_WILL_CHANGE,
            value
                .split(',')
                .any(|name| WILL_CHANGE_STACKING.contains(&name.trim())),
        ),
        "contain" => (
            STACKING_CONTAIN,
            value
                .split_whitespace()
                .any(|word| ["paint", "layout", "strict", "content"].contains(&word)),
        ),
        // `inset()` and `none` are applied above; any other shape still groups the box.
        _ => (STACKING_CLIP_PATH, value != "none"),
    };
    if on {
        style.stacking |= bit;
    } else {
        style.stacking &= !bit;
    }
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
/** The summed `blur()` radii of a filter list (successive Gaussian blurs add their variances,
 * so the deviations combine as a root sum of squares); 0 for none or an unreadable length. */
fn filter_blur(value: &str, font: f64, context: RenderContext) -> f64 {
    let mut variance = 0.0;
    let mut rest = value;
    while let Some(start) = rest.find("blur(") {
        let after = &rest[start + "blur(".len()..];
        let Some(end) = after.find(')') else { break };
        let radius = signed_length(after[..end].trim(), font, context).unwrap_or(0.0);
        if radius.is_finite() && radius > 0.0 && radius <= CSS_MAX_SCALAR {
            variance += radius * radius;
        }
        rest = &after[end..];
    }
    variance.sqrt()
}
/**
 * `letter-spacing` and `word-spacing` as (used px, percentage), after Blink's ConvertSpacing and
 * FontDescription::LetterSpacing/WordSpacing: `normal` is zero, and a percentage (alone or in
 * `calc()`) is of the element's computed font size. The percentage is kept so a child with
 * another font size re-resolves it (see `respace`). Bounded by the largest font size.
 */
fn spacing(value: &str, font: f64, context: RenderContext) -> EngineResult<(f64, f64)> {
    if value == "normal" {
        return Ok((0.0, 0.0));
    }
    let length = dimension(value, font, context)?;
    let used = length.of(font);
    if used.is_finite() && used.abs() <= LETTER_SPACING_MAX {
        Ok((used, length.percent))
    } else {
        Err(unsupported())
    }
}
/** After the font size changes from `old`, inherited percentage spacing follows the new size,
 * as Blink's inherited Length resolves against each element's own font. */
pub(super) fn respace(style: &mut ComputedStyle, old: f64) {
    let delta = (style.font_size - old) / PERCENT;
    let bound = |v: f64| v.clamp(-LETTER_SPACING_MAX, LETTER_SPACING_MAX);
    style.letter_spacing = bound(style.letter_spacing + style.spacing_percent[0] * delta);
    style.word_spacing = bound(style.word_spacing + style.spacing_percent[1] * delta);
}
/** `tab-size`: a non-negative number of spaces, or a non-negative length. */
fn tab_size(value: &str, font: f64, context: RenderContext) -> EngineResult<(f64, bool)> {
    if let Ok(count) = value.parse::<f64>() {
        return (count.is_finite() && count >= 0.0)
            .then_some((count, false))
            .ok_or_else(unsupported);
    }
    let used = dimension(value, font, context)?.of(font);
    (used.is_finite() && (0.0..=crate::constants::LAYOUT_EXTENT).contains(&used))
        .then_some((used, true))
        .ok_or_else(unsupported)
}
/** `none | [capitalize | uppercase | lowercase] || full-width || full-size-kana` as the case
mapping and the full-width and full-size-kana flags; a keyword twice is invalid. */
fn text_transform(value: &str) -> EngineResult<(TextTransform, bool, bool)> {
    if value == "none" {
        return Ok((TextTransform::None, false, false));
    }
    let (mut case, mut full_width, mut kana) = (None, false, false);
    for word in value.split_ascii_whitespace() {
        let seen = match word {
            "uppercase" | "lowercase" | "capitalize" if case.is_none() => {
                case = Some(match word {
                    "uppercase" => TextTransform::Uppercase,
                    "lowercase" => TextTransform::Lowercase,
                    _ => TextTransform::Capitalize,
                });
                false
            }
            "full-width" => std::mem::replace(&mut full_width, true),
            "full-size-kana" => std::mem::replace(&mut kana, true),
            _ => return Err(unsupported()),
        };
        if seen {
            return Err(unsupported());
        }
    }
    Ok((case.unwrap_or(TextTransform::None), full_width, kana))
}
/** Whitespace keywords; `break-spaces` behaves as pre-wrap here. CSS Text 4 also accepts the
`white-space` shorthand as its two longhands, `<white-space-collapse> || <text-wrap-mode>`. */
fn white_space(value: &str) -> EngineResult<WhiteSpace> {
    Ok(match value {
        "normal" => WhiteSpace::Normal,
        "pre" => WhiteSpace::Pre,
        "nowrap" => WhiteSpace::Nowrap,
        "pre-wrap" | "break-spaces" => WhiteSpace::PreWrap,
        "pre-line" => WhiteSpace::PreLine,
        _ => {
            let (mut collapsing, mut wrapping) = (None, None);
            for part in value.split_ascii_whitespace() {
                match (collapse(part), wrap_mode(part)) {
                    (Ok(found), _) if collapsing.is_none() => collapsing = Some(found),
                    (_, Ok(found)) if wrapping.is_none() => wrapping = Some(found),
                    _ => return Err(unsupported()),
                }
            }
            if collapsing.is_none() && wrapping.is_none() {
                return Err(unsupported());
            }
            longhands(
                Some(collapsing.unwrap_or(Collapse::Spaces)),
                Some(wrapping.unwrap_or(true)),
                WhiteSpace::Normal,
            )
        }
    })
}
/** `white-space-collapse` values Eye tells apart; `break-spaces` and `preserve-spaces` keep
spaces as `preserve` does. */
#[derive(Clone, Copy, PartialEq)]
enum Collapse {
    Spaces,
    Preserve,
    PreserveBreaks,
}
/** One `white-space-collapse` keyword. */
fn collapse(value: &str) -> EngineResult<Collapse> {
    Ok(match value {
        "collapse" => Collapse::Spaces,
        "preserve" | "break-spaces" | "preserve-spaces" => Collapse::Preserve,
        "preserve-breaks" => Collapse::PreserveBreaks,
        _ => return Err(unsupported()),
    })
}
/** One `text-wrap-mode` keyword (or a `text-wrap` shorthand naming it): whether lines wrap.
`text-wrap: balance | pretty | stable` keep wrapping. */
fn wrap_mode(value: &str) -> EngineResult<bool> {
    Ok(match value {
        "wrap" | "balance" | "pretty" | "stable" | "auto" => true,
        "nowrap" => false,
        _ => return Err(unsupported()),
    })
}
/** Folds the two CSS Text 4 longhands back into the CSS 2 keyword Eye lays out by, keeping
whichever half `current` already had. `preserve-breaks nowrap` has no CSS 2 keyword; it lays
out as `pre-line`. */
fn longhands(
    collapsing: Option<Collapse>,
    wrapping: Option<bool>,
    current: WhiteSpace,
) -> WhiteSpace {
    let (old_collapse, old_wrap) = match current {
        WhiteSpace::Normal => (Collapse::Spaces, true),
        WhiteSpace::Nowrap => (Collapse::Spaces, false),
        WhiteSpace::Pre => (Collapse::Preserve, false),
        WhiteSpace::PreWrap => (Collapse::Preserve, true),
        WhiteSpace::PreLine => (Collapse::PreserveBreaks, true),
    };
    match (
        collapsing.unwrap_or(old_collapse),
        wrapping.unwrap_or(old_wrap),
    ) {
        (Collapse::Spaces, true) => WhiteSpace::Normal,
        (Collapse::Spaces, false) => WhiteSpace::Nowrap,
        (Collapse::Preserve, true) => WhiteSpace::PreWrap,
        (Collapse::Preserve, false) => WhiteSpace::Pre,
        (Collapse::PreserveBreaks, _) => WhiteSpace::PreLine,
    }
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
    // The horizontal radius is the first top-level token, so `calc(10px + 5%)` stays whole.
    let value = value.trim();
    let first = &value[..super::syntax::find_top(value, 0, b" \t\n")];
    if first.is_empty() {
        return Err(unsupported());
    }
    let length = dimension(first, style.font_size, context)?;
    let basis = match style.width {
        Some(width) if !style.percent_width => width,
        _ if length.percent >= RADIUS_HALF => CSS_MAX_SCALAR * PERCENT / RADIUS_HALF,
        _ => RADIUS_PERCENT_BASIS,
    };
    let px = length.of(basis);
    let px = if super::values::is_math(first) {
        px.max(0.0)
    } else {
        px
    };
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

/** Whether the winning `background-clip` (or its `-webkit-` alias) clips to text. */
pub(super) fn clips_to_text(chosen: &super::cascade::Chosen) -> bool {
    ["background-clip", "-webkit-background-clip"]
        .iter()
        .filter_map(|key| chosen.get(*key))
        .any(|(_, value)| {
            value
                .split(',')
                .any(|layer| layer.trim().eq_ignore_ascii_case("text"))
        })
}
/**
 * `background-clip: text` paints the background only through the glyphs. The painter has no
 * clipped gradient text, so the text takes the gradient's middle stop (or the background
 * color) and no box is painted; before, headings like nodejs.org's painted as a dark slab over
 * transparent text.
 */
pub(super) fn clip_to_text(style: &mut ComputedStyle) {
    use super::BackgroundLayer;
    let stop = style.backgrounds.iter().find_map(|layer| match layer {
        BackgroundLayer::Linear { stops, .. } | BackgroundLayer::Radial { stops, .. } => {
            stops.get(stops.len() / 2).map(|stop| stop.color.clone())
        }
        BackgroundLayer::Image { .. } => None,
    });
    if let Some(color) = stop.or_else(|| style.background.clone()) {
        style.color = Some(color);
    }
    style.background = None;
    style.backgrounds.clear();
}
