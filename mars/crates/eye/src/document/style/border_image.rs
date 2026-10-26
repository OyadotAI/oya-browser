/**
 * `border-image` and its longhands: the computed nine-slice description a painter draws in
 * place of the border, parsed as Blink's `CSSParsingUtils::ConsumeBorderImage*` reads them.
 */
use super::{
    BackgroundLayer, ComputedStyle, Extent,
    syntax::{find_top, tokens},
    unsupported,
    values::{dimension, number},
};
use crate::{constants::CSS_MAX_SCALAR, document::RenderContext, protocol::EngineResult};
use serde::{Deserialize, Serialize};

/** The longhands the `border-image` shorthand sets, in its own grammar's order. */
pub(super) const LONGHANDS: [&str; 5] = [
    "border-image-source",
    "border-image-slice",
    "border-image-width",
    "border-image-outset",
    "border-image-repeat",
];
/** Initial values of LONGHANDS, which the shorthand resets when it leaves one out. */
const INITIAL: [&str; 5] = ["none", "100%", "1", "0", "stretch"];
/** Sides a box-edge value names at most: top, right, bottom, left. */
const SIDES: usize = 4;
/** The `/` that separates slice, width and outset inside the shorthand. */
const SLASH: u8 = b'/';

/// One side of `border-image-width` or `border-image-outset`.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BorderImageLength {
    /// A multiple of the box's border width on that side.
    Number(f64),
    /// A length, with a percentage of the border image area (widths only).
    Length(Extent),
    /// The image slice's own size (widths only).
    Auto,
}
/// How an edge or the middle fills its area along one axis (`border-image-repeat`).
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BorderImageRepeat {
    /// One tile scaled to the area.
    #[default]
    Stretch,
    /// Tiles at their scaled size, centered, cut at both ends.
    Repeat,
    /// Tiles rescaled so a whole number fills the area.
    Round,
    /// Whole tiles with equal gaps between and around them.
    Space,
}
/// The computed `border-image`: with no source the box paints its ordinary border.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct BorderImage {
    /// `border-image-source`: a `url()` image or a gradient; None is `none`.
    pub source: Option<BackgroundLayer>,
    /// `border-image-slice` per side: a number of image pixels (`px`) or a percentage of it.
    pub slice: [Extent; SIDES],
    /// `fill`: the middle slice paints over the background as well.
    pub fill: bool,
    /// `border-image-width` per side.
    pub width: [BorderImageLength; SIDES],
    /// `border-image-outset` per side.
    pub outset: [BorderImageLength; SIDES],
    /// `border-image-repeat`, horizontal then vertical.
    pub repeat: [BorderImageRepeat; 2],
}
impl Default for BorderImage {
    fn default() -> Self {
        let whole = Extent {
            px: 0.0,
            percent: 100.0,
        };
        Self {
            source: None,
            slice: [whole; SIDES],
            fill: false,
            width: [BorderImageLength::Number(1.0); SIDES],
            outset: [BorderImageLength::Number(0.0); SIDES],
            repeat: [BorderImageRepeat::Stretch; 2],
        }
    }
}
impl BorderImage {
    /** Serialization leaves the initial value out, so styles without one keep their shape. */
    pub(super) fn is_initial(&self) -> bool {
        *self == Self::default()
    }
    /** Every number is finite and within the engine's scalar bound, as parsing leaves them. */
    pub(super) fn valid(&self) -> bool {
        let ok = |value: f64| value.is_finite() && value.abs() <= CSS_MAX_SCALAR;
        let side = |length: &BorderImageLength| match length {
            BorderImageLength::Number(n) => ok(*n),
            BorderImageLength::Length(extent) => ok(extent.px) && ok(extent.percent),
            BorderImageLength::Auto => true,
        };
        self.slice.iter().all(|e| ok(e.px) && ok(e.percent))
            && self.width.iter().chain(&self.outset).all(side)
    }
}

/** The `url()` source of a box's border image, which the app fetches under its identity. */
pub(super) fn url(style: &ComputedStyle) -> Option<&String> {
    match &style.border_image.source {
        Some(BackgroundLayer::Image { url }) => Some(url),
        _ => None,
    }
}
/**
 * Whether the bytes fetched under an element's identity are its border image: one identity
 * holds one image, and a mask or background `url()` claims it first (see `background_urls`).
 */
