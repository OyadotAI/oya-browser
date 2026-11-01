/** Resolved paint values carried by display items: clips, shadows, gradients and opacity groups. */
use super::{DecorationLine, DisplayItem, DisplayOutline};
use serde::{Deserialize, Serialize};

/// An absolute clip rectangle in document points, rounded when any radius is positive.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ClipRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Corner radii top-left, top-right, bottom-right, bottom-left of the clipping box.
    #[serde(default, skip_serializing_if = "is_square")]
    pub radius: [f64; 4],
}
impl ClipRect {
    /// The overlap of two clips; it keeps a radius only when one clip lies wholly inside the other.
    pub fn intersect(&self, other: &ClipRect) -> ClipRect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = (self.x + self.width).min(other.x + other.width);
        let bottom = (self.y + self.height).min(other.y + other.height);
        let result = ClipRect {
            x,
            y,
            width: (right - x).max(0.0),
            height: (bottom - y).max(0.0),
            radius: [0.0; 4],
        };
        let radius = [self, other]
            .into_iter()
            .find(|clip| clip.same_box(&result))
            .map(|clip| clip.radius)
            .unwrap_or_default();
        ClipRect { radius, ..result }
    }
    /** Geometry equality ignoring radius. */
    fn same_box(&self, other: &ClipRect) -> bool {
        self.x == other.x
            && self.y == other.y
            && self.width == other.width
            && self.height == other.height
    }
}

/// One resolved CSS box shadow in points; the color is a validated literal.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DisplayShadow {
    pub x: f64,
    pub y: f64,
    pub blur: f64,
    pub spread: f64,
    pub color: String,
    /// Paints inside the padding box instead of outside the border box.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub inset: bool,
    /// A `text-shadow`: cast by the item's text runs, never by its box. Kept in the same list
    /// so the many display item builders need no new field; box painters skip these.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub text: bool,
}

/// A shadow shape's corner `[rx, ry]` of a `size` box grown by `spread` (shrunk when negative),
/// per CSS Backgrounds 3 corner shaping as Blink's `ComputeOutsetAdjustedBorderRadius` does it:
/// a square corner stays square, and a radius smaller than a positive spread grows by
/// `spread * (1 - (1 - ratio)^3 * (1 - coverage^3))`, so a slightly rounded box does not cast a
/// fully rounded shadow while a circle's stays a circle. Both painters share it.
pub fn spread_radius(corner: [f64; 2], size: [f64; 2], spread: f64) -> [f64; 2] {
    if corner[0] <= 0.0 || corner[1] <= 0.0 {
        return [0.0; 2];
    }
    if spread <= 0.0 {
        return corner.map(|r| (r + spread).max(0.0));
    }
    let coverage = 2.0 * (corner[0] / size[0]).min(corner[1] / size[1]);
    corner.map(|r| {
        if r > spread || coverage > 1.0 {
            return r + spread;
        }
        r + spread * (1.0 - (1.0 - r / spread).powi(3) * (1.0 - coverage.powi(3)))
    })
}

/// A gradient stop with its position resolved to 0..1 along the gradient line, nondecreasing.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GradientStop {
    pub color: String,
    pub position: f64,
}

