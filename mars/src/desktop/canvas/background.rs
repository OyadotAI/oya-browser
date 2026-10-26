/** Background image layers: the bytes the app fetched for an item, sized, positioned and tiled in its border box. */
use super::{DisplayItem, rect};
use crate::document::{DisplayBackground, ImageSize, ObjectFit};
use objc2::rc::Retained;
use objc2_app_kit::{
    NSBezierPath, NSColor, NSCompositingOperation, NSGraphicsContext, NSImage,
    NSRectFillUsingOperation,
};
use objc2_core_graphics::CGContext;
use objc2_foundation::{NSRect, NSSize};

/// Tiles one layer draws at most; a tiny tile over a huge box stops here instead of stalling paint.
const TILES: usize = 4096;

/// Draws a replaced `<img>` element inside its content box with `object-fit`, `object-position`,
/// and rounded-corner clipping.
pub(super) fn paint_replaced_image(
    item: &DisplayItem,
    shape: &super::paint::Shape,
    painter: &super::paint::Painter,
    image: &NSImage,
    alpha: f64,
) {
    if !usable(image) {
        return;
    }
    let Some(context) = NSGraphicsContext::currentContext() else {
        return;
    };
    let [cx, cy, cw, ch] = item
        .content_box
        .unwrap_or([item.x, item.y, item.width, item.height]);
    if cw <= 0.0 || ch <= 0.0 {
        return;
    }
    let insets = [
        (cy - item.y).max(0.0),
        ((item.x + item.width) - (cx + cw)).max(0.0),
        ((item.y + item.height) - (cy + ch)).max(0.0),
        (cx - item.x).max(0.0),
    ];
    let inner = shape.inset_sides(insets);
    let natural = image.size();
    let (iw, ih) = (natural.width, natural.height);
    let (dw, dh) = match item.object_fit {
        ObjectFit::Fill => (cw, ch),
        ObjectFit::Contain => {
            let scale = (cw / iw).min(ch / ih);
            (iw * scale, ih * scale)
        }
        ObjectFit::Cover => {
            let scale = (cw / iw).max(ch / ih);
            (iw * scale, ih * scale)
        }
        ObjectFit::None => (iw, ih),
        ObjectFit::ScaleDown => {
            let scale = (cw / iw).min(ch / ih).min(1.0);
            (iw * scale, ih * scale)
        }
    };
    let [ox_px, ox_pct, oy_px, oy_pct] = item.object_position;
    let dx = cx + ox_px + (cw - dw) * ox_pct / 100.0;
    let dy = cy + oy_px + (ch - dh) * oy_pct / 100.0;
    context.saveGraphicsState();
    // `overflow: visible` lets the whole object rectangle paint past the content box.
    if !item.overflow_visible {
        painter.clip_shape(&inner);
    }
    // The view is flipped; without respectFlipped AppKit draws bitmaps upside down.
    unsafe {
        image.drawInRect_fromRect_operation_fraction_respectFlipped_hints(
            rect(dx, dy, dw, dh),
            NSRect::ZERO,
            NSCompositingOperation::SourceOver,
            alpha,
            true,
            None,
        );
    }
    context.restoreGraphicsState();
}

