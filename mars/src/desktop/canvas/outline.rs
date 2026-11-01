/** Outlines: the ring a box paints just outside its border edge, in the style, width, offset and color it carries. */
use super::{
    faded, literal_color,
    paint::{Painter, Shape},
    pattern::{dash_pattern, stroke_width},
};
use crate::document::{BorderStyle, DisplayItem, DisplayOutline};
use objc2_app_kit::NSColor;

/// A dashed outline's dash and gap, in ring widths.
const DASH_PATTERN: [f64; 2] = [3.0, 3.0];
/// A dotted outline's dot and gap, in ring widths.
const DOT_PATTERN: [f64; 2] = [1.0, 1.0];
/// A double outline is three equal parts: a ring, a gap and a ring.
const DOUBLE_PARTS: f64 = 3.0;

/**
 * Paints the item's outline ring. It sits outside the border box, so it never changes layout and
 * is painted after the border, on top of the box's own edge.
 */
pub(super) fn paint(item: &DisplayItem, painter: &Painter, shape: &Shape, alpha: f64) {
    let Some(ring) = &item.outline else {
        return;
    };
    let color = faded(&ring_color(item, ring), alpha);
    let width = stroke_width(ring.width);
    // A dash pattern of a ring's perimeter; None (solid) when it would need too many dashes.
    let perimeter = 2.0 * (shape.width + shape.height);
    match ring.style {
        BorderStyle::Double => {
            let part = width / DOUBLE_PARTS;
            ring_at(painter, shape, ring.offset + 2.0 * part, part, &color, None);
            ring_at(painter, shape, ring.offset, part, &color, None);
        }
        BorderStyle::Dashed => {
            let dashes = dash_pattern(&DASH_PATTERN.map(|part| part * width), perimeter);
            ring_at(painter, shape, ring.offset, width, &color, dashes);
        }
        BorderStyle::Dotted => {
            let dashes = dash_pattern(&DOT_PATTERN.map(|part| part * width), perimeter);
            ring_at(painter, shape, ring.offset, width, &color, dashes);
        }
        // ponytail: groove, ridge, inset and outset paint solid; add their light and dark bands when a page needs them.
        _ => ring_at(painter, shape, ring.offset, width, &color, None),
    }
}

/** The ring's color: its own literal color, else the element's text color, else black. */
fn ring_color(item: &DisplayItem, ring: &DisplayOutline) -> objc2::rc::Retained<NSColor> {
    ring.color
        .as_deref()
        .or(item.color.as_deref())
        .and_then(literal_color)
        .unwrap_or_else(NSColor::blackColor)
}

/**
 * One ring of `width`, starting `start` points outside the border edge. `Painter::stroke` centers
 * its line half a width inside the shape it is given, so the shape grows by `start + width` to put
 * the ring's centerline at `start + width / 2`.
 */
fn ring_at(
    painter: &Painter,
    shape: &Shape,
    start: f64,
    width: f64,
    color: &NSColor,
    dashes: Option<Vec<f64>>,
) {
    let grown = shape.outset(start + width);
    match dashes {
        Some(dashes) => painter.stroke_dashed(&grown, width, color, &dashes),
        None => painter.stroke(&grown, width, color),
    }
}