/// One background layer with geometry relative to the item's top-left corner.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum DisplayBackground {
    /// Linear gradient from (x0, y0) to (x1, y1), extended past both ends.
    Linear {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        stops: Vec<GradientStop>,
        /// Where the gradient paints when `background-size`, `-position` or `-repeat` place it;
        /// its geometry is then relative to the tile's top-left. None fills the box.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tile: Option<GradientTile>,
    },
    /// Elliptical gradient centered at (cx, cy) with radii rx and ry.
    Radial {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        stops: Vec<GradientStop>,
        /// As for `Linear`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tile: Option<GradientTile>,
    },
    /// An image layer at its absolute or authored URL, drawn once the app fetched its bytes.
    Image {
        url: String,
        /// How the image scales in the box.
        #[serde(default)]
        size: ImageSize,
        /// Offsets x px, x percent, y px, y percent; percentages place the image's own point there.
        #[serde(default)]
        position: [f64; 4],
        /// Tiles horizontally and vertically.
        #[serde(default)]
        repeat: [bool; 2],
        /// Per axis, a repeating layer spaces whole tiles across the positioning area
        /// (`background-repeat: space`); a painter that ignores it tiles them flush.
        #[serde(default, skip_serializing_if = "is_flush")]
        space: [bool; 2],
        /// Per axis, a repeating layer rescales its tile so whole tiles fill the area (`round`).
        #[serde(default, skip_serializing_if = "is_flush")]
        round: [bool; 2],
        /// `background-attachment: fixed`: the positioning area `[x, y, width, height]` in
        /// document points (the viewport) that replaces the box's own; painters still clip
        /// the layer to the box. None positions it in the box.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        area: Option<[f64; 4]>,
        /// The key the app stores this layer's image under when it is not the item's own
        /// identity: an element's later image layers and the canvas's (`layer_key`,
        /// `canvas_key`). None is the first layer, under the item's identity.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<usize>,
    },
}
/// One gradient tile, relative to the item's top-left: painted there, and repeated from there
/// across the box along each axis that tiles.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GradientTile {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Tiles horizontally and vertically.
    pub repeat: [bool; 2],
    /// Space left between repeated tiles per axis (`background-repeat: space`).
    #[serde(default, skip_serializing_if = "is_gapless")]
    pub gap: [f64; 2],
}
/** No space between tiles, left out of serialized items. */
fn is_gapless(gap: &[f64; 2]) -> bool {
    *gap == [0.0; 2]
}
impl GradientTile {
    /// Maps a point relative to the item's top-left into the tile it falls in, relative to that
    /// tile's top-left; None where no tile paints (outside a non-repeating axis, or an empty tile).
    pub fn local(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let axis = |at: f64, start: f64, size: f64, axis: usize| {
            let offset = at - start;
            let offset = match self.repeat[axis] {
                true => offset.rem_euclid(size + self.gap[axis]),
                false => offset,
            };
            (size > 0.0 && (0.0..size).contains(&offset)).then_some(offset)
        };
        Some((
            axis(x, self.x, self.width, 0)?,
            axis(y, self.y, self.height, 1)?,
        ))
    }
    /// The top-left corners of the tiles that reach into a box `width` by `height`, relative to
    /// its top-left, at most `GRADIENT_TILES` of them, for painters that draw tile by tile.
    pub fn origins(&self, width: f64, height: f64) -> Vec<(f64, f64)> {
        let axis = |start: f64, size: f64, extent: f64, axis: usize| -> Vec<f64> {
            let step = size + self.gap[axis];
            if !self.repeat[axis] || step <= 0.0 {
                return vec![start];
            }
            let first = start - ((start / step).ceil() * step);
            let count = ((extent - first) / step)
                .ceil()
                .clamp(0.0, GRADIENT_TILES as f64);
            (0..count as usize)
                .map(|i| first + i as f64 * step)
                .collect()
        };
        let xs = axis(self.x, self.width, width, 0);
        let ys = axis(self.y, self.height, height, 1);
        ys.iter()
            .flat_map(|y| xs.iter().map(move |x| (*x, *y)))
            .take(GRADIENT_TILES)
            .collect()
    }
}
/// Gradient tiles a painter draws one by one at most; past it the rest of the box goes unpainted.
pub const GRADIENT_TILES: usize = 4096;
/** An outer shadow's rectangle `[x, y, width, height]` for a sliced inline fragment at
 * `[x, width]` whose left or right side is cut (`slice_cut`): under `box-decoration-break:
 * slice` the shadow is the whole box's, so on a cut side it runs on to the fragment's edge
 * instead of ending offset there (css-break-3 5.4). Both painters share it. */
pub fn sliced_shadow(rect: [f64; 4], [x, width]: [f64; 2], cut: [bool; 2]) -> [f64; 4] {
    let left = if cut[0] { x } else { rect[0] };
    let right = if cut[1] { x + width } else { rect[0] + rect[2] };
    [left, rect[1], (right - left).max(0.0), rect[3]]
}
/** Neither axis set: the default `space` and `round`, left out of serialized items. */
pub(super) fn is_flush(axes: &[bool; 2]) -> bool {
    !axes[0] && !axes[1]
}
/// A background image's used size rule, resolved against the box except where the image's own size decides.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ImageSize {
    /// The image's natural size.
    #[default]
    Auto,
    /// Scaled to cover the box.
    Cover,
    /// Scaled to fit in the box.
    Contain,
    /// Width and height in px; a missing one keeps the aspect ratio.
    Fixed {
        width: Option<f64>,
        height: Option<f64>,
    },
}

