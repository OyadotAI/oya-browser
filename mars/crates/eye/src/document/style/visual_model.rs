/** Computed visual values the painter consumes: positioning, clipping, decoration and text shaping. */
use serde::{Deserialize, Serialize};
/// The CSS positioning scheme; layout decides how far each scheme is honored.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Position {
    /// Normal flow.
    #[default]
    Static,
    /// Offset from its normal-flow place.
    Relative,
    /// Placed against the nearest positioned ancestor.
    Absolute,
    /// Placed against the viewport.
    Fixed,
    /// Normal flow until scrolled past its inset.
    Sticky,
}
/// Overflow handling, with overflow-x and overflow-y folded into the stricter one.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Overflow {
    /// Content paints outside the box.
    #[default]
    Visible,
    /// Scrolls only when needed.
    Auto,
    /// Always a scroll container.
    Scroll,
    /// Clipped, still programmatically scrollable.
    Hidden,
    /// Clipped without a scroll container.
    Clip,
}
/// Text case mapping applied when painting, inherited like CSS.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TextTransform {
    /// Source case.
    #[default]
    None,
    /// Every letter upper case.
    Uppercase,
    /// Every letter lower case.
    Lowercase,
    /// First letter of each word upper case.
    Capitalize,
}
/// Whitespace collapsing and wrapping, inherited like CSS.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WhiteSpace {
    /// Collapse spaces and wrap.
    #[default]
    Normal,
    /// Keep spaces and newlines, never wrap.
    Pre,
    /// Collapse spaces, never wrap.
    Nowrap,
    /// Keep spaces and newlines, wrap.
    PreWrap,
    /// Collapse spaces, keep newlines, wrap.
    PreLine,
}
/// The generic face the painter picks; author family names map onto one of these.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FontFamily {
    /// Chromium's default face (Times).
    #[default]
    Serif,
    /// Helvetica/Arial-like faces.
    SansSerif,
    /// Fixed-pitch faces such as code and pre.
    Monospace,
    /// The platform interface face.
    SystemUi,
}
/// One `box-shadow` layer in CSS pixels; `color` is a validated literal.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct BoxShadow {
    /// Horizontal offset.
    pub x: f64,
    /// Vertical offset.
    pub y: f64,
    /// Nonnegative blur radius.
    pub blur: f64,
    /// Spread distance, may be negative.
    pub spread: f64,
    /// Literal color (`#rrggbb`, `#rrggbbaa` or a palette name).
    pub color: String,
    /// Inner shadow instead of an outer one.
    pub inset: bool,
    /// The color is `currentcolor` (authored or omitted): it computes to the keyword, so an
    /// inheriting box paints its own `color`, not `color` here, which is only its fallback.
    #[serde(default)]
    pub current: bool,
}
/// One gradient color stop; position is a fraction of the gradient line when authored in percent.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ColorStop {
    /// Literal color.
    pub color: String,
    /// 0..1 when authored as a percentage; None lets the painter distribute stops evenly.
    pub position: Option<f64>,
    /// A length part of the position in px, added to `position` of the gradient line once the
    /// painter knows the line's length (`10px`, `calc(100% - 4px)`); None for a bare percentage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub px: Option<f64>,
}
/// One `background-image` layer, first listed paints on top as in CSS.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundLayer {
    /// A linear gradient; angle in degrees with CSS's convention (`to right` is 90).
    Linear {
        /// Gradient line direction in degrees.
        angle: f64,
        /// Ordered stops.
        stops: Vec<ColorStop>,
        /// `to <corner>` as (x, y) signs, right and bottom positive: the angle then depends on
        /// the box, so the 50% line joins the two other corners (CSS Images 3.1.1).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        corner: Option<[i8; 2]>,
        /// `repeating-linear-gradient`: the stop pattern tiles along the whole line.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        repeating: bool,
    },
    /// A radial gradient with its ending shape, size and center.
    Radial {
        /// Ordered stops.
        stops: Vec<ColorStop>,
        /// Shape, size and center; the default is a centered farthest-corner ellipse.
        #[serde(default, skip_serializing_if = "RadialShape::is_default")]
        shape: RadialShape,
        /// `repeating-radial-gradient`: the stop pattern tiles outward along the ray.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        repeating: bool,
    },
    /// An image the painter may fetch; the URL is as authored, relative to its stylesheet.
    Image {
        /// Authored URL text.
        url: String,
    },
}
/// A radial gradient's ending shape: `circle` or ellipse, its size rule, and its center.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct RadialShape {
    /// `circle` rather than the default ellipse.
    pub circle: bool,
    /// How far the ending shape reaches.
    pub size: RadialSize,
    /// Center as `background-position` offsets; initial `center`.
    pub center: [Extent; 2],
}
impl Default for RadialShape {
    fn default() -> Self {
        let half = Extent {
            px: 0.0,
            percent: 50.0,
        };
        Self {
            circle: false,
            size: RadialSize::FarthestCorner,
            center: [half, half],
        }
    }
}
impl RadialShape {
    /// Serialization skips the initial shape, so older consumers see the same layers.
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
    /// Center and radii (cx, cy, rx, ry) in px within a `width` x `height` box, per CSS Images.
    pub fn geometry(&self, width: f64, height: f64) -> [f64; 4] {
        let cx = self.center[0].px + width * self.center[0].percent / 100.0;
        let cy = self.center[1].px + height * self.center[1].percent / 100.0;
        let (near_x, far_x) = (
            (cx).abs().min((width - cx).abs()),
            (cx).abs().max((width - cx).abs()),
        );
        let (near_y, far_y) = (
            (cy).abs().min((height - cy).abs()),
            (cy).abs().max((height - cy).abs()),
        );
        let [rx, ry] = match self.size {
            RadialSize::ClosestSide => self.sides(near_x, near_y),
            RadialSize::FarthestSide => self.sides(far_x, far_y),
            RadialSize::ClosestCorner => self.corner(near_x, near_y),
            RadialSize::FarthestCorner => self.corner(far_x, far_y),
            RadialSize::Explicit(x, y) => {
                let rx = x.px + width * x.percent / 100.0;
                [
                    rx,
                    if self.circle {
                        rx
                    } else {
                        y.px + height * y.percent / 100.0
                    },
                ]
            }
        };
        [cx, cy, rx.max(0.0), ry.max(0.0)]
    }
    /// Radii touching sides `x` and `y` away; a circle takes the nearer (closest) or farther one.
    fn sides(&self, x: f64, y: f64) -> [f64; 2] {
        match (self.circle, self.size) {
            (true, RadialSize::ClosestSide) => [x.min(y); 2],
            (true, _) => [x.max(y); 2],
            _ => [x, y],
        }
    }
    /// Radii passing through the corner `x`, `y` away, keeping the side-ratio for ellipses.
    fn corner(&self, x: f64, y: f64) -> [f64; 2] {
        if self.circle {
            [x.hypot(y); 2]
        } else {
            [x * std::f64::consts::SQRT_2, y * std::f64::consts::SQRT_2]
        }
    }
}
/// A radial gradient's size: a keyword or explicit radii (one length for a circle).
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum RadialSize {
    /// To the nearest side.
    ClosestSide,
    /// To the farthest side.
    FarthestSide,
    /// Through the nearest corner.
    ClosestCorner,
    /// Through the farthest corner, the initial size.
    FarthestCorner,
    /// Horizontal and vertical radii; a circle uses the first.
    Explicit(Extent, Extent),
}
/// A length that may be a percentage of its containing block, resolved only at layout time.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Extent {
    /// Absolute part in px.
    pub px: f64,
    /// Percentage part, 0 for a plain length.
    pub percent: f64,
}
impl Extent {
    /// Pixels against `basis`; a percentage without a definite basis behaves as `auto` (None).
    pub fn resolve(self, basis: Option<f64>) -> Option<f64> {
        if self.percent == 0.0 {
            return Some(self.px);
        }
        basis.map(|basis| self.px + basis * self.percent / 100.0)
    }
    /// As `resolve`, for a box size: a mixed `calc()` such as `100% - 560px` clamps at zero.
    pub fn resolve_size(self, basis: Option<f64>) -> Option<f64> {
        self.resolve(basis).map(|value| value.max(0.0))
    }
}
/// The CSS table box a non-table element asks for with `display`, so `display: table` divs lay
/// their `table-cell` children out side by side as Chromium does instead of stacking them.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TableBox {
    /// Not a CSS table part.
    #[default]
    None,
    /// `display: table`: lays out its rows or cells as a grid.
    Table,
    /// `display: table-row`.
    Row,
    /// `display: table-cell`.
    Cell,
    /// `display: table-caption`: spans the table above or below its grid.
    Caption,
    /// `display: table-row-group`: a section of rows; a block outside a table.
    RowGroup,
    /// `display: table-header-group`: the section a table places first.
    HeaderGroup,
    /// `display: table-footer-group`: the section a table places last.
    FooterGroup,
    /// `display: table-column` or `table-column-group`: no box of its own in the grid.
    Column,
}
/// `float` sides; floats leave normal flow and shorten the line boxes beside them.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Float {
    /// In flow.
    #[default]
    None,
    /// Hugs the left edge.
    Left,
    /// Hugs the right edge.
    Right,
}
/// `clear` sides a block moves below.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Clear {
    /// Sits beside floats.
    #[default]
    None,
    /// Below left floats.
    Left,
    /// Below right floats.
    Right,
    /// Below every float.
    Both,
}
/// Vertical placement of an inline-level box or table cell content within its line or row.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum VerticalAlign {
    /// Baselines line up.
    #[default]
    Baseline,
    /// Centered on the parent's baseline plus half its x-height.
    Middle,
    /// Top of the line box.
    Top,
    /// Bottom of the line box.
    Bottom,
    /// Top of the parent's text.
    TextTop,
    /// Bottom of the parent's text.
    TextBottom,
    /// Lowered for subscripts.
    Sub,
    /// Raised for superscripts.
    Super,
}
/// List marker kinds Eye paints.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ListStyle {
    /// Filled bullet.
    #[default]
    Disc,
    /// Hollow bullet.
    Circle,
    /// Filled square.
    Square,
    /// Decimal number and a period.
    Decimal,
    /// No marker.
    None,
}
/// `background-size` of image layers.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundSize {
    /// The image's own size.
    #[default]
    Auto,
    /// Scaled to cover the box.
    Cover,
    /// Scaled to fit inside the box.
    Contain,
    /// Width and height; None is auto.
    Explicit(Option<Extent>, Option<Extent>),
}
/// A `url()` mask image: the painter fills the box's background through the image's alpha.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct MaskLayer {
    /// Authored URL text, relative to its stylesheet unless rebased.
    pub url: String,
    /// `mask-size`, as `background-size`.
    pub size: BackgroundSize,
    /// `mask-position`, as `background-position`; initial `0% 0%`.
    pub position: [Extent; 2],
    /// `mask-repeat`, as `background-repeat`.
    pub repeat: BackgroundRepeat,
}
/// `background-repeat` of image layers.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BackgroundRepeat {
    /// Tiled both ways.
    #[default]
    Repeat,
    /// Painted once.
    NoRepeat,
    /// Tiled horizontally.
    RepeatX,
    /// Tiled vertically.
    RepeatY,
    /// Each axis on its own, horizontal then vertical, when either one is `space` or `round`.
    Axes(RepeatStyle, RepeatStyle),
}
/// One axis of `background-repeat`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RepeatStyle {
    /// Tiled at the tile size.
    #[default]
    Repeat,
    /// Painted once.
    NoRepeat,
    /// As many whole tiles as fit, the leftover spread between them.
    Space,
    /// Tiles rescaled so a whole number fits.
    Round,
}
/// One grid track sizing function: `min` and `max` of `minmax()`, equal for a plain size.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct Track {
    /// Lower bound.
    pub min: TrackSize,
    /// Upper bound.
    pub max: TrackSize,
    /// Which content keyword an `Auto` bound was, and a `fit-content()` limit.
    #[serde(default, skip_serializing_if = "TrackContent::is_auto")]
    pub content: TrackContent,
}
/// The intrinsic keywords behind a track's `TrackSize::Auto` bounds: track sizing treats
/// `auto`, `min-content`, `max-content` and `fit-content()` differently (css-grid-2 12.5).
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct TrackContent {
    /// The minimum's keyword.
    pub min: ContentKeyword,
    /// The maximum's keyword.
    pub max: ContentKeyword,
    /// `fit-content(limit)`: an `auto` minimum and a max-content maximum capped at `limit`.
    pub fit_limit: Option<Extent>,
}
impl TrackContent {
    /// Plain `auto` bounds, the default.
    pub fn is_auto(&self) -> bool {
        *self == Self::default()
    }
}
/// An intrinsic track sizing keyword.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ContentKeyword {
    /// `auto`.
    #[default]
    Auto,
    /// `min-content`.
    MinContent,
    /// `max-content`.
    MaxContent,
}
/// A single track size.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TrackSize {
    /// Fixed length or percentage of the grid's content box.
    Length(Extent),
    /// Share of the leftover space.
    Fr(f64),
    /// Content sized.
    Auto,
    /// `max(a, b)` of a length and a percentage, which only the grid's width can decide.
    Larger(Extent, Extent),
    /// `min(a, b)` of a length and a percentage.
    Smaller(Extent, Extent),
    /// A math function mixing percentages and lengths beyond one `min`/`max`, kept as the
    /// template's expression at this index and decided by the grid's size at layout.
    Expr(u16),
}
impl TrackSize {
    /// Pixels of a fixed size against the grid's `basis`; None for `fr`, content sizes and unresolved percentages.
    pub fn length(self, basis: Option<f64>) -> Option<f64> {
        match self {
            Self::Length(l) => l.resolve(basis),
            Self::Larger(a, b) => Some(a.resolve(basis)?.max(b.resolve(basis)?)),
            Self::Smaller(a, b) => Some(a.resolve(basis)?.min(b.resolve(basis)?)),
            Self::Fr(_) | Self::Auto | Self::Expr(_) => None,
        }
    }
}
/// `grid-template-columns` or `-rows`: explicit tracks, plus one `repeat(auto-fill|auto-fit)` group.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct GridTemplate {
    /// Tracks in order; the auto-repeat group is inserted at `repeat_at`.
    pub tracks: Vec<Track>,
    /// Tracks repeated as many times as fit.
    pub repeat: Vec<Track>,
    /// Index in `tracks` where the auto-repeat group goes.
    pub repeat_at: usize,
    /// The group is `auto-fit`: its repetitions no item occupies collapse.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub repeat_fit: bool,
    /// `[name]` line names with their one-based line numbers, in source order, an auto-repeat
    /// group counted as one repetition.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub names: Vec<(String, u16)>,
    /// Line names inside the auto-repeat group, numbered from 1 at the line before its first
    /// track; layout repeats them with the group (`line_names`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repeat_names: Vec<(String, u16)>,
    /// Math-function track sizes `TrackSize::Expr` indexes, units other than `%` already px.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exprs: Vec<String>,
    /// The `subgrid` keyword: this axis takes its tracks and line names from the parent grid.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub subgrid: bool,
    /// The number of `[...]` line-name lists after `subgrid`, which an auto-placed subgrid's
    /// span follows (css-grid-2 9.1: one less than the lists).
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub subgrid_lines: u16,
}
/// Whether a count is zero, for skipping it when serializing.
fn is_zero_u16(value: &u16) -> bool {
    *value == 0
}
impl GridTemplate {
    /// The line names of the grid once its auto-repeat group is repeated into `explicit` tracks:
    /// the group's names once per repetition, and the names after it moved past them
    /// (css-grid-2 7.2.3.2). Without a group these are `names`.
    pub fn line_names(&self, explicit: usize) -> Vec<(String, u16)> {
        let group = self.repeat.len();
        if group == 0 {
            return self.names.clone();
        }
        let times = (explicit.saturating_sub(self.tracks.len()) / group).max(1);
        let first = self.repeat_at + 1;
        let shift = (times - 1) * group;
        let moved = |line: u16| {
            let line = usize::from(line);
            let line = if line >= first + group {
                line + shift
            } else {
                line
            };
            u16::try_from(line).unwrap_or(u16::MAX)
        };
        let mut names = self
            .names
            .iter()
            .map(|(name, line)| (name.clone(), moved(*line)))
            .collect::<Vec<_>>();
        for time in 0..times {
            for (name, offset) in &self.repeat_names {
                let line = first + time * group + usize::from(*offset) - 1;
                names.push((name.clone(), u16::try_from(line).unwrap_or(u16::MAX)));
            }
        }
        names
    }
    /// Pixels of a track size against the grid's `basis`, expression tracks included.
    pub fn length(&self, size: TrackSize, basis: Option<f64>) -> Option<f64> {
        match size {
            TrackSize::Expr(index) => {
                super::flex::resolve_track_expr(self.exprs.get(usize::from(index))?, basis?)
            }
            other => other.length(basis),
        }
    }
}
/// One named area of `grid-template-areas`: a rectangle of zero-based cells a grid item can name.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct GridArea {
    /// Lowercased area name, the same case-folding every grid value gets.
    pub name: String,
    /// First row.
    pub row: u16,
    /// First column.
    pub column: u16,
    /// Rows covered.
    pub rows: u16,
    /// Columns covered.
    pub columns: u16,
}
/// Where one grid item goes along one axis: a 1-based start line (negative counts from the end) and a span.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct GridLine {
    /// Explicit start line, None for auto-placement.
    pub start: Option<i32>,
    /// Explicit end line, used when `start` is set and no span was given.
    pub end: Option<i32>,
    /// Tracks covered.
    pub span: u16,
}
impl Default for GridLine {
    fn default() -> Self {
        Self {
            start: None,
            end: None,
            span: 1,
        }
    }
}
/// A named side of `grid-row` or `grid-column` (css-grid-2 8.3): `name`, `name <integer>` or
/// `span [<integer>] name`, resolved against the container's line names at layout because a
/// subgrid only learns its names from its parent then.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct LineName {
    /// The line name.
    pub name: String,
    /// Which line of that name, negative counting from the end; 0 when no integer was given,
    /// which means `name-start` / `name-end` first, then the first `name`.
    pub nth: i32,
    /// `span name`: count `nth` lines of that name from the opposite side.
    pub span: bool,
}
/// `container-type`: which axes of a box `@container` size queries may read.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ContainerType {
    /// Not a size container.
    #[default]
    Normal,
    /// Queries may read the inline size (width).
    InlineSize,
    /// Queries may read both sizes.
    Size,
}
/// `object-fit` sizing rules for replaced images.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ObjectFit {
    /// Stretches to fill the content box.
    #[default]
    Fill,
    /// Scales to fit inside the content box, preserving aspect ratio.
    Contain,
    /// Scales to cover the content box, preserving aspect ratio.
    Cover,
    /// Keeps the image's natural size.
    None,
    /// The smaller of `none` and `contain`.
    ScaleDown,
}
