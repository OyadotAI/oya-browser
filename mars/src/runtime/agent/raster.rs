/**
 * page.screenshot without a window: a PNG painted from the same display list the native window
 * draws, so a headless or attached agent can see layout. It paints boxes, borders, images and
 * text runs as real glyphs (CoreText on macOS, bars where no rasterizer exists); the desktop
 * window answers page.screenshot itself with real pixels.
 */
#[path = "glyphs.rs"]
mod glyphs;
#[cfg(test)]
#[path = "raster_svg_tests.rs"]
mod svg_tests;

use super::super::supervisor::Runtime;
use crate::document::{
    BorderStyle, ClipRect, DisplayBackground, DisplayItem, ImageSize, agent_visible, painter_rgba,
};
use crate::protocol::{EngineError, EngineResult, parameters};
use serde::Deserialize;
use serde_json::{Value, json};
use std::rc::Rc;

/// Longest edge a screenshot may have, bounding its memory to a few megabytes.
const SHOT_EDGE: f64 = 2048.0;
/// Text bars cover the middle of each line box, roughly where glyph ink sits.
const INK_TOP: f64 = 0.25;
/// Share of the font size a text bar is tall.
const INK_HEIGHT: f64 = 0.55;
/// Images paint as a neutral placeholder of their box.
const IMAGE_FILL: [u8; 4] = [0xd0, 0xd4, 0xda, 0xff];
/// The canvas when the page names no background.
const WHITE: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
/// Text and borders when the page names no color.
const BLACK: [u8; 4] = [0, 0, 0, 0xff];

/** Optionally clip the shot to one element's box. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Shot {
    #[serde(default)]
    node_id: Option<String>,
}

/// Blur taps (mask samples plus both convolution passes) one screenshot may spend on box
/// shadows; a shadow that does not fit in what is left paints nothing.
const SHOT_BLUR_WORK: usize = 1 << 28;
/// Widest Gaussian a shadow blurs with: sigma SHOT_EDGE, a blur radius of twice the shot edge,
/// already wider than any canvas it could shade.
const MAX_SIGMA: f64 = SHOT_EDGE;
/// SVG drawing work one screenshot may spend: markup bytes parsed, tile pixels allocated,
/// flattened edges built, edge rows scanned and pixels painted. A drawing that does not fit in
/// what is left paints nothing, like a broken image.
const SHOT_DRAW_WORK: usize = 1 << 24;
/// Flattened edges (each cubic counts CUBIC_STEPS) one screenshot's drawings may fill in all;
/// past them drawings paint nothing, like a broken image.
const SHOT_DRAWN_EDGES: usize = 1 << 18;
/// Distinct tile sizes one drawing may be rasterized at per screenshot; each size parses and
/// flattens it again, so past them further sizes paint nothing.
const DRAWN_SIZES: usize = 8;
/// Work units one markup byte costs to parse: parsing a byte takes about as long as blending a
/// dozen or more pixels, so bytes are weighted to keep the budget a bound on time.
const PARSE_WEIGHT: usize = 16;

/** What SVG drawing may still spend this screenshot. */
struct DrawBudget {
    /** Work units left, as SHOT_DRAW_WORK counts them. */
    work: usize,
    /** Flattened edges left, out of SHOT_DRAWN_EDGES. */
    edges: usize,
}
impl DrawBudget {
    /** Spends `work` units; None, spending nothing, when they do not fit. */
    fn charge(&mut self, work: usize) -> Option<()> {
        self.work = self.work.checked_sub(work)?;
        Some(())
    }
}

/** SVG tiles drawn this screenshot, by markup and tile size; None when the budget refused one. */
type Drawn = std::collections::HashMap<(Rc<str>, u64, u64), Option<Rc<Bitmap>>>;

/** A window onto document coordinates, one pixel per logical point. */
struct Canvas {
    left: f64,
    top: f64,
    width: usize,
    height: usize,
    pixels: Vec<u8>,
    /** Glyph bitmap pixels this screenshot may still draw; past them runs paint as bars. */
    glyph_pixels: usize,
    /** Blur taps this screenshot may still spend; past them blurred shadows paint nothing. */
    blur_work: usize,
    /** SVG drawing work this screenshot may still spend; past it drawings paint nothing. */
    draw: DrawBudget,
    /** Distinct tile sizes each drawing has been rasterized at this screenshot. */
    sizes: std::collections::HashMap<Rc<str>, usize>,
    /** Drawings already rasterized, so a tile repeated across layers or boxes draws once. */
    drawn: Drawn,
    /** Natural sizes of drawings already parsed, by markup. */
    naturals: std::collections::HashMap<Rc<str>, Natural>,
}

