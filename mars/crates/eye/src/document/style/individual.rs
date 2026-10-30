/**
 * The individual transform properties `translate`, `rotate` and `scale`, and `transform-origin`.
 * Sites write `translate: -50% -50%` or `scale: 1.05` instead of a `transform` list, and set
 * `transform-origin: top left` on menus and scaled cards; without these, centered overlays sat
 * off by half their size and hover zooms grew from the wrong corner.
 */
use super::{
    ComputedStyle, unsupported,
    values::{angle, dimension},
};
use crate::{constants::CSS_MAX_SCALAR, document::RenderContext, protocol::EngineResult};

/** The individual properties in the order CSS Transforms 2 composes them, ahead of `transform`. */
pub(super) const KEYS: [&str; 3] = ["translate", "rotate", "scale"];
/** A percentage as a fraction: `scale: 150%` is 1.5. */
const PERCENT: f64 = 100.0;
/** The initial `transform-origin`, the center of the border box, as percentages. */
const CENTER: f64 = 50.0;

/** One property as a matrix and percentage translation, None for `none`; 3D forms other than a
rotation about z, or a z translation or scale, are refused like a 3D `transform`. */
pub(super) fn matrix(
    key: &str,
    value: &str,
    font: f64,
    context: RenderContext,
) -> EngineResult<(Option<[f64; 6]>, [f64; 2])> {
    match as_function(key, value)? {
        Some(function) => super::paint::transform(&function, font, context),
        None => Ok((None, [0.0; 2])),
    }
}

/** The equivalent transform function text, such as `translate(10px,50%)`, or None for `none`. */
fn as_function(key: &str, value: &str) -> EngineResult<Option<String>> {
    let parts: Vec<&str> = value.split_whitespace().collect();
    if parts == ["none"] {
        return Ok(None);
    }
    let planar = |parts: &[&str]| -> EngineResult<String> {
        match parts {
            [x] => Ok((*x).to_owned()),
            [x, y] => Ok(format!("{x},{y}")),
            [x, y, z] if flat(z) => Ok(format!("{x},{y}")),
            _ => Err(unsupported()),
        }
    };
    Ok(Some(match key {
        "translate" => format!("translate({})", planar(&parts)?),
        "scale" => {
            let factors = fractions(&parts);
            let factors: Vec<&str> = factors.iter().map(String::as_str).collect();
            format!("scale({})", planar(&factors)?)
        }
        "rotate" => format!("rotate({})", z_angle(&parts)?),
        _ => return Err(unsupported()),
    }))
}

/** A z component that leaves the box flat: zero length for translate, 1 for scale. */
fn flat(z: &str) -> bool {
    ["0", "0px", "1"].contains(&z)
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

/** The angle of `rotate: 45deg`, `rotate: z 45deg` or `rotate: 0 0 1 45deg`. */
fn z_angle<'a>(parts: &[&'a str]) -> EngineResult<&'a str> {
    match parts {
        [degrees] | ["z", degrees] => Ok(degrees),
        ["0", "0", z, degrees] if z.parse::<f64>().is_ok_and(|z| z > 0.0) => Ok(degrees),
        _ => Err(unsupported()),
    }
    .and_then(|degrees| angle(degrees).map(|_| *degrees))
}

/** Composes the individual properties over the computed `transform`: translate, then rotate,
then scale, then the list. ponytail: a percentage `translate` adds to the list's own percentage
offset, exact when no rotate or scale sits between them (the common centering case). */
pub(super) fn compose(style: &mut ComputedStyle, parts: &[(Option<[f64; 6]>, [f64; 2])]) {
    if parts
        .iter()
        .all(|(matrix, percent)| matrix.is_none() && *percent == [0.0; 2])
    {
        return;
    }
    let mut composed = super::paint::IDENTITY;
    // This slice interpolates the transform property alone, not independently composed properties.
    style.transform_animation_excluded = true;
    for (matrix, _) in parts {
        composed = super::paint::multiply(composed, matrix.unwrap_or(super::paint::IDENTITY));
    }
    let own = style.transform.unwrap_or(super::paint::IDENTITY);
    style.transform = Some(super::paint::multiply(composed, own));
    let added = parts.first().map_or([0.0; 2], |(_, percent)| *percent);
    style.translate_percent = [
        style.translate_percent[0] + added[0],
        style.translate_percent[1] + added[1],
    ];
}

/** `transform-origin` as [x px, x percent, y px, y percent] of the border box. One or two
keywords or lengths; a third (z) value must be zero. */
pub(super) fn origin(value: &str, font: f64, context: RenderContext) -> EngineResult<[f64; 4]> {
    let parts: Vec<&str> = value.split_whitespace().collect();
    let (x, y) = match parts.as_slice() {
        [only] if ["top", "bottom"].contains(only) => ("center", *only),
        [only] => (*only, "center"),
        [first, second] | [first, second, "0" | "0px"] => ordered(first, second),
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