pub(crate) fn fetched_border_image(style: &ComputedStyle) -> bool {
    let background = style
        .backgrounds
        .iter()
        .any(|layer| matches!(layer, BackgroundLayer::Image { .. }));
    url(style).is_some() && style.mask.is_none() && !background
}
/** Applies one `border-image-*` longhand; Ok(false) for every other property. */
pub(super) fn apply(
    style: &mut ComputedStyle,
    key: &str,
    value: &str,
    context: RenderContext,
) -> EngineResult<bool> {
    let font = style.font_size;
    let image = &mut style.border_image;
    match key {
        "border-image-source" => image.source = source(value, context)?,
        "border-image-slice" => (image.slice, image.fill) = slice(value)?,
        "border-image-width" => image.width = sides(value, &|v| width(v, font, context))?,
        "border-image-outset" => image.outset = sides(value, &|v| outset(v, font, context))?,
        "border-image-repeat" => image.repeat = repeat(value)?,
        _ => return Ok(false),
    }
    Ok(true)
}
/** `none`, a `url()` or a gradient, as one `background-image` layer. */
fn source(value: &str, context: RenderContext) -> EngineResult<Option<BackgroundLayer>> {
    if value.trim().eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let mut notes = Vec::new();
    let layers = super::paint::backgrounds(value, &ComputedStyle::default(), context, &mut notes)?;
    match layers.as_slice() {
        [layer] if notes.is_empty() => Ok(Some(layer.clone())),
        _ => Err(unsupported()),
    }
}
/** One to four numbers or percentages, clockwise from the top, and an optional `fill`. */
fn slice(value: &str) -> EngineResult<([Extent; SIDES], bool)> {
    let words = tokens(value);
    let fill = words
        .iter()
        .filter(|w| w.eq_ignore_ascii_case("fill"))
        .count();
    let first_or_last = |word: Option<&&str>| word.is_some_and(|w| w.eq_ignore_ascii_case("fill"));
    if fill > 1 || (fill == 1 && !first_or_last(words.first()) && !first_or_last(words.last())) {
        return Err(unsupported());
    }
    let numbers: Vec<&str> = words
        .into_iter()
        .filter(|w| !w.eq_ignore_ascii_case("fill"))
        .collect();
    let parsed = expand(&numbers, &|word| {
        Ok(match word.strip_suffix('%') {
            Some(percent) => Extent {
                px: 0.0,
                percent: number(percent)?,
            },
            None => Extent {
                px: number(word)?,
                percent: 0.0,
            },
        })
    })?;
    Ok((parsed, fill == 1))
}
/** A box-edge value of one to four parts, each parsed by `part`. */
fn sides(
    value: &str,
    part: &dyn Fn(&str) -> EngineResult<BorderImageLength>,
) -> EngineResult<[BorderImageLength; SIDES]> {
    expand(&tokens(value), part)
}
/** CSS's one-to-four side expansion: top, right (left), bottom (top), left (right). */
fn expand<T: Copy>(
    words: &[&str],
    part: &dyn Fn(&str) -> EngineResult<T>,
) -> EngineResult<[T; SIDES]> {
    let parsed = words
        .iter()
        .map(|word| part(word))
        .collect::<EngineResult<Vec<T>>>()?;
    let pick = |indices: [usize; SIDES]| indices.map(|index| parsed[index]);
    Ok(match parsed.len() {
        1 => pick([0, 0, 0, 0]),
        2 => pick([0, 1, 0, 1]),
        3 => pick([0, 1, 2, 1]),
        4 => pick([0, 1, 2, 3]),
        _ => return Err(unsupported()),
    })
}
/** A nonnegative number, length or percentage, or `auto`. */
fn width(value: &str, font: f64, context: RenderContext) -> EngineResult<BorderImageLength> {
    if value.eq_ignore_ascii_case("auto") {
        return Ok(BorderImageLength::Auto);
    }
    outset_or_width(value, font, context)
}
/** A nonnegative number or length; percentages are not outsets. */
fn outset(value: &str, font: f64, context: RenderContext) -> EngineResult<BorderImageLength> {
    match outset_or_width(value, font, context)? {
        BorderImageLength::Length(extent) if extent.percent != 0.0 => Err(unsupported()),
        length => Ok(length),
    }
}
/** The grammar both share: a bare number multiplies the border width, else a length. */
fn outset_or_width(
    value: &str,
    font: f64,
    context: RenderContext,
) -> EngineResult<BorderImageLength> {
    if let Ok(multiple) = number(value) {
        return Ok(BorderImageLength::Number(multiple));
    }
    let length = dimension(value, font, context)?;
    let ok = |v: f64| v.is_finite() && v.abs() <= CSS_MAX_SCALAR;
    let negative = length.percent < 0.0 || (length.px < 0.0 && length.percent == 0.0);
    if !ok(length.px) || !ok(length.percent) || negative {
        return Err(unsupported());
    }
    Ok(BorderImageLength::Length(Extent {
        px: length.px,
        percent: length.percent,
    }))
}
/** One or two of stretch, repeat, round and space; one keyword sets both axes. */
fn repeat(value: &str) -> EngineResult<[BorderImageRepeat; 2]> {
    let keyword = |word: &str| -> EngineResult<BorderImageRepeat> {
        Ok(match word.to_ascii_lowercase().as_str() {
            "stretch" => BorderImageRepeat::Stretch,
            "repeat" => BorderImageRepeat::Repeat,
            "round" => BorderImageRepeat::Round,
            "space" => BorderImageRepeat::Space,
            _ => return Err(unsupported()),
        })
    };
    match tokens(value).as_slice() {
        [both] => Ok([keyword(both)?; 2]),
        [x, y] => Ok([keyword(x)?, keyword(y)?]),
        _ => Err(unsupported()),
    }
}
/** Copies one longhand for a CSS-wide keyword; false for every other property. */
pub(super) fn copy(style: &mut ComputedStyle, source: &ComputedStyle, key: &str) -> bool {
    let (to, from) = (&mut style.border_image, &source.border_image);
    match key {
        "border-image-source" => to.source = from.source.clone(),
        "border-image-slice" => (to.slice, to.fill) = (from.slice, from.fill),
        "border-image-width" => to.width = from.width,
        "border-image-outset" => to.outset = from.outset,
        "border-image-repeat" => to.repeat = from.repeat,
        _ => return false,
    }
    true
}

