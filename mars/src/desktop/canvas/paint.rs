/** CoreGraphics replays Eye's effect layers: clips, transforms, rounded boxes, shadows, gradients and groups. */
use super::{DisplayItem, literal_color, rgba};
use objc2::rc::Retained;
use objc2_app_kit::{NSColor, NSGraphicsContext};
use objc2_core_foundation::CGAffineTransform;
use objc2_core_graphics::{
    CGColorSpace, CGContext, CGGradient, CGGradientDrawingOptions, kCGColorSpaceSRGB,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};

/// A segment between stops of different alpha is sampled this many times, so CoreGraphics'
/// straight-alpha interpolation follows CSS's premultiplied one closely.
const GRADIENT_STEPS: usize = 8;
/// Shadow and inset-shadow paths are surrounded by this much canvas so even-odd fills cover the blur.
const SHADOW_REACH: f64 = 10_000.0;

/** A box in document points with per-corner radii top-left, top-right, bottom-right, bottom-left. */
#[derive(Clone, Copy)]
pub(super) struct Shape {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub radius: [f64; 4],
}
impl Shape {
    /// The item's border box.
    pub fn of(item: &DisplayItem) -> Self {
        Self {
            x: item.x,
            y: item.y,
            width: item.width,
            height: item.height,
            radius: item.radius,
        }
    }
    /// Grows (or with a negative amount shrinks) every edge; radii follow, never below zero.
    fn outset(&self, amount: f64) -> Self {
        Self {
            x: self.x - amount,
            y: self.y - amount,
            width: (self.width + amount * 2.0).max(0.0),
            height: (self.height + amount * 2.0).max(0.0),
            radius: self
                .radius
                .map(|r| if r > 0.0 { (r + amount).max(0.0) } else { 0.0 }),
        }
    }
    /// Moves the box.
    fn offset(&self, x: f64, y: f64) -> Self {
        Self {
            x: self.x + x,
            y: self.y + y,
            ..*self
        }
    }
    /// Shrinks each side independently by `[top, right, bottom, left]`, clamping corner radii.
    pub fn inset_sides(&self, [top, right, bottom, left]: [f64; 4]) -> Self {
        Self {
            x: self.x + left,
            y: self.y + top,
            width: (self.width - left - right).max(0.0),
            height: (self.height - top - bottom).max(0.0),
            radius: [
                (self.radius[0] - top.max(left)).max(0.0),
                (self.radius[1] - top.max(right)).max(0.0),
                (self.radius[2] - bottom.max(right)).max(0.0),
                (self.radius[3] - bottom.max(left)).max(0.0),
            ],
        }
    }
}

