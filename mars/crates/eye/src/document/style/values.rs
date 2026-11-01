/** Shared lengths, angles and colors remain finite and bounded before native rendering. */
use super::constants::{ENV_FUNCTIONS, PERCENT, PX_PER_PT};
use super::unsupported;
use crate::constants::{CSS_MAX_SCALAR, CSS_ROOT_FONT_SIZE};
use crate::document::RenderContext;
use crate::protocol::EngineResult;
/** A resolved length is a px part plus a percentage of some reference the consumer knows. */
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Dimension {
    /// Absolute part in CSS pixels.
    pub px: f64,
    /// Percentage part, 100 meaning the whole reference.
    pub percent: f64,
}
impl Dimension {
    /** Pure pixels; a percentage part needs a reference this caller does not have. */
    pub(super) fn pixels(self) -> EngineResult<f64> {
        if self.percent == 0.0 {
            Ok(self.px)
        } else {
            Err(unsupported())
        }
    }
    /** Resolve against a known reference size. */
    pub(super) fn of(self, reference: f64) -> f64 {
        self.px + self.percent / PERCENT * reference
    }
}
/** Supported lengths are finite nonnegative CSS pixels; `auto` keeps its historic zero. */
pub(super) fn length(value: &str, font: f64) -> EngineResult<f64> {
    contextual_length(value, font, RenderContext::default())
}
/** Resource-safe numeric ranges exclude negative placement and extreme native geometry. */
pub(super) fn number(value: &str) -> EngineResult<f64> {
    math_number(value)
        .filter(|n| (0.0..=CSS_MAX_SCALAR).contains(n))
        .ok_or_else(unsupported)
}
/** A finite signed number of any magnitude the caller then bounds. */
pub(super) fn signed_number(value: &str) -> EngineResult<f64> {
    math_number(value)
        .filter(|n| n.abs() <= f64::from(u32::MAX))
        .ok_or_else(unsupported)
}
/** A finite number clamped into the range `signed_number` accepts. */
fn saturated(value: &str) -> EngineResult<f64> {
    let bound = f64::from(u32::MAX);
    math_number(value)
        .map(|n| n.clamp(-bound, bound))
        .ok_or_else(unsupported)
}
/** The math functions `calc::tokens` evaluates, which Blink parses as CSSMathFunctionValue. */
const MATH_FUNCTIONS: [&str; 9] = [
    "calc(", "min(", "max(", "clamp(", "round(", "mod(", "rem(", "abs(", "sign(",
];
/** Whether `value` is one math function (so out-of-range results clamp instead of failing). */
pub(super) fn is_math(value: &str) -> bool {
    let value = value.trim_start();
    MATH_FUNCTIONS
        .iter()
        .any(|function| value.starts_with(function))
}
/** A finite number literal, or a `calc()`/`min()`/`max()`/`clamp()` of numbers evaluated to one. */
fn math_number(value: &str) -> Option<f64> {
    let value = value.trim();
    if is_math(value) {
        let tokens = super::calc::tokens(value).ok()?;
        return super::calc::evaluate_number(&tokens).ok();
    }
    value
        .strip_prefix('+')
        .unwrap_or(value)
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
}
/** Nonnegative pixel lengths in any supported unit or `calc()`, bounded by CSS_MAX_SCALAR. */
pub(super) fn contextual_length(
    value: &str,
    font: f64,
    context: RenderContext,
) -> EngineResult<f64> {
    if value.trim() == "auto" {
        return Ok(0.0);
    }
    let px = dimension(value, font, context)?.pixels()?;
    // A math function out of the property's range is clamped, not invalid (CSS Values 4
    // section 10.12, Blink's CSSMathFunctionValue::ClampToPermittedRange); a huge length is
    // clamped too, as Blink saturates it at LayoutUnit's maximum, so `height: 100000px` is tall
    // rather than ignored.
    let px = if is_math(value) {
        px.clamp(0.0, CSS_MAX_SCALAR)
    } else if px > CSS_MAX_SCALAR {
        CSS_MAX_SCALAR
    } else {
        px
    };
    if (0.0..=CSS_MAX_SCALAR).contains(&px) {
        Ok(px)
    } else {
        Err(unsupported())
    }
}
/** Signed pixels clamped into the geometry range, so off-screen offsets stay off-screen. */
pub(super) fn signed_length(value: &str, font: f64, context: RenderContext) -> EngineResult<f64> {
    Ok(dimension(value, font, context)?
        .pixels()?
        .clamp(-CSS_MAX_SCALAR, CSS_MAX_SCALAR))
}
/** Any length, percentage or `calc()`/`min()`/`max()`/`clamp()` of them. */
pub(super) fn dimension(value: &str, font: f64, context: RenderContext) -> EngineResult<Dimension> {
    let value = without_env(value.trim())?;
    let value = value.as_ref();
    if is_math(value) {
        let tokens = super::calc::tokens(value)?;
        return super::calc::evaluate(&tokens, font, context);
    }
    unit(value, font, context)
}
/**
 * `env()` reads user-agent environment values. Eye's desktop host has no safe-area insets or
 * title-bar areas, so each becomes its fallback, or `0px` without one, as Chromium computes
 * them on a desktop window. The number of `env()`s one value may hold is bounded.
 */