/**
 * `border-image: <source> || <slice> [ / <width>? [ / <outset> ]? ]? || <repeat>` as its five
 * longhands, each part it leaves out reset to its initial value. None when it does not parse.
 */
pub(super) fn shorthand(raw: &str) -> Option<Vec<(String, String)>> {
    let mut parts: [Option<String>; 5] = Default::default();
    let mut geometry = String::new();
    let mut repeats = Vec::new();
    for word in tokens(raw) {
        let lower = word.to_ascii_lowercase();
        if lower == "none" || (lower.contains('(') && !geometry_word(&lower)) {
            once(&mut parts[0], word)?;
        } else if ["stretch", "repeat", "round", "space"].contains(&lower.as_str()) {
            repeats.push(word);
        } else {
            geometry.push_str(word);
            geometry.push(' ');
        }
    }
    if !repeats.is_empty() {
        once(&mut parts[4], &repeats.join(" "))?;
    }
    let pieces = split_slashes(&geometry);
    if pieces.len() > 3
        || pieces.first().is_some_and(|slice| slice.trim().is_empty()) && pieces.len() > 1
    {
        return None;
    }
    for (index, piece) in pieces.iter().enumerate() {
        if !piece.trim().is_empty() {
            parts[index + 1] = Some(piece.trim().to_owned());
        }
    }
    Some(
        LONGHANDS
            .iter()
            .zip(parts)
            .zip(INITIAL)
            .map(|((name, part), initial)| {
                (name.to_string(), part.unwrap_or_else(|| initial.into()))
            })
            .collect(),
    )
}
/** Math functions that stand for a number or length inside slice, width or outset. */
fn geometry_word(lower: &str) -> bool {
    ["calc(", "min(", "max(", "clamp("]
        .iter()
        .any(|f| lower.starts_with(f))
}
/** Sets a shorthand part once; a second one makes the value invalid. */
fn once(slot: &mut Option<String>, word: &str) -> Option<()> {
    if slot.is_some() {
        return None;
    }
    *slot = Some(word.to_owned());
    Some(())
}
/** Splits on top-level slashes, keeping empty pieces so `10 / / 2` leaves the width out. */
fn split_slashes(text: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut start = 0;
    while start <= text.len() {
        let end = find_top(text, start, &[SLASH]);
        pieces.push(&text[start..end.min(text.len())]);
        start = end + 1;
    }
    if pieces.len() == 1 && pieces[0].trim().is_empty() {
        pieces.clear();
    }
    pieces
}