impl Canvas {
    /** A blank RGB canvas over a document rectangle. */
    fn new(left: f64, top: f64, width: f64, height: f64, background: [u8; 4]) -> Self {
        let (width, height) = (
            width.clamp(1.0, SHOT_EDGE) as usize,
            height.clamp(1.0, SHOT_EDGE) as usize,
        );
        let pixels = background[..3].repeat(width * height);
        Self {
            left,
            top,
            width,
            height,
            pixels,
            glyph_pixels: glyphs::SHOT_GLYPH_PIXELS,
            blur_work: SHOT_BLUR_WORK,
            draw: DrawBudget {
                work: SHOT_DRAW_WORK,
                edges: SHOT_DRAWN_EDGES,
            },
            sizes: std::collections::HashMap::new(),
            drawn: Drawn::new(),
            naturals: std::collections::HashMap::new(),
        }
    }
    /** Blend one color over a document rectangle, clipped to the canvas and an optional clip. */
    fn fill(&mut self, rect: [f64; 4], clip: Option<[f64; 4]>, color: [u8; 4]) {
        let [mut x0, mut y0, mut x1, mut y1] =
            [rect[0], rect[1], rect[0] + rect[2], rect[1] + rect[3]];
        if let Some([cx, cy, cw, ch]) = clip {
            (x0, y0, x1, y1) = (x0.max(cx), y0.max(cy), x1.min(cx + cw), y1.min(cy + ch));
        }
        let column = |x: f64, edge: usize| ((x - self.left).round().max(0.0) as usize).min(edge);
        let row = |y: f64, edge: usize| ((y - self.top).round().max(0.0) as usize).min(edge);
        let alpha = u32::from(color[3]);
        for y in row(y0, self.height)..row(y1, self.height) {
            for x in column(x0, self.width)..column(x1, self.width) {
                let at = (y * self.width + x) * 3;
                for (pixel, over) in self.pixels[at..at + 3].iter_mut().zip(color) {
                    let (under, over) = (u32::from(*pixel), u32::from(over));
                    *pixel = ((over * alpha + under * (255 - alpha)) / 255) as u8;
                }
            }
        }
    }
    /** Blend one color over the pixel at canvas column `x`, row `y`. */
    fn blend(&mut self, x: usize, y: usize, color: [u8; 4]) {
        let at = (y * self.width + x) * 3;
        let alpha = u32::from(color[3]);
        for (pixel, over) in self.pixels[at..at + 3].iter_mut().zip(color) {
            let (under, over) = (u32::from(*pixel), u32::from(over));
            *pixel = ((over * alpha + under * (255 - alpha)) / 255) as u8;
        }
    }
    /**
     * Visit every canvas pixel whose center lies in a document rectangle and the optional clip,
     * handing the shader the center in document coordinates.
     */
    fn shade(
        &mut self,
        rect: [f64; 4],
        clip: Option<&ClipRect>,
        mut shader: impl FnMut(f64, f64) -> Option<[u8; 4]>,
    ) {
        let [mut x0, mut y0, mut x1, mut y1] =
            [rect[0], rect[1], rect[0] + rect[2], rect[1] + rect[3]];
        if let Some(c) = clip {
            (x0, y0) = (x0.max(c.x), y0.max(c.y));
            (x1, y1) = (x1.min(c.x + c.width), y1.min(c.y + c.height));
        }
        let (left, top, width, height) = (self.left, self.top, self.width, self.height);
        let column = |x: f64| ((x - left).round().max(0.0) as usize).min(width);
        let row = |y: f64| ((y - top).round().max(0.0) as usize).min(height);
        for y in row(y0)..row(y1) {
            for x in column(x0)..column(x1) {
                let (px, py) = (left + x as f64 + 0.5, top + y as f64 + 0.5);
                let clipped = clip.is_some_and(|c| {
                    let bounds = [c.x, c.y, c.width, c.height];
                    !inside_rounded(px, py, bounds, c.radius.map(|r| [r, r]))
                });
                if let Some(color) = shader(px, py).filter(|_| !clipped) {
                    self.blend(x, y, color);
                }
            }
        }
    }
    /** Paint one display item in Blink's box order: shadow, background color, background
     * layers bottom first, border, then the image placeholder and text runs. */
    fn item(&mut self, item: &DisplayItem, foreground: [u8; 4], image: Option<&Picture>) {
        let item = &snapped(item);
        let clip = item.clip;
        let fade = |mut color: [u8; 4]| {
            color[3] = (f64::from(color[3]) * item.opacity.clamp(0.0, 1.0)).round() as u8;
            color
        };
        let bounds = [item.x, item.y, item.width, item.height];
        let corners = item.radius.map(|r| [r, r]);
        for shadow in item.shadows.iter().rev().filter(|s| !s.inset) {
            let Some(color) = painter_rgba(&shadow.color).map(fade) else {
                continue;
            };
            let spread = shadow.spread;
            let rect = [
                item.x + shadow.x - spread,
                item.y + shadow.y - spread,
                item.width + 2.0 * spread,
                item.height + 2.0 * spread,
            ];
            let radii = item.radius.map(|r| [(r + spread).max(0.0); 2]);
            self.shadow(
                rect,
                clip.as_ref(),
                shadow.blur,
                &|x, y| inside_rounded(x, y, rect, radii),
                &|x, y| !inside_rounded(x, y, bounds, corners),
                color,
            );
        }
        if let Some(color) = item.background.as_deref().and_then(painter_rgba) {
            let color = fade(color);
            self.shade(bounds, clip.as_ref(), |x, y| {
                inside_rounded(x, y, bounds, corners).then_some(color)
            });
        }
        for layer in item.backgrounds.iter().rev() {
            let Some((radial, geometry, stops)) = layer.gradient() else {
                continue;
            };
            let stops: Vec<(f64, [u8; 4])> = stops
                .iter()
                .filter_map(|stop| Some((stop.position, fade(painter_rgba(&stop.color)?))))
                .collect();
            if stops.is_empty() {
                continue;
            }
            self.shade(bounds, clip.as_ref(), |x, y| {
                let t = gradient_offset(radial, geometry, x - item.x, y - item.y);
                inside_rounded(x, y, bounds, corners).then(|| stop_color(&stops, t))
            });
        }
        if let Some(picture) = image.filter(|_| item.tag != "img") {
            for layer in item.backgrounds.iter().rev() {
                self.background_image(item, layer, picture, &fade);
            }
        }
        if item.tag == "img" {
            let dest = item.content_box.unwrap_or(bounds);
            match image.and_then(Picture::raster) {
                // ponytail: object-fit and object-position paint as `fill`; add them when a
                // replaced-content suite needs them.
                Some(bitmap) => self.tile(
                    dest,
                    clip.as_ref(),
                    Tiling {
                        tile: dest,
                        repeat: [false; 2],
                        gap: [0.0; 2],
                    },
                    bitmap,
                    &fade,
                ),
                None => self.fill(
                    bounds,
                    clip.map(|c| [c.x, c.y, c.width, c.height]),
                    fade(IMAGE_FILL),
                ),
            }
        }
        self.inset_shadows(item, &fade);
        let ink = fade(
            item.color
                .as_deref()
                .and_then(painter_rgba)
                .unwrap_or(foreground),
        );
        self.border(item, ink, &fade);
        self.text(item, ink);
    }
    /**
     * One box shadow as Blink's `BoxPainterBase::PaintNormalBoxShadow` and `PaintInsetBoxShadow`
     * draw it: the `shape` mask over `rect`, blurred by a Gaussian of sigma `blur / 2` (Skia's
     * `BlurRadiusToStdDev`), painted only where `keep` holds (outside the border box for an
     * outer shadow, inside the padding box for an inset one).
     */
    fn shadow(
        &mut self,
        rect: [f64; 4],
        clip: Option<&ClipRect>,
        blur: f64,
        shape: &dyn Fn(f64, f64) -> bool,
        keep: &dyn Fn(f64, f64) -> bool,
        color: [u8; 4],
    ) {
        let sigma = (blur.max(0.0) / 2.0).min(MAX_SIGMA);
        if sigma <= 0.0 {
            self.shade(rect, clip, |x, y| {
                (shape(x, y) && keep(x, y)).then_some(color)
            });
            return;
        }
        let reach = (sigma * BLUR_REACH).ceil();
        let Some(region) = self.blur_region(rect, clip, reach) else {
            return;
        };
        let work = region[2] * region[3] * (4.0 * reach + 3.0);
        if work > self.blur_work as f64 {
            return;
        }
        self.blur_work -= work as usize;
        let coverage = blurred(region, sigma, shape);
        let columns = region[2] as usize;
        self.shade(region, clip, |x, y| {
            let (column, row) = ((x - region[0]) as usize, (y - region[1]) as usize);
            let alpha = *coverage.get(row * columns + column)?;
            if alpha <= 0.0 || !keep(x, y) {
                return None;
            }
            let mut out = color;
            out[3] = (f64::from(color[3]) * alpha.min(1.0)).round() as u8;
            Some(out)
        });
    }
    /**
     * The whole-pixel region a shadow over `rect` blurs: the rect grown by `reach`, cut to the
     * canvas and the clip grown by `reach` (plus a pixel of slack), since pixels farther out
     * cannot reach a painted one. The grid stays on the uncut region's whole pixels, so every
     * painted pixel's coverage is unchanged; None when nothing is left.
     */
    fn blur_region(&self, rect: [f64; 4], clip: Option<&ClipRect>, reach: f64) -> Option<[f64; 4]> {
        let mut seen = [
            self.left,
            self.top,
            self.left + self.width as f64,
            self.top + self.height as f64,
        ];
        if let Some(c) = clip {
            seen = [
                seen[0].max(c.x),
                seen[1].max(c.y),
                seen[2].min(c.x + c.width),
                seen[3].min(c.y + c.height),
            ];
        }
        let (x0, y0) = ((rect[0] - reach).floor(), (rect[1] - reach).floor());
        let x1 = x0 + (rect[2] + 2.0 * reach).ceil() + 1.0;
        let y1 = y0 + (rect[3] + 2.0 * reach).ceil() + 1.0;
        let (x0, y0) = (
            x0.max((seen[0] - reach).floor() - 1.0),
            y0.max((seen[1] - reach).floor() - 1.0),
        );
        let (x1, y1) = (
            x1.min((seen[2] + reach).ceil() + 1.0),
            y1.min((seen[3] + reach).ceil() + 1.0),
        );
        (x1 > x0 && y1 > y0).then_some([x0, y0, x1 - x0, y1 - y0])
    }
    /** Inset shadows inside the padding box: everything but the hole, the padding box moved by
     * the offset and shrunk by the spread with its inner radii shrunk to match. */
    fn inset_shadows(&mut self, item: &DisplayItem, fade: &dyn Fn([u8; 4]) -> [u8; 4]) {
        let [bt, br, bb, bl] = if item.border > 0.0 {
            [item.border; 4]
        } else {
            item.border_widths
        };
        let padding = [
            item.x + bl,
            item.y + bt,
            (item.width - bl - br).max(0.0),
            (item.height - bt - bb).max(0.0),
        ];
        let inner = item.radius.map(|r| [(r - bt.max(bl)).max(0.0); 2]);
        for shadow in item.shadows.iter().rev().filter(|s| s.inset) {
            let Some(color) = painter_rgba(&shadow.color).map(fade) else {
                continue;
            };
            let spread = shadow.spread;
            let hole = [
                padding[0] + shadow.x + spread,
                padding[1] + shadow.y + spread,
                (padding[2] - 2.0 * spread).max(0.0),
                (padding[3] - 2.0 * spread).max(0.0),
            ];
            let radii = inner.map(|[r, _]| [(r - spread).max(0.0); 2]);
            self.shadow(
                padding,
                item.clip.as_ref(),
                shadow.blur,
                &|x, y| !inside_rounded(x, y, hole, radii),
                &|x, y| inside_rounded(x, y, padding, inner),
                color,
            );
        }
    }
    /**
     * Text runs as real glyphs drawn by the platform's font rasterizer at their laid-out places;
     * a run nothing can draw (no rasterizer, or AppKit refused the bitmap) paints as a bar over
     * the middle of its line box, roughly where glyph ink sits.
     */
    fn text(&mut self, item: &DisplayItem, ink: [u8; 4]) {
        if item.lines.is_empty() {
            return;
        }
        let baseline = glyphs::baseline_offset(item);
        for line in &item.lines {
            let canvas = [self.left, self.top, self.width as f64, self.height as f64];
            match glyphs::rasterize(item, line, baseline, canvas, &mut self.glyph_pixels) {
                Some(coverage) => self.glyphs(&coverage, item.clip.as_ref(), ink),
                None => self.fill(
                    [
                        line.x,
                        line.y + item.font_size * INK_TOP,
                        line.width,
                        item.font_size * INK_HEIGHT,
                    ],
                    item.clip.map(|c| [c.x, c.y, c.width, c.height]),
                    ink,
                ),
            }
        }
    }
    /** Blend `ink` through a run's coverage, inside the canvas and the item's clip. */
    fn glyphs(&mut self, coverage: &glyphs::Coverage, clip: Option<&ClipRect>, ink: [u8; 4]) {
        for row in 0..coverage.height {
            let y = coverage.top + row as i64;
            if y < 0 || y as usize >= self.height {
                continue;
            }
            for column in 0..coverage.width {
                let (x, alpha) = (
                    coverage.left + column as i64,
                    coverage.alpha[row * coverage.width + column],
                );
                if alpha == 0 || x < 0 || x as usize >= self.width {
                    continue;
                }
                let (px, py) = (self.left + x as f64 + 0.5, self.top + y as f64 + 0.5);
                if clip.is_some_and(|c| {
                    !inside_rounded(
                        px,
                        py,
                        [c.x, c.y, c.width, c.height],
                        c.radius.map(|r| [r, r]),
                    )
                }) {
                    continue;
                }
                let mut color = ink;
                color[3] = ((u32::from(ink[3]) * u32::from(alpha) + 127) / 255) as u8;
                self.blend(x as usize, y as usize, color);
            }
        }
    }
    /**
     * One bitmap stretched to `tile` ([x, y, width, height]) and, per axis, repeated across the
     * painting area `area`, sampled nearest-neighbor at each pixel center.
     */
    fn tile(
        &mut self,
        area: [f64; 4],
        clip: Option<&ClipRect>,
        tiling: Tiling,
        bitmap: &Bitmap,
        fade: &dyn Fn([u8; 4]) -> [u8; 4],
    ) {
        let [tx, ty, tw, th] = tiling.tile;
        if tw <= 0.0 || th <= 0.0 {
            return;
        }
        // A repeating axis has period tile + gap; the gap between tiles paints nothing.
        let along = |at: f64, start: f64, size: f64, axis: usize| {
            let offset = at - start;
            let offset = if tiling.repeat[axis] {
                offset.rem_euclid(size + tiling.gap[axis])
            } else {
                offset
            };
            offset / size
        };
        self.shade(area, clip, |x, y| {
            let (u, v) = (along(x, tx, tw, 0), along(y, ty, th, 1));
            ((0.0..1.0).contains(&u) && (0.0..1.0).contains(&v)).then(|| fade(bitmap.sample(u, v)))
        });
    }
    /**
     * A `url()` layer as Blink's `BackgroundImageGeometry` places it: sized against the
     * padding box (the initial `background-origin`), offset there by its position, tiled
     * where it repeats, and clipped to the border box (the initial `background-clip`).
     */
    fn background_image(
        &mut self,
        item: &DisplayItem,
        layer: &DisplayBackground,
        picture: &Picture,
        fade: &dyn Fn([u8; 4]) -> [u8; 4],
    ) {
        let DisplayBackground::Image {
            size,
            position,
            repeat,
            space,
            round,
            ..
        } = layer
        else {
            return;
        };
        let [bt, br, bb, bl] = if item.border > 0.0 {
            [item.border; 4]
        } else {
            item.border_widths
        };
        let origin = [
            item.x + bl,
            item.y + bt,
            (item.width - bl - br).max(0.0),
            (item.height - bt - bb).max(0.0),
        ];
        let (w, h) = concrete_size(*size, [origin[2], origin[3]], self.natural(picture));
        let auto = match *size {
            ImageSize::Auto => [true; 2],
            ImageSize::Fixed { width, height } => [width.is_none(), height.is_none()],
            _ => [false; 2],
        };
        let (w, h) = rounded([origin[2], origin[3]], (w, h), *round, auto);
        let drawn;
        let bitmap = match picture {
            Picture::Raster(bitmap) => bitmap,
            Picture::Vector(markup) => {
                let Some(bitmap) = self.drawing(markup, w, h) else {
                    return;
                };
                drawn = bitmap;
                &*drawn
            }
        };
        let tiling = Tiling::place(origin, (w, h), *position, *repeat, *space);
        let bounds = [item.x, item.y, item.width, item.height];
        self.tile(bounds, item.clip.as_ref(), tiling, bitmap, fade);
    }
    /** A picture's natural size, parsing each drawing's markup once per screenshot. */
    fn natural(&mut self, picture: &Picture) -> Natural {
        let Picture::Vector(markup) = picture else {
            return picture.natural();
        };
        if let Some(natural) = self.naturals.get(markup) {
            return *natural;
        }
        let natural = picture.natural();
        self.naturals.insert(markup.clone(), natural);
        natural
    }
    /** `markup` drawn into a `w` by `h` tile, once per screenshot and size and at no more than
     * DRAWN_SIZES sizes, within the drawing budget; None when a bound refused it. */
    fn drawing(&mut self, markup: &Rc<str>, w: f64, h: f64) -> Option<Rc<Bitmap>> {
        let key = (markup.clone(), w.to_bits(), h.to_bits());
        if let Some(drawn) = self.drawn.get(&key) {
            return drawn.clone();
        }
        let sizes = self.sizes.entry(markup.clone()).or_default();
        *sizes += 1;
        let drawn = if *sizes > DRAWN_SIZES {
            None
        } else {
            Bitmap::draw(markup, w, h, &mut self.draw).map(Rc::new)
        };
        self.drawn.insert(key, drawn.clone());
        drawn
    }
    /**
     * Borders as Blink's `BoxBorderPainter` draws them: each side owns the part of the border
     * ring between the miter lines joining its outer and inner corners, the inner edge follows
     * the radii shrunk by the adjacent widths, and the side's style decides which of its pixels
     * paint and in which shade. An unset color is `currentcolor`, the item's own text color.
     */
    fn border(&mut self, item: &DisplayItem, current: [u8; 4], fade: &dyn Fn([u8; 4]) -> [u8; 4]) {
        let widths = if item.border > 0.0 {
            [item.border; 4]
        } else {
            item.border_widths
        };
        if widths.iter().all(|w| *w <= 0.0) {
            return;
        }
        let colors: [Option<[u8; 4]>; 4] = std::array::from_fn(|side| {
            let literal = item.border_colors[side].as_deref();
            let literal = literal.or(item.border_color.as_deref());
            Some(fade(literal.map_or(Some(current), painter_rgba)?))
        });
        let bounds = [item.x, item.y, item.width, item.height];
        let ring = Ring::new(bounds, item.radius, widths);
        self.shade(bounds, item.clip.as_ref(), |x, y| {
            let side = ring.side(x, y)?;
            let style = BorderPaint {
                style: item.border_styles[side],
                side,
                width: widths[side],
                length: if side % 2 == 0 {
                    item.width
                } else {
                    item.height
                },
            };
            style.color(
                colors[side]?,
                ring.depth(side, x, y),
                ring.along(side, x, y),
            )
        });
    }
    /** Encode as an 8-bit RGB PNG. */
    fn png(&self) -> EngineResult<Vec<u8>> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, self.width as u32, self.height as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .and_then(|mut writer| writer.write_image_data(&self.pixels))
            .map_err(|_| EngineError::invalid("The screenshot could not be encoded."))?;
        Ok(bytes)
    }
}

