/**
 * Parses `transform` lists into 4x4 matrices, CSS Transforms 2: every 2D and 3D function, each
 * length, percentage, angle and `calc()` argument, and the planar `matrix()` they reduce to.
 * A list that stays in the plane keeps only its 2D form, so 2D pages cost nothing extra; a list
 * with a real z component also keeps its 4x4 matrix for the 3D paint pass. Refusing a 3D form
 * is what made Airbnb's flip cards and `translate3d` menus silently lose their transforms.
 */
use super::{
    ComputedStyle,
    constants::{TRANSFORM_FUNCTIONS, TRANSFORM_MAX},
    paint::adjacent,
    syntax::{split_top, tokens},
    unsupported,
    values::{angle, dimension, signed_number},
};
use crate::{
    constants::CSS_MAX_SCALAR,
    document::{
        RenderContext,
        matrix::{
            IDENTITY4, Matrix4, from_affine, is_planar, multiply, perspective, planar, translation,
        },
    },
    protocol::EngineResult,
};

/** One parsed transform: its matrix, and the percentage translation that resolves against the
box at layout (`[x, y]` in the box's own size). */
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Parsed {
    /// The composed matrix with every px translation; percentages stay in `percent`.
    pub matrix: Matrix4,
    /// Percent of the box's width (x) and height (y), carried through the list's linear parts
    /// so a translate after a rotation moves along the rotated axes.
    pub percent: [f64; 2],
    /// The cross terms of that carry: y from a width percent, x from a height percent. Zero
    /// unless a rotate or skew sits before a percentage translate.
    pub cross: [f64; 2],
}

/** The `transform` value of `none`, which parses to no transform at all. */
const NONE: &str = "none";
/** The z of a planar `translate()`, `scale()` and `rotate()` placed in the plane. */
const FLAT_Z: f64 = 0.0;
/** A unit scale on z, which leaves a planar box where it was. */
const UNIT_SCALE: f64 = 1.0;
/** A zero-length rotation axis has no direction; the spec leaves it as the identity. */
const DEGENERATE_AXIS: f64 = 0.0;
/** A percentage's divisor when a scale factor is written as one. */
const PERCENT_SCALE: f64 = 100.0;
/** Argument count of `matrix()` and `matrix3d()`. */
const AFFINE_ARGS: usize = 6;
/** Argument count of `matrix3d()`. */
const MATRIX3D_ARGS: usize = 16;

impl Parsed {
    /** Whether the matrix leaves the plane, so painting needs the 3D pass. */
    pub fn spatial(&self) -> bool {
        !is_planar(&self.matrix)
    }
}

/** A `transform` list as one matrix and percentage, or None for `none`. */
pub(super) fn parse(
    value: &str,
    font: f64,
    context: RenderContext,
) -> EngineResult<Option<Parsed>> {
    if value.trim() == NONE {
        return Ok(None);
    }
    let functions: Vec<&str> = tokens(value).into_iter().flat_map(adjacent).collect();
    if functions.len() > TRANSFORM_FUNCTIONS {
        return Err(unsupported());
    }
    let (mut matrix, mut percent, mut cross) = (IDENTITY4, [0.0; 2], [0.0; 2]);
    for function in functions {
        let (next, pct) = function_matrix(function, font, context)?;
        // A percentage translation is expressed in the frame of the functions before it: its
        // width part maps through their x column and its height part through their y column.
        percent[0] += matrix[0] * pct[0];
        cross[0] += matrix[1] * pct[0];
        cross[1] += matrix[4] * pct[1];
        percent[1] += matrix[5] * pct[1];
        matrix = multiply(&matrix, &next);
    }
    bounded(&matrix, &percent)?;
    Ok(Some(Parsed {
        matrix,
        percent,
        cross,
    }))
}

/** Every entry of the matrix and percentage must be finite and within the transform bound. */
fn bounded(matrix: &Matrix4, percent: &[f64; 2]) -> EngineResult<()> {
    let ok = matrix
        .iter()
        .chain(percent.iter())
        .all(|v| v.is_finite() && v.abs() <= TRANSFORM_MAX);
    if ok { Ok(()) } else { Err(unsupported()) }
}

/** Writes a parsed transform, or `none`, into the style's planar and 4x4 fields. */
pub(super) fn store(style: &mut ComputedStyle, parsed: Option<Parsed>) {
    match parsed {
        None => {
            style.transform = None;
            style.transform_4 = None;
            style.translate_percent = [0.0; 2];
            style.translate_percent_cross = [0.0; 2];
        }
        Some(parsed) => {
            style.transform = Some(planar(&parsed.matrix));
            style.transform_4 = parsed.spatial().then_some(parsed.matrix);
            style.translate_percent = parsed.percent;
            style.translate_percent_cross = parsed.cross;
        }
    }
}

