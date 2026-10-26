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
/// How far toward the corner a quarter ellipse's Bezier control points sit (4/3 (sqrt 2 - 1)).
const BEZIER_ARC: f64 = 0.552_284_749_8;
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
    /// Vertical radii in the same order; equal to `radius` for circular corners.
    pub radius_y: [f64; 4],
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
            radius_y: item.radius_y.unwrap_or(item.radius),
        }
    }
    /// Grows (or with a negative amount shrinks) every edge; radii follow, never below zero.
    fn outset(&self, amount: f64) -> Self {
        Self {
            x: self.x - amount,
            y: self.y - amount,
            width: (self.width + amount * 2.0).max(0.0),
            height: (self.height + amount * 2.0).max(0.0),
            radius: std::array::from_fn(|i| self.spread_corner(i, amount)[0]),
            radius_y: std::array::from_fn(|i| self.spread_corner(i, amount)[1]),
        }
    }
    /// Corner `index` (`[rx, ry]`) grown by `amount`, as a spread shadow's (eye `spread_radius`).
    fn spread_corner(&self, index: usize, amount: f64) -> [f64; 2] {
        let corner = [self.radius[index], self.radius_y[index]];
        crate::document::spread_radius(corner, [self.width, self.height], amount)
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
                (self.radius[0] - left).max(0.0),
                (self.radius[1] - right).max(0.0),
                (self.radius[2] - right).max(0.0),
                (self.radius[3] - left).max(0.0),
            ],
            radius_y: [
                (self.radius_y[0] - top).max(0.0),
                (self.radius_y[1] - top).max(0.0),
                (self.radius_y[2] - bottom).max(0.0),
                (self.radius_y[3] - bottom).max(0.0),
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
    pub fn enter(&self, item: &DisplayItem, shift: f64) {
        CGContext::save_g_state(self.cg());
        // A sticky or fixed box moves with the viewport, clip and all.
        if shift != 0.0 {
            CGContext::translate_ctm(self.cg(), 0.0, shift);
        }
        if let Some(clip) = &item.clip {
            self.path(&Shape {
                x: clip.x,
                y: clip.y,
                width: clip.width,
                height: clip.height,
                radius: clip.radius,
                radius_y: clip.radius,
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
        CGContext::begin_path(self.cg());
        self.add(shape);
    }
    /** Adds a rounded rectangle to the current path as its own subpath. */
    fn add(&self, shape: &Shape) {
        let cg = self.cg();
        let Shape {
            x,
            y,
            width: w,
            height: h,
            radius: [tl, tr, br, bl],
            radius_y,
        } = *shape;
        if [tl, tr, br, bl] == [0.0; 4] && radius_y == [0.0; 4] {
            CGContext::add_rect(cg, rect(x, y, w, h));
            return;
        }
        if radius_y != shape.radius {
            return self.add_elliptical(shape);
        }
        CGContext::move_to_point(cg, x + tl, y);
        CGContext::add_arc_to_point(cg, x + w, y, x + w, y + h, tr);
        CGContext::add_arc_to_point(cg, x + w, y + h, x, y + h, br);
        CGContext::add_arc_to_point(cg, x, y + h, x, y, bl);
        CGContext::add_arc_to_point(cg, x, y, x + w, y, tl);
        CGContext::close_path(cg);
    }
    /** Adds a box whose corners are quarter ellipses `[rx, ry]`, each a cubic Bezier. */
    fn add_elliptical(&self, shape: &Shape) {
        let cg = self.cg();
        let (x, y, w, h) = (shape.x, shape.y, shape.width, shape.height);
        let ([tl, tr, br, bl], [vtl, vtr, vbr, vbl]) = (shape.radius, shape.radius_y);
        // Each corner: where the edge before it ends, the corner point, where the next begins.
        let corners = [
            ((x + w - tr, y), (x + w, y), (x + w, y + vtr)),
            ((x + w, y + h - vbr), (x + w, y + h), (x + w - br, y + h)),
            ((x + bl, y + h), (x, y + h), (x, y + h - vbl)),
            ((x, y + vtl), (x, y), (x + tl, y)),
        ];
        CGContext::move_to_point(cg, x + tl, y);
        for (start, corner, end) in corners {
            CGContext::add_line_to_point(cg, start.0, start.1);
            let control = |from: (f64, f64)| {
                (
                    from.0 + (corner.0 - from.0) * BEZIER_ARC,
                    from.1 + (corner.1 - from.1) * BEZIER_ARC,
                )
            };
            let (a, b) = (control(start), control(end));
            CGContext::add_curve_to_point(cg, a.0, a.1, b.0, b.1, end.0, end.1);
        }
        CGContext::close_path(cg);
    }
    /// Fills the ring between two rounded boxes (`background-clip: border-area`).
    pub fn fill_ring(&self, outer: &Shape, inner: &Shape, color: &NSColor) {
        color.setFill();
        self.path(outer);
        self.add(inner);
        CGContext::eo_fill_path(self.cg());
    }
    /// Fills a rounded box with an AppKit color.
    pub fn fill(&self, shape: &Shape, color: &NSColor) {
        color.setFill();
        self.path(shape);
        CGContext::fill_path(self.cg());
    }
    /// Saves the state and clips what follows to an item's background painting area: the
    /// rounded `background-clip` shape `area` (unless `border-area` paints the ring) and its
    /// `background_rect`, as the raster painter clips the color, gradients and image layers.
    /// Without it image layers spilled past rounded corners and `content-box` clips.
    pub fn clip_background(&self, area: &Shape, item: &DisplayItem) {
        CGContext::save_g_state(self.cg());
        if item.background_border_area {
            // The border's ring: inside the border box, outside the padding box.
            let shape = Shape::of(item);
            self.clip_shape(&shape);
            let ring = if item.border > 0.0 {
                [item.border; 4]
            } else {
                item.border_widths
            };
            self.outside(&shape.inset_sides(ring));
        } else {
            self.clip_shape(area);
        }
        if let Some([x, y, width, height]) = item.background_rect {
            let (radius, radius_y) = ([0.0; 4], [0.0; 4]);
            self.clip_shape(&Shape {
                x,
                y,
                width,
                height,
                radius,
                radius_y,
            });
        }
    }
    /// Restores the state `clip_background` saved.
    pub fn unclip(&self) {
        CGContext::restore_g_state(self.cg());
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
            // The shape is filled SHADOW_REACH to the left and its shadow offset back, so only
            // the blurred shadow lands on the page: filling in place painted the solid shape
            // too, a hard band wherever the box has no background to hide it (YouTube's cards).
            self.cast_from_afar(shadow.blur, &color);
            let mut cast = shape
                .outset(shadow.spread)
                .offset(shadow.x - SHADOW_REACH, shadow.y);
            let [x, _, width, _] = crate::document::sliced_shadow(
                [cast.x, cast.y, cast.width, cast.height],
                [shape.x, shape.width],
                item.slice_cut,
            );
            (cast.x, cast.width) = (x, width);
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
    /** A shadow offset that moves what is drawn SHADOW_REACH user units to the left back into
                place. CoreGraphics applies shadow offsets in device space whatever the transform, so the
                user-space shift is mapped through the current transform first. */
    fn cast_from_afar(&self, blur: f64, color: &NSColor) {
        let [r, g, b, a] = components(color);
        let cg_color = objc2_core_graphics::CGColor::new_srgb(r, g, b, a);
        let device = CGContext::user_space_to_device_space_transform(self.cg());
        let (dx, dy) = (device.a * SHADOW_REACH, device.b * SHADOW_REACH);
        CGContext::set_shadow_with_color(self.cg(), NSSize::new(dx, dy), blur, Some(&cg_color));
    }
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
            let origins = layer.tile().map_or(vec![(0.0, 0.0)], |tile| {
                tile.origins(shape.width, shape.height)
            });
            for (ox, oy) in origins {
                CGContext::save_g_state(self.cg());
                self.path(shape);
                CGContext::clip(self.cg());
                if let Some(tile) = layer.tile() {
                    let cell = rect(shape.x + ox, shape.y + oy, tile.width, tile.height);
                    CGContext::clip_to_rect(self.cg(), cell);
                }
                let (x, y) = (shape.x + ox, shape.y + oy);
                with_gradient(&colors, &locations, |gradient| {
                    if radial {
                        self.radial(gradient, x + a, y + b, c, d);
                    } else {
                        CGContext::draw_linear_gradient(
                            self.cg(),
                            Some(gradient),
                            NSPoint::new(x + a, y + b),
                            NSPoint::new(x + c, y + d),
                            CGGradientDrawingOptions::DrawsBeforeStartLocation
                                | CGGradientDrawingOptions::DrawsAfterEndLocation,
                        );
                    }
                });
                CGContext::restore_g_state(self.cg());
            }
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