impl Runtime {
    /** page.screenshot: the visible viewport, or one element's box, as an inline PNG. */
    pub(super) fn screenshot(&mut self, params: Value) -> EngineResult<Value> {
        let input: Shot = parameters(params)?;
        let page = self.current()?;
        // Web fonts live in a per-thread registry; the page's faces must be there to draw.
        crate::document::install_fonts(&page.document.formatting);
        let display = page.display.as_ref().ok_or_else(|| {
            EngineError::new("no_page", "The page has no layout yet.", "Navigate first.")
        })?;
        let mut area = [
            0.0,
            self.scroll_y,
            self.render_context.width,
            self.render_context.height,
        ];
        if let Some(node_id) = &input.node_id {
            let item = display
                .items
                .iter()
                .find(|item| &page.node_id(item.local_id) == node_id)
                .ok_or_else(|| {
                    EngineError::new(
                        "invalid_target",
                        "No painted box has that node_id.",
                        "Use a node_id from page_find on the current page.",
                    )
                })?;
            area = [item.x, item.y, item.width, item.height];
        }
        let background = display
            .canvas_background
            .as_deref()
            .and_then(painter_rgba)
            .unwrap_or(WHITE);
        let foreground = display
            .canvas_foreground
            .as_deref()
            .and_then(painter_rgba)
            .unwrap_or(BLACK);
        let mut canvas = Canvas::new(area[0], area[1], area[2], area[3], background);
        let mut bitmaps = std::collections::HashMap::new();
        for item in display.items.iter().filter(|item| agent_visible(item)) {
            let bitmap = bitmaps.entry(item.local_id).or_insert_with(|| {
                page.images
                    .get(&item.local_id)
                    .and_then(|asset| Picture::decode(&asset.mime, &asset.bytes))
            });
            canvas.item(item, foreground, bitmap.as_ref());
        }
        let png = canvas.png()?;
        Ok(
            json!({"page_id":page.id(),"revision":page.revision,"layout_generation":page.layout_generation,"x":area[0],"y":area[1],"width":canvas.width,"height":canvas.height,"unit":"logical_points","render":"wireframe","mime":"image/png","png_base64":base64(&png)}),
        )
    }
}

