/**
 * `calc-size(<basis>, <calc-sum>)` on the sizing properties (CSS Values 5): the basis sizes the
 * box as it would alone (`auto`, a content keyword, or a length), and the sum then maps that
 * size, named `size`, to the used one. The sum is kept as `min(scale * size + offset, cap)`,
 * which covers `size`, `size +/- L`, `size * N`, `size / N` and `min(size, L)`; layout applies
 * it where each property's automatic or content size is found (render/sizing.rs `calc_size`).
 */
use super::{ComputedStyle, unsupported, values::dimension};
use crate::{document::RenderContext, protocol::EngineResult};
use serde::{Deserialize, Serialize};

/// The sizing properties in `ComputedStyle::calc_size` order.
pub(super) const KEYS: [&str; 6] = [
    "width",
    "min-width",
    "max-width",
    "height",
    "min-height",
    "max-height",
];
/// Index of `width` in `ComputedStyle::calc_size`.
pub const WIDTH: usize = 0;
/// Index of `min-width`.
pub const MIN_WIDTH: usize = 1;
/// Index of `height`.
pub const HEIGHT: usize = 3;
/// Index of `min-height`.
pub const MIN_HEIGHT: usize = 4;
/// The `size` the sum is sampled at, besides zero, to read its slope.
const PROBE: f64 = 100.0;

/// A `calc-size()` sum as `min(scale * size + offset, cap)`, in px.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CalcSize {
    /// How much the used size grows per px of the basis size.
    pub scale: f64,
    /// The used size at a zero basis size.
    pub offset: f64,
    /// The `min(size, L)` ceiling, if the sum has one.
    pub cap: Option<f64>,
}

impl CalcSize {
    /** The used size for basis size `size`, never negative. */
    pub fn apply(self, size: f64) -> f64 {
        let used = self.scale * size + self.offset;
        self.cap.map_or(used, |cap| used.min(cap)).max(0.0)
    }
}

/** `size` through property `index`'s `calc-size()` of `style`, or unchanged without one. */
pub fn through(style: &ComputedStyle, index: usize, size: f64) -> f64 {
    style.calc_size[index].map_or(size, |calc| calc.apply(size))
}

/**
 * Applies a sizing property's value: a `calc-size()` records its sum and hands back the basis
 * for the property's own parser (a length basis comes back already mapped, as a px length);
 * any other value clears the sum and comes back as it was.
 */
pub(super) fn split(
    style: &mut ComputedStyle,
    index: usize,
    value: &str,
    (font, context): (f64, RenderContext),
) -> EngineResult<String> {
    style.calc_size[index] = None;
    let Some(inner) = value
        .strip_prefix("calc-size(")
        .and_then(|v| v.strip_suffix(')'))
    else {
        return Ok(value.to_string());
    };
    let parts = super::syntax::split_top(inner, b',');
    let [basis, sum] = parts.as_slice() else {
        return Err(unsupported());
    };
    let calc = sum_of(sum.trim(), font, context)?;
    let basis = basis.trim();
    if matches!(
        basis,
        "auto" | "fit-content" | "min-content" | "max-content"
    ) {
        style.calc_size[index] = Some(calc);
        return Ok(basis.to_string());
    }
    let length = px(basis, font, context)?;
    Ok(format!("{}px", calc.apply(length)))
}

/** The sum's affine form, read by sampling it at two sizes, and its `min(size, L)` cap. */
fn sum_of(sum: &str, font: f64, context: RenderContext) -> EngineResult<CalcSize> {
    let capped = sum
        .strip_prefix("min(")
        .and_then(|rest| rest.strip_suffix(')'))
        .map(|args| super::syntax::split_top(args, b','));
    let (sum, cap) = match capped.as_deref() {
        Some([first, second]) if first.trim() == "size" => ("size", Some(second.trim())),
        _ => (sum, None),
    };
    let at = |size: f64| {
        px(
            &format!("calc({})", sum.replace("size", &format!("{size}px"))),
            font,
            context,
        )
    };
    let offset = at(0.0)?;
    Ok(CalcSize {
        scale: (at(PROBE)? - offset) / PROBE,
        offset,
        cap: cap.map(|cap| px(cap, font, context)).transpose()?,
    })
}

/** A length with no percentage part, in px. */
fn px(value: &str, font: f64, context: RenderContext) -> EngineResult<f64> {
    let length = dimension(value, font, context)?;
    if length.percent != 0.0 || !length.px.is_finite() {
        return Err(unsupported());
    }
    Ok(length.px)
}
