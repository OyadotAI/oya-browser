/**
 * The individual transform properties `translate`, `rotate` and `scale`, and `transform-origin`.
 * Sites write `translate: -50% -50%` or `scale: 1.05` instead of a `transform` list, and set
 * `transform-origin: top left` on menus and scaled cards; without these, centered overlays sat
 * off by half their size and hover zooms grew from the wrong corner.
 */
use super::{
    ComputedStyle,
    transform3d::{self, Parsed},
    unsupported,
    values::dimension,
};
use crate::{
    constants::CSS_MAX_SCALAR,
    document::{
        RenderContext,
        matrix::{IDENTITY4, from_affine, is_planar, multiply, planar},
    },
    protocol::EngineResult,
};

/** The individual properties in the order CSS Transforms 2 composes them, ahead of `transform`. */
pub(super) const KEYS: [&str; 3] = ["translate", "rotate", "scale"];
/** A percentage as a fraction: `scale: 150%` is 1.5. */
const PERCENT: f64 = 100.0;
/** The initial `transform-origin`, the center of the border box, as percentages. */
const CENTER: f64 = 50.0;

/** One property as a parsed transform, None for `none`. The property's value is rewritten as the
transform function it stands for, so `translate: 1px 2px 3px` parses as `translate3d()` and the
two spellings cannot disagree. */
pub(super) fn matrix(
    key: &str,
    value: &str,
    font: f64,
    context: RenderContext,
) -> EngineResult<Option<Parsed>> {
    match as_function(key, value)? {
        Some(function) => transform3d::parse(&function, font, context),
        None => Ok(None),
    }
}

/** The equivalent transform function text, such as `translate(10px,50%)` or `rotate3d(...)`, or
None for `none`. Two or three arguments pick the planar or the 3D function of the same name. */
fn as_function(key: &str, value: &str) -> EngineResult<Option<String>> {
    let parts: Vec<&str> = value.split_whitespace().collect();
    if parts == ["none"] {
        return Ok(None);
    }
    let function = match key {
        "translate" => by_arity("translate", &parts)?,
        "scale" => {
            let factors = fractions(&parts);
            let factors: Vec<&str> = factors.iter().map(String::as_str).collect();
            by_arity("scale", &factors)?
        }
        "rotate" => rotate_function(&parts)?,
        _ => return Err(unsupported()),
    };
    Ok(Some(function))
}

/** `name(x)`, `name(x,y)` for two components, or `name3d(x,y,z)` for three. */
fn by_arity(name: &str, parts: &[&str]) -> EngineResult<String> {
    Ok(match parts {
        [x] => format!("{name}({x})"),
        [x, y] => format!("{name}({x},{y})"),
        [x, y, z] => format!("{name}3d({x},{y},{z})"),
        _ => return Err(unsupported()),
    })
}

/** `rotate: 45deg`, `rotate: x 45deg` and the axis-angle `rotate: 0 1 1 45deg`. */
fn rotate_function(parts: &[&str]) -> EngineResult<String> {
    Ok(match parts {
        [degrees] => format!("rotate({degrees})"),
        [axis @ ("x" | "y" | "z"), degrees] => format!("rotate{axis}({degrees})"),
        [x, y, z, degrees] => format!("rotate3d({x},{y},{z},{degrees})"),
        _ => return Err(unsupported()),
    })
}

/** `scale` percentages as plain factors. */
fn fractions(parts: &[&str]) -> Vec<String> {
    parts
        .iter()
        .map(|part| match part.strip_suffix('%').map(str::parse::<f64>) {
            Some(Ok(percent)) => (percent / PERCENT).to_string(),
            _ => (*part).to_owned(),
        })
        .collect()
}