/// Below this relative luminance (rgb(32, 32, 32)) Blink lightens both 3D border shades.
const DARK_LUMINANCE: f64 = 0.014443844;
/// Above this relative luminance (rgb(235, 235, 235)) the light 3D shade keeps the color as is.
const LIGHT_LUMINANCE: f64 = 0.83077;
/// How far `Color::Dark` and `Color::Light` move the largest channel.
const SHADE_STEP: f64 = 0.33;
/// Blink's lightened black and darkened white, special-cased ahead of the formula.
const LIGHTENED_BLACK: u8 = 84;
const DARKENED_WHITE: u8 = 171;
/// Dotted lines up to this thickness stroke as square dashes; thicker ones paint round dots.
const THIN_DOTS: f64 = 3.0;
/// Skia's dot interval falls this short of a whole period so the end dot is never dropped.
const DOT_EPSILON: f64 = 1.0e-2;

/** A fetched PNG or JPEG decoded to 8-bit RGBA, so the shot shows images instead of placeholders. */
struct Bitmap {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}
impl Bitmap {
    /** Decode PNG or JPEG bytes by their signature; other formats keep the placeholder. */
    fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(b"\x89PNG") {
            Self::png(bytes)
        } else if bytes.starts_with(&[0xff, 0xd8]) {
            Self::jpeg(bytes)
        } else {
            None
        }
    }
    /** PNG expanded to 8-bit channels, then widened to RGBA. */
    fn png(bytes: &[u8]) -> Option<Self> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().ok()?;
        let mut buffer = vec![0; reader.output_buffer_size()];
        let frame = reader.next_frame(&mut buffer).ok()?;
        let channels = frame.color_type.samples();
        let rgba = buffer[..frame.buffer_size()]
            .chunks_exact(channels)
            .flat_map(|p| match p.len() {
                1 => [p[0], p[0], p[0], 255],
                2 => [p[0], p[0], p[0], p[1]],
                3 => [p[0], p[1], p[2], 255],
                _ => [p[0], p[1], p[2], p[3]],
            })
            .collect();
        Some(Self {
            width: frame.width,
            height: frame.height,
            rgba,
        })
    }
    /** Baseline and progressive JPEG in gray or RGB; CMYK keeps the placeholder. */
    fn jpeg(bytes: &[u8]) -> Option<Self> {
        let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(bytes));
        let pixels = decoder.decode().ok()?;
        let info = decoder.info()?;
        let rgba = match info.pixel_format {
            jpeg_decoder::PixelFormat::RGB24 => pixels
                .chunks_exact(3)
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect(),
            jpeg_decoder::PixelFormat::L8 => {
                pixels.iter().flat_map(|l| [*l, *l, *l, 255]).collect()
            }
            _ => return None,
        };
        Some(Self {
            width: u32::from(info.width),
            height: u32::from(info.height),
            rgba,
        })
    }
    /** The texel under normalized coordinates `u`, `v` in [0, 1). */
    fn sample(&self, u: f64, v: f64) -> [u8; 4] {
        let x = ((u * f64::from(self.width)) as u32).min(self.width - 1) as usize;
        let y = ((v * f64::from(self.height)) as u32).min(self.height - 1) as usize;
        let at = (y * self.width as usize + x) * 4;
        [
            self.rgba[at],
            self.rgba[at + 1],
            self.rgba[at + 2],
            self.rgba[at + 3],
        ]
    }
}

/** Where one image layer's tiles go: the anchor tile `[x, y, width, height]`, which axes
 * repeat, and the empty gap between neighbouring tiles on each. */
#[derive(Clone, Copy, Debug, PartialEq)]
struct Tiling {
    tile: [f64; 4],
    repeat: [bool; 2],
    gap: [f64; 2],
}
impl Tiling {
    /**
     * Blink's `BackgroundImageGeometry`: the tile sits at its `position` in the positioning
     * `area`, and a `space` axis that fits two or more whole tiles instead starts at the area's
     * edge with the leftover spread evenly between them (`GetSpaceBetweenImageTiles`); one that
     * fits fewer paints once, as `no-repeat`, at its position.
     */
    fn place(
        area: [f64; 4],
        (w, h): (f64, f64),
        [xpx, xpct, ypx, ypct]: [f64; 4],
        mut repeat: [bool; 2],
        space: [bool; 2],
    ) -> Self {
        let mut tile = [
            area[0] + xpx + (area[2] - w) * xpct / 100.0,
            area[1] + ypx + (area[3] - h) * ypct / 100.0,
            w,
            h,
        ];
        let mut gap = [0.0; 2];
        for axis in 0..2 {
            if !(space[axis] && repeat[axis]) {
                continue;
            }
            let (size, extent) = (tile[axis + 2], area[axis + 2]);
            let count = if size > 0.0 {
                (extent / size).floor()
            } else {
                0.0
            };
            if count > 1.0 {
                gap[axis] = (extent - count * size) / (count - 1.0);
                tile[axis] = area[axis];
            } else {
                repeat[axis] = false;
            }
        }
        Self { tile, repeat, gap }
    }
}

/**
 * `background-repeat: round` as Blink's `BackgroundImageGeometry` rescales the tile: a round
 * axis takes the area's extent over the nearest whole number of tiles (at least one), and when
 * only that axis rounds and the other's size is `auto`, the other scales with it to keep the
 * image's ratio. The horizontal axis is settled first, as Blink does.
 */
