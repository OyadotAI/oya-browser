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
    // section 10.12, Blink's CSSMathFunctionValue::ClampToPermittedRange).
    let px = if is_math(value) {
        px.clamp(0.0, CSS_MAX_SCALAR)
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
        let fallback = if fallback.is_empty() { "0px" } else { fallback };
        out = format!("{}{fallback}{}", &out[..start], &out[close + 1..]);
    }
    Err(unsupported())
}
/// CSS pixels per inch.
const PX_PER_IN: f64 = 96.0;
/// Centimetres per inch.
const CM_PER_IN: f64 = 2.54;
/// Cap height as a share of the em, for `cap` (a typical Latin face's).
const CAP_HEIGHT: f64 = 0.7;
/// Blink's `line-height: normal` for the default faces, as a multiple of the font size.
const NORMAL_LINE_HEIGHT: f64 = 1.2;
/** One number with a unit; a bare zero is a length. */
pub(super) fn unit(value: &str, font: f64, context: RenderContext) -> EngineResult<Dimension> {
    // Blink stores lengths as floats and saturates them at layout, so a huge finite length such
    // as Tailwind's `calc(infinity * 1px)` (serialized `3.40282e38px`) is valid and as large as
    // a length can be; the caller's own bound then applies.
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
    let (width, height) = (context.width / PERCENT, context.height / PERCENT);
    let root = root_font();
    // Blink's CSSPrimitiveValue length units. Longer suffixes come first so `rem` is not read
    // as `em`. A desktop window has no dynamic toolbars, so the small, large and dynamic
    // viewport units equal the plain ones; `vi`/`vb` follow horizontal writing. `lh` uses the
    // `normal` line height, as the declared one is not known here.
    // `ch` and `ic` measure the element's face (see `set_font_face`); ex and cap stay em ratios.
    // ponytail: ex/cap/rch/ric from em ratios; use font metrics when the platform exposes them.
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
        ("rlh", root * NORMAL_LINE_HEIGHT),
        ("em", font),
        ("ex", font / 2.0),
        ("ch", font / 2.0),
        ("ic", font),
        ("lh", font * NORMAL_LINE_HEIGHT),
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
                _ => scale,
            };
            return scaled(number, scale);
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
    static FONT_FACE: std::cell::Cell<Option<crate::platform::Face>> = const { std::cell::Cell::new(None) };
}
thread_local! {
    /** Whether the current cascade measured a face for `ch` or `ic`. */
    static MEASURED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
/** Sets (or clears) the face `ch` and `ic` measure for the lengths resolved next. */
pub(super) fn set_font_face(face: Option<crate::platform::Face>) {
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
pub(super) fn style_face(style: &super::ComputedStyle) -> crate::platform::Face {
    use crate::{document::FontFamily, platform::Family};
    let weight = if style.bold && style.font_weight < 600 {
        700
    } else {
        style.font_weight
    };
    let web = super::select_font(&style.font_families, weight, style.font_style, None);
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
    }
}
/** The current face's advance of `sample` per em, or None outside the cascade. */
fn advance_per_em(sample: &str) -> Option<f64> {
    let face = FONT_FACE.with(std::cell::Cell::get)?;
    let ratio = crate::platform::measure(sample, face) / face.size;
    (ratio.is_finite() && ratio > 0.0).then_some(ratio)
}
/** Blink's `ch`: the advance of "0" in the primary font (FontMetrics zero width), else 0.5em. */
fn zero_advance() -> f64 {
    advance_per_em("0").unwrap_or(0.5)
}
/**
 * Blink's `ic`: the advance of U+6C34 in the primary font, else 1em. The generic faces are Latin
 * fonts without that glyph, so only a page font is measured; the rest take the 1em fallback.
 */
fn ideograph_advance() -> f64 {
    let web = FONT_FACE.with(|face| {
        face.get()
            .is_some_and(|face| matches!(face.family, crate::platform::Family::Web(_)))
    });
    if web {
        advance_per_em("\u{6C34}").unwrap_or(1.0)
    } else {
        1.0
    }
}
thread_local! {
    /** The root element's font size for `rem`, set by the cascade once `html` is resolved. */
    static ROOT_FONT: std::cell::Cell<f64> = const { std::cell::Cell::new(CSS_ROOT_FONT_SIZE) };
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
const INTEGER: [&str; 6] = [
    "z-index",
    "order",
    "orphans",
    "widows",
    "column-count",
    "-webkit-line-clamp",
];
/** `value` with a numeric math function rounded to an integer for `<integer>` properties. */
pub(super) fn integer_math<'v>(key: &str, value: &'v str) -> std::borrow::Cow<'v, str> {
    let trimmed = value.trim();
    let math = ["calc(", "min(", "max(", "clamp(", "round("]
        .iter()
        .any(|function| trimmed.to_ascii_lowercase().starts_with(function));
    if !INTEGER.contains(&key) || !math {
        return value.into();
    }
    super::calc::tokens(trimmed)
        .and_then(|tokens| super::calc::evaluate_number(&tokens))
        .map_or(value.into(), |number| {
            (number + 0.5).floor().to_string().into()
        })
}
