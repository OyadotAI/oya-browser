/**
 * Upright glyphs in vertical text, once every item is in physical coordinates. Layout measured
 * an upright character one em down its line (`platform::upright`); painters turn a vertical
 * item's runs a quarter, so its upright characters move to a text-only twin item that paints
 * unturned: each centred across the column and stacked one em apart down it, where layout put
 * them. The turned stretches stay on the item at their offsets down the line.
 */
use super::DisplayItem;
use crate::document::{PaintLine, WRITING_CCW};
use crate::platform::{Face, measure, upright};

/** Splits every vertical item's runs into turned stretches and an upright twin, which follows
its item so it paints in the same place in paint order. */
pub(super) fn split(items: &mut Vec<DisplayItem>) {
    let mut at = 0;
    while at < items.len() {
        if let Some(twin) = split_item(&mut items[at]) {
            items.insert(at + 1, twin);
            at += 1;
        }
        at += 1;
    }
}

/** One item's split: its runs keep their turned stretches and the upright characters come back
as a twin; None when nothing on it stands upright. */
fn split_item(item: &mut DisplayItem) -> Option<DisplayItem> {
    let turned = item.writing != 0 && item.writing != WRITING_CCW;
    if item.upright == upright::TURNED || !turned {
        return None;
    }
    let face = flat_face(item);
    let column = item.line_height.unwrap_or(item.font_size);
    let mut kept = Vec::new();
    let mut standing = Vec::new();
    for line in std::mem::take(&mut item.lines) {
        let mut y = line.y;
        for (stands, part) in upright::segments(&line.text, item.upright) {
            let width = measure(part, face);
            let x = if stands {
                line.x + (column - width) / 2.0
            } else {
                line.x
            };
            let piece = PaintLine {
                color: None,
                text: part.to_string(),
                x,
                y,
                width,
            };
            y += if stands {
                item.font_size + face.spacing
            } else {
                width
            };
            if stands {
                standing.push(piece)
            } else {
                kept.push(piece)
            }
        }
    }
    item.lines = kept;
    (!standing.is_empty()).then(|| twin(item, standing))
}

/** The face the item's runs were measured in, with every glyph flat: an upright character's
own width across the column, and a turned stretch's length down it. */
fn flat_face(item: &DisplayItem) -> Face {
    Face {
        size: item.font_size,
        weight: item.font_weight,
        family: item.font_family,
        spacing: item.letter_spacing,
        italic: item.italic,
        autospace: item.autospace,
        upright: upright::TURNED,
    }
}

/** A text-only copy of `item` holding the upright characters, drawn unturned: one em per line
box, so each glyph's em box fills its stretch of the column. */
fn twin(item: &DisplayItem, lines: Vec<PaintLine>) -> DisplayItem {
    let mut twin = item.clone();
    twin.writing = 0;
    twin.upright = upright::TURNED;
    twin.lines = lines;
    twin.line_height = Some(item.font_size);
    twin.validation.clear();
    twin.background = None;
    twin.backgrounds.clear();
    twin.border = 0.0;
    twin.border_widths = [0.0; 4];
    twin.shadows.clear();
    twin.mask = None;
    twin.border_image = None;
    twin.vector.clear();
    twin.underline = false;
    twin.strikethrough = false;
    twin
}