/** One function as a matrix and its own percentage translation. */
fn function_matrix(
    source: &str,
    font: f64,
    context: RenderContext,
) -> EngineResult<(Matrix4, [f64; 2])> {
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
    let arguments = Arguments {
        args,
        font,
        context,
    };
    arguments.build(&name.trim().to_ascii_lowercase())
}

/** A function's arguments with the font and box context its lengths need. */
struct Arguments<'a> {
    /// Each argument's text, already split at commas or spaces.
    args: Vec<&'a str>,
    /// The element's font size, for `em` and friends.
    font: f64,
    /// The viewport and containing block a length measures against.
    context: RenderContext,
}

impl Arguments<'_> {
    /** A length's px and percent, bounded so far offsets stay off-screen. */
    fn length(&self, index: usize) -> EngineResult<(f64, f64)> {
        let arg = self.args.get(index).ok_or_else(unsupported)?;
        let dim = dimension(arg, self.font, self.context)?;
        if !dim.px.is_finite() || !dim.percent.is_finite() {
            return Err(unsupported());
        }
        Ok((
            dim.px.clamp(-CSS_MAX_SCALAR, CSS_MAX_SCALAR),
            dim.percent.clamp(-CSS_MAX_SCALAR, CSS_MAX_SCALAR),
        ))
    }
    /** A length with no percentage, as a z offset or a perspective distance must be. */
    fn pixels(&self, index: usize) -> EngineResult<f64> {
        let (px, percent) = self.length(index)?;
        if percent != 0.0 {
            return Err(unsupported());
        }
        Ok(px)
    }
    /** A plain number argument. */
    fn number(&self, index: usize) -> EngineResult<f64> {
        let arg = self.args.get(index).ok_or_else(unsupported)?;
        signed_number(arg)
    }
    /** A scale factor: a number, or a percentage of one (`scale(60%)` is `scale(0.6)`). */
    fn factor(&self, index: usize) -> EngineResult<f64> {
        let arg = self.args.get(index).ok_or_else(unsupported)?;
        match arg.trim().strip_suffix('%') {
            Some(percent) => Ok(signed_number(percent)? / PERCENT_SCALE),
            None => signed_number(arg),
        }
    }
    /** An angle argument, in radians. */
    fn radians(&self, index: usize) -> EngineResult<f64> {
        let arg = self.args.get(index).ok_or_else(unsupported)?;
        Ok(angle(arg)?.to_radians())
    }
    /** The argument count, which each function checks before reading arguments. */
    fn count(&self) -> usize {
        self.args.len()
    }
    /** Builds the named function, or refuses a name Eye does not know. */
    fn build(&self, name: &str) -> EngineResult<(Matrix4, [f64; 2])> {
        if let Some(result) = self.build_2d(name)? {
            return Ok(result);
        }
        self.build_3d(name)?.ok_or_else(unsupported)
    }
    /** The planar functions: translate, scale, rotate, skew and the 2D matrix. */
    fn build_2d(&self, name: &str) -> EngineResult<Option<(Matrix4, [f64; 2])>> {
        let tan = |radians: f64| radians.tan();
        Ok(Some(match name {
            "matrix" if self.count() == AFFINE_ARGS => {
                let mut m = [0.0; AFFINE_ARGS];
                for (index, slot) in m.iter_mut().enumerate() {
                    *slot = self.number(index)?;
                }
                (from_affine(m), [0.0; 2])
            }
            "translate" if self.count() <= 2 => {
                let ((x, px), (y, py)) = (self.length(0)?, self.optional_length(1)?);
                (translation([x, y, FLAT_Z]), [px, py])
            }
            "translatex" => {
                let (x, px) = self.length(0)?;
                (translation([x, 0.0, FLAT_Z]), [px, 0.0])
            }
            "translatey" => {
                let (y, py) = self.length(0)?;
                (translation([0.0, y, FLAT_Z]), [0.0, py])
            }
            "scale" if self.count() <= 2 => {
                let x = self.factor(0)?;
                let y = if self.count() == 2 {
                    self.factor(1)?
                } else {
                    x
                };
                (scaled([x, y, UNIT_SCALE]), [0.0; 2])
            }
            "scalex" => (scaled([self.factor(0)?, 1.0, UNIT_SCALE]), [0.0; 2]),
            "scaley" => (scaled([1.0, self.factor(0)?, UNIT_SCALE]), [0.0; 2]),
            "rotate" => (rotation([0.0, 0.0, 1.0], self.radians(0)?), [0.0; 2]),
            "skew" if self.count() <= 2 => {
                let x = tan(self.radians(0)?);
                let y = if self.count() == 2 {
                    tan(self.radians(1)?)
                } else {
                    0.0
                };
                (from_affine([1.0, y, x, 1.0, 0.0, 0.0]), [0.0; 2])
            }
            "skewx" => (
                from_affine([1.0, 0.0, tan(self.radians(0)?), 1.0, 0.0, 0.0]),
                [0.0; 2],
            ),
            "skewy" => (
                from_affine([1.0, tan(self.radians(0)?), 0.0, 1.0, 0.0, 0.0]),
                [0.0; 2],
            ),
            _ => return Ok(None),
        }))
    }
    /** The 3D functions, whose z terms the planar forms above do not have. */
    fn build_3d(&self, name: &str) -> EngineResult<Option<(Matrix4, [f64; 2])>> {
        Ok(Some(match name {
            "matrix3d" if self.count() == MATRIX3D_ARGS => {
                let mut m = [0.0; MATRIX3D_ARGS];
                for (index, slot) in m.iter_mut().enumerate() {
                    *slot = self.number(index)?;
                }
                (m, [0.0; 2])
            }
            "translate3d" if self.count() == 3 => {
                let ((x, px), (y, py)) = (self.length(0)?, self.length(1)?);
                (translation([x, y, self.pixels(2)?]), [px, py])
            }
            "translatez" => (translation([0.0, 0.0, self.pixels(0)?]), [0.0; 2]),
            "scale3d" if self.count() == 3 => (
                scaled([self.factor(0)?, self.factor(1)?, self.factor(2)?]),
                [0.0; 2],
            ),
            "scalez" => (scaled([1.0, 1.0, self.number(0)?]), [0.0; 2]),
            "rotatex" => (rotation([1.0, 0.0, 0.0], self.radians(0)?), [0.0; 2]),
            "rotatey" => (rotation([0.0, 1.0, 0.0], self.radians(0)?), [0.0; 2]),
            "rotatez" => (rotation([0.0, 0.0, 1.0], self.radians(0)?), [0.0; 2]),
            "rotate3d" if self.count() == 4 => {
                let axis = normalized([self.number(0)?, self.number(1)?, self.number(2)?]);
                (rotation(axis, self.radians(3)?), [0.0; 2])
            }
            "perspective" => (self.perspective()?, [0.0; 2]),
            _ => return Ok(None),
        }))
    }
    /** `translate`'s optional second axis, which is zero when the function has one argument. */
    fn optional_length(&self, index: usize) -> EngineResult<(f64, f64)> {
        if index < self.count() {
            self.length(index)
        } else {
            Ok((0.0, 0.0))
        }
    }
    /** `perspective(d)`: `none` and a zero distance are the identity, a negative one is refused. */
    fn perspective(&self) -> EngineResult<Matrix4> {
        if self.args.first().is_some_and(|arg| arg.trim() == NONE) {
            return Ok(IDENTITY4);
        }
        let distance = self.pixels(0)?;
        if distance < 0.0 {
            return Err(unsupported());
        }
        Ok(if distance == 0.0 {
            IDENTITY4
        } else {
            perspective(distance)
        })
    }
}

