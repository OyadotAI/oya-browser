/**
 * The canvas's background layers: the root's background (or the body's, when the root has
 * none) propagates to the whole canvas (CSS Backgrounds 3 section 2.11.2). Its layers
 * are sized and positioned against the root element's box and tile across the canvas; the
 * propagated color is `DisplayList::canvas_background`.
 */
use super::{DisplayBackground, DisplayItem, GradientTile};
use crate::document::{
    ComputedStyle, Document,
    style::{BackgroundLayer, BackgroundRepeat, RepeatStyle},
};

/// The stand-in's tag: it is no element, only the canvas's layers to paint.
const CANVAS: &str = "::canvas";

/**
 * A stand-in box over the whole document, `height` tall in a `viewport`, whose layers are the
 * propagated layers (gradient tiles and image positioning areas in document coordinates, images
 * under the canvas's keys); painters draw it under every item. None when nothing propagates.
 */
pub(super) fn canvas_layers(
    document: &Document,
    viewport: (f64, f64),
    height: f64,
) -> Option<DisplayItem> {
    let style = crate::document::style::layer_urls::propagating(document)?;
    // ponytail: a vertical root's canvas positioning is the writing-modes work's.
    if document.formatting.html.style.vertical_writing {
        return None;
    }
    let root = &document.formatting.html.style;
    let placed = super::root_box::root_box(document, viewport);
    let auto = height - placed.y - root.margin[2];
    // The root's padding box, inside its border, is the propagated background's area.
    let [top, right, bottom, left] = super::sizing::border_sides(root);
    let area = [
        placed.x + left,
        placed.y + top,
        (placed.width - left - right).max(0.0),
        (placed.height.unwrap_or(auto) - top - bottom).max(0.0),
    ];
    let mut layers: Vec<DisplayBackground> = style
        .backgrounds
        .iter()
        .enumerate()
        .map(|(index, layer)| {
            // A `fixed` first layer is positioned against the viewport (render/fixed.rs).
            let fixed = index == 0 && style.background_fixed;
            let area = if fixed {
                [0.0, 0.0, viewport.0, viewport.1]
            } else {
                area
            };
            anchored(index, layer, style, area)
        })
        .collect();
    // Image layers paint the bytes the app fetched under the canvas's keys.
    super::effects::key_layers(&mut layers, |layer| {
        Some(crate::document::canvas_key(layer))
    });
    if layers.is_empty() {
        return None;
    }
    let mut item = stand_in(viewport.0, height.max(viewport.1))?;
    item.backgrounds = layers;
    Some(item)
}

/** Gradient layer `index` laid out in the root's box `area`, its tile moved into document
 * coordinates; a layer filling the whole box tiles at the box's size. */
fn anchored(
    index: usize,
    layer: &BackgroundLayer,
    style: &ComputedStyle,
    [x, y, width, height]: [f64; 4],
) -> DisplayBackground {
    let mut laid = super::effects::tiled((index, layer), style, width, height);
    if let DisplayBackground::Image { area, .. } = &mut laid {
        // An image is positioned in the root's box (or the viewport), in document points.
        *area = Some([x, y, width, height]);
        return laid;
    }
    let repeat = crate::document::style::background_layers::geometry(style, index).2;
    let mut tile = laid.tile().unwrap_or(GradientTile {
        x: 0.0,
        y: 0.0,
        width,
        height,
        repeat: axes(repeat),
        gap: [0.0; 2],
    });
    (tile.x, tile.y) = (tile.x + x, tile.y + y);
    laid.with_tile(tile)
}

/** Whether each axis of `repeat` tiles. */
fn axes(repeat: BackgroundRepeat) -> [bool; 2] {
    match repeat {
        BackgroundRepeat::Repeat => [true; 2],
        BackgroundRepeat::NoRepeat => [false; 2],
        BackgroundRepeat::RepeatX => [true, false],
        BackgroundRepeat::RepeatY => [false, true],
        BackgroundRepeat::Axes(x, y) => [x != RepeatStyle::NoRepeat, y != RepeatStyle::NoRepeat],
    }
}

/** An empty box `width` by `height` at the document's origin, with no element identity. */
fn stand_in(width: f64, height: f64) -> Option<DisplayItem> {
    serde_json::from_value(serde_json::json!({
        "local_id": usize::MAX, "tag": CANVAS, "role": "presentation", "x": 0.0, "y": 0.0,
        "width": width, "height": height, "font_size": 0.0, "bold": false, "lines": [],
    }))
    .ok()
}