fn rounded(
    area: [f64; 2],
    (mut w, mut h): (f64, f64),
    round: [bool; 2],
    auto: [bool; 2],
) -> (f64, f64) {
    if round[0] && area[0] > 0.0 && w > 0.0 {
        let fitted = area[0] / (area[0] / w).round().max(1.0);
        if auto[1] && !round[1] {
            h = h * fitted / w;
        }
        w = fitted;
    }
    if round[1] && area[1] > 0.0 && h > 0.0 {
        let fitted = area[1] / (area[1] / h).round().max(1.0);
        if auto[0] && !round[0] {
            w = w * fitted / h;
        }
        h = fitted;
    }
    (w, h)
}

/// Longest edge a drawn SVG tile may rasterize to; a larger tile samples it stretched.
// ponytail: an extreme tile (a 2^31 viewBox under `cover`) is drawn at this cap and scaled,
// so fine detail along that axis blurs; draw only the visible part when a test needs it.
const DRAWN_EDGE: f64 = 2048.0;
/// Straight pieces each cubic Bezier flattens into before filling.
const CUBIC_STEPS: usize = 16;

/// Sigmas past which a Gaussian's weight is negligible, as Skia sizes its blur kernels.
const BLUR_REACH: f64 = 3.0;

/** The `shape` mask sampled at each pixel center of `region` (whole pixels), then blurred
 * horizontally and vertically by a Gaussian of `sigma`, row-major. */
fn blurred(region: [f64; 4], sigma: f64, shape: &dyn Fn(f64, f64) -> bool) -> Vec<f64> {
    let (columns, rows) = (region[2] as usize, region[3] as usize);
    let mut mask: Vec<f64> = (0..rows * columns)
        .map(|at| {
            let (x, y) = ((at % columns) as f64 + 0.5, (at / columns) as f64 + 0.5);
            f64::from(u8::from(shape(region[0] + x, region[1] + y)))
        })
        .collect();
    let reach = (sigma * BLUR_REACH).ceil() as isize;
    let kernel: Vec<f64> = (-reach..=reach)
        .map(|d| (-((d * d) as f64) / (2.0 * sigma * sigma)).exp())
        .collect();
    let total: f64 = kernel.iter().sum();
    for (stride, count, lines) in [(1, columns, rows), (columns, rows, columns)] {
        let line_step = if stride == 1 { columns } else { 1 };
        let mut out = vec![0.0; mask.len()];
        for line in 0..lines {
            for i in 0..count {
                let mut sum = 0.0;
                for (k, weight) in kernel.iter().enumerate() {
                    let j = i as isize + k as isize - reach;
                    if (0..count as isize).contains(&j) {
                        sum += weight * mask[line * line_step + j as usize * stride];
                    }
                }
                out[line * line_step + i * stride] = sum / total;
            }
        }
        mask = out;
    }
    mask
}

/** A background picture: a decoded bitmap, or an SVG drawing rasterized at its tile size. */
enum Picture {
    Raster(Bitmap),
    Vector(Rc<str>),
}
/** Natural width, height and aspect ratio (width over height); any may be absent. */
type Natural = (Option<f64>, Option<f64>, Option<f64>);
impl Picture {
    /** A bitmap by its signature, or `image/svg+xml` markup kept for drawing. */
    fn decode(mime: &str, bytes: &[u8]) -> Option<Self> {
        if mime.eq_ignore_ascii_case("image/svg+xml") {
            return Some(Self::Vector(String::from_utf8_lossy(bytes).into()));
        }
        Bitmap::decode(bytes).map(Self::Raster)
    }
    /** The bitmap, for painters that cannot draw vectors. */
    fn raster(&self) -> Option<&Bitmap> {
        match self {
            Self::Raster(bitmap) => Some(bitmap),
            Self::Vector(_) => None,
        }
    }
    /** Natural dimensions as Blink's `SVGImage` reports them: the root's absolute `width`
     * and `height` (percentages count as absent), and the ratio of those when both are
     * present, else of the viewBox. */
    fn natural(&self) -> Natural {
        let markup = match self {
            Self::Raster(bitmap) => {
                let (w, h) = (f64::from(bitmap.width), f64::from(bitmap.height));
                return (Some(w), Some(h), Some(w / h));
            }
            Self::Vector(markup) => markup,
        };
        let Some(svg) = crate::document::parse(markup)
            .ok()
            .and_then(|doc| doc.nodes.into_iter().find(|node| node.tag == "svg"))
        else {
            return (None, None, None);
        };
        let length = |name: &str| {
            let value = svg.attributes.get(name)?.trim();
            let value = value.strip_suffix("px").unwrap_or(value);
            value
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite() && *v >= 0.0)
        };
        let (width, height) = (length("width"), length("height"));
        let view = svg.attributes.get("viewbox").and_then(|value| {
            let parts: Vec<f64> = value
                .split(|c: char| c == ',' || c.is_ascii_whitespace())
                .filter(|part| !part.is_empty())
                .filter_map(|part| part.parse().ok())
                .collect();
            (parts.len() == 4 && parts[2] > 0.0 && parts[3] > 0.0).then(|| parts[2] / parts[3])
        });
        let ratio = match (width, height) {
            (Some(w), Some(h)) if w > 0.0 && h > 0.0 => Some(w / h),
            (Some(_), Some(_)) => None,
            _ => view,
        };
        (width, height, ratio)
    }
}

/**
 * The tile size of an image layer by CSS Images' default sizing algorithm, as Blink's
 * `BackgroundImageGeometry::CalculateFillTileSize` resolves it against the positioning
 * `area`: `cover` and `contain` scale the natural ratio (or take the area without one), a
 * missing dimension follows the ratio, then the natural size, then the area, and an image
 * with only a ratio is sized as if `contain`.
 */
fn concrete_size(size: ImageSize, [aw, ah]: [f64; 2], (nw, nh, ratio): Natural) -> (f64, f64) {
    let contain = |cover: bool| match ratio {
        Some(r) if (aw / ah > r) != cover => (ah * r, ah),
        Some(r) => (aw, aw / r),
        None => (aw, ah),
    };
    match size {
        ImageSize::Cover => contain(true),
        ImageSize::Contain => contain(false),
        ImageSize::Fixed {
            width: Some(w),
            height: Some(h),
        } => (w, h),
        ImageSize::Fixed {
            width: Some(w),
            height: None,
        } => (w, ratio.map(|r| w / r).or(nh).unwrap_or(ah)),
        ImageSize::Fixed {
            width: None,
            height: Some(h),
        } => (ratio.map(|r| h * r).or(nw).unwrap_or(aw), h),
        _ => match (nw, nh, ratio) {
            (Some(w), Some(h), _) => (w, h),
            (Some(w), None, Some(r)) => (w, w / r),
            (None, Some(h), Some(r)) => (h * r, h),
            (Some(w), None, None) => (w, ah),
            (None, Some(h), None) => (aw, h),
            (None, None, Some(_)) => contain(false),
            (None, None, None) => (aw, ah),
        },
    }
}

