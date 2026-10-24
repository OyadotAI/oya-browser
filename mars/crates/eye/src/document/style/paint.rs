/** Box shadows, background layers and 2D transforms parse into bounded painter data. */
use super::{
    BackgroundLayer, BoxShadow, ColorStop, ComputedStyle, RadialShape, RadialSize,
    constants::{
        BACKGROUND_LAYERS, GRADIENT_STOPS, PERCENT, SHADOWS, TRANSFORM_FUNCTIONS, TRANSFORM_MAX,
    },
    layout_props::{background_position, extent},
    note,
    syntax::{split_top, tokens},
    unsupported,
    values::{angle, contextual_color, dimension, signed_length},
};
use crate::{
    constants::{CSS_MAX_SCALAR, URL_BYTES},
    document::RenderContext,
    protocol::EngineResult,
};
/** Opaque black, the color of a shadow without one and without a current color. */
const BLACK: &str = "#000000";
/** `none` or up to SHADOWS comma-separated layers; extra layers are ignored. */
pub(super) fn shadows(
    value: &str,
    style: &ComputedStyle,
    context: RenderContext,
) -> EngineResult<Vec<BoxShadow>> {
    if value == "none" {
        return Ok(Vec::new());
    }
    split_top(value, b',')
        .into_iter()
        .take(SHADOWS)
        .map(|layer| shadow(layer, style, context))
        .collect()
}
/** `[inset] x y [blur [spread]] [color]` in any order of the color and inset. */
fn shadow(layer: &str, style: &ComputedStyle, context: RenderContext) -> EngineResult<BoxShadow> {
    let mut lengths = Vec::new();
    let (mut color, mut inset) = (None, false);
    for token in tokens(layer) {
        if token == "inset" && !inset {
            inset = true;
        } else if let Ok(length) = signed_length(token, style.font_size, context) {
            lengths.push(length);
        } else if color.is_none() {
            color = Some(
                contextual_color(token, style.used_scheme, style.color.as_deref())?
                    .unwrap_or_else(|| BLACK.into()),
            );
        } else {
            return Err(unsupported());
        }
    }
    let [x, y, blur, spread] = match lengths.as_slice() {
        [x, y] => [*x, *y, 0.0, 0.0],
        [x, y, blur] => [*x, *y, *blur, 0.0],
        [x, y, blur, spread] => [*x, *y, *blur, *spread],
        _ => return Err(unsupported()),
    };
    if blur < 0.0 {
        return Err(unsupported());
    }
    Ok(BoxShadow {
        x,
        y,
        blur,
        spread,
        color: color
            .or_else(|| style.color.clone())
            .unwrap_or_else(|| BLACK.into()),
        inset,
    })
}
/** Layers in order; unsupported image kinds are skipped with a diagnostic, not fatal. */
pub(super) fn backgrounds(
    raw: &str,
    style: &ComputedStyle,
    context: RenderContext,
    diagnostics: &mut Vec<String>,
) -> EngineResult<Vec<BackgroundLayer>> {
    let mut layers = Vec::new();
    for layer in split_top(raw, b',').into_iter().take(BACKGROUND_LAYERS) {
        match image(layer, style, context) {
            Ok(Some(layer)) => layers.push(layer),
            Ok(None) => {}
            Err(_) => note(
                diagnostics,
                "A CSS background image kind is not supported and was ignored.".into(),
            ),
        }
    }
    Ok(layers)
}
/** One image: `none`, `url()`, or a (repeating) linear or radial gradient. */
fn image(
    layer: &str,
    style: &ComputedStyle,
    context: RenderContext,
) -> EngineResult<Option<BackgroundLayer>> {
    let lower = layer.trim().to_ascii_lowercase();
    if lower == "none" {
        return Ok(None);
    }
    if lower.starts_with("url(") {
        return url(layer.trim()).map(|url| Some(BackgroundLayer::Image { url }));
    }
    let (name, inner) = lower
        .strip_suffix(')')
        .and_then(|value| value.split_once('('))
        .ok_or_else(unsupported)?;
    let arguments = split_top(inner, b',');
    let layer = match name.trim_start_matches("-webkit-") {
        "linear-gradient" | "repeating-linear-gradient" => linear(&arguments, style)?,
        "radial-gradient" | "repeating-radial-gradient" => radial(&arguments, style, context)?,
        _ => return Err(unsupported()),
    };
    Ok(Some(layer))
}
/** The first `url()` layer of a `mask-image` list, if any; gradient masks have none. */
pub(super) fn mask_url(value: &str) -> Option<String> {
    split_top(value, b',')
        .into_iter()
        .find(|layer| {
            layer
                .get(..4)
                .is_some_and(|head| head.eq_ignore_ascii_case("url("))
        })
        .and_then(|layer| url(layer.trim()).ok())
}
/** The authored URL text, unquoted, bounded and free of control characters. */
fn url(source: &str) -> EngineResult<String> {
    let inner = source
        .get(4..source.len().saturating_sub(1))
        .ok_or_else(unsupported)?
        .trim();
    let unquoted = inner
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| inner.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(inner);
    if unquoted.is_empty() || unquoted.len() > URL_BYTES || unquoted.chars().any(char::is_control) {
        return Err(unsupported());
    }
    Ok(unquoted.into())
}
/** An optional angle or `to side` direction, then stops; the default points down (180). */
fn linear(arguments: &[&str], style: &ComputedStyle) -> EngineResult<BackgroundLayer> {
    let first = arguments.first().ok_or_else(unsupported)?;
    let direction = if let Some(sides) = first.strip_prefix("to ") {
        Some(to_angle(sides)?)
    } else {
        angle(first).ok()
    };
    Ok(BackgroundLayer::Linear {
        angle: direction.unwrap_or(180.0).rem_euclid(360.0),
        stops: stops(arguments, style, direction.is_some())?,
    })
}
/** An optional `[shape] [size] [at position]` configuration, then stops. */
fn radial(
    arguments: &[&str],
    style: &ComputedStyle,
    context: RenderContext,
) -> EngineResult<BackgroundLayer> {
    let first = arguments.first().ok_or_else(unsupported)?;
    let shape = radial_shape(first, style.font_size, context).ok();
    Ok(BackgroundLayer::Radial {
        stops: stops(arguments, style, shape.is_some())?,
        shape: shape.unwrap_or_default(),
    })
}
/** Parses a radial configuration such as `circle at 50% 0` or `60% 40% at top`; Err when it is a stop. */
fn radial_shape(config: &str, font: f64, context: RenderContext) -> EngineResult<RadialShape> {
    let words = tokens(config);
    let at = words.iter().position(|word| *word == "at");
    let (head, position) = words.split_at(at.unwrap_or(words.len()));
    let mut shape = RadialShape::default();
    if let Some(position) = position.get(1..) {
        shape.center = background_position(&position.join(" "), font, context)?;
    }
    let mut radii = Vec::new();
    let mut ellipse = false;
    for word in head {
        match *word {
            "circle" => shape.circle = true,
            "ellipse" => ellipse = true,
            "closest-side" => shape.size = RadialSize::ClosestSide,
            "farthest-side" => shape.size = RadialSize::FarthestSide,
            "closest-corner" => shape.size = RadialSize::ClosestCorner,
            "farthest-corner" => shape.size = RadialSize::FarthestCorner,
            length => radii.push(extent(length, font, context)?),
        }
    }
    shape.size = match radii.as_slice() {
        [] if head.is_empty() && at.is_none() => return Err(unsupported()),
        [] => shape.size,
        [r] if !ellipse => {
            shape.circle = true;
            RadialSize::Explicit(*r, *r)
        }
        [x, y] if !shape.circle => RadialSize::Explicit(*x, *y),
        _ => return Err(unsupported()),
    };
    Ok(shape)
}
/** `to` keywords as CSS angles; corners use the square-box diagonal. */
fn to_angle(sides: &str) -> EngineResult<f64> {
    let mut words: Vec<&str> = sides.split_whitespace().collect();
    words.sort_unstable();
    Ok(match words.as_slice() {
        ["top"] => 0.0,
        ["right"] => 90.0,
        ["bottom"] => 180.0,
        ["left"] => 270.0,
        ["right", "top"] => 45.0,
        ["bottom", "right"] => 135.0,
        ["bottom", "left"] => 225.0,
        ["left", "top"] => 315.0,
        _ => return Err(unsupported()),
    })
}
/** Color stops; a leading non-color argument is configuration and lone lengths are hints, both skipped. */
fn stops(
    arguments: &[&str],
    style: &ComputedStyle,
    skip_first: bool,
) -> EngineResult<Vec<ColorStop>> {
    let mut out = Vec::new();
    for (index, argument) in arguments.iter().enumerate() {
        if index == 0 && skip_first {
            continue;
        }
        let parts = tokens(argument);
        let Some(color) = parts.first() else {
            continue;
        };
        let color = match contextual_color(color, style.used_scheme, style.color.as_deref()) {
            Ok(color) => color.unwrap_or_else(|| BLACK.into()),
            Err(_) if index == 0 => continue,
            Err(_)
                if parts.len() == 1 && dimension(color, 0.0, RenderContext::default()).is_ok() =>
            {
                continue;
            }
            Err(error) => return Err(error),
        };
        let positions: Vec<Option<f64>> =
            parts[1..].iter().map(|part| stop_position(part)).collect();
        for position in if positions.is_empty() {
            vec![None]
        } else {
            positions
        } {
            out.push(ColorStop {
                color: color.clone(),
                position,
            });
        }
    }
    out.truncate(GRADIENT_STOPS);
    if out.len() < 2 {
        return Err(unsupported());
    }
    Ok(out)
}
/** Percent positions as 0..1 fractions; lengths need the box, so they are left to the painter as None. */
fn stop_position(part: &str) -> Option<f64> {
    let percent = part.strip_suffix('%')?.trim().parse::<f64>().ok()?;
    percent
        .is_finite()
        .then(|| (percent / PERCENT).clamp(0.0, 1.0))
}
/** `none`, or a composed list of 2D transform functions and percentage translations. */
pub(super) fn transform(
    value: &str,
    font: f64,
    context: RenderContext,
) -> EngineResult<(Option<[f64; 6]>, [f64; 2])> {
    if value == "none" {
        return Ok((None, [0.0; 2]));
    }
    let functions: Vec<&str> = tokens(value).into_iter().flat_map(adjacent).collect();
    if functions.len() > TRANSFORM_FUNCTIONS {
        return Err(unsupported());
    }
    let (mut matrix, mut percent) = (IDENTITY, [0.0; 2]);
    for function in functions {
        let (next, pct) = transform_function(function, font, context)?;
        percent = [
            matrix[0] * pct[0] + matrix[2] * pct[1] + percent[0],
            matrix[1] * pct[0] + matrix[3] * pct[1] + percent[1],
        ];
        matrix = multiply(matrix, next);
    }
    if matrix
        .iter()
        .chain(percent.iter())
        .all(|v| v.is_finite() && v.abs() <= TRANSFORM_MAX)
    {
        Ok((Some(matrix), percent))
    } else {
        Err(unsupported())
    }
}
/** Splits functions minifiers write without a space, as in `translateY(4px)scale(.9)`. */
fn adjacent(token: &str) -> Vec<&str> {
    let (mut parts, mut start, mut depth) = (Vec::new(), 0, 0usize);
    for (index, byte) in token.bytes().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' if depth == 1 => {
                depth = 0;
                parts.push(&token[start..=index]);
                start = index + 1;
            }
            b')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    if start < token.len() {
        parts.push(&token[start..]);
    }
    parts
}
/** The identity affine matrix. */
const IDENTITY: [f64; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
/** `m1 × m2` for affine matrices in (a, b, c, d, e, f) order. */
fn multiply(m: [f64; 6], n: [f64; 6]) -> [f64; 6] {
    [
        m[0] * n[0] + m[2] * n[1],
        m[1] * n[0] + m[3] * n[1],
        m[0] * n[2] + m[2] * n[3],
        m[1] * n[2] + m[3] * n[3],
        m[0] * n[4] + m[2] * n[5] + m[4],
        m[1] * n[4] + m[3] * n[5] + m[5],
    ]
}
/** One transform function as a matrix and optional `[tx_percent, ty_percent]` of the box size. */
fn transform_function(
    source: &str,
    font: f64,
    context: RenderContext,
) -> EngineResult<([f64; 6], [f64; 2])> {
    let (name, inner) = source
        .strip_suffix(')')
        .and_then(|value| value.split_once('('))
        .ok_or_else(unsupported)?;
    let args = split_top(inner, b',');
    let args = if args.len() == 1 {
        tokens(args[0])
    } else {
        args
    };
    let offset = |index: usize| -> EngineResult<(f64, f64)> {
        let Some(arg) = args.get(index) else {
            return Ok((0.0, 0.0));
        };
        let dim = dimension(arg, font, context)?;
        if !dim.px.is_finite() || !dim.percent.is_finite() {
            return Err(unsupported());
        }
        Ok((
            dim.px.clamp(-CSS_MAX_SCALAR, CSS_MAX_SCALAR),
            dim.percent.clamp(-CSS_MAX_SCALAR, CSS_MAX_SCALAR),
        ))
    };
    let number = |index: usize| -> EngineResult<f64> {
        let arg = args.get(index).ok_or_else(unsupported)?;
        super::values::signed_number(arg)
    };
    let tan = |degrees: f64| degrees.to_radians().tan();
    Ok(match name.trim() {
        "translate" | "translate3d" => {
            let ((x, px), (y, py)) = (offset(0)?, offset(1)?);
            ([1.0, 0.0, 0.0, 1.0, x, y], [px, py])
        }
        "translatex" => {
            let (x, px) = offset(0)?;
            ([1.0, 0.0, 0.0, 1.0, x, 0.0], [px, 0.0])
        }
        "translatey" => {
            let (y, py) = offset(0)?;
            ([1.0, 0.0, 0.0, 1.0, 0.0, y], [0.0, py])
        }
        "scale" | "scale3d" => {
            let x = number(0)?;
            let y = if args.len() > 1 { number(1)? } else { x };
            ([x, 0.0, 0.0, y, 0.0, 0.0], [0.0; 2])
        }
        "scalex" => ([number(0)?, 0.0, 0.0, 1.0, 0.0, 0.0], [0.0; 2]),
        "scaley" => ([1.0, 0.0, 0.0, number(0)?, 0.0, 0.0], [0.0; 2]),
        "rotate" | "rotatez" => {
            let radians = angle(args.first().ok_or_else(unsupported)?)?.to_radians();
            let (sin, cos) = radians.sin_cos();
            ([cos, sin, -sin, cos, 0.0, 0.0], [0.0; 2])
        }
        "skew" => {
            let x = angle(args.first().ok_or_else(unsupported)?)?;
            let y = args.get(1).map_or(Ok(0.0), |arg| angle(arg))?;
            ([1.0, tan(y), tan(x), 1.0, 0.0, 0.0], [0.0; 2])
        }
        "skewx" => (
            [
                1.0,
                0.0,
                tan(angle(args.first().ok_or_else(unsupported)?)?),
                1.0,
                0.0,
                0.0,
            ],
            [0.0; 2],
        ),
        "skewy" => (
            [
                1.0,
                tan(angle(args.first().ok_or_else(unsupported)?)?),
                0.0,
                1.0,
                0.0,
                0.0,
            ],
            [0.0; 2],
        ),
        "matrix" if args.len() == 6 => {
            let mut m = [0.0; 6];
            for (index, slot) in m.iter_mut().enumerate() {
                *slot = number(index)?;
            }
            (m, [0.0; 2])
        }
        _ => return Err(unsupported()),
    })
}