/// Paints in display-list order inside one CoreGraphics context, tracking open opacity groups.
pub(super) struct Painter {
    cg: Retained<CGContext>,
    open: Vec<(usize, f64)>,
}
impl Painter {
    /// The current AppKit drawing context's CoreGraphics context, if drawing is in progress.
    pub fn current() -> Option<Self> {
        NSGraphicsContext::currentContext().map(|context| Self {
            cg: context.CGContext(),
            open: Vec::new(),
        })
    }
    /** The raw context handle every CoreGraphics call takes. */
    fn cg(&self) -> Option<&CGContext> {
        Some(&self.cg)
    }
    /// Closes groups the item is outside of and opens the ones it enters, as transparency layers.
    pub fn groups(&mut self, item: &DisplayItem) {
        let common = self
            .open
            .iter()
            .zip(&item.stacking_opacity)
            .take_while(|(open, group)| open.0 == group.id)
            .count();
        while self.open.len() > common {
            self.close_one();
        }
        for group in &item.stacking_opacity[common..] {
            CGContext::save_g_state(self.cg());
            CGContext::set_alpha(self.cg(), group.opacity);
            // SAFETY: a null auxiliary dictionary is documented; the layer closes in close_one.
            unsafe { CGContext::begin_transparency_layer(self.cg(), None) };
            self.open.push((group.id, group.opacity));
        }
    }
    /** Ends the innermost open group. */
    fn close_one(&mut self) {
        CGContext::end_transparency_layer(self.cg());
        CGContext::restore_g_state(self.cg());
        self.open.pop();
    }
    /// Ends every open group once the list is done.
    pub fn finish(&mut self) {
        while !self.open.is_empty() {
            self.close_one();
        }
    }
    /// The alpha an item still needs after its groups composite: its opacity divided by theirs.
    pub fn residual(&self, item: &DisplayItem) -> f64 {
        let groups: f64 = item.stacking_opacity.iter().map(|g| g.opacity).product();
        if groups > 0.0 {
            (item.opacity / groups).min(1.0)
        } else {
            0.0
        }
    }
    /// Saves state and applies the item's clip, then its transform.
    pub fn enter(&self, item: &DisplayItem) {
        CGContext::save_g_state(self.cg());
        if let Some(clip) = &item.clip {
            self.path(&Shape {
                x: clip.x,
                y: clip.y,
                width: clip.width,
                height: clip.height,
                radius: clip.radius,
            });
            CGContext::clip(self.cg());
        }
        if let Some([a, b, c, d, tx, ty]) = item.transform {
            CGContext::concat_ctm(self.cg(), CGAffineTransform { a, b, c, d, tx, ty });
        }
    }
    /// Restores the state saved by `enter`.
    pub fn leave(&self) {
        CGContext::restore_g_state(self.cg());
    }
    /** Replaces the current path with a rounded rectangle. */
    fn path(&self, shape: &Shape) {
        let cg = self.cg();
        CGContext::begin_path(cg);
        let Shape {
            x,
            y,
            width: w,
            height: h,
            radius: [tl, tr, br, bl],
        } = *shape;
        if [tl, tr, br, bl] == [0.0; 4] {
            CGContext::add_rect(cg, rect(x, y, w, h));
            return;
        }
        CGContext::move_to_point(cg, x + tl, y);
        CGContext::add_arc_to_point(cg, x + w, y, x + w, y + h, tr);
        CGContext::add_arc_to_point(cg, x + w, y + h, x, y + h, br);
        CGContext::add_arc_to_point(cg, x, y + h, x, y, bl);
        CGContext::add_arc_to_point(cg, x, y, x + w, y, tl);
        CGContext::close_path(cg);
    }
    /// Fills a rounded box with an AppKit color.
    pub fn fill(&self, shape: &Shape, color: &NSColor) {
        color.setFill();
        self.path(shape);
        CGContext::fill_path(self.cg());
    }
    /// Intersects the current clipping region with a rounded shape.
    pub fn clip_shape(&self, shape: &Shape) {
        self.path(shape);
        CGContext::clip(self.cg());
    }
    /// Strokes a uniform border inside the border box, following its corners.
    pub fn stroke(&self, shape: &Shape, width: f64, color: &NSColor) {
        color.setStroke();
        CGContext::set_line_width(self.cg(), width);
        self.path(&shape.outset(-width / 2.0));
        CGContext::stroke_path(self.cg());
    }
    /// Fills each side of a non-uniform border as its own trapezoid with its own color.
    pub fn sides(&self, shape: &Shape, widths: [f64; 4], colors: [Option<Retained<NSColor>>; 4]) {
        let (x, y, w, h) = (shape.x, shape.y, shape.width, shape.height);
        let [top, right, bottom, left] = widths;
        let (ix, iy, iw, ih) = (x + left, y + top, x + w - right, y + h - bottom);
        let quads = [
            [(x, y), (x + w, y), (iw, iy), (ix, iy)],
            [(x + w, y), (x + w, y + h), (iw, ih), (iw, iy)],
            [(x + w, y + h), (x, y + h), (ix, ih), (iw, ih)],
            [(x, y + h), (x, y), (ix, iy), (ix, ih)],
        ];
        for ((quad, width), color) in quads.into_iter().zip(widths).zip(colors) {
            let Some(color) = color.filter(|_| width > 0.0) else {
                continue;
            };
            color.setFill();
            let cg = self.cg();
            CGContext::begin_path(cg);
            CGContext::move_to_point(cg, quad[0].0, quad[0].1);
            for (px, py) in &quad[1..] {
                CGContext::add_line_to_point(cg, *px, *py);
            }
            CGContext::close_path(cg);
            CGContext::fill_path(cg);
        }
    }
    /// Outer shadows, last listed painted first, clipped so they never show through the box itself.
    /// The blurred shape is the box grown by its spread: CoreGraphics, like CSS, keeps the blur's 50%
    /// line on the shape's edge, so shrinking it by half the blur would hide most of the shadow.
    pub fn outer_shadows(&self, shape: &Shape, item: &DisplayItem, alpha: f64) {
        for shadow in item.shadows.iter().rev().filter(|shadow| !shadow.inset) {
            let Some(color) = shadow_color(&shadow.color, alpha) else {
                continue;
            };
            CGContext::save_g_state(self.cg());
            self.outside(shape);
            self.shadowed(shadow.blur, &color);
            let cast = shape.outset(shadow.spread).offset(shadow.x, shadow.y);
            self.fill(&cast, &color);
            CGContext::restore_g_state(self.cg());
        }
    }
    /// Inset shadows paint inside the padding box, from the edges inward.
    pub fn inset_shadows(&self, shape: &Shape, item: &DisplayItem, alpha: f64) {
        let border = item.border;
        for shadow in item.shadows.iter().rev().filter(|shadow| shadow.inset) {
            let Some(color) = shadow_color(&shadow.color, alpha) else {
                continue;
            };
            let inner = if border > 0.0 {
                shape.outset(-border)
            } else if item.border_widths != [0.0; 4] {
                shape.inset_sides(item.border_widths)
            } else {
                *shape
            };
            CGContext::save_g_state(self.cg());
            self.path(&inner);
            CGContext::clip(self.cg());
            self.shadowed(shadow.blur, &color);
            let hole = inner.outset(-shadow.spread).offset(shadow.x, shadow.y);
            color.setFill();
            self.path(&hole);
            CGContext::add_rect(
                self.cg(),
                rect(
                    inner.x - SHADOW_REACH,
                    inner.y - SHADOW_REACH,
                    inner.width + SHADOW_REACH * 2.0,
                    inner.height + SHADOW_REACH * 2.0,
                ),
            );
            CGContext::eo_fill_path(self.cg());
            CGContext::restore_g_state(self.cg());
        }
    }
    /** Clips to everything except the shape, as CSS clips an outer shadow to outside the border box. */
    fn outside(&self, shape: &Shape) {
        self.path(shape);
        CGContext::add_rect(
            self.cg(),
            rect(
                shape.x - SHADOW_REACH,
                shape.y - SHADOW_REACH,
                shape.width + SHADOW_REACH * 2.0,
                shape.height + SHADOW_REACH * 2.0,
            ),
        );
        CGContext::eo_clip(self.cg());
    }
    /** A zero-offset blur in the shadow's color; the cast shape itself carries the offset.
     * CoreGraphics' blur argument is a Gaussian of sigma blur/2, the same rule as a CSS blur radius,
     * so the CSS value passes through unchanged. */
    fn shadowed(&self, blur: f64, color: &NSColor) {
        let [r, g, b, a] = components(color);
        let cg_color = objc2_core_graphics::CGColor::new_srgb(r, g, b, a);
        CGContext::set_shadow_with_color(self.cg(), NSSize::new(0.0, 0.0), blur, Some(&cg_color));
    }
    /// Gradient layers, last listed at the bottom, clipped to the rounded border box.
    /// Image layers are drawn by `background::paint` from the bytes the app fetched.
    pub fn backgrounds(&self, shape: &Shape, item: &DisplayItem, alpha: f64) {
        for layer in item.backgrounds.iter().rev() {
            let Some((radial, [a, b, c, d], stops)) = layer.gradient() else {
                continue;
            };
            let Some(parsed) = stops
                .iter()
                .map(|stop| rgba(&stop.color).map(|color| (color, stop.position)))
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let resolved = premultiplied(&parsed);
            if resolved.len() < 2 {
                continue;
            }
            let colors = resolved
                .iter()
                .flat_map(|([r, g, b, a], _)| [*r, *g, *b, a * alpha])
                .collect::<Vec<_>>();
            let locations = resolved.iter().map(|(_, at)| *at).collect::<Vec<_>>();
            CGContext::save_g_state(self.cg());
            self.path(shape);
            CGContext::clip(self.cg());
            with_gradient(&colors, &locations, |gradient| {
                if radial {
                    self.radial(gradient, shape.x + a, shape.y + b, c, d);
                } else {
                    CGContext::draw_linear_gradient(
                        self.cg(),
                        Some(gradient),
                        NSPoint::new(shape.x + a, shape.y + b),
                        NSPoint::new(shape.x + c, shape.y + d),
                        CGGradientDrawingOptions::DrawsBeforeStartLocation
                            | CGGradientDrawingOptions::DrawsAfterEndLocation,
                    );
                }
            });
            CGContext::restore_g_state(self.cg());
        }
    }
    /** An elliptical gradient is a unit circle scaled to the ellipse's radii. */
    fn radial(&self, gradient: &CGGradient, cx: f64, cy: f64, rx: f64, ry: f64) {
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        CGContext::translate_ctm(self.cg(), cx, cy);
        CGContext::scale_ctm(self.cg(), rx, ry);
        let center = NSPoint::new(0.0, 0.0);
        CGContext::draw_radial_gradient(
            self.cg(),
            Some(gradient),
            center,
            0.0,
            center,
            1.0,
            CGGradientDrawingOptions::DrawsAfterEndLocation,
        );
    }
}

