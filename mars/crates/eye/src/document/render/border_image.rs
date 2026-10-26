/**
 * A box's computed `border-image` resolved for painters: outsets and widths in px against the
 * box's final size and border widths, and a gradient source sized to the border image area.
 * The image-dependent rest (slices against the image, `auto` widths) stays for `nine_pieces`.
 */
use super::{DisplayBackground, DisplayItem};
use crate::document::{BorderImageLength, BorderImageRepeat, ComputedStyle, Node};
use serde::{Deserialize, Serialize};

/// A percentage's whole.
const PERCENT: f64 = 100.0;

/// What a painter needs to draw a box's border image in place of its border.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DisplayBorderImage {
    /// How far the border image area reaches past the border box: top, right, bottom, left.
    pub outset: [f64; 4],
    /// The source: a gradient with geometry relative to the area's top-left corner, or an
    /// Image layer whose bytes the app fetched under the item's identity.
    pub source: DisplayBackground,
    /// Slices per side as (image px, percent of the image's size), top, right, bottom, left.
    pub slice: [[f64; 2]; 4],
    /// `fill`: the middle slice paints too.
    pub fill: bool,
    /// Border image widths in px per side; None is `auto`, the slice's own size.
    pub widths: [Option<f64>; 4],
    /// Tiling rule along the horizontal edges, then the vertical ones.
    pub repeat: [BorderImageRepeat; 2],
}

/**
 * Sets the item's border image from its style, the element's or (`generated`) its pseudo's. A
 * `url()` source paints only from the element's own fetched bytes, so a pseudo-element or an
 * element whose identity holds another image keeps its ordinary border, as before the image
 * loads in Blink.
 */
pub(super) fn decorate(item: &mut DisplayItem, node: &Node, generated: Option<&ComputedStyle>) {
    let style = generated.unwrap_or(&node.style);
    let image = &style.border_image;
    let Some(layer) = image.source.as_ref() else {
        return;
    };
    let fetched = generated.is_none()
        && node.tag != "img"
        && crate::document::style::fetched_border_image(style);
    if matches!(layer, crate::document::BackgroundLayer::Image { .. }) && !fetched {
        return;
    }
    // The painted widths: a `none` or `hidden` side is zero wide whatever its authored width.
    let borders = if item.border > 0.0 {
        [item.border; 4]
    } else {
        item.border_widths
    };
    let outset: [f64; 4] =
        std::array::from_fn(|side| resolve(image.outset[side], borders[side], 0.0).unwrap_or(0.0));
    let area = [
        item.width + outset[1] + outset[3],
        item.height + outset[0] + outset[2],
    ];
    item.border_image = Some(DisplayBorderImage {
        outset,
        source: super::effects::background(layer, style, area[0], area[1]),
        slice: image.slice.map(|extent| [extent.px, extent.percent]),
        fill: image.fill,
        widths: std::array::from_fn(|side| {
            resolve(image.width[side], borders[side], area[1 - side % 2])
        }),
        repeat: image.repeat,
    });
}
/** One side in px: a number multiplies the border width, a percentage takes `basis` (the
 * area's height for top and bottom, its width for left and right); `auto` stays None. */
fn resolve(length: BorderImageLength, border: f64, basis: f64) -> Option<f64> {
    match length {
        BorderImageLength::Number(multiple) => Some(multiple * border),
        BorderImageLength::Length(extent) => {
            Some((extent.px + extent.percent / PERCENT * basis).max(0.0))
        }
        BorderImageLength::Auto => None,
    }
}
