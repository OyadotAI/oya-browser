/** CSS filter lists: each function parses to a color matrix or a blur, and a list applies them in order to a layer. */
use crate::canvas::{Bitmap, CHANNELS};
use crate::protocol::EngineResult;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/** A 4x5 affine color transform: rows are R, G, B, A; columns are the four channels and a constant. */
pub type Matrix = [[f64; 5]; 4];

/** Gradians per turn convert CSS angles to radians. */
const GRADIANS_PER_TURN: f64 = 400.0;
/** Full-scale channel value for converting between 8-bit storage and unit ranges. */
const FULL: f64 = u8::MAX as f64;

/** One parsed function: a per-pixel matrix, or a blur over the whole layer. */
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Step {
    /** Straight-alpha color transform applied to every pixel. */
    Color(Matrix),
    /** Gaussian blur of the premultiplied layer, with this standard deviation in pixels. */
    Blur(f64),
}

/** A parsed filter list and its canonical text, which the getter reports verbatim. */
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    /** Steps in application order. */
    pub steps: Vec<Step>,
    /** Canonical serialization, such as "blur(4px) grayscale(0.5)". */
    pub text: String,
}

/** The initial filter is none: no steps and the canonical text none. */
impl Default for Filter {
    fn default() -> Self {
        Self {
            steps: Vec::new(),
            text: "none".into(),
        }
    }
}

/** Parse a CSS filter value. None means invalid, so the previous filter stays in effect. */
pub fn parse(text: &str) -> Option<Filter> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("none") {
        return Some(Filter {
            steps: Vec::new(),
            text: "none".into(),
        });
    }
    let mut steps = Vec::new();
    let mut parts = Vec::new();
    for function in functions(text)? {
        if function.name == "url" {
            continue;
        }
        let (step, canonical) = function_step(&function)?;
        steps.push(step);
        parts.push(canonical);
    }
    let text = if parts.is_empty() {
        "none".into()
    } else {
        parts.join(" ")
    };
    Some(Filter { steps, text })
}

/** A function token: its lowercase name and the raw argument text between its parentheses. */
struct Function {
    name: String,
    argument: String,
}

/** Split a list into functions, rejecting anything that is not name(argument) with balanced parentheses. */
fn functions(text: &str) -> Option<Vec<Function>> {
    let mut out = Vec::new();
    let mut rest = text;
    while !rest.trim_start().is_empty() {
        rest = rest.trim_start();
        let open = rest.find('(')?;
        let name = rest[..open].trim().to_ascii_lowercase();
        let close = rest[open..].find(')')? + open;
        out.push(Function {
            name,
            argument: rest[open + 1..close].trim().to_string(),
        });
        rest = &rest[close + 1..];
    }
    (!out.is_empty()).then_some(out)
}

/** Convert one function to its step and canonical text; unknown names and bad arguments are invalid. */
fn function_step(function: &Function) -> Option<(Step, String)> {
    let arg = function.argument.as_str();
    let canonical = |value: f64, unit: &str| format!("{}({}{unit})", function.name, value);
    Some(match function.name.as_str() {
        "blur" => {
            let px = length(arg)?;
            (Step::Blur(px), canonical(px, "px"))
        }
        "hue-rotate" => {
            let radians = angle(arg)?;
            (
                Step::Color(hue_rotate(radians)),
                canonical(radians.to_degrees(), "deg"),
            )
        }
        "saturate" => {
            let amount = amount(arg, false)?;
            (Step::Color(saturate(amount)), canonical(amount, ""))
        }
        "brightness" | "contrast" => {
            let amount = amount(arg, false)?;
            let matrix = if function.name == "brightness" {
                brightness(amount)
            } else {
                contrast(amount)
            };
            (Step::Color(matrix), canonical(amount, ""))
        }
        "grayscale" | "sepia" | "invert" | "opacity" => {
            let amount = amount(arg, true)?;
            let matrix = match function.name.as_str() {
                "grayscale" => grayscale(amount),
                "sepia" => sepia(amount),
                "invert" => invert(amount),
                _ => opacity(amount),
            };
            (Step::Color(matrix), canonical(amount, ""))
        }
        _ => return None,
    })
}

/** A non-negative length in CSS pixels; only px, pt and a bare zero are accepted. */
fn length(text: &str) -> Option<f64> {
    if text == "0" {
        return Some(0.0);
    }
    let value = number(text.strip_suffix("px")?)?;
    (value >= 0.0).then_some(value)
}

