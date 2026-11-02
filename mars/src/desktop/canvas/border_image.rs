/**
 * `border-image` in the desktop window: the same Eye `nine_pieces` the screenshot raster
 * draws, each tile drawn from its source rectangle of the fetched image, or of the gradient
 * painted at the border image area's size, clipped to its piece.
 */
use super::{DisplayItem, paint, rect};
use crate::document::{DisplayBackground, NinePiece, nine_pieces};
use objc2_app_kit::{NSBezierPath, NSCompositingOperation, NSGraphicsContext, NSImage};
use objc2_core_graphics::CGContext;

/// Tiles one item's border image draws at most; hairline tiles over a huge area stop here.
const TILES: usize = 4096;

/**
 * Paints the item's border image in place of its border; false when it has none or its source
 * cannot paint (no usable fetched image), so the ordinary border paints, as Blink falls back.
 */
pub(super) fn paint(
    item: &DisplayItem,
    painter: &paint::Painter,
    image: Option<&NSImage>,
    alpha: f64,
) -> bool {
    let Some(border) = item.border_image.as_ref() else {
        return false;
    };
    let bounds = [item.x, item.y, item.width, item.height];
    let [top, right, bottom, left] = border.outset;
    let area = [item.width + left + right, item.height + top + bottom];
    if border.source.gradient().is_some() {
        let source = gradient_item(item, &border.source, area);
        let pieces = nine_pieces(bounds, border, area);
        draw_pieces(&pieces, |tile, from| {
            gradient_tile(painter, &source, tile, from, alpha)
        });
        return true;
    }
    let Some(image) = image.filter(|image| image.size().width > 0.0 && image.size().height > 0.0)
    else {
        return false;
    };
    let natural = [image.size().width, image.size().height];
    let pieces = nine_pieces(bounds, border, natural);
    draw_pieces(&pieces, |tile, from| {
        image_tile(image, tile, from, natural[1], alpha)
    });
    true
}
/** A stand-in item the size of the area, so the painter's gradient code draws the source. */
fn gradient_item(item: &DisplayItem, source: &DisplayBackground, [w, h]: [f64; 2]) -> DisplayItem {
    let mut stand_in = item.clone();
    (stand_in.x, stand_in.y, stand_in.width, stand_in.height) = (0.0, 0.0, w, h);
    stand_in.radius = [0.0; 4];
    stand_in.radius_y = None;
    stand_in.backgrounds = vec![source.clone()];
    stand_in
}
/** Each piece's tiles, clipped to the piece, handed to `draw` with the source rectangle. */
fn draw_pieces(pieces: &[NinePiece], mut draw: impl FnMut([f64; 4], [f64; 4])) {
    let Some(context) = NSGraphicsContext::currentContext() else {
        return;
    };
    for piece in pieces {
        let [x, y, w, h] = piece.dest;
        context.saveGraphicsState();
        NSBezierPath::clipRect(rect(x, y, w, h));
        for tile in tiles(piece) {
            draw(tile, piece.source);
        }
        context.restoreGraphicsState();
    }
}
/** The tile rectangles covering a piece: its phase back to before its start, then every
 * period (tile plus gap) to its end, within TILES. */
fn tiles(piece: &NinePiece) -> Vec<[f64; 4]> {
    let starts = |axis: usize| -> Vec<f64> {
        let (start, extent) = (piece.dest[axis], piece.dest[axis + 2]);
        let period = piece.tile[axis] + piece.gap[axis];
        let first = start + piece.phase[axis] - (piece.phase[axis] / period).ceil() * period;
        let count = (((start + extent - first) / period).ceil().max(0.0) as usize).min(TILES);
        (0..count).map(|i| first + i as f64 * period).collect()
    };
    let (xs, ys) = (starts(0), starts(1));
    if xs.len().saturating_mul(ys.len()) > TILES {
        return Vec::new();
    }
    ys.iter()
        .flat_map(|y| {
            xs.iter()
                .map(move |x| [*x, *y, piece.tile[0], piece.tile[1]])
        })
        .collect()
}
/** One tile of the fetched image: `from` (image pixels, top-left origin) drawn into `tile`. */
fn image_tile(image: &NSImage, tile: [f64; 4], from: [f64; 4], height: f64, alpha: f64) {
    let [sx, sy, sw, sh] = from;
    // NSImage source rectangles start at the bottom; the view is flipped, so respectFlipped.
    let source = rect(sx, height - sy - sh, sw, sh);
    unsafe {
        image.drawInRect_fromRect_operation_fraction_respectFlipped_hints(
            rect(tile[0], tile[1], tile[2], tile[3]),
            source,
            NSCompositingOperation::SourceOver,
            alpha,
            true,
            None,
        );
    }
}
/** One tile of the gradient: the area-sized gradient mapped so `from` lands on `tile`. */
fn gradient_tile(
    painter: &paint::Painter,
    source: &DisplayItem,
    tile: [f64; 4],
    from: [f64; 4],
    alpha: f64,
) {
    let Some(context) = NSGraphicsContext::currentContext() else {
        return;
    };
    let cg = context.CGContext();
    context.saveGraphicsState();
    NSBezierPath::clipRect(rect(tile[0], tile[1], tile[2], tile[3]));
    CGContext::translate_ctm(Some(&cg), tile[0], tile[1]);
    CGContext::scale_ctm(Some(&cg), tile[2] / from[2], tile[3] / from[3]);
    CGContext::translate_ctm(Some(&cg), -from[0], -from[1]);
    painter.backgrounds(&paint::Shape::of(source), source, alpha);
    context.restoreGraphicsState();
}