/** Stops that CoreGraphics, which interpolates straight (unpremultiplied) color, paints the way CSS
 * interpolates premultiplied color. Without it `linear-gradient(transparent, #fff)` passes through
 * translucent grey, since `transparent` is transparent black; in Chrome it is a clean white fade.
 * A transparent end takes its neighbor's color exactly; other alpha changes are sampled. */
fn premultiplied(stops: &[([f64; 4], f64)]) -> Vec<([f64; 4], f64)> {
    let mut resolved = Vec::with_capacity(stops.len() * 2);
    for pair in stops.windows(2) {
        let [(from, start), (to, end)] = [pair[0], pair[1]];
        let steps = if from[3] == to[3] || from[3] == 0.0 || to[3] == 0.0 {
            1
        } else {
            GRADIENT_STEPS
        };
        for step in 0..=steps {
            let t = step as f64 / steps as f64;
            resolved.push((mix(from, to, t), start + (end - start) * t));
        }
    }
    resolved
}
/** Premultiplied interpolation in straight components; a fully transparent result keeps the
 * color of whichever end is visible, so its neighbors fade from it instead of from black. */
fn mix(from: [f64; 4], to: [f64; 4], t: f64) -> [f64; 4] {
    let alpha = from[3] + (to[3] - from[3]) * t;
    if alpha <= 0.0 {
        let [r, g, b, _] = if to[3] > 0.0 { to } else { from };
        return [r, g, b, 0.0];
    }
    let channel = |i: usize| (from[i] * from[3] * (1.0 - t) + to[i] * to[3] * t) / alpha;
    [channel(0), channel(1), channel(2), alpha]
}
/** A shadow's literal color faded by the item's residual alpha. */
fn shadow_color(value: &str, alpha: f64) -> Option<Retained<NSColor>> {
    literal_color(value).map(|color| super::faded(&color, alpha))
}
/** sRGB components of an AppKit color, or opaque black when it has no RGB form. */
fn components(color: &NSColor) -> [f64; 4] {
    match color.colorUsingColorSpace(&objc2_app_kit::NSColorSpace::sRGBColorSpace()) {
        Some(color) => [
            color.redComponent(),
            color.greenComponent(),
            color.blueComponent(),
            color.alphaComponent(),
        ],
        None => [0.0, 0.0, 0.0, color.alphaComponent()],
    }
}
/** Draws with a CoreGraphics sRGB gradient built from resolved components and locations. */
fn with_gradient(colors: &[f64], locations: &[f64], draw: impl FnOnce(&CGGradient)) {
    // SAFETY: the sRGB name is Apple's constant; `colors` holds four components per location.
    let gradient = unsafe {
        CGColorSpace::with_name(Some(kCGColorSpaceSRGB)).and_then(|space| {
            CGGradient::with_color_components(
                Some(&space),
                colors.as_ptr(),
                locations.as_ptr(),
                locations.len(),
            )
        })
    };
    if let Some(gradient) = gradient {
        draw(&gradient);
    }
}
/** One rectangle constructor in document points. */
fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}