impl Bitmap {
    /** An SVG drawing's fills rasterized into a `width` by `height` tile (capped at
     * DRAWN_EDGE a side), each pixel painted whose center the path's winding rule covers. The
     * markup parsed, the tile's pixels and every edge built, scanned and painted are charged to
     * `budget` before the work is done; a drawing that does not fit is None and paints nothing. */
    fn draw(markup: &str, width: f64, height: f64, budget: &mut DrawBudget) -> Option<Self> {
        let edge = |v: f64| v.ceil().clamp(1.0, DRAWN_EDGE) as u32;
        let (bw, bh) = (edge(width), edge(height));
        let (sx, sy) = (
            f64::from(bw) / width.max(f64::MIN_POSITIVE),
            f64::from(bh) / height.max(f64::MIN_POSITIVE),
        );
        budget.charge(markup.len().saturating_mul(PARSE_WEIGHT))?;
        budget.charge(bw as usize * bh as usize)?;
        let mut bitmap = Self {
            width: bw,
            height: bh,
            rgba: vec![0; (bw * bh * 4) as usize],
        };
        for path in crate::document::VectorPath::from_image(markup, [0.0, 0.0, width, height]) {
            let Some(mut color) = path.fill.as_deref().and_then(painter_rgba) else {
                continue;
            };
            color[3] = (f64::from(color[3]) * path.fill_opacity.clamp(0.0, 1.0)).round() as u8;
            let pieces = pieces(&path.commands);
            budget.edges = budget.edges.checked_sub(pieces)?;
            budget.charge(pieces)?;
            let edges = flatten(&path.commands, sx, sy);
            let even_odd = path.fill_rule == crate::document::FillRule::EvenOdd;
            bitmap.fill(&edges, even_odd, color, &mut budget.work)?;
        }
        Some(bitmap)
    }
    /**
     * Source-over `color` into every pixel whose center the edges enclose, scanning an active
     * edge table so each edge is visited only on the rows it crosses. Those rows are charged to
     * `work` first and each span's pixels before it paints; None once they do not fit.
     */
    fn fill(
        &mut self,
        edges: &[[f64; 4]],
        even_odd: bool,
        color: [u8; 4],
        work: &mut usize,
    ) -> Option<()> {
        // A row's center y crosses an edge when min(y0, y1) <= y < max(y0, y1).
        let height = f64::from(self.height);
        let first = |y: f64| (y - 0.5).ceil().clamp(0.0, height) as u32;
        let mut spans: Vec<(u32, u32, usize)> = edges
            .iter()
            .enumerate()
            .map(|(at, [_, y0, _, y1])| (first(y0.min(*y1)), first(y0.max(*y1)), at))
            .filter(|(start, end, _)| start < end)
            .collect();
        let rows: usize = spans.iter().map(|(s, e, _)| (e - s) as usize).sum();
        *work = work.checked_sub(rows)?;
        spans.sort_by_key(|span| span.0);
        let (mut next, mut active) = (0, Vec::<(u32, usize)>::new());
        for row in 0..self.height {
            while spans.get(next).is_some_and(|span| span.0 <= row) {
                active.push((spans[next].1, spans[next].2));
                next += 1;
            }
            active.retain(|(end, _)| *end > row);
            if active.is_empty() {
                continue;
            }
            let y = f64::from(row) + 0.5;
            let mut crossings: Vec<(f64, i32)> = active
                .iter()
                .map(|(_, at)| {
                    let [x0, y0, x1, y1] = edges[*at];
                    let x = x0 + (y - y0) * (x1 - x0) / (y1 - y0);
                    (x, if y1 > y0 { 1 } else { -1 })
                })
                .collect();
            crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut winding = 0;
            for pair in crossings.windows(2) {
                winding += pair[0].1;
                let inside = if even_odd {
                    winding % 2 != 0
                } else {
                    winding != 0
                };
                if inside {
                    *work = work.checked_sub(self.columns(pair[0].0, pair[1].0).len())?;
                    self.span(row, pair[0].0, pair[1].0, color);
                }
            }
        }
        Some(())
    }
    /** The columns whose pixel centers lie in [x0, x1). */
    fn columns(&self, x0: f64, x1: f64) -> std::ops::Range<u32> {
        let start = (x0 - 0.5).ceil().max(0.0) as u32;
        let end = ((x1 - 0.5).ceil().max(0.0) as u32).min(self.width);
        start..end
    }
    /** Blend `color` over the pixels of `row` whose centers lie in [x0, x1). */
    fn span(&mut self, row: u32, x0: f64, x1: f64, color: [u8; 4]) {
        let columns = self.columns(x0, x1);
        let line = (row * self.width) as usize;
        let texels = &mut self.rgba[(line + columns.start as usize) * 4..][..columns.len() * 4];
        // A run of equal pixels (every pixel of a fresh or flatly filled tile) blends alike, so
        // the last result is reused; the arithmetic per pixel is unchanged.
        let mut last: Option<([u8; 4], [u8; 4])> = None;
        for texel in texels.chunks_exact_mut(4) {
            let under = [texel[0], texel[1], texel[2], texel[3]];
            let out = match last {
                Some((seen, out)) if seen == under => out,
                _ => source_over(under, color),
            };
            last = Some((under, out));
            texel.copy_from_slice(&out);
        }
    }
}

/** Straight-alpha source-over of `color` onto one RGBA texel `under`. */
fn source_over(under: [u8; 4], color: [u8; 4]) -> [u8; 4] {
    let alpha = f64::from(color[3]) / 255.0;
    let coverage = f64::from(under[3]) / 255.0;
    let out = alpha + coverage * (1.0 - alpha);
    let channel = |c: usize| {
        let mixed = f64::from(color[c]) * alpha + f64::from(under[c]) * coverage * (1.0 - alpha);
        if out > 0.0 {
            (mixed / out).round() as u8
        } else {
            0
        }
    };
    [
        channel(0),
        channel(1),
        channel(2),
        (out * 255.0).round() as u8,
    ]
}

/** The edges `flatten` makes of `commands`: one per command, CUBIC_STEPS per cubic, plus the
 * closing edge, counted before any are built. */
fn pieces(commands: &[crate::document::PathCommand]) -> usize {
    let piece = |command: &crate::document::PathCommand| match command {
        crate::document::PathCommand::Cubic { .. } => CUBIC_STEPS,
        _ => 1,
    };
    commands.iter().map(piece).sum::<usize>() + 1
}

/** Path commands as straight edges `[x0, y0, x1, y1]` scaled by `sx`, `sy`, every subpath
 * closed (fills close implicitly) and cubics flattened into CUBIC_STEPS pieces. */
fn flatten(commands: &[crate::document::PathCommand], sx: f64, sy: f64) -> Vec<[f64; 4]> {
    use crate::document::PathCommand;
    let mut edges = Vec::new();
    let (mut start, mut at) = ((0.0, 0.0), (0.0, 0.0));
    let push = |edges: &mut Vec<[f64; 4]>, from: (f64, f64), to: (f64, f64)| {
        edges.push([from.0 * sx, from.1 * sy, to.0 * sx, to.1 * sy]);
    };
    for command in commands {
        match *command {
            PathCommand::Move { x, y } => {
                push(&mut edges, at, start);
                (start, at) = ((x, y), (x, y));
            }
            PathCommand::Line { x, y } => {
                push(&mut edges, at, (x, y));
                at = (x, y);
            }
            PathCommand::Cubic {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                let from = at;
                for step in 1..=CUBIC_STEPS {
                    let t = step as f64 / CUBIC_STEPS as f64;
                    let u = 1.0 - t;
                    let point = |a: f64, b: f64, c: f64, d: f64| {
                        u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
                    };
                    let next = (point(from.0, x1, x2, x), point(from.1, y1, y2, y));
                    push(&mut edges, at, next);
                    at = next;
                }
            }
            PathCommand::Close => {
                push(&mut edges, at, start);
                at = start;
            }
        }
    }
    push(&mut edges, at, start);
    edges
}

/** One edge pair `[start, length]` snapped as Blink's `PixelSnappedIntRect` does: both edges
 * round to whole pixels and the length is what lies between them. */
fn snap(start: f64, length: f64) -> (f64, f64) {
    let (low, high) = (start.round(), (start + length).round());
    (low, high - low)
}

/**
 * The item with its border box, content box and clip pixel-snapped, as Blink snaps box geometry
 * before painting backgrounds and borders. Without it a box at y = 50.5 lost a row: pixel
 * centers decided coverage, so a 20px box covered only 19 rows while its text covered 20.
 * Text runs keep their exact positions; the glyph rasterizer places those.
 */
fn snapped(item: &DisplayItem) -> DisplayItem {
    let mut item = item.clone();
    (item.x, item.width) = snap(item.x, item.width);
    (item.y, item.height) = snap(item.y, item.height);
    if let Some(clip) = &mut item.clip {
        (clip.x, clip.width) = snap(clip.x, clip.width);
        (clip.y, clip.height) = snap(clip.y, clip.height);
    }
    if let Some([x, y, w, h]) = item.content_box {
        let ((x, w), (y, h)) = (snap(x, w), snap(y, h));
        item.content_box = Some([x, y, w, h]);
    }
    item
}