impl DisplayBackground {
    /// Gradient geometry for painters: radial or not, (x0, y0, x1, y1) or (cx, cy, rx, ry), and stops.
    /// None for image layers.
    pub fn gradient(&self) -> Option<(bool, [f64; 4], &[GradientStop])> {
        match self {
            Self::Linear {
                x0,
                y0,
                x1,
                y1,
                stops,
                ..
            } => Some((false, [*x0, *y0, *x1, *y1], stops)),
            Self::Radial {
                cx,
                cy,
                rx,
                ry,
                stops,
                ..
            } => Some((true, [*cx, *cy, *rx, *ry], stops)),
            Self::Image { .. } => None,
        }
    }
    /// The layer painted in `tile` (a gradient whose geometry spans it); an image is unchanged.
    pub fn with_tile(mut self, placed: GradientTile) -> Self {
        if let Self::Linear { tile, .. } | Self::Radial { tile, .. } = &mut self {
            *tile = Some(placed);
        }
        self
    }
    /// A gradient's tile, when size, position or repeat place it; None fills the box.
    pub fn tile(&self) -> Option<GradientTile> {
        match self {
            Self::Linear { tile, .. } | Self::Radial { tile, .. } => *tile,
            Self::Image { .. } => None,
        }
    }
}

/// A translucent stacking context the painter composites as one group.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OpacityGroup {
    /// Identity of the element that created the group, stable within one display list.
    pub id: usize,
    /// The group's own opacity, applied once to its composited content.
    pub opacity: f64,
}

/// The item's painted bounding box in document points, after its transform.
#[allow(dead_code)]
pub fn painted_bounds(item: &DisplayItem) -> ClipRect {
    let rect = ClipRect {
        x: item.x,
        y: item.y,
        width: item.width,
        height: item.height,
        radius: [0.0; 4],
    };
    transform_rect(&rect, item.transform)
}
/** The axis-aligned bounding box of a rectangle under an affine map. */
pub(super) fn transform_rect(rect: &ClipRect, matrix: Option<[f64; 6]>) -> ClipRect {
    let Some([a, b, c, d, e, f]) = matrix else {
        return *rect;
    };
    let corners = [
        (rect.x, rect.y),
        (rect.x + rect.width, rect.y),
        (rect.x, rect.y + rect.height),
        (rect.x + rect.width, rect.y + rect.height),
    ]
    .map(|(x, y)| (a * x + c * y + e, b * x + d * y + f));
    let left = corners.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let right = corners
        .iter()
        .map(|p| p.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let top = corners.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let bottom = corners
        .iter()
        .map(|p| p.1)
        .fold(f64::NEG_INFINITY, f64::max);
    ClipRect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
        radius: rect.radius,
    }
}
/// How a sticky or fixed box follows the viewport's scroll. The display is painted at scroll 0;
/// at scroll `s` painters move the box down by `shift(s)`, as Blink's sticky and fixed paint
/// offsets do. A fixed box is a pin with no `top`, no start and unlimited room.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    /// The `top` inset it sticks at, from the viewport's top edge.
    pub top: f64,
    /// Its unstuck top edge in document coordinates.
    pub start: f64,
    /// How far its containing block lets it travel.
    pub room: f64,
}
impl Pin {
    /// The sticky offset at viewport scroll `scroll`.
    pub fn offset(&self, scroll: f64) -> f64 {
        (scroll.max(0.0) + self.top - self.start).clamp(0.0, self.room.max(0.0))
    }
    /// How much further than at scroll 0 the box moves at `scroll`.
    pub fn shift(&self, scroll: f64) -> f64 {
        self.offset(scroll) - self.offset(0.0)
    }
}
/// A scroll container whose contents overflow it: its padding box where it paints, its scroll
/// range and the offset its contents are painted at. Hosts hit-test wheels against these.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScrollArea {
    /// The container's node identity, the key hosts scroll it by.
    pub local_id: usize,
    /// Padding box in document coordinates, after any outer scrolling.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Furthest offsets along each axis (scroll size less client size).
    pub max_x: f64,
    pub max_y: f64,
    /// The clamped offsets this display was painted at.
    pub scroll_x: f64,
    pub scroll_y: f64,
    /// Whether a person's wheel scrolls each axis: `auto` or `scroll`, not `hidden`.
    pub user_x: bool,
    pub user_y: bool,
}
/// The scroll container a wheel at document point (`x`, `y`) moving by `delta` scrolls: the
/// innermost one under the point that can still move that way, as Blink's scroll chaining
/// bubbles from the innermost scroller. None leaves the wheel to the viewport.
/// ponytail: innermost is the smallest containing area, which nesting guarantees; a z-order
/// hit test is the upgrade for overlapping siblings.
pub fn wheel_target(areas: &[ScrollArea], x: f64, y: f64, delta: [f64; 2]) -> Option<usize> {
    let room = |user: bool, offset: f64, max: f64, delta: f64| {
        user && ((delta > 0.0 && offset < max) || (delta < 0.0 && offset > 0.0))
    };
    areas
        .iter()
        .filter(|area| {
            (area.x..area.x + area.width).contains(&x)
                && (area.y..area.y + area.height).contains(&y)
        })
        .filter(|area| {
            room(area.user_x, area.scroll_x, area.max_x, delta[0])
                || room(area.user_y, area.scroll_y, area.max_y, delta[1])
        })
        .min_by(|a, b| (a.width * a.height).total_cmp(&(b.width * b.height)))
        .map(|area| area.local_id)
}
/// Agents target only what a person could see: not fully transparent and not clipped away.
#[allow(dead_code)]
pub fn agent_visible(item: &DisplayItem) -> bool {
    if item.opacity <= 0.0 {
        return false;
    }
    let Some(clip) = item.clip else {
        return true;
    };
    let bounds = painted_bounds(item);
    overlaps(bounds.x, bounds.width, clip.x, clip.width)
        && overlaps(bounds.y, bounds.height, clip.y, clip.height)
}
/** Positive extents need real overlap; a zero extent (an empty inline) only needs to lie inside. */
#[allow(dead_code)]
fn overlaps(start: f64, length: f64, clip: f64, extent: f64) -> bool {
    if length <= 0.0 {
        start >= clip && start <= clip + extent
    } else {
        start < clip + extent && start + length > clip && extent > 0.0
    }
}