/** A CSS angle converted to radians; deg, grad, rad and turn are all valid. */
fn angle(text: &str) -> Option<f64> {
    if text == "0" {
        return Some(0.0);
    }
    for (unit, scale) in [
        ("deg", PI / 180.0),
        ("grad", PI / (GRADIANS_PER_TURN / 2.0)),
        ("rad", 1.0),
        ("turn", 2.0 * PI),
    ] {
        if let Some(value) = text.strip_suffix(unit) {
            return number(value).map(|value| value * scale);
        }
    }
    None
}

/** A factor that may be written as a number or a percentage; percentages divide by one hundred. */
fn amount(text: &str, clamp_to_unit: bool) -> Option<f64> {
    let value = match text.strip_suffix('%') {
        Some(percent) => number(percent)? / 100.0,
        None => number(text)?,
    };
    if value < 0.0 {
        return None;
    }
    Some(if clamp_to_unit { value.min(1.0) } else { value })
}

/** A finite decimal number, with no exponent or other syntax CSS would reject. */
fn number(text: &str) -> Option<f64> {
    let value: f64 = text.trim().parse().ok()?;
    value.is_finite().then_some(value)
}

/** Each function is a fixed matrix, so applying a list costs only its own multiplications per pixel. */
fn grayscale(amount: f64) -> Matrix {
    let c = 1.0 - amount;
    [
        [
            0.2126 + 0.7874 * c,
            0.7152 - 0.7152 * c,
            0.0722 - 0.0722 * c,
            0.0,
            0.0,
        ],
        [
            0.2126 - 0.2126 * c,
            0.7152 + 0.2848 * c,
            0.0722 - 0.0722 * c,
            0.0,
            0.0,
        ],
        [
            0.2126 - 0.2126 * c,
            0.7152 - 0.7152 * c,
            0.0722 + 0.9278 * c,
            0.0,
            0.0,
        ],
        [0.0, 0.0, 0.0, 1.0, 0.0],
    ]
}

/** Sepia interpolates toward the classic brown matrix, per the Filter Effects specification. */
fn sepia(amount: f64) -> Matrix {
    let c = 1.0 - amount;
    [
        [
            0.393 + 0.607 * c,
            0.769 - 0.769 * c,
            0.189 - 0.189 * c,
            0.0,
            0.0,
        ],
        [
            0.349 - 0.349 * c,
            0.686 + 0.314 * c,
            0.168 - 0.168 * c,
            0.0,
            0.0,
        ],
        [
            0.272 - 0.272 * c,
            0.534 - 0.534 * c,
            0.131 + 0.869 * c,
            0.0,
            0.0,
        ],
        [0.0, 0.0, 0.0, 1.0, 0.0],
    ]
}

/** Saturation scales chroma about the luminance axis; zero is grayscale and above one oversaturates. */
fn saturate(s: f64) -> Matrix {
    [
        [
            0.213 + 0.787 * s,
            0.715 - 0.715 * s,
            0.072 - 0.072 * s,
            0.0,
            0.0,
        ],
        [
            0.213 - 0.213 * s,
            0.715 + 0.285 * s,
            0.072 - 0.072 * s,
            0.0,
            0.0,
        ],
        [
            0.213 - 0.213 * s,
            0.715 - 0.715 * s,
            0.072 + 0.928 * s,
            0.0,
            0.0,
        ],
        [0.0, 0.0, 0.0, 1.0, 0.0],
    ]
}

/** Hue rotation is a rotation in the plane orthogonal to the luminance axis. */
fn hue_rotate(radians: f64) -> Matrix {
    let (cos, sin) = (radians.cos(), radians.sin());
    [
        [
            0.213 + cos * 0.787 - sin * 0.213,
            0.715 - cos * 0.715 - sin * 0.715,
            0.072 - cos * 0.072 + sin * 0.928,
            0.0,
            0.0,
        ],
        [
            0.213 - cos * 0.213 + sin * 0.143,
            0.715 + cos * 0.285 + sin * 0.140,
            0.072 - cos * 0.072 - sin * 0.283,
            0.0,
            0.0,
        ],
        [
            0.213 - cos * 0.213 - sin * 0.787,
            0.715 - cos * 0.715 + sin * 0.715,
            0.072 + cos * 0.928 + sin * 0.072,
            0.0,
            0.0,
        ],
        [0.0, 0.0, 0.0, 1.0, 0.0],
    ]
}