/** Composes the individual properties over the computed `transform`: translate, then rotate,
then scale, then the list. ponytail: a percentage `translate` adds to the list's own percentage
offset, exact when no rotate or scale sits between them (the common centering case). */
pub(super) fn compose(style: &mut ComputedStyle, parts: &[Option<Parsed>]) {
    if parts.iter().all(Option::is_none) {
        return;
    }
    // This slice interpolates the transform property alone, not independently composed properties.
    style.transform_animation_excluded = true;
    style.rotation_radians = None;
    style.scale_factors = None;
    let composed = parts
        .iter()
        .flatten()
        .fold(IDENTITY4, |acc, part| multiply(&acc, &part.matrix));
    let own = match (style.transform_4, style.transform) {
        (Some(matrix), _) => matrix,
        (None, Some(planar)) => from_affine(planar),
        (None, None) => IDENTITY4,
    };
    let total = multiply(&composed, &own);
    style.transform = Some(planar(&total));
    style.transform_4 = (!is_planar(&total)).then_some(total);
    let (added, cross) = parts
        .first()
        .and_then(Option::as_ref)
        .map_or(([0.0; 2], [0.0; 2]), |part| (part.percent, part.cross));
    style.translate_percent = [
        style.translate_percent[0] + added[0],
        style.translate_percent[1] + added[1],
    ];
    style.translate_percent_cross = [
        style.translate_percent_cross[0] + cross[0],
        style.translate_percent_cross[1] + cross[1],
    ];
}

/** The z of `transform-origin` or `perspective-origin`'s third value, a length in px (0 when
absent). Percentages have no z, so a three-value origin must end in a length. */
pub(super) fn depth(value: &str, font: f64, context: RenderContext) -> EngineResult<f64> {
    let parts: Vec<&str> = value.split_whitespace().collect();
    match parts.as_slice() {
        [_, _, z] => {
            let dim = dimension(z, font, context)?;
            if dim.percent != 0.0 || !dim.px.is_finite() {
                return Err(unsupported());
            }
            Ok(dim.px.clamp(-CSS_MAX_SCALAR, CSS_MAX_SCALAR))
        }
        _ => Ok(0.0),
    }
}

/** `transform-origin` as [x px, x percent, y px, y percent] of the border box. One or two
keywords or lengths; a third (z) value must be zero. */
pub(super) fn origin(value: &str, font: f64, context: RenderContext) -> EngineResult<[f64; 4]> {
    let parts: Vec<&str> = value.split_whitespace().collect();
    let (x, y) = match parts.as_slice() {
        [only] if ["top", "bottom"].contains(only) => ("center", *only),
        [only] => (*only, "center"),
        [first, second] | [first, second, _] => ordered(first, second),
        _ => return Err(unsupported()),
    };
    let (x_px, x_percent) = position(x, ["left", "right"], font, context)?;
    let (y_px, y_percent) = position(y, ["top", "bottom"], font, context)?;
    Ok([x_px, x_percent, y_px, y_percent])
}

/** Puts a vertical keyword written first (`top left`) into the y slot. */
fn ordered<'a>(first: &'a str, second: &'a str) -> (&'a str, &'a str) {
    if ["top", "bottom"].contains(&first) || ["left", "right"].contains(&second) {
        (second, first)
    } else {
        (first, second)
    }
}

/** One axis: its start or end keyword, `center`, or a length or percentage. */
fn position(
    value: &str,
    [start, end]: [&str; 2],
    font: f64,
    context: RenderContext,
) -> EngineResult<(f64, f64)> {
    if value == start {
        return Ok((0.0, 0.0));
    }
    if value == end {
        return Ok((0.0, PERCENT));
    }
    if value == "center" {
        return Ok((0.0, CENTER));
    }
    let dim = dimension(value, font, context)?;
    if !dim.px.is_finite() || !dim.percent.is_finite() {
        return Err(unsupported());
    }
    let bound = |v: f64| v.clamp(-CSS_MAX_SCALAR, CSS_MAX_SCALAR);
    Ok((bound(dim.px), bound(dim.percent)))
}

/** The box-relative origin an absent `transform-origin` means. */
pub fn default_origin() -> [f64; 4] {
    [0.0, CENTER, 0.0, CENTER]
}