/** Square corners and uniform zero edges keep the established serialized shape. */
pub(super) fn is_square(values: &[f64; 4]) -> bool {
    *values == [0.0; 4]
}
/** No per-side colors were authored. */
pub(super) fn no_colors(values: &[Option<String>; 4]) -> bool {
    values.iter().all(Option::is_none)
}
/** The root stacking layer is omitted. */
pub(super) fn is_zero(value: &i32) -> bool {
    *value == 0
}
/** No letter spacing is omitted. */
pub(super) fn is_zero_f64(value: &f64) -> bool {
    *value == 0.0
}
/** Regular weight is the default. */
pub(super) fn normal_weight() -> u16 {
    400
}
/** Regular weight is omitted. */
pub(super) fn is_normal_weight(value: &u16) -> bool {
    *value == 400
}
/** The native interface face is the default for older display lists. */
pub(super) fn is_system(family: &crate::platform::Family) -> bool {
    *family == crate::platform::Family::System
}
/** `object-fit: fill` is the CSS initial value and omitted from serialized items. */
pub(super) fn is_fill(fit: &crate::document::style::ObjectFit) -> bool {
    *fit == crate::document::style::ObjectFit::Fill
}
/** Initial `object-position` (`50% 50%`). */
pub(super) fn default_object_position() -> [f64; 4] {
    [0.0, 50.0, 0.0, 50.0]
}
/** Centered `object-position` is omitted from serialized items. */
pub(super) fn is_center_object_position(position: &[f64; 4]) -> bool {
    *position == default_object_position()
}

/** The solid decoration line is the default and omitted from serialized items. */
pub(super) fn is_solid_line(style: &crate::document::style::DecorationStyle) -> bool {
    *style == crate::document::style::DecorationStyle::Solid
}

/**
 * The narrowest line a decoration or outline paints, in points. A thinner authored width (0.01px)
 * would vanish at device resolution and make painters draw a stroke of near-zero size, so it is
 * raised to this; zero still means no line at all.
 */
pub(super) const MIN_LINE_WIDTH: f64 = 0.5;

/** The decoration line's paint when it differs from the solid default; None keeps the painter's own. */
pub(super) fn decoration_line(style: &crate::document::ComputedStyle) -> Option<DecorationLine> {
    let line = DecorationLine {
        style: style.decoration_style,
        thickness: style.decoration_thickness.map(|t| t.max(MIN_LINE_WIDTH)),
        offset: style.underline_offset,
    };
    (line != DecorationLine::default()).then_some(line)
}

/** The outline ring a box paints: a line style other than `none` and a positive width. */
pub(super) fn outline(style: &crate::document::ComputedStyle) -> Option<DisplayOutline> {
    let kind = style.outline_style?;
    (style.outline_width > 0.0).then(|| DisplayOutline {
        style: kind,
        width: style.outline_width.max(MIN_LINE_WIDTH),
        color: style.outline_color.clone(),
        offset: style.outline_offset,
    })
}