/** Whether a point lies inside a rectangle whose corners (tl, tr, br, bl) are ellipses `[rx, ry]`. */
fn inside_rounded(px: f64, py: f64, [x, y, w, h]: [f64; 4], radii: [[f64; 2]; 4]) -> bool {
    if px < x || py < y || px >= x + w || py >= y + h {
        return false;
    }
    let centers = [
        (x + radii[0][0], y + radii[0][1], -1.0, -1.0),
        (x + w - radii[1][0], y + radii[1][1], 1.0, -1.0),
        (x + w - radii[2][0], y + h - radii[2][1], 1.0, 1.0),
        (x + radii[3][0], y + h - radii[3][1], -1.0, 1.0),
    ];
    centers
        .iter()
        .zip(radii)
        .all(|(&(cx, cy, sx, sy), [rx, ry])| {
            let (dx, dy) = ((px - cx) * sx, (py - cy) * sy);
            if rx <= 0.0 || ry <= 0.0 || dx <= 0.0 || dy <= 0.0 {
                return true;
            }
            (dx / rx).powi(2) + (dy / ry).powi(2) <= 1.0
        })
}

/** A box's border ring: the outer rounded border edge and the inner rounded padding edge. */
struct Ring {
    outer: [f64; 4],
    radii: [[f64; 2]; 4],
    inner: [f64; 4],
    inner_radii: [[f64; 2]; 4],
    widths: [f64; 4],
}
impl Ring {
    /** The padding edge's radii are the outer radii less the adjacent widths (CSS Backgrounds 5.2). */
    fn new(outer: [f64; 4], radius: [f64; 4], widths: [f64; 4]) -> Self {
        let [t, r, b, l] = widths;
        let [x, y, w, h] = outer;
        let shrink = |radius: f64, across: f64, down: f64| {
            [(radius - across).max(0.0), (radius - down).max(0.0)]
        };
        Self {
            outer,
            radii: radius.map(|r| [r, r]),
            inner: [x + l, y + t, (w - l - r).max(0.0), (h - t - b).max(0.0)],
            inner_radii: [
                shrink(radius[0], l, t),
                shrink(radius[1], r, t),
                shrink(radius[2], r, b),
                shrink(radius[3], l, b),
            ],
            widths,
        }
    }
    /** Distance from the outer edge on `side` (top, right, bottom, left). */
    fn depth(&self, side: usize, px: f64, py: f64) -> f64 {
        let [x, y, w, h] = self.outer;
        [py - y, x + w - px, y + h - py, px - x][side]
    }
    /** Distance along `side` from its start: the left end of a row, the top end of a column. */
    fn along(&self, side: usize, px: f64, py: f64) -> f64 {
        let [x, y, ..] = self.outer;
        if side.is_multiple_of(2) {
            px - x
        } else {
            py - y
        }
    }
    /** Which side owns a point of the ring: the one whose miter-normalized depth is least. */
    fn side(&self, px: f64, py: f64) -> Option<usize> {
        if !inside_rounded(px, py, self.outer, self.radii)
            || inside_rounded(px, py, self.inner, self.inner_radii)
        {
            return None;
        }
        (0..4)
            .filter(|side| self.widths[*side] > 0.0)
            .map(|side| (side, self.depth(side, px, py) / self.widths[side]))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(side, _)| side)
    }
}

/** One side's style, width and length, deciding which of its pixels paint and in which shade. */
struct BorderPaint {
    style: BorderStyle,
    side: usize,
    width: f64,
    /// The side's full outer length, the line Blink strokes dashes and dots along.
    length: f64,
}
impl BorderPaint {
    /** Blink's `BorderEdge::EffectiveStyle`: thin doubles and 3D grooves fall back to solid. */
    fn effective(&self) -> BorderStyle {
        match self.style {
            BorderStyle::Double if self.width < 3.0 => BorderStyle::Solid,
            BorderStyle::Groove | BorderStyle::Ridge if self.width <= 1.0 => BorderStyle::Solid,
            style => style,
        }
    }
    /** Blink's `DarkenBoxSide`: top and left darken for inset, bottom and right for outset. */
    fn darken(&self, style: BorderStyle) -> bool {
        (self.side == 0 || self.side == 3) == (style == BorderStyle::Inset)
    }
    /** The pixel's color at `depth` into the side and `along` it, or None where it is clear. */
    fn color(&self, color: [u8; 4], depth: f64, along: f64) -> Option<[u8; 4]> {
        let width = self.width;
        match self.effective() {
            BorderStyle::Solid => Some(color),
            // The outer and inner stripes are a rounded third each (`GetDoubleBorderStripeWidth`).
            BorderStyle::Double => (depth < (width / 3.0).round()
                || depth >= (width * 2.0 / 3.0).round())
            .then_some(color),
            style @ (BorderStyle::Inset | BorderStyle::Outset) => {
                Some(inset_outset(self.darken(style), color))
            }
            style @ (BorderStyle::Groove | BorderStyle::Ridge) => {
                // The outer half paints as the first style, the inner half as the other.
                let groove = style == BorderStyle::Groove;
                let outer = depth < (width / 2.0).ceil();
                let inset = groove == outer;
                let part = if inset {
                    BorderStyle::Inset
                } else {
                    BorderStyle::Outset
                };
                Some(inset_outset(self.darken(part), color))
            }
            BorderStyle::Dashed | BorderStyle::Dotted => self.stroke(depth, along).then_some(color),
        }
    }
    /** Dashed and dotted sides as Blink's `StyledStrokeData` dash effect strokes their midline. */
    fn stroke(&self, depth: f64, along: f64) -> bool {
        let (width, length) = (self.width, self.length);
        if self.style == BorderStyle::Dotted && width > THIN_DOTS {
            // Zero-length dashes with round caps: dots of the border's width, the first one a
            // radius in from the start, spaced by the best gap.
            let period = if length < width * 2.0 {
                f64::INFINITY
            } else {
                best_gap(length, width, width) + width - DOT_EPSILON
            };
            let offset = along - width / 2.0;
            let nearest = if period.is_finite() {
                (offset / period).round().max(0.0) * period
            } else {
                0.0
            };
            let radius = width / 2.0;
            return (offset - nearest).powi(2) + (depth - radius).powi(2) <= radius * radius;
        }
        let (mut dash, mut gap) = if self.style == BorderStyle::Dashed {
            let thick = width >= THIN_DOTS;
            (
                width * if thick { 2.0 } else { 3.0 },
                width * if thick { 1.0 } else { 2.0 },
            )
        } else {
            (width, width)
        };
        if length <= dash * 2.0 {
            return true;
        }
        if length <= dash * 2.0 + gap {
            let scale = length / (dash * 2.0 + gap);
            (dash, gap) = (dash * scale, gap * scale);
        } else if self.style == BorderStyle::Dashed {
            gap = best_gap(length, dash, gap);
        }
        along.rem_euclid(dash + gap) < dash
    }
}

/** Blink's `SelectBestDashGap` for an open line: the gap nearest the ideal that fits whole dashes. */
fn best_gap(length: f64, dash: f64, gap: f64) -> f64 {
    let fewer = ((length + gap) / (dash + gap)).floor();
    let more = fewer + 1.0;
    let fewer_gap = (length - fewer * dash) / (fewer - 1.0);
    let more_gap = (length - more * dash) / (more - 1.0);
    if more_gap <= 0.0 || (fewer_gap - gap).abs() < (more_gap - gap).abs() {
        fewer_gap
    } else {
        more_gap
    }
}

