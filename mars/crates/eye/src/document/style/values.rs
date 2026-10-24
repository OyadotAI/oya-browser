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
/** A finite number literal, or a `calc()`/`min()`/`max()`/`clamp()` of numbers evaluated to one. */
fn math_number(value: &str) -> Option<f64> {
    let value = value.trim();
    if ["calc(", "min(", "max(", "clamp("]
        .iter()
        .any(|function| value.starts_with(function))
    {
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
    if ["calc(", "min(", "max(", "clamp("]
        .iter()
        .any(|function| value.starts_with(function))
    {
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
/** One number with a unit; a bare zero is a length. */
pub(super) fn unit(value: &str, font: f64, context: RenderContext) -> EngineResult<Dimension> {
    let scaled = |number: &str, scale: f64| -> EngineResult<Dimension> {
        Ok(Dimension {
            px: signed_number(number)? * scale,
            percent: 0.0,
        })
    };
    if let Some(number) = value.strip_suffix('%') {
        return Ok(Dimension {
            px: 0.0,
            percent: signed_number(number)?,
        });
    }
    let units: [(&str, f64); 12] = [
        ("rem", root_font()),
        ("em", font),
        ("px", 1.0),
        ("pt", PX_PER_PT),
        ("vw", context.width / PERCENT),
        ("vh", context.height / PERCENT),
        ("vmin", context.width.min(context.height) / PERCENT),
        ("vmax", context.width.max(context.height) / PERCENT),
        ("ch", font / 2.0),
        ("ex", font / 2.0),
        ("pc", PX_PER_PT * 12.0),
        ("in", 96.0),
    ];
    for (suffix, scale) in units {
        if let Some(number) = value.strip_suffix(suffix) {
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
    color(value).map(Some)
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