/** A scale matrix with the given factors on x, y and z. */
fn scaled([x, y, z]: [f64; 3]) -> Matrix4 {
    let mut m = IDENTITY4;
    m[0] = x;
    m[5] = y;
    m[10] = z;
    m
}

/** A unit vector along `axis`, or the zero vector when the axis has no direction. */
fn normalized(axis: [f64; 3]) -> [f64; 3] {
    let length = axis.iter().map(|v| v * v).sum::<f64>().sqrt();
    if length == DEGENERATE_AXIS {
        return [DEGENERATE_AXIS; 3];
    }
    axis.map(|v| v / length)
}

/** Rotation by `radians` about a unit axis (Rodrigues' formula, CSS `rotate3d()`). A zero axis
gives the identity, as the spec leaves a rotation about no direction undefined. */
fn rotation([x, y, z]: [f64; 3], radians: f64) -> Matrix4 {
    if x == 0.0 && y == 0.0 && z == 0.0 {
        return IDENTITY4;
    }
    let (s, c) = radians.sin_cos();
    let t = 1.0 - c;
    [
        t * x * x + c,
        t * x * y + z * s,
        t * x * z - y * s,
        0.0,
        t * x * y - z * s,
        t * y * y + c,
        t * y * z + x * s,
        0.0,
        t * x * z + y * s,
        t * y * z - x * s,
        t * z * z + c,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ]
}