/// Draws the item's image layers, bottom first, clipped to its border box: the first with the
/// item's own image, each layer keyed apart (later layers, the canvas's) with its image in
/// `keyed`.
pub(super) fn paint(
    item: &DisplayItem,
    image: Option<&NSImage>,
    keyed: &std::collections::HashMap<usize, Retained<NSImage>>,
    alpha: f64,
) {
    let Some(context) = NSGraphicsContext::currentContext() else {
        return;
    };
    context.saveGraphicsState();
    NSBezierPath::clipRect(rect(item.x, item.y, item.width, item.height));
    for layer in item.backgrounds.iter().rev() {
        let picture = match layer {
            DisplayBackground::Image { key: Some(key), .. } => keyed.get(key).map(|i| &**i),
            DisplayBackground::Image { .. } => image,
            _ => None,
        };
        if let Some(picture) = picture.filter(|picture| usable(picture)) {
            draw_layer(picture, layer, item, alpha);
        }
    }
    context.restoreGraphicsState();
}
/// The images of an item's layers keyed apart from its identity, decoded once each.
pub(super) fn keyed_images<'a>(
    item: &DisplayItem,
    bytes: &dyn Fn(usize) -> Option<&'a [u8]>,
    painted: &mut super::painted::Painted,
) -> std::collections::HashMap<usize, Retained<NSImage>> {
    item.backgrounds
        .iter()
        .filter_map(|layer| match layer {
            DisplayBackground::Image { key: Some(key), .. } => Some(*key),
            _ => None,
        })
        .filter_map(|key| painted.image(key, bytes(key)).map(|image| (key, image)))
        .collect()
}
/// Fills the item's border box with `color` only where its `url()` mask image is opaque: the
/// mask tiles draw into a transparency layer and the color replaces them with source-in. Without
/// fetched mask bytes nothing paints, as Chromium shows nothing until the mask loads.
pub(super) fn paint_mask(item: &DisplayItem, image: Option<&NSImage>, color: &NSColor) {
    let (Some(layer), Some(image)) = (item.mask.as_ref(), image.filter(|image| usable(image)))
    else {
        return;
    };
    let Some(context) = NSGraphicsContext::currentContext() else {
        return;
    };
    let bounds = rect(item.x, item.y, item.width, item.height);
    let cg = context.CGContext();
    context.saveGraphicsState();
    NSBezierPath::clipRect(bounds);
    // SAFETY: a null auxiliary dictionary is documented; the layer ends below in the same state.
    unsafe { CGContext::begin_transparency_layer(Some(&cg), None) };
    draw_layer(image, layer, item, 1.0);
    color.setFill();
    NSRectFillUsingOperation(bounds, NSCompositingOperation::SourceIn);
    CGContext::end_transparency_layer(Some(&cg));
    context.restoreGraphicsState();
}
/** Only an image with a natural size can be sized and tiled. */
fn usable(image: &NSImage) -> bool {
    let natural = image.size();
    natural.width > 0.0 && natural.height > 0.0
}
/** Sizes, positions and tiles one image layer in the item's box. */
fn draw_layer(image: &NSImage, layer: &DisplayBackground, item: &DisplayItem, alpha: f64) {
    let DisplayBackground::Image {
        size,
        position,
        repeat,
        area,
        ..
    } = layer
    else {
        return;
    };
    // A fixed layer is positioned in the viewport Eye gave it, then clipped to the box.
    let [ax, ay, aw, ah] = area.unwrap_or([item.x, item.y, item.width, item.height]);
    let natural = image.size();
    let tile = tile_size(*size, (natural.width, natural.height), (aw, ah));
    if tile.0 > 0.0 && tile.1 > 0.0 {
        let origin = (
            ax + position[0] + (aw - tile.0) * position[1] / 100.0,
            ay + position[2] + (ah - tile.1) * position[3] / 100.0,
        );
        draw_tiles(image, origin, tile, *repeat, item, alpha);
    }
}
/** The drawn size of one tile from the size rule and the image's natural size. */
fn tile_size(size: ImageSize, (iw, ih): (f64, f64), (bw, bh): (f64, f64)) -> (f64, f64) {
    match size {
        ImageSize::Auto => (iw, ih),
        ImageSize::Cover => {
            let scale = (bw / iw).max(bh / ih);
            (iw * scale, ih * scale)
        }
        ImageSize::Contain => {
            let scale = (bw / iw).min(bh / ih);
            (iw * scale, ih * scale)
        }
        ImageSize::Fixed { width, height } => match (width, height) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => (w, ih * w / iw),
            (None, Some(h)) => (iw * h / ih, h),
            (None, None) => (iw, ih),
        },
    }
}
/** Repeats the tile from `origin` across the box on the tiled axes. */
fn draw_tiles(
    image: &NSImage,
    origin: (f64, f64),
    (tw, th): (f64, f64),
    [repeat_x, repeat_y]: [bool; 2],
    item: &DisplayItem,
    alpha: f64,
) {
    let starts = |origin: f64, tile: f64, low: f64, high: f64, repeat: bool| -> Vec<f64> {
        if !repeat {
            return vec![origin];
        }
        let first = origin - ((origin - low) / tile).ceil() * tile;
        let count = (((high - first) / tile).ceil().max(0.0) as usize).min(TILES);
        (0..count).map(|i| first + i as f64 * tile).collect()
    };
    let xs = starts(origin.0, tw, item.x, item.x + item.width, repeat_x);
    let ys = starts(origin.1, th, item.y, item.y + item.height, repeat_y);
    if xs.len().saturating_mul(ys.len()) > TILES {
        return;
    }
    for y in &ys {
        for x in &xs {
            // The view is flipped; without respectFlipped AppKit draws bitmaps upside down.
            unsafe {
                image.drawInRect_fromRect_operation_fraction_respectFlipped_hints(
                    NSRect::new(objc2_foundation::NSPoint::new(*x, *y), NSSize::new(tw, th)),
                    NSRect::ZERO,
                    objc2_app_kit::NSCompositingOperation::SourceOver,
                    alpha,
                    true,
                    None,
                );
            }
        }
    }
}
