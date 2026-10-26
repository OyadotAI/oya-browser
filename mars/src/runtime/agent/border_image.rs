/**
 * `border-image` in the screenshot raster: Eye's `nine_pieces` cut the source (the fetched
 * bitmap or drawing, or a gradient sized to the border image area) and each piece's pixels
 * sample it nearest-neighbor, tiled as the piece says. The desktop canvas draws the same pieces.
 */
use super::{Canvas, Picture, gradient_offset, painter_rgba, stop_color};
use crate::document::{DisplayBackground, DisplayItem, NinePiece, nine_pieces};

/** Where a border image's pixels come from, in image pixels. */
type Sampler<'a> = &'a dyn Fn(f64, f64) -> [u8; 4];

impl Canvas {
    /**
     * Paints the item's border image in place of its border; false when it has none or its
     * source cannot paint (no fetched bytes, an undecodable image), so the border paints instead,
     * as Blink's `NinePieceImagePainter::Paint` falls back.
     */
    pub(super) fn border_image(
        &mut self,
        item: &DisplayItem,
        picture: Option<&Picture>,
        fade: &dyn Fn([u8; 4]) -> [u8; 4],
    ) -> bool {
        let Some(image) = item.border_image.as_ref() else {
            return false;
        };
        let [top, right, bottom, left] = image.outset;
        let area = [item.width + left + right, item.height + top + bottom];
        if image.source.gradient().is_some() {
            self.gradient_border(item, area, fade);
            return true;
        }
        let DisplayBackground::Image { .. } = image.source else {
            return false;
        };
        self.bitmap_border(item, picture, area, fade)
    }
    /** A gradient source: its color at each image point of an area-sized image. */
    fn gradient_border(
        &mut self,
        item: &DisplayItem,
        area: [f64; 2],
        fade: &dyn Fn([u8; 4]) -> [u8; 4],
    ) {
        let Some(image) = item.border_image.as_ref() else {
            return;
        };
        let Some((radial, geometry, stops)) = image.source.gradient() else {
            return;
        };
        let stops: Vec<(f64, [u8; 4])> = stops
            .iter()
            .filter_map(|stop| Some((stop.position, fade(painter_rgba(&stop.color)?))))
            .collect();
        if stops.is_empty() {
            return;
        }
        let sample = |x: f64, y: f64| stop_color(&stops, gradient_offset(radial, geometry, x, y));
        let bounds = [item.x, item.y, item.width, item.height];
        self.pieces(item, &nine_pieces(bounds, image, area), &sample);
    }
    /** A fetched source: the bitmap, or the drawing rasterized at its natural size (the
     * area's where it has none, as Blink sizes a generated image to the default object size);
     * false when there is nothing to draw. */
    fn bitmap_border(
        &mut self,
        item: &DisplayItem,
        picture: Option<&Picture>,
        area: [f64; 2],
        fade: &dyn Fn([u8; 4]) -> [u8; 4],
    ) -> bool {
        let Some(image) = item.border_image.as_ref() else {
            return false;
        };
        let drawn;
        let bitmap = match picture {
            Some(Picture::Raster(bitmap)) => bitmap,
            Some(vector @ Picture::Vector(markup)) => {
                let (width, height, _) = self.natural(vector);
                let size = [width.unwrap_or(area[0]), height.unwrap_or(area[1])];
                let Some(bitmap) = self.drawing(markup, size[0], size[1]) else {
                    return false;
                };
                drawn = bitmap;
                &*drawn
            }
            None => return false,
        };
        let natural = [f64::from(bitmap.width), f64::from(bitmap.height)];
        let sample = |x: f64, y: f64| fade(bitmap.sample(x / natural[0], y / natural[1]));
        let bounds = [item.x, item.y, item.width, item.height];
        self.pieces(item, &nine_pieces(bounds, image, natural), &sample);
        true
    }
    /** Each piece's destination pixels, sampled from its source rectangle tile by tile. */
    fn pieces(&mut self, item: &DisplayItem, pieces: &[NinePiece], sample: Sampler) {
        for piece in pieces {
            self.shade(piece.dest, item.clip.as_ref(), |x, y| {
                let u = along(piece, 0, x)?;
                let v = along(piece, 1, y)?;
                let [sx, sy, sw, sh] = piece.source;
                Some(sample(sx + u * sw, sy + v * sh))
            });
        }
    }
}
/** Where a pixel center at `at` falls in its tile on `axis`, 0..1; None in a `space` gap. */
fn along(piece: &NinePiece, axis: usize, at: f64) -> Option<f64> {
    let (tile, gap) = (piece.tile[axis], piece.gap[axis]);
    let offset = (at - piece.dest[axis] - piece.phase[axis]).rem_euclid(tile + gap);
    (offset < tile).then(|| offset / tile)
}