/** Invert maps each channel c to a + c(1 - 2a), so amount one is a full inversion. */
fn invert(amount: f64) -> Matrix {
    let scale = 1.0 - 2.0 * amount;
    [
        [scale, 0.0, 0.0, 0.0, amount],
        [0.0, scale, 0.0, 0.0, amount],
        [0.0, 0.0, scale, 0.0, amount],
        [0.0, 0.0, 0.0, 1.0, 0.0],
    ]
}

/** Opacity scales alpha only, leaving the straight color channels alone. */
fn opacity(amount: f64) -> Matrix {
    [
        [1.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, amount, 0.0],
    ]
}

/** Brightness multiplies the color channels; values above one clip at full scale. */
fn brightness(amount: f64) -> Matrix {
    [
        [amount, 0.0, 0.0, 0.0, 0.0],
        [0.0, amount, 0.0, 0.0, 0.0],
        [0.0, 0.0, amount, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0, 0.0],
    ]
}

/** Contrast stretches each channel about middle gray. */
fn contrast(amount: f64) -> Matrix {
    let offset = 0.5 - 0.5 * amount;
    [
        [amount, 0.0, 0.0, 0.0, offset],
        [0.0, amount, 0.0, 0.0, offset],
        [0.0, 0.0, amount, 0.0, offset],
        [0.0, 0.0, 0.0, 1.0, 0.0],
    ]
}

/** Apply every step to a layer in order; blur runs on premultiplied channels, color steps on straight ones. */
pub fn apply(layer: &mut Bitmap, steps: &[Step]) -> EngineResult<()> {
    layer.validate()?;
    for step in steps {
        match step {
            Step::Color(matrix) => color(layer, matrix)?,
            Step::Blur(sigma) => blur(layer, *sigma)?,
        }
    }
    Ok(())
}

/** Per-pixel matrix on straight color, with each output clamped to the channel range. */
fn color(layer: &mut Bitmap, matrix: &Matrix) -> EngineResult<()> {
    for pixel in layer.pixels.make_mut()?.chunks_exact_mut(CHANNELS) {
        let input: [f64; 4] = std::array::from_fn(|i| f64::from(pixel[i]) / FULL);
        for (channel, row) in matrix.iter().enumerate() {
            let value = row[0] * input[0]
                + row[1] * input[1]
                + row[2] * input[2]
                + row[3] * input[3]
                + row[4];
            pixel[channel] = (value.clamp(0.0, 1.0) * FULL).round() as u8;
        }
    }
    Ok(())
}

/** Blur premultiplied color planes and alpha, then restore straight color where any alpha remains. */
fn blur(layer: &mut Bitmap, sigma: f64) -> EngineResult<()> {
    let (width, height) = (layer.width as usize, layer.height as usize);
    let pixels = layer.pixels.make_mut()?;
    let mut planes: [Vec<f32>; CHANNELS] = std::array::from_fn(|channel| {
        pixels
            .chunks_exact(CHANNELS)
            .map(|pixel| premultiplied(pixel, channel))
            .collect()
    });
    for plane in planes.iter_mut() {
        super::blur::blur(plane, width, height, sigma);
    }
    for (index, pixel) in pixels.chunks_exact_mut(CHANNELS).enumerate() {
        let alpha = planes[CHANNELS - 1][index];
        for (channel, plane) in planes.iter().enumerate().take(CHANNELS - 1) {
            let color = if alpha > 0.0 {
                plane[index] / alpha * FULL as f32
            } else {
                0.0
            };
            pixel[channel] = color.round().clamp(0.0, FULL as f32) as u8;
        }
        pixel[CHANNELS - 1] = alpha.round().clamp(0.0, FULL as f32) as u8;
    }
    Ok(())
}

/** Color channels scale by alpha before blurring; alpha itself is blurred as stored. */
fn premultiplied(pixel: &[u8], channel: usize) -> f32 {
    let alpha = f32::from(pixel[CHANNELS - 1]);
    if channel == CHANNELS - 1 {
        alpha
    } else {
        f32::from(pixel[channel]) * alpha / FULL as f32
    }
}
