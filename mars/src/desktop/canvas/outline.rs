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
    match ring.style {
        BorderStyle::Double => {
            let part = width / DOUBLE_PARTS;
            ring_at(
                painter,
                shape,
                (ring.offset + 2.0 * part, part),
                &color,
                None,
            );
            ring_at(painter, shape, (ring.offset, part), &color, None);
        }
        BorderStyle::Dashed => {
            ring_at(
                painter,
                shape,
                (ring.offset, width),
                &color,
                Some(DASH_PATTERN),
            );
        }
        BorderStyle::Dotted => {
            ring_at(
                painter,
                shape,
                (ring.offset, width),
                &color,
                Some(DOT_PATTERN),
            );
        }
        // ponytail: groove, ridge, inset and outset paint solid; add their light and dark bands when a page needs them.
        _ => ring_at(painter, shape, (ring.offset, width), &color, None),
    }
}

/**
 * The dash pattern of the ring `(start, width)` around `shape`, planned against the path it
 * actually strokes: the box grown by `start + width`. None paints the ring solid.
 */
pub(super) fn ring_dashes(
    shape: (f64, f64),
    (start, width): (f64, f64),
    unit: [f64; 2],
) -> Option<Vec<f64>> {
    let (grown_width, grown_height) = (
        shape.0 + 2.0 * (start + width),
        shape.1 + 2.0 * (start + width),
    );
    let perimeter = 2.0 * (grown_width + grown_height);
    dash_pattern(&unit.map(|part| part * width), perimeter)
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
 * the ring's centerline at `start + width / 2`. Dashes are planned on that grown path.
 */
fn ring_at(
    painter: &Painter,
    shape: &Shape,
    (start, width): (f64, f64),
    color: &NSColor,
    unit: Option<[f64; 2]>,
) {
    let grown = shape.outset(start + width);
    let dashes =
        unit.and_then(|unit| ring_dashes((shape.width, shape.height), (start, width), unit));
    match dashes {
        Some(dashes) => painter.stroke_dashed(&grown, width, color, &dashes),
        None => painter.stroke(&grown, width, color),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop::canvas::pattern::MAX_SEGMENTS;

    /**
     * A huge outline offset on a dotted ring still plans a bounded dash count: the grown path is
     * what the dashes are planned against, so the offset cannot inflate the count past the cap.
     */
    #[test]
    fn huge_offset_dotted_ring_stays_bounded() {
        let offset = crate::document::MAX_OUTLINE_OFFSET;
        let width = stroke_width(1e-9);
        let (w, h) = (100.0_f64, 50.0_f64);
        let grown = 2.0 * ((w + 2.0 * (offset + width)) + (h + 2.0 * (offset + width)));
        let count = match ring_dashes((w, h), (offset, width), DOT_PATTERN) {
            Some(dashes) => {
                let period: f64 = dashes.iter().sum();
                grown / period * dashes.len() as f64
            }
            None => 0.0,
        };
        assert!(count <= MAX_SEGMENTS as f64, "{count}");
    }
}