/** WCAG relative luminance of an sRGB color, as `color_utils::GetRelativeLuminance4f`. */
fn luminance([r, g, b, _]: [u8; 4]) -> f64 {
    let linear = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

/** Scale the channels so the largest moves to `target(v)`, quantized as Blink's `QuantizeTo8Bit`. */
fn rescale(color: [u8; 4], target: impl Fn(f64) -> f64) -> [u8; 4] {
    let v = color[..3]
        .iter()
        .copied()
        .max()
        .map_or(0.0, |c| f64::from(c) / 255.0);
    let multiplier = if v == 0.0 { 0.0 } else { target(v) / v };
    let channel = |c: u8| (f64::from(c) * multiplier).round().clamp(0.0, 255.0) as u8;
    [
        channel(color[0]),
        channel(color[1]),
        channel(color[2]),
        color[3],
    ]
}

/** Blink's `Color::Light`: the largest channel up by a third, black to rgb(84, 84, 84). */
fn light(color: [u8; 4]) -> [u8; 4] {
    if color[..3] == [0, 0, 0] {
        return [LIGHTENED_BLACK, LIGHTENED_BLACK, LIGHTENED_BLACK, color[3]];
    }
    rescale(color, |v| (v + SHADE_STEP).min(1.0))
}

/** Blink's `Color::Dark`: the largest channel down by a third, white to rgb(171, 171, 171). */
fn dark(color: [u8; 4]) -> [u8; 4] {
    if color == [255; 4] {
        return [DARKENED_WHITE, DARKENED_WHITE, DARKENED_WHITE, 255];
    }
    rescale(color, |v| (v - SHADE_STEP).max(0.0))
}

/** Blink's `CalculateInsetOutsetColor`: the shadowed and lit shades of a 3D border side. */
fn inset_outset(darken: bool, color: [u8; 4]) -> [u8; 4] {
    let luminance = luminance(color);
    if luminance <= DARK_LUMINANCE {
        return if darken {
            light(color)
        } else {
            light(light(color))
        };
    }
    if darken {
        dark(color)
    } else if luminance > LIGHT_LUMINANCE {
        color
    } else {
        light(color)
    }
}

/** Where a point (relative to the box's top-left) falls on a gradient line or ellipse, 0 to 1 and beyond. */
fn gradient_offset(radial: bool, [a, b, c, d]: [f64; 4], x: f64, y: f64) -> f64 {
    if radial {
        let (rx, ry) = (c.max(f64::MIN_POSITIVE), d.max(f64::MIN_POSITIVE));
        return (((x - a) / rx).powi(2) + ((y - b) / ry).powi(2)).sqrt();
    }
    let (dx, dy) = (c - a, d - b);
    let squared = dx * dx + dy * dy;
    if squared == 0.0 {
        return 0.0;
    }
    ((x - a) * dx + (y - b) * dy) / squared
}

/** The color at offset `t` between resolved stops, interpolated in premultiplied sRGB as CSS
 * Images 4 asks; before the first stop and past the last the end colors extend. */
fn stop_color(stops: &[(f64, [u8; 4])], t: f64) -> [u8; 4] {
    let after = stops.iter().position(|(at, _)| *at > t);
    let (low, high) = match after {
        None => return stops[stops.len() - 1].1,
        Some(0) => return stops[0].1,
        Some(i) => (stops[i - 1], stops[i]),
    };
    let share = (t - low.0) / (high.0 - low.0);
    let alpha = |c: [u8; 4]| f64::from(c[3]) / 255.0;
    let a = alpha(low.1) + (alpha(high.1) - alpha(low.1)) * share;
    let mut out = [0, 0, 0, (a * 255.0).round() as u8];
    for (i, channel) in out.iter_mut().take(3).enumerate() {
        let lo = f64::from(low.1[i]) * alpha(low.1);
        let hi = f64::from(high.1[i]) * alpha(high.1);
        let premultiplied = lo + (hi - lo) * share;
        *channel = if a > 0.0 {
            (premultiplied / a).round().clamp(0.0, 255.0) as u8
        } else {
            0
        };
    }
    out
}
/** Standard padded base64, the encoding MCP image content uses. */
pub(crate) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        BorderPaint, BorderStyle, Ring, base64, best_gap, inset_outset, inside_rounded, stop_color,
    };

    /** Base64 matches the RFC 4648 test vectors, padding included. */
    #[test]
    fn base64_matches_rfc_vectors() {
        for (input, output) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input.as_bytes()), output);
        }
    }

    /** A zero-area padding box holds no pixel, so a lone 1px top border still paints. */
    #[test]
    fn inner_edge_is_half_open() {
        let ring = Ring::new([0.0, 0.0, 10.0, 1.0], [0.0; 4], [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(ring.side(5.5, 0.5), Some(0));
        assert!(!inside_rounded(
            5.0,
            1.0,
            [0.0, 0.0, 10.0, 1.0],
            [[0.0; 2]; 4]
        ));
    }

    /** Miter lines split a corner between the two sides by their widths. */
    #[test]
    fn corners_split_along_the_miter() {
        let ring = Ring::new([0.0, 0.0, 100.0, 100.0], [0.0; 4], [10.0, 10.0, 10.0, 20.0]);
        assert_eq!(ring.side(5.5, 8.5), Some(3));
        assert_eq!(ring.side(15.5, 2.5), Some(0));
        assert_eq!(ring.side(50.5, 50.5), None);
    }

    /** A rounded corner leaves its outside unpainted. */
    #[test]
    fn radius_cuts_the_corner() {
        let ring = Ring::new([0.0, 0.0, 100.0, 100.0], [20.0; 4], [5.0; 4]);
        assert_eq!(ring.side(0.5, 0.5), None);
        assert_eq!(ring.side(50.5, 0.5), Some(0));
    }

    /** Blink's `Color::Dark`/`Light` and `CalculateInsetOutsetColor` shades. */
    #[test]
    fn three_d_shades_match_blink() {
        assert_eq!(inset_outset(true, [0, 0, 0, 255]), [84, 84, 84, 255]);
        assert_eq!(inset_outset(false, [0, 0, 0, 255]), [168, 168, 168, 255]);
        assert_eq!(
            inset_outset(true, [255, 255, 255, 255]),
            [171, 171, 171, 255]
        );
        assert_eq!(
            inset_outset(false, [255, 255, 255, 255]),
            [255, 255, 255, 255]
        );
        assert_eq!(inset_outset(true, [0, 0, 255, 255]), [0, 0, 171, 255]);
    }

    /** A double border paints its outer and inner rounded thirds and leaves the middle clear. */
    #[test]
    fn double_paints_two_stripes() {
        let side = BorderPaint {
            style: BorderStyle::Double,
            side: 0,
            width: 9.0,
            length: 100.0,
        };
        let red = [255, 0, 0, 255];
        let painted: Vec<bool> = (0..9)
            .map(|d| side.color(red, d as f64 + 0.5, 0.0).is_some())
            .collect();
        assert_eq!(
            painted,
            [true, true, true, false, false, false, true, true, true]
        );
        let thin = BorderPaint { width: 2.0, ..side };
        assert!(thin.color(red, 1.0, 0.0).is_some());
    }

    /** Dashes are two widths long at 3px and up, with the gap chosen to fit whole dashes. */
    #[test]
    fn dashes_follow_blinks_pattern() {
        assert_eq!(best_gap(100.0, 20.0, 10.0), 20.0 / 3.0);
        let side = BorderPaint {
            style: BorderStyle::Dashed,
            side: 0,
            width: 10.0,
            length: 100.0,
        };
        let red = [255, 0, 0, 255];
        assert!(side.color(red, 5.0, 10.0).is_some());
        assert!(side.color(red, 5.0, 25.0).is_none());
        let dots = BorderPaint {
            style: BorderStyle::Dotted,
            ..side
        };
        assert!(dots.color(red, 5.0, 5.0).is_some());
        assert!(dots.color(red, 0.5, 0.5).is_none());
    }

    /** Gradients interpolate premultiplied, so a fade to transparent keeps its hue. */
    #[test]
    fn gradient_stops_interpolate_premultiplied() {
        let stops = [(0.0, [255, 0, 0, 255]), (1.0, [0, 0, 255, 0])];
        assert_eq!(stop_color(&stops, 0.5), [255, 0, 0, 128]);
        assert_eq!(stop_color(&stops, -1.0), [255, 0, 0, 255]);
        assert_eq!(stop_color(&stops, 2.0), [0, 0, 255, 0]);
    }
}