fn without_env(value: &str) -> EngineResult<std::borrow::Cow<'_, str>> {
    if !value.contains("env(") {
        return Ok(value.into());
    }
    let mut out = value.to_string();
    for _ in 0..ENV_FUNCTIONS {
        let Some(start) = out.find("env(") else {
            return Ok(out.into());
        };
        let open = start + "env".len();
        let close = super::syntax::find_top(&out, open + 1, b")");
        if close >= out.len() {
            return Err(unsupported());
        }
        let inner = &out[open + 1..close];
        let comma = super::syntax::find_top(inner, 0, b",");
        let fallback = inner.get(comma + 1..).map(str::trim).unwrap_or_default();
        // A unitless zero is a length here: env() takes its fallback's type, and calc() rejects a
        // bare number beside a length.
        let fallback = match fallback {
            "" | "0" => ZERO_LENGTH,
            other => other,
        };
        out = format!("{}{fallback}{}", &out[..start], &out[close + 1..]);
    }
    Err(unsupported())
}
/// The length an `env()` without a usable fallback resolves to.
const ZERO_LENGTH: &str = "0px";
/// CSS pixels per inch.
const PX_PER_IN: f64 = 96.0;
/// Centimetres per inch.
const CM_PER_IN: f64 = 2.54;
/** The physical units, whose px factors are not binary fractions. */
const PHYSICAL_UNITS: [&str; 6] = ["in", "cm", "mm", "q", "pt", "pc"];
/** Physical unit conversions round to this many steps per px, far below a device pixel. */
const UNIT_STEPS_PER_PX: f64 = 1.0e6;
/** Drops the binary rounding error a unit conversion leaves: `25.4mm` is exactly 96px, not
95.99999999999999, which a floored border width turned into 95px (CSS2 left-055). */
fn snap_unit(px: f64) -> f64 {
    let snapped = (px * UNIT_STEPS_PER_PX).round() / UNIT_STEPS_PER_PX;
    if snapped.is_finite() { snapped } else { px }
}
/// Cap height as a share of the em, for `cap` (a typical Latin face's).
const CAP_HEIGHT: f64 = 0.7;
/// Blink's `line-height: normal` for the default faces, as a multiple of the font size.
const NORMAL_LINE_HEIGHT: f64 = 1.2;
/** One number with a unit; a bare zero is a length. */
pub(super) fn unit(value: &str, font: f64, context: RenderContext) -> EngineResult<Dimension> {
    // Blink stores lengths as floats and saturates them at layout, so a huge finite length such
    // as Tailwind's `calc(infinity * 1px)` (serialized `3.40282e38px`) is valid and as large as
    // a length can be; the caller's own bound then applies.
    // Units are ASCII case-insensitive (`105Q`, `10PX`).
    let lower = value.to_ascii_lowercase();
    let value = lower.as_str();
    let scaled = |number: &str, scale: f64| -> EngineResult<Dimension> {
        Ok(Dimension {
            px: saturated(number)? * scale,
            percent: 0.0,
        })
    };
    if let Some(number) = value.strip_suffix('%') {
        return Ok(Dimension {
            px: 0.0,
            percent: signed_number(number)?,
        });
    }
    if let Some(length) = container_unit(value, context)? {
        return Ok(length);
    }
    let (width, height) = (context.width / PERCENT, context.height / PERCENT);
    let root = root_font();
    // Blink's CSSPrimitiveValue length units. Longer suffixes come first so `rem` is not read
    // as `em`. A desktop window has no dynamic toolbars, so the small, large and dynamic
    // viewport units equal the plain ones; `vi`/`vb` follow horizontal writing. `lh` uses the
    // `normal` line height, as the declared one is not known here.
    // `ch`, `ic`, `ex` and `cap` measure the element's face (see `set_font_face`).
    // ponytail: rex/rcap/rch/ric from em ratios; measure the root face when a page needs them.
    let units: [(&str, f64); 37] = [
        ("svmin", width.min(height)),
        ("svmax", width.max(height)),
        ("lvmin", width.min(height)),
        ("lvmax", width.max(height)),
        ("dvmin", width.min(height)),
        ("dvmax", width.max(height)),
        ("vmin", width.min(height)),
        ("vmax", width.max(height)),
        ("rcap", root * CAP_HEIGHT),
        ("cap", font * CAP_HEIGHT),
        ("svw", width),
        ("svh", height),
        ("lvw", width),
        ("lvh", height),
        ("dvw", width),
        ("dvh", height),
        ("svi", width),
        ("svb", height),
        ("lvi", width),
        ("lvb", height),
        ("dvi", width),
        ("dvb", height),
        ("rem", root),
        ("rex", root / 2.0),
        ("rch", root / 2.0),
        ("ric", root),
        ("rlh", root_line()),
        ("em", font),
        ("ex", font / 2.0),
        ("ch", font / 2.0),
        ("ic", font),
        ("lh", own_line().unwrap_or(font * NORMAL_LINE_HEIGHT)),
        ("px", 1.0),
        ("pt", PX_PER_PT),
        ("pc", PX_PER_PT * 12.0),
        ("in", PX_PER_IN),
        ("cm", PX_PER_IN / CM_PER_IN),
    ];
    let more: [(&str, f64); 6] = [
        ("mm", PX_PER_IN / CM_PER_IN / 10.0),
        ("vw", width),
        ("vh", height),
        ("vi", width),
        ("vb", height),
        ("q", PX_PER_IN / CM_PER_IN / 40.0),
    ];
    for (suffix, scale) in units.into_iter().chain(more) {
        if let Some(number) = value.strip_suffix(suffix) {
            let scale = match suffix {
                "ch" => font * zero_advance(),
                "ic" => font * ideograph_advance(),
                "ex" => font * edge_per_em(false).unwrap_or(EX_HEIGHT),
                "cap" => font * edge_per_em(true).unwrap_or(CAP_HEIGHT),
                _ => scale,
            };
            let mut length = scaled(number, scale)?;
            if PHYSICAL_UNITS.contains(&suffix) {
                length.px = snap_unit(length.px);
            }
            return Ok(length);
        }
    }
    if signed_number(value)? == 0.0 {
        return Ok(Dimension {
            px: 0.0,
            percent: 0.0,
        });
    }
    Err(unsupported())
}
/** Angles in degrees from deg, rad, grad or turn; a bare zero is zero degrees. */
pub(super) fn angle(value: &str) -> EngineResult<f64> {
    let value = value.trim();
    let units: [(&str, f64); 4] = [
        ("deg", 1.0),
        ("grad", 0.9),
        ("rad", 180.0 / std::f64::consts::PI),
        ("turn", 360.0),
    ];
    for (suffix, scale) in units {
        if let Some(number) = value.strip_suffix(suffix) {
            return Ok(signed_number(number)? * scale);
        }
    }
    match signed_number(value)? {
        0.0 => Ok(0.0),
        _ => Err(unsupported()),
    }
}
/** Any supported CSS color becomes a painter literal. */
pub(crate) fn color(value: &str) -> EngineResult<String> {
    super::colors::parse(value)
}
/** Both light-dark branches are validated; `currentcolor` uses the element's resolved color. */
pub(super) fn contextual_color(
    value: &str,
    scheme: crate::document::Appearance,
    current: Option<&str>,
) -> EngineResult<Option<String>> {
    if value.trim() == "currentcolor" {
        return Ok(current.map(Into::into));
    }
    if let Some(pair) = value
        .trim()
        .strip_prefix("light-dark(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let (light, dark) = super::syntax::split_top(pair, b',')
            .into_iter()
            .collect::<Vec<_>>()
            .try_into()
            .map(|pair: [&str; 2]| (pair[0], pair[1]))
            .map_err(|_| unsupported())?;
        let light = contextual_color(light, scheme, current)?;
        let dark = contextual_color(dark, scheme, current)?;
        return Ok(if scheme == crate::document::Appearance::Light {
            light
        } else {
            dark
        });
    }
    if let Some(color) = super::colors::system(
        &value.trim().to_ascii_lowercase(),
        scheme == crate::document::Appearance::Dark,
    ) {
        return Ok(Some(color));
    }
    if let Some(arguments) = value
        .trim()
        .strip_prefix("color-mix(")
        .and_then(|value| value.strip_suffix(')'))
    {
        // Each argument resolves in context, so `currentcolor` mixes with the element's color.
        let resolve = |color: &str| {
            contextual_color(color, scheme, current)
                .map(|literal| literal.and_then(|literal| super::colors::linear_of(&literal)))
        };
        let mixed = super::color_spaces::mix(&arguments.to_ascii_lowercase(), &resolve)?;
        return Ok(mixed.map(super::colors::from_linear));
    }
    color(value).map(Some)
}
thread_local! {
    /** The face of the element whose lengths are resolving, for `ch` and `ic`; None outside the
     * cascade. Font-size resolves against the parent's face, everything after against its own. */
    static FONT_FACE: std::cell::Cell<Option<StyleFaces>> = const { std::cell::Cell::new(None) };
}
thread_local! {
    /** Whether the current cascade measured a face for `ch` or `ic`. */
    static MEASURED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
/// A style's faces for font-relative units: its first available font (`ch`, `ex`, `cap`) and
/// the face its family list draws U+6C34 with (`ic`, which CSS Values 4 measures in the font
/// that has the ideograph, not in a Latin-only first font).
pub(super) type StyleFaces = (crate::platform::Face, crate::platform::Face);
/** Sets (or clears) the faces `ch` and `ic` measure for the lengths resolved next. */
pub(super) fn set_font_face(face: Option<StyleFaces>) {
    if face.is_some() {
        MEASURED.with(|measured| measured.set(true));
    }
    FONT_FACE.with(|current| current.set(face));
}
/** Whether a style measured a face since the last call, which clears it: such styles change
 * when the page's web fonts load, so the page styles again then (Blink invalidates
 * font-relative lengths on a font load). */
pub(super) fn take_measured() -> bool {
    MEASURED.with(|measured| measured.replace(false))
}
/**
 * The face a computed style shapes text with, as layout picks it: a loaded page font the family
 * list names, else the generic family. Mirrors the render side so `ch` matches measured text.
 */
pub(super) fn style_face(style: &super::ComputedStyle) -> StyleFaces {
    // The first available font is the first whose faces cover a space (CSS Fonts 4 5.4), so a
    // face whose `unicode-range` leaves the space out sets no `ex`, `cap` or `ch`.
    (face_for(style, ' '), face_for(style, IDEOGRAPH))
}
/// The character `ic` measures: the CJK water ideograph.
const IDEOGRAPH: char = '\u{6C34}';
/** The face `style` draws `sample` with: the first loaded page font of its family list that
 * covers it, else the generic family. */
fn face_for(style: &super::ComputedStyle, sample: char) -> crate::platform::Face {
    use crate::{document::FontFamily, platform::Family};
    let weight = if style.bold && style.font_weight < 600 {
        700
    } else {
        style.font_weight
    };
    let web = super::select_font(&style.font_families, weight, style.font_style, Some(sample));
    crate::platform::Face {
        size: style.font_size,
        weight: web.map_or(weight, |(_, clamped)| clamped),
        family: match (web, style.font_family) {
            (Some((key, _)), _) => Family::Web(key),
            (None, FontFamily::Serif) => Family::Serif,
            (None, FontFamily::SansSerif) => Family::SansSerif,
            (None, FontFamily::Monospace) => Family::Monospace,
            (None, FontFamily::SystemUi) => Family::System,
        },
        spacing: 0.0,
        italic: style.font_style != crate::document::FontStyle::Normal,
        autospace: crate::platform::autospace::OFF,
        // Upright vertical text measures `ch` by the "0"'s vertical advance (CSS Values 6.1.1).
        upright: match style.vertical_writing
            && !style.writing_sideways
            && style.text_orientation == 1
        {
            true => crate::platform::upright::UPRIGHT,
            false => crate::platform::upright::TURNED,
        },
    }
}
/** The current style's first available font, or None outside the cascade. */
fn primary_face() -> Option<crate::platform::Face> {
    FONT_FACE
        .with(std::cell::Cell::get)
        .map(|(primary, _)| primary)
}
/** The current face's advance of `sample` per em, or None outside the cascade. */
fn advance_per_em(sample: &str) -> Option<f64> {
    let face = primary_face()?;
    let ratio = crate::platform::measure(sample, face) / face.size;
    (ratio.is_finite() && ratio > 0.0).then_some(ratio)
}
/** The primary font's cap height (`cap`) or x-height (`ex`) per em, as Blink's `FontMetrics`
 * reads them; None before a face is known. */
fn edge_per_em(cap: bool) -> Option<f64> {
    let face = primary_face()?;
    let (cap_height, x_height) = crate::platform::edges(face);
    let ratio = if cap { cap_height } else { x_height } / face.size;
    (ratio.is_finite() && ratio > 0.0).then_some(ratio)
}
/// The x-height as a share of the em when no face is measured.
const EX_HEIGHT: f64 = 0.5;
/** Blink's `ch`: the advance of "0" in the primary font (FontMetrics zero width), else 0.5em. */
fn zero_advance() -> f64 {
    advance_per_em("0").unwrap_or(0.5)
}
/**
 * Blink's `ic`: the advance of U+6C34 in the primary font, else 1em. The generic faces are Latin
 * fonts without that glyph, so only a page font is measured; the rest take the 1em fallback.
 */
fn ideograph_advance() -> f64 {
    let Some((_, face)) = FONT_FACE.with(std::cell::Cell::get) else {
        return 1.0;
    };
    if !matches!(face.family, crate::platform::Family::Web(_)) {
        return 1.0;
    }
    let ratio = crate::platform::measure(&IDEOGRAPH.to_string(), face) / face.size;
    if ratio.is_finite() && ratio > 0.0 {
        ratio
    } else {
        1.0
    }
}
thread_local! {
    /** The `cq*` basis of the element being resolved: its nearest size container's content
     * width and its nearest `size` container's content height; None uses the viewport. */
    static CONTAINER: std::cell::Cell<[Option<f64>; 2]> = const { std::cell::Cell::new([None, None]) };
}
/** Sets (or with `[None, None]` clears) the `cq*` basis for the lengths resolved next. */
pub(super) fn set_container_extent(extent: [Option<f64>; 2]) {
    CONTAINER.with(|basis| basis.set(extent));
}
/** Container query units, longest suffix first so `cqmin` is not read as `in`. */
const CONTAINER_UNITS: [&str; 6] = ["cqmin", "cqmax", "cqw", "cqh", "cqi", "cqb"];
/**
 * A `cq*` length against the current basis (`cqi`/`cqb` as `cqw`/`cqh` in horizontal writing);
 * None when `value` has another unit. Without a qualifying container CSS uses the small
 * viewport, which on a desktop window is the viewport.
 */
fn container_unit(value: &str, context: RenderContext) -> EngineResult<Option<Dimension>> {
    let Some(suffix) = CONTAINER_UNITS.iter().find(|unit| value.ends_with(*unit)) else {
        return Ok(None);
    };
    let [width, height] = CONTAINER.with(std::cell::Cell::get);
    let width = width.unwrap_or(context.width) / PERCENT;
    let height = height.unwrap_or(context.height) / PERCENT;
    let scale = match *suffix {
        "cqmin" => width.min(height),
        "cqmax" => width.max(height),
        "cqw" | "cqi" => width,
        _ => height,
    };
    let number = &value[..value.len() - suffix.len()];
    Ok(Some(Dimension {
        px: saturated(number)? * scale,
        percent: 0.0,
    }))
}
/** Whether a declared value uses a container query unit, so its cascade needs container sizes. */
pub(super) fn container_relative(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.match_indices("cq").any(|(at, _)| {
        let numeric = lower[..at].ends_with(|c: char| c.is_ascii_digit() || c == '.');
        numeric
            && CONTAINER_UNITS
                .iter()
                .any(|unit| lower[at..].starts_with(unit))
    })
}
thread_local! {
    /** The root element's font size for `rem`, set by the cascade once `html` is resolved. */
    static ROOT_FONT: std::cell::Cell<f64> = const { std::cell::Cell::new(CSS_ROOT_FONT_SIZE) };
    /** The root element's used line height for `rlh`. */
    static ROOT_LINE: std::cell::Cell<f64> =
        const { std::cell::Cell::new(CSS_ROOT_FONT_SIZE * NORMAL_LINE_HEIGHT) };
    /** The element being resolved's used line height for `lh`, once its own is computed; None
    while the font and line-height properties themselves resolve (against the parent's). */
    static OWN_LINE: std::cell::Cell<Option<f64>> = const { std::cell::Cell::new(None) };
}
/** The current cascade's `rlh` size. */
fn root_line() -> f64 {
    ROOT_LINE.with(std::cell::Cell::get)
}
/** The current element's `lh` size, if it is known yet. */
fn own_line() -> Option<f64> {
    OWN_LINE.with(std::cell::Cell::get)
}
/** Sets the `rlh` size for the rest of this cascade, as `set_root_font` does `rem`. */
pub(super) fn set_root_line(size: f64) {
    let size = if size.is_finite() && size > 0.0 {
        size
    } else {
        CSS_ROOT_FONT_SIZE * NORMAL_LINE_HEIGHT
    };
    ROOT_LINE.with(|root| root.set(size));
}
/** Sets (or with None clears) the element's own `lh` size. */
pub(super) fn set_own_line(size: Option<f64>) {
    OWN_LINE.with(|own| own.set(size.filter(|size| size.is_finite() && *size > 0.0)));
}
/** The current cascade's `rem` size. */
fn root_font() -> f64 {
    ROOT_FONT.with(std::cell::Cell::get)
}
/** Sets the `rem` size for the rest of this cascade; non-finite sizes fall back to the default. */
pub(super) fn set_root_font(size: f64) {
    let size = if size.is_finite() && size > 0.0 {
        size
    } else {
        CSS_ROOT_FONT_SIZE
    };
    ROOT_FONT.with(|root| root.set(size));
}
/**
 * Properties whose grammar is `<integer>`. A math function there resolves to the nearest
 * integer, halves toward positive infinity (CSS Values 4 "Range Checking", Blink's
 * CSSMathFunctionValue::ComputeInteger), so `z-index: calc(-10)` is -10 rather than invalid.
 */
const INTEGER: [&str; 7] = [
    "z-index",
    "order",
    "orphans",
    "widows",
    "column-count",
    "-webkit-line-clamp",
    "max-lines",
];
/** `value` with a numeric math function rounded to an integer for `<integer>` properties. */
pub(super) fn integer_math<'v>(key: &str, value: &'v str) -> std::borrow::Cow<'v, str> {
    if !INTEGER.contains(&key) {
        return value.into();
    }
    let trimmed = value.trim();
    let lower = trimmed.to_ascii_lowercase();
    let math = ["calc(", "min(", "max(", "clamp(", "round("]
        .iter()
        .any(|function| lower.starts_with(function));
    if !math {
        return value.into();
    }
    super::calc::tokens(trimmed)
        .and_then(|tokens| super::calc::evaluate_number(&tokens))
        .map_or(value.into(), |number| {
            (number + 0.5).floor().to_string().into()
        })
}