/** Effect values reach CoreGraphics only when finite and bounded, like every other coordinate. */
pub(super) fn validate(item: &DisplayItem) -> crate::protocol::EngineResult<()> {
    use super::validation::offset as bounded;
    let mut values = Vec::new();
    values.extend(item.radius);
    values.extend(item.border_widths);
    values.push(item.letter_spacing);
    values.push(item.word_spacing);
    values.extend(item.object_position);
    if let Some(line) = item.decoration {
        values.extend([line.thickness.unwrap_or(0.0), line.offset.unwrap_or(0.0)]);
    }
    if let Some(outline) = &item.outline {
        values.extend([outline.width, outline.offset]);
    }
    if let Some(content) = item.content_box {
        values.extend(content);
    }
    for shadow in &item.shadows {
        values.extend([shadow.x, shadow.y, shadow.blur, shadow.spread]);
    }
    for layer in &item.backgrounds {
        match layer {
            DisplayBackground::Linear {
                x0,
                y0,
                x1,
                y1,
                stops,
                tile,
            } => {
                values.extend([*x0, *y0, *x1, *y1]);
                values.extend(stops.iter().map(|stop| stop.position));
                values.extend(tile.iter().flat_map(|t| [t.x, t.y, t.width, t.height]));
            }
            DisplayBackground::Radial {
                cx,
                cy,
                rx,
                ry,
                stops,
                tile,
            } => {
                values.extend([*cx, *cy, *rx, *ry]);
                values.extend(stops.iter().map(|stop| stop.position));
                values.extend(tile.iter().flat_map(|t| [t.x, t.y, t.width, t.height]));
            }
            DisplayBackground::Image { size, position, .. } => {
                values.extend(position);
                if let ImageSize::Fixed { width, height } = size {
                    values.extend(width.iter().chain(height.iter()));
                }
            }
        }
    }
    if let Some(clip) = item.clip {
        values.extend([clip.x, clip.y, clip.width, clip.height]);
        values.extend(clip.radius);
    }
    values.extend(item.transform.unwrap_or_default());
    values.extend(item.quad.iter().flatten().flatten().copied());
    for value in values {
        bounded(value)?;
    }
    if let Some(clip) = item.clip
        && (clip.width < 0.0 || clip.height < 0.0)
    {
        return Err(crate::protocol::EngineError::invalid(
            "Invalid rendered clip.",
        ));
    }
    if item
        .stacking_opacity
        .iter()
        .any(|group| !(0.0..=1.0).contains(&group.opacity))
    {
        return Err(crate::protocol::EngineError::invalid(
            "Invalid rendered opacity.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /** A plain clip with no radius. */
    fn clip(x: f64, y: f64, width: f64, height: f64) -> ClipRect {
        ClipRect {
            x,
            y,
            width,
            height,
            radius: [0.0; 4],
        }
    }

    /** Intersection keeps the inner clip's radius only when that clip is the result. */
    #[test]
    fn intersection_keeps_inner_radius() {
        let outer = clip(0.0, 0.0, 100.0, 100.0);
        let inner = ClipRect {
            radius: [4.0; 4],
            ..clip(10.0, 10.0, 20.0, 20.0)
        };
        assert_eq!(outer.intersect(&inner), inner);
        let partial = outer.intersect(&ClipRect {
            radius: [4.0; 4],
            ..clip(90.0, 90.0, 20.0, 20.0)
        });
        assert_eq!(partial, clip(90.0, 90.0, 10.0, 10.0));
        assert_eq!(outer.intersect(&clip(200.0, 0.0, 5.0, 5.0)).width, 0.0);
    }

    /** Clipped-out and fully transparent items are not agent-visible; partly visible ones are. */
    #[test]
    fn agent_visibility_follows_clip_and_opacity() {
        let (document, display) = {
            let document =
                crate::document::compute_styles(&crate::document::parse("<p>x</p>").unwrap(), &[])
                    .unwrap();
            let display = crate::document::render(&document, 640.0).unwrap();
            (document, display)
        };
        assert!(!document.nodes.is_empty());
        let mut item = display.items[0].clone();
        assert!(agent_visible(&item));
        item.clip = Some(clip(item.x + item.width + 10.0, item.y, 10.0, 10.0));
        assert!(!agent_visible(&item));
        item.clip = Some(clip(item.x, item.y, 1.0, 1.0));
        assert!(agent_visible(&item));
        item.opacity = 0.0;
        assert!(!agent_visible(&item));
    }
}