#[cfg(test)]
mod tests {
    use super::premultiplied;

    /** A transparent stop fades from its neighbor's color, never through grey. */
    #[test]
    fn transparent_stop_takes_its_neighbors_color() {
        let white = [1.0, 1.0, 1.0, 1.0];
        let stops = premultiplied(&[([0.0; 4], 0.0), (white, 1.0)]);
        assert_eq!(stops, vec![([1.0, 1.0, 1.0, 0.0], 0.0), (white, 1.0)]);
    }

    /** A transparent middle stop splits into each side's color at the same position. */
    #[test]
    fn transparent_middle_stop_splits() {
        let red = [1.0, 0.0, 0.0, 1.0];
        let blue = [0.0, 0.0, 1.0, 1.0];
        let stops = premultiplied(&[(red, 0.0), ([0.0; 4], 0.5), (blue, 1.0)]);
        assert_eq!(stops[1], ([1.0, 0.0, 0.0, 0.0], 0.5));
        assert_eq!(stops[2], ([0.0, 0.0, 1.0, 0.0], 0.5));
    }

    /** Partly translucent ends interpolate premultiplied: the opaque end's hue dominates. */
    #[test]
    fn translucent_segment_weights_by_alpha() {
        let stops = premultiplied(&[([1.0, 0.0, 0.0, 1.0], 0.0), ([0.0, 0.0, 1.0, 0.25], 1.0)]);
        let middle = stops[super::GRADIENT_STEPS / 2].0;
        assert!((middle[3] - 0.625).abs() < 1e-9);
        assert!((middle[0] - 0.8).abs() < 1e-9 && (middle[2] - 0.2).abs() < 1e-9);
    }
}
