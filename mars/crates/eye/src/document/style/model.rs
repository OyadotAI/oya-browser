/** Computed styles are bounded shared data consumed by native layout, painting and control guards. */
use {
    super::flex_model::*,
    super::formatting::{ColorScheme, LineHeight},
    super::visual_model::*,
    crate::document::Appearance,
    serde::{Deserialize, Serialize},
};
/// Required resources retain their document order before acquisition.
/** `align_side`: `left`. */
pub const SIDE_LEFT: u8 = 1;
/** `align_side`: `right`. */
pub const SIDE_RIGHT: u8 = 2;
/** `align_side`: `self-start`. */
pub const SIDE_SELF_START: u8 = 3;
/** `align_side`: `self-end`. */
pub const SIDE_SELF_END: u8 = 4;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum StyleSource {
    Inline(String),
    Linked(String),
    /// A linked sheet with a `media` attribute: once fetched it applies as `@media <media> { }`,
    /// so the cascade evaluates the query against the viewport like any other media rule.
    LinkedMedia {
        href: String,
        media: String,
    },
}
/// One computed style is shared by rendering, native widgets and action guards.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ComputedStyle {
    pub hidden: bool,
    /// Inherited `visibility: hidden` or `collapse`: the box takes part in layout but paints
    /// nothing, and a `visibility: visible` descendant shows again. Agents still treat it as hidden.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub invisible: bool,
    pub interactive: bool,
    /// Inherited: inside an `inert` subtree, which no `pointer-events` value makes actionable.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub inert: bool,
    pub inline: bool,
    pub font_size: f64,
    /// Inherited: the `font-size` keyword (1 = xx-small .. 8 = xxx-large) the size came from,
    /// 0 for a length. Blink refetches a keyword size when the generic family switches to or
    /// from `monospace`, which is how `<pre>` gets 13px and `font-family: monospace` text too.
    #[serde(skip)]
    pub font_keyword: u8,
    /// Inherited: the size came from an absolute length, so a generic family change keeps it.
    #[serde(skip)]
    pub font_size_absolute: bool,
    pub bold: bool,
    pub color: Option<String>,
    pub background: Option<String>,
    pub margin: [f64; 4],
    /// Automatic margins resolve only after the containing block and border box are known.
    pub margin_auto: [bool; 4],
    /// Inherited factors remain relative to each descendant's own font.
    pub line_height: LineHeight,
    /// Offered schemes retain the author's choice independently of the host preference.
    pub color_scheme: ColorScheme,
    /// Light-dark colors use this resolved authored scheme.
    pub used_scheme: Appearance,
    pub padding: [f64; 4],
    pub width: Option<f64>,
    pub percent_width: bool,
    pub max_width: Option<f64>,
    /// `max_width` is a percentage of the containing block's width.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub percent_max_width: bool,
    pub border: f64,
    pub border_color: Option<String>,
    pub align: String,
    /// Inherited CSS `text-align-last`: how the last line and a line before a forced break
    /// align; empty is `auto` (as `text-align`, except `justify` aligns those lines at start).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub align_last: String,
    /// Inherited CSS `text-justify`: where justification adds space (`auto`, `inter-word`,
    /// `inter-character` or `none`).
    pub text_justify: String,
    /// Inherited CSS `text-autospace` other than `no-autospace`: an eighth of an em between
    /// ideographs and letters or digits.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub text_autospace: bool,
    /// Inherited content language (the nearest `lang` attribute, lowercase), which tailors
    /// `text-transform`'s case mapping.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub lang: String,
    /// Inherited CSS `hyphens: none`: soft hyphens are no break opportunities.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub hyphens_none: bool,
    /// Inherited CSS `hyphenate-character`: the string a hyphenated line ends with; None is
    /// `auto` (U+2010).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyphenate_character: Option<String>,
    /// The parent's direction, which `text-align: match-parent` resolves `start` and `end`
    /// against; set as the style inherits, never authored. None on the root, whose
    /// `match-parent` keeps the logical initial `start`.
    #[serde(skip)]
    pub parent_rtl: Option<bool>,
    /// Authored dimensions name either content or the complete painted border box.
    pub box_sizing: BoxSizing,
    /// Flex participation is explicit rather than inferred from child control types.
    pub display_flex: bool,
    /// The main axis determines which authored basis can be definite.
    pub flex_direction: FlexDirection,
    /// Wrapped rows form lines without introducing unsupported column heights.
    pub flex_wrap: FlexWrap,
    /// `row-reverse` or `column-reverse`: items run from the main-end edge.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flex_reverse: bool,
    /// `wrap-reverse`: lines stack from the cross-end edge.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub wrap_reverse: bool,
    /// `flex-wrap: balance`: lines are broken to even out their free space (Blink's
    /// `FlexWrap::IsBalanced`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flex_balance: bool,
    /// `display: -webkit-box`: a legacy box never wraps (Blink's `IsDeprecatedFlexbox`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub webkit_box: bool,
    /// `flex-line-count`, the minimum number of balanced lines; None is its initial `1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flex_line_count: Option<u32>,
    /// Automatic, pixel and percentage bases retain distinct layout meaning.
    pub flex_basis: FlexBasis,
    /// Nonnegative growth distributes only real positive main-axis free space.
    pub flex_grow: f64,
    /// Shrinkage is weighted by the item's base content size during allocation.
    pub flex_shrink: f64,
    /// Row separation remains independent of column separation when wrapping.
    pub row_gap: f64,
    /// Only between-item separation contributes to the line's horizontal budget.
    pub column_gap: f64,
    /// Percentage parts of `column-gap` and `row-gap`, resolved by grid layout against the
    /// content box in that axis (zero against an indefinite one, as Blink's GridGap does).
    #[serde(skip_serializing_if = "default")]
    pub gap_percent: [f64; 2],
    /// Whether `column-gap` and `row-gap` are set to something other than `normal`; a subgrid's
    /// `normal` gap is its parent's (css-grid-2 9.2).
    #[serde(skip_serializing_if = "default")]
    pub gap_set: [bool; 2],
    /// css-multicol: column count, width, fill, rule and span, inert unless a count or width is set.
    #[serde(skip_serializing_if = "default")]
    pub multicol: super::Multicol,
    /// Main-axis content distribution (`justify-content`); `normal` is `Stretch`.
    pub justify_content: JustifyContent,
    /// Cross-axis content distribution of flex lines and grid rows (`align-content`).
    #[serde(default = "JustifyContent::normal")]
    pub align_content: JustifyContent,
    /// Whether `align-content` and `justify-content` (in that order) are `safe`: content that
    /// overflows its flex container packs at the start instead of past it.
    #[serde(default, skip_serializing_if = "default")]
    pub content_safe: [bool; 2],
    /// Cross-axis alignment controls the allocated item subtree as one unit.
    pub align_items: AlignItems,
    /// None retains the content minimum; explicit zero permits supported shrinking.
    pub min_width: Option<f64>,
    /// A percentage `min-width` of the containing block's width, which only layout resolves;
    /// `min_width` is None then. LinkedIn's "Who is LinkedIn for?" column is `min-width: 50%`
    /// beside an 840px image, and shrank under it without this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_width_percent: Option<f64>,
    /// Own opacity in [0, 1]; it is not inherited, and display multiplies ancestors.
    #[serde(skip_serializing_if = "opaque")]
    pub opacity: f64,
    /// Inherited base direction from the `dir` attribute; it maps start and end alignment.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub rtl: bool,
    /// Positioning scheme; offsets live in `inset`.
    #[serde(skip_serializing_if = "default")]
    pub position: Position,
    /// Resolved px offsets top, right, bottom, left; None is `auto` or an unresolved percentage.
    #[serde(skip_serializing_if = "default")]
    pub inset: [Option<f64>; 4],
    /// Stacking order; None is `auto`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z_index: Option<i32>,
    /// The stricter of overflow-x and overflow-y.
    #[serde(skip_serializing_if = "default")]
    pub overflow: Overflow,
    /// Each axis on its own, so a wheel scrolls `overflow-x: hidden; overflow-y: scroll`
    /// vertically even though `overflow` says hidden.
    #[serde(default, skip_serializing_if = "default")]
    pub overflow_x: Overflow,
    #[serde(default, skip_serializing_if = "default")]
    pub overflow_y: Overflow,
    /// Corner radii in px: top-left, top-right, bottom-right, bottom-left.
    #[serde(skip_serializing_if = "default")]
    pub radius: [f64; 4],
    /// Box shadows in paint order, bounded.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shadows: Vec<BoxShadow>,
    /// Background image layers, first on top, bounded; the color stays in `background`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub backgrounds: Vec<BackgroundLayer>,
    /// 2D affine transform (a, b, c, d, e, f) about the box's transform origin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform: Option<[f64; 6]>,
    /// Inherited: the nearest author declaration removed link underlines.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub text_decoration_none: bool,
    /// Numeric weight 1..1000; `bold` is kept equal to `font_weight >= 600`.
    #[serde(skip_serializing_if = "normal_weight")]
    pub font_weight: u16,
    /// Inherited extra advance per character in px, may be negative.
    #[serde(skip_serializing_if = "default")]
    pub letter_spacing: f64,
    /// Inherited extra advance per space (CSS `word-spacing`) in px, may be negative.
    #[serde(skip_serializing_if = "default")]
    pub word_spacing: f64,
    /// The percentage part of `letter-spacing` and `word-spacing`, inherited as Blink inherits
    /// their Length, so each element re-resolves it against its own font size.
    #[serde(skip_serializing_if = "default")]
    pub spacing_percent: [f64; 2],
    /// Inherited first-line indent (CSS `text-indent`) as px plus a percentage of the block's width.
    #[serde(skip_serializing_if = "default")]
    pub text_indent: (f64, f64),
    /// Inherited case mapping.
    #[serde(skip_serializing_if = "default")]
    pub text_transform: TextTransform,
    /// Inherited `text-transform: full-width`: ASCII and halfwidth forms paint as their
    /// fullwidth compatibility forms (Blink's Character::FullWidth mapping).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub text_transform_full_width: bool,
    /// Inherited `text-transform: full-size-kana`: small kana paint as their full-size letters.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub text_transform_full_size_kana: bool,
    /// Inherited whitespace handling.
    #[serde(skip_serializing_if = "default")]
    pub white_space: WhiteSpace,
    /// Inherited `white-space-collapse: break-spaces`: preserved spaces never hang and every one
    /// is a break opportunity. It lays out as `pre-wrap` otherwise.
    #[serde(skip_serializing_if = "default")]
    pub break_spaces: bool,
    /// Inherited `tab-size` when authored: (value, whether it is a length in px rather than a
    /// number of spaces). None is the initial 8 spaces.
    #[serde(skip_serializing_if = "default")]
    pub tab_size: Option<(f64, bool)>,
    /// Inherited generic face, the fallback when no listed family has a loaded face.
    #[serde(skip_serializing_if = "default")]
    pub font_family: FontFamily,
    /// Inherited `font-family` names as authored (lowercased, unquoted), matched against the
    /// document's `@font-face` rules; empty when only the generic is known.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub font_families: Vec<String>,
    /// Inherited `font-style`, which picks italic faces of a loaded family.
    #[serde(skip_serializing_if = "default")]
    pub font_style: super::FontStyle,
    /// Inherited: the pointer shows a hand over this box.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cursor_pointer: bool,
    /// The author set every border side's style to none or hidden, so native control borders go too.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub border_style_none: bool,
    /// Per-side painted border widths top, right, bottom, left; `border` equals them when uniform, else 0.
    #[serde(skip_serializing_if = "default")]
    pub border_widths: [f64; 4],
    /// Per-side computed border widths before a `none` or `hidden` style zeroes the used ones:
    /// what `border-width: inherit` passes down (border-width-012).
    #[serde(skip_serializing_if = "default")]
    pub border_specified: [f64; 4],
    /// Per-side `border-style: hidden`, which in the collapsing model beats every other border
    /// on its edge (border-conflict-style-101).
    #[serde(skip_serializing_if = "default")]
    pub border_hidden: [bool; 4],
    /// Per-side border color is `currentcolor` (the initial value), which an inheriting child
    /// resolves against its own color.
    #[serde(skip_serializing_if = "default")]
    pub border_current: [bool; 4],
    /// Per-side literal border colors; None paints the default border color.
    #[serde(skip_serializing_if = "default")]
    pub border_colors: [Option<String>; 4],
    /// Per-side painted line style top, right, bottom, left (solid unless authored).
    #[serde(default, skip_serializing_if = "default")]
    pub border_styles: [super::BorderStyle; 4],
    /// Authored height; a percentage resolves only against a definite containing height.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<Extent>,
    /// Authored minimum height.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_height: Option<Extent>,
    /// Authored maximum height.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_height: Option<Extent>,
    /// Float side; floated boxes leave flow and shorten neighbouring lines.
    #[serde(skip_serializing_if = "default")]
    pub float: Float,
    /// Sides of earlier floats this block moves below.
    #[serde(skip_serializing_if = "default")]
    pub clear: Clear,
    /// An atomic inline: inline-block, inline-flex, inline-grid or inline-table.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub inline_block: bool,
    /// Placement within the line box or table row.
    #[serde(skip_serializing_if = "default")]
    pub vertical_align: VerticalAlign,
    /// `display: list-item` boxes get a marker.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub list_item: bool,
    /// Inherited marker kind.
    #[serde(skip_serializing_if = "default")]
    pub list_style: ListStyle,
    /// Grid container.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub display_grid: bool,
    /// `display: contents`: no box of its own, its children lay out in its parent; it falls
    /// back to an inline box when it has text or generated content of its own.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub display_contents: bool,
    /// `display: flow-root`: a block that starts its own formatting context, so it contains its
    /// floats (IANA's header grew around its floated logo and navigation in Chromium).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub flow_root: bool,
    /// `margin-trim` in the block axis: [block-start, block-end]. A block container drops the
    /// margins of children adjoining that edge of its content box.
    #[serde(default, skip_serializing_if = "default")]
    pub margin_trim: [bool; 2],
    /// The CSS table part `display` names; `None` for every other display.
    #[serde(skip_serializing_if = "default")]
    pub table_box: TableBox,
    /// Column tracks of a grid container.
    #[serde(skip_serializing_if = "default")]
    pub grid_columns: GridTemplate,
    /// Row tracks of a grid container.
    #[serde(skip_serializing_if = "default")]
    pub grid_rows: GridTemplate,
    /// Sizes of implicit row tracks (`grid-auto-rows`), cycled; empty is `auto`.
    #[serde(default, skip_serializing_if = "default")]
    pub grid_auto_rows: GridTemplate,
    /// Sizes of implicit column tracks (`grid-auto-columns`), cycled; empty is `auto`.
    #[serde(default, skip_serializing_if = "default")]
    pub grid_auto_columns: GridTemplate,
    /// `grid-auto-flow: column`: auto-placement fills columns before rows.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub grid_flow_column: bool,
    /// `grid-auto-flow: dense`: auto-placement restarts from the grid's start for every item.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub grid_flow_dense: bool,
    /// `display: grid-lanes` (css-grid-3): the grid container stacks items into lanes.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub grid_lanes: bool,
    /// Grid-lanes `flow-tolerance` in pixels; None is `normal` (1em), infinity `infinite`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow_tolerance: Option<f64>,
    /// Grid-lanes `flow-tolerance` percentage of the stacking-axis available size, when given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow_tolerance_percent: Option<f64>,
    /// `grid-lanes-direction` orientation: None is `normal`, Some(true) `row`, Some(false) `column`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_lanes_rows: Option<bool>,
    /// `grid-lanes-direction: fill-reverse`: items stack from the stacking axis's end.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub grid_lanes_fill_reverse: bool,
    /// `grid-lanes-direction: track-reverse`: ties go to the last lane and the cursor walks back.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub grid_lanes_track_reverse: bool,
    /// `grid-lanes-pack: dense`: items may fill openings earlier items left behind.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub grid_lanes_dense: bool,
    /// Placement of a grid item along the columns.
    #[serde(skip_serializing_if = "default")]
    pub grid_column: GridLine,
    /// Placement of a grid item along the rows.
    #[serde(skip_serializing_if = "default")]
    pub grid_row: GridLine,
    /// Named areas of a grid container, from `grid-template-areas`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub grid_areas: Vec<GridArea>,
    /// Named areas a grid item asks for on its row and column axes (`grid-area: name`); they win
    /// over the numeric lines of that axis when the container defines them.
    #[serde(skip_serializing_if = "default")]
    pub grid_area_names: [Option<String>; 2],
    /// Named start and end lines of `grid-row` and `grid-column` (`content-start / content-end`,
    /// `a 2`, `span a`), resolved against the container's line names at layout.
    #[serde(default, skip_serializing_if = "default")]
    pub grid_line_names: [[Option<LineName>; 2]; 2],
    /// Inline-axis alignment of grid items in their areas.
    #[serde(skip_serializing_if = "default")]
    pub justify_items: AlignItems,
    /// Size of background image layers.
    #[serde(skip_serializing_if = "default")]
    pub background_size: BackgroundSize,
    /// Position of background image layers.
    #[serde(skip_serializing_if = "default")]
    pub background_position: [Extent; 2],
    /// Every layer's own size, position and repeat, as their comma-separated lists give them.
    #[serde(skip_serializing_if = "super::background_layers::LayerGeometry::is_empty")]
    pub background_geometry: super::background_layers::LayerGeometry,
    /// Inherited gap between table cells, in px.
    #[serde(skip_serializing_if = "default")]
    pub border_spacing: f64,
    /// Inherited vertical gap between table rows, in px, when `border-spacing` names two
    /// lengths; None repeats the horizontal one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border_spacing_rows: Option<f64>,
    /// Inherited: adjacent table cell borders collapse and cells touch.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub border_collapse: bool,
    /// `table-layout: fixed` (not inherited): column widths come from the first row alone.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub table_layout_fixed: bool,
    /// A form control whose font the page set (`font: inherit` included), so it is drawn in that
    /// font rather than Chromium's 13.33px control font.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub font_authored: bool,
    /// A form control whose padding or border the page set, so its box is those edges around its
    /// title rather than the native button metrics.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub edges_authored: bool,
    /// Quirks mode drops this box's UA top margin in layout, as the first box of the body or a
    /// table cell; the computed margin stays, as `getComputedStyle` reports it in Chromium.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub quirk_top: bool,
    /// Inherited `caption-side: bottom`: the caption follows the grid instead of leading it.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub caption_bottom: bool,
    /// `::before` and `::after` boxes with content, in that order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub generated: Vec<Generated>,
    /// `scroll-marker-group` (not inherited): Some(after) places the scroll container's
    /// `::scroll-marker-group` box after it (true) or before it (false); None is `none`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker_group_side: Option<bool>,
    /// The `::scroll-marker-group` box's style, when `marker_group_side` asks for one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker_group: Option<Box<ComputedStyle>>,
    /// The element's `::scroll-marker` box, when a rule gives it content: it is laid out in its
    /// scroll container's marker group (render/scroll_markers.rs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll_marker: Option<Box<Generated>>,
    /// The element's `::scroll-button()` boxes with content, in block-start, inline-start,
    /// inline-end, block-end order; laid out as its next siblings (render/scroll_markers.rs).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scroll_buttons: Vec<Generated>,
    /// A mask image shapes this box; Eye cannot apply masks, so its fills are not painted.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub masked: bool,
    /// The first `url()` mask layer and its geometry, painted with the box's fill as alpha.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask: Option<super::MaskLayer>,
    /// `border-image`: with a source, painters draw its nine slices instead of the border.
    #[serde(skip_serializing_if = "super::BorderImage::is_initial")]
    pub border_image: super::BorderImage,
    /// Tiling of background image layers.
    #[serde(skip_serializing_if = "default")]
    pub background_repeat: BackgroundRepeat,
    /// Own cross-axis alignment in a flex line or grid area; None is `auto` (the parent's `align-items`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align_self: Option<AlignItems>,
    /// `align-self: safe <position>` on a flex item: the position, which falls back to the start
    /// when the item overflows its line. Kept apart from `align_self`, which grid reads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align_self_safe: Option<AlignItems>,
    /// Whether `align-items`, `align-self`, `justify-items` and `justify-self`, in that order,
    /// were literally `stretch` rather than `normal`, which their values share: grid-lanes
    /// stacking stretches only the former.
    #[serde(default, skip_serializing_if = "default")]
    pub align_stretch: [bool; 4],
    /// Whether the same four properties carried `safe`, so an overflowing box aligns to the
    /// start instead (css-align-3 overflow alignment).
    #[serde(default, skip_serializing_if = "default")]
    pub align_safe: [bool; 4],
    /// For the same four properties, a keyword not relative to the container's direction:
    /// `SIDE_LEFT`, `SIDE_RIGHT`, `SIDE_SELF_START` or `SIDE_SELF_END`, else zero. The
    /// position itself is folded into start or end, which grid resolves against the sides.
    #[serde(default, skip_serializing_if = "default")]
    pub align_side: [u8; 4],
    /// Own inline-axis alignment in a grid area; None is `auto` (the parent's `justify-items`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justify_self: Option<AlignItems>,
    /// Flex and grid placement order; equal values keep source order.
    #[serde(skip_serializing_if = "default")]
    pub order: i32,
    /// Percentage parts of the paddings, top, right, bottom, left, all of the containing block's
    /// width; layout adds them to `padding` once that width is known.
    #[serde(skip_serializing_if = "default")]
    pub padding_percent: [f64; 4],
    /// Percentage parts of the margins, all of the containing block's width, like `padding_percent`.
    #[serde(skip_serializing_if = "default")]
    pub margin_percent: [f64; 4],
    /// Preferred width-to-height ratio for an automatic height; None is `auto`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<f64>,
    /// `aspect-ratio: auto && <ratio>`: a replaced box with a natural ratio uses that ratio instead.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub aspect_ratio_auto: bool,
    /// Declared `overflow-wrap`: Some(true) for `break-word` or `anywhere`, None when this
    /// element declares nothing and inherits it (layout walks the ancestors).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overflow_wrap_break: Option<bool>,
    /// Declared `overflow-wrap: anywhere` (Some(true)) or another value, None when inherited:
    /// unlike `break-word`, its breaks count toward min-content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overflow_wrap_anywhere: Option<bool>,
    /// Declared `word-break`: Some(true) for `break-all` or `break-word`, None when inherited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_break_all: Option<bool>,
    /// Declared `word-break: break-all` (Some(true)) or another value (Some(false)), None when
    /// inherited: unlike `break-word`, it makes every letter a break opportunity, so a word
    /// starts on the current line and fills the width left there.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_break_fill: Option<bool>,
    /// Declared `word-break: keep-all` (Some(true)) or another value, None when inherited:
    /// ideographs then break only where letters would (UAX #14 with ID treated as AL).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_break_keep_all: Option<bool>,
    /// Declared `word-break: auto-phrase` (Some(true)) or another value, None when inherited:
    /// CJK text breaks only between phrases.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_break_auto_phrase: Option<bool>,
    /// Declared `line-break: anywhere` (Some(true)) or another value, None when inherited: a
    /// break opportunity between every two typographic letter units, preserved spaces included.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_break_anywhere: Option<bool>,
    /// Declared `line-break: loose` (Some(true)) or another value, None when inherited: CJK
    /// text may break before nonstarters and inseparable leaders.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_break_loose: Option<bool>,
    /// `text-overflow: ellipsis` (not inherited): a clipping block ends overflowing lines in "…".
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub text_overflow_ellipsis: bool,
    /// `-webkit-box-orient: vertical` (not inherited), which with `-webkit-line-clamp` turns a
    /// `-webkit-box` into a clamping flow-root.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub box_orient_vertical: bool,
    /// `background-clip` (not inherited) of the bottom layer, which the color follows.
    #[serde(skip_serializing_if = "default")]
    pub background_clip: super::BackgroundBox,
    /// `background-origin` (not inherited) of the image layer.
    #[serde(skip_serializing_if = "is_padding_box")]
    pub background_origin: super::BackgroundBox,
    /// The element's `::first-letter` box style, when a rule styles it: layout gives the first
    /// letter of its first formatted line this style instead of the text's own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_letter: Option<Box<ComputedStyle>>,
    /// `display: none` (not inherited): no box at all, where `hidden` alone may be
    /// `visibility: hidden`, a box that takes part in layout but does not paint.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub display_none: bool,
    /// The element's `::first-line` style, when a rule styles it: the first formatted line's
    /// text takes its color (render/first_line.rs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_line: Option<Box<ComputedStyle>>,
    /// `initial-letter` (not inherited), read on a `::first-letter` box; None is `normal`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_letter: Option<super::InitialLetter>,
    /// `background-attachment: fixed` (not inherited) on the first layer: image layers are
    /// positioned and sized against the viewport instead of the box.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub background_fixed: bool,
    /// `background-attachment: local` (not inherited) on the first layer: in a scroll
    /// container, image layers are positioned against the scrolled contents, not the box.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub background_local: bool,
    /// `max-lines` (not inherited): the line count a collapsing `continue` clamps at; 0 is
    /// `none`, which with `continue: collapse` clamps where the box's height runs out.
    #[serde(skip_serializing_if = "default")]
    pub max_lines: u32,
    /// `block-ellipsis` (inherited): the string a clamp's last line ends in; None is
    /// `no-ellipsis`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_ellipsis: Option<String>,
    /// `continue: collapse` (not inherited): this block is a line-clamp container.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub continue_collapse: bool,
    /// `continue: -webkit-legacy` (not inherited), which `-webkit-line-clamp` sets: it clamps
    /// only a `-webkit-box` with a vertical `-webkit-box-orient`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub continue_legacy: bool,
    /// `text-box-trim` (not inherited) as whether the (block-start, block-end) edge trims.
    #[serde(skip_serializing_if = "super::text_box::untrimmed")]
    pub text_box_trim: [bool; 2],
    /// `text-box-edge` (inherited) as (over, under) codes from `style/text_box.rs`.
    #[serde(skip_serializing_if = "super::text_box::auto_edge")]
    pub text_box_edge: [u8; 2],
    /// `vertical-align: <length-percentage>` as (px, percent of the element's line-height); the
    /// box's baseline rises by it. None for the keywords.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertical_shift: Option<(f64, f64)>,
    /// `clip: rect()` as insets from the border box top, right, bottom, left; only absolute and
    /// fixed boxes honor it, as in CSS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip_rect: Option<[Extent; 4]>,
    /// `clip-path: inset()` as insets from the border box top, right, bottom, left.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip_path: Option<[Extent; 4]>,
    /// Size containment for container queries.
    #[serde(skip_serializing_if = "default")]
    pub container_type: ContainerType,
    /// Names `@container` queries can select this container by, bounded.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub container_names: Vec<String>,
    /// Replaced content sizing rule (`object-fit`).
    #[serde(skip_serializing_if = "default")]
    pub object_fit: ObjectFit,
    /// Replaced content alignment (`object-position`), initial `50% 50%`.
    #[serde(skip_serializing_if = "center_object_position")]
    pub object_position: [Extent; 2],
    /// Inherited: `text-decoration` includes `underline`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
    /// Inherited: `text-decoration` includes `line-through`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub strikethrough: bool,
    /// Percentage parts of `top`, `right`, `bottom`, `left` insets.
    #[serde(skip_serializing_if = "default")]
    pub inset_percent: [f64; 4],
    /// Percentage parts of corner radii when no fixed px width was authored.
    #[serde(skip_serializing_if = "default")]
    pub radius_percent: [f64; 4],
    /// The length part of a corner's horizontal radius whose percentage resolves at layout
    /// (`calc(100% - 1px)` keeps its `-1px`); unused while `radius_percent` is zero.
    #[serde(skip_serializing_if = "default")]
    pub radius_px: [f64; 4],
    /// Each authored corner's vertical radius as (px, percent of the box height); None leaves
    /// the corner circular at its horizontal radius.
    #[serde(skip_serializing_if = "default")]
    pub radius_y: [Option<(f64, f64)>; 4],
    /// Percentage translation `[tx_percent, ty_percent]` of the box's own size.
    #[serde(skip_serializing_if = "default")]
    pub translate_percent: [f64; 2],
    /// Pixel part of a mixed `calc()` width such as `calc(70% - 12px)`, added once the
    /// percentage in `width` resolves; 0 for a plain width. Without it the whole declaration
    /// was refused and a two-column row stacked.
    #[serde(skip_serializing_if = "default")]
    pub width_offset: f64,
    /// `width: stretch` (`-webkit-fill-available`): the margin box fills the containing block
    /// whatever `box-sizing` says, as Blink's fill-available size does. Without it LinkedIn's
    /// header row shrank to its content and the nav ended mid-page.
    #[serde(default, skip_serializing_if = "default")]
    pub width_stretch: bool,
    /// `width: fit-content` (or `min-content`, `max-content`): content-sized, so unlike `auto`
    /// a flex or grid item is not stretched and a block shrinks to fit. Without it LinkedIn's
    /// "Show more" pill spanned its whole column.
    /// ponytail: all three size as fit-content; min-content and max-content differ only when
    /// the room is narrower or wider than the content.
    #[serde(default, skip_serializing_if = "default")]
    pub width_fit: bool,
    /// `height: fit-content`, `min-content` or `max-content` (or a vertical writing mode's
    /// content-keyword `width`): content-sized, so a grid or flex item is not stretched.
    #[serde(default, skip_serializing_if = "default")]
    pub height_fit: bool,
    /// `min-height` and `max-height` name a content keyword (`min-content`, `max-content`,
    /// `fit-content`), in that order: on the block axis each is the content height, so a
    /// `min-height: max-content` box grows to its content however small its `height`.
    #[serde(default, skip_serializing_if = "default")]
    pub height_bounds_fit: [bool; 2],
    /// `height`, `min-height` and `max-height` are `stretch` (or `-webkit-fill-available`), in
    /// that order: the border box fills the containing block's definite height less margins.
    #[serde(default, skip_serializing_if = "default")]
    pub height_stretch: [bool; 3],
    /// The content keyword of `width`, `min-width` and `max-width`, in that order: which
    /// intrinsic size the box takes, or the `fit-content(<length-percentage>)` limit.
    #[serde(default, skip_serializing_if = "default")]
    pub size_keywords: [SizeKeyword; 3],
    /// The `calc-size()` sum of `width`, `min-width`, `max-width`, `height`, `min-height` and
    /// `max-height`, in that order (style/calc_size.rs); None maps the basis size unchanged.
    #[serde(default, skip_serializing_if = "default")]
    pub calc_size: [Option<super::CalcSize>; 6],
    /// Inherited: `writing-mode` is vertical, so `width` sizes the block axis. A box whose
    /// parent is horizontal roots a vertical subtree, laid out in render/vertical.rs.
    #[serde(default, skip_serializing_if = "default")]
    pub vertical_writing: bool,
    /// Inherited: the vertical mode stacks lines left to right (`vertical-lr`, `sideways-lr`)
    /// rather than right to left.
    #[serde(default, skip_serializing_if = "default")]
    pub writing_lr: bool,
    /// Inherited: `sideways-rl` or `sideways-lr`, whose dominant baseline stays alphabetic.
    #[serde(default, skip_serializing_if = "default")]
    pub writing_sideways: bool,
    /// Inherited `text-orientation`: 0 `mixed`, 1 `upright`, 2 `sideways`.
    #[serde(default, skip_serializing_if = "default")]
    pub text_orientation: u8,
    /// Used glyph orientation inside a vertical subtree's rotated layout (`platform::upright`
    /// codes); vertical.rs sets it on the rotated copy only, so measurement stands CJK upright.
    #[serde(default, skip_serializing_if = "default")]
    pub glyphs_upright: u8,
    /// `unicode-bidi` (not inherited): which explicit embedding, override or isolate the
    /// element's text takes (render/bidi.rs `controls`); 0 is `normal`.
    #[serde(default, skip_serializing_if = "default")]
    pub unicode_bidi: u8,
    /// Layout only: the box sits in a rotated vertical subtree whose dominant baseline is
    /// central (render/vertical.rs), so baseline-aligned atomics center on the line.
    #[serde(skip)]
    pub central_baseline: bool,
    /// Layout only: a horizontal box inside a rotated vertical subtree, which lays out turned
    /// back (render/vertical.rs `Turn::undo`): 0 none, else the `DisplayItem::writing` value of
    /// the rotation it undoes.
    #[serde(skip)]
    pub counter_rotate: u8,
    /// SVG paint properties (`fill`, `stroke`, `stroke-width`, ...) authored in CSS, by name.
    /// The SVG painter reads them before presentation attributes, as the cascade orders them;
    /// without this a stylesheet `fill` loses to `<svg fill="none">` and icons paint nothing.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub svg_paint: std::collections::BTreeMap<String, String>,
    /// Bits of the grouping properties that make this box a stacking context in Blink without
    /// positioning (`filter`, `backdrop-filter`, `mix-blend-mode`, `isolation`, `will-change`,
    /// `contain`, a shaped `clip-path`); see `visual::STACKING_*`. Paint order reads it.
    #[serde(skip_serializing_if = "default")]
    pub stacking: u8,
    /// The standard deviation of a `filter: blur()` in px, 0 when the box is not blurred.
    /// Paint spreads a blurred solid box into a soft glow instead of a hard shape.
    #[serde(default, skip_serializing_if = "default")]
    pub filter_blur: f64,
    /// `overflow-clip-margin` length in px: how far past its reference box an `overflow: clip`
    /// or paint-contained box lets content paint (css-overflow-4). Scroll containers ignore it.
    #[serde(default, skip_serializing_if = "default")]
    pub overflow_clip_margin: f64,
    /// The `overflow-clip-margin` reference box: 0 padding box (initial), 1 border box,
    /// 2 content box.
    #[serde(default, skip_serializing_if = "default")]
    pub overflow_clip_box: u8,
    /// `contain: paint` (or `strict`, `content`): the box clips its content at its overflow
    /// clip edge like `overflow: clip`, without becoming a scroll container.
    #[serde(default, skip_serializing_if = "default")]
    pub contain_paint: bool,
    /// `contain: size` (or `strict`) contains both axes, `contain: inline-size` the width only:
    /// a contained axis is sized as if the box were empty, so its content never props it open.
    #[serde(default, skip_serializing_if = "default")]
    pub contain_size: [bool; 2],
    /// `contain-intrinsic-width` and `-height` in px, None for `none`: the size a
    /// size-contained axis takes in place of its content's (css-sizing-4).
    #[serde(default, skip_serializing_if = "default")]
    pub contain_intrinsic: [Option<f64>; 2],
    /// The fixed px parts of the right and left padding, then the right and left margin, once
    /// layout folded their percentages into px; None when none had a percentage. Min- and
    /// max-content contributions count only these, since a percentage of the width being
    /// measured is cyclic (css-sizing-3 "cyclic percentage contribution").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_inline_sides: Option<[f64; 4]>,
}
/// A sizing content keyword (css-sizing-3 4.1): `min-content`, `max-content`, `fit-content`
/// (stretch-fit clamped by both), or `fit-content(<length-percentage>)` (that limit clamped).
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
pub enum SizeKeyword {
    /// A length, a percentage or `auto`: no content keyword.
    #[default]
    None,
    /// `min-content`.
    MinContent,
    /// `max-content`.
    MaxContent,
    /// `fit-content`.
    FitContent,
    /// `fit-content(<length-percentage>)`.
    FitContentLimit(Extent),
}
/// A generated `::before` or `::after` box: its text and its own computed style.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Generated {
    /// `::after` rather than `::before`.
    pub after: bool,
    /// Literal generated text, bounded.
    pub content: String,
    /// The box's style; its own `generated` list stays empty.
    pub style: Box<ComputedStyle>,
}
/** Default-valued visual fields are omitted so existing serialized shapes stay stable. */
fn default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}
/** Centered object-position is the initial value. */
fn center_object_position(position: &[Extent; 2]) -> bool {
    *position
        == [Extent {
            px: 0.0,
            percent: 50.0,
        }; 2]
}
/** Normal weight is the default serialized shape. */
fn normal_weight(weight: &u16) -> bool {
    *weight == super::constants::WEIGHT_NORMAL
}
/** Fully opaque styles keep their established serialized shape. */
fn opaque(opacity: &f64) -> bool {
    *opacity == 1.0
}
impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            hidden: false,
            invisible: false,
            interactive: true,
            inert: false,
            inline: false,
            font_size: crate::constants::CSS_ROOT_FONT_SIZE,
            font_keyword: super::font::MEDIUM_KEYWORD,
            font_size_absolute: false,
            bold: false,
            color: None,
            background: None,
            margin: [0.0; 4],
            margin_auto: [false; 4],
            line_height: LineHeight::Normal,
            color_scheme: ColorScheme::Normal,
            used_scheme: Appearance::Light,
            padding: [0.0; 4],
            width: None,
            percent_width: false,
            max_width: None,
            percent_max_width: false,
            border: 0.0,
            border_color: None,
            align: "start".into(),
            align_last: String::new(),
            text_justify: "auto".into(),
            text_autospace: false,
            lang: String::new(),
            hyphens_none: false,
            hyphenate_character: None,
            parent_rtl: None,
            box_sizing: BoxSizing::ContentBox,
            display_flex: false,
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::NoWrap,
            flex_reverse: false,
            wrap_reverse: false,
            flex_balance: false,
            webkit_box: false,
            flex_line_count: None,
            flex_basis: FlexBasis::Auto,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            row_gap: 0.0,
            column_gap: 0.0,
            gap_percent: [0.0; 2],
            gap_set: [false; 2],
            multicol: super::Multicol::default(),
            justify_content: JustifyContent::Stretch,
            align_content: JustifyContent::Stretch,
            content_safe: [false; 2],
            align_items: AlignItems::Stretch,
            min_width: None,
            min_width_percent: None,
            opacity: 1.0,
            rtl: false,
            position: Position::Static,
            inset: [None; 4],
            z_index: None,
            overflow: Overflow::Visible,
            overflow_x: Overflow::Visible,
            overflow_y: Overflow::Visible,
            radius: [0.0; 4],
            shadows: Vec::new(),
            backgrounds: Vec::new(),
            transform: None,
            text_decoration_none: false,
            font_weight: super::constants::WEIGHT_NORMAL,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            spacing_percent: [0.0; 2],
            text_indent: (0.0, 0.0),
            text_transform: TextTransform::None,
            text_transform_full_width: false,
            text_transform_full_size_kana: false,
            white_space: WhiteSpace::Normal,
            break_spaces: false,
            tab_size: None,
            font_family: FontFamily::Serif,
            font_families: Vec::new(),
            font_style: super::FontStyle::Normal,
            cursor_pointer: false,
            border_style_none: false,
            border_widths: [0.0; 4],
            border_specified: [0.0; 4],
            border_hidden: [false; 4],
            border_current: [false; 4],
            border_colors: [None, None, None, None],
            border_styles: [super::BorderStyle::Solid; 4],
            height: None,
            min_height: None,
            max_height: None,
            float: Float::None,
            clear: Clear::None,
            inline_block: false,
            vertical_align: VerticalAlign::Baseline,
            list_item: false,
            list_style: ListStyle::Disc,
            display_grid: false,
            display_contents: false,
            table_box: TableBox::None,
            flow_root: false,
            margin_trim: [false; 2],
            grid_columns: GridTemplate::default(),
            grid_rows: GridTemplate::default(),
            grid_auto_rows: GridTemplate::default(),
            grid_auto_columns: GridTemplate::default(),
            grid_flow_column: false,
            grid_flow_dense: false,
            grid_lanes: false,
            flow_tolerance: None,
            flow_tolerance_percent: None,
            grid_lanes_rows: None,
            grid_lanes_fill_reverse: false,
            grid_lanes_track_reverse: false,
            grid_lanes_dense: false,
            grid_column: GridLine::default(),
            grid_row: GridLine::default(),
            grid_areas: Vec::new(),
            grid_area_names: [None, None],
            grid_line_names: Default::default(),
            justify_items: AlignItems::Stretch,
            background_size: BackgroundSize::Auto,
            background_position: [Extent::default(); 2],
            background_geometry: Default::default(),
            background_repeat: BackgroundRepeat::Repeat,
            generated: Vec::new(),
            marker_group_side: None,
            marker_group: None,
            scroll_marker: None,
            scroll_buttons: Vec::new(),
            masked: false,
            mask: None,
            border_image: super::BorderImage::default(),
            border_spacing: 0.0,
            border_spacing_rows: None,
            border_collapse: false,
            table_layout_fixed: false,
            caption_bottom: false,
            quirk_top: false,
            font_authored: false,
            edges_authored: false,
            align_self: None,
            align_self_safe: None,
            align_stretch: [false; 4],
            align_safe: [false; 4],
            align_side: [0; 4],
            justify_self: None,
            order: 0,
            padding_percent: [0.0; 4],
            margin_percent: [0.0; 4],
            aspect_ratio: None,
            aspect_ratio_auto: false,
            overflow_wrap_break: None,
            overflow_wrap_anywhere: None,
            word_break_all: None,
            word_break_fill: None,
            word_break_keep_all: None,
            word_break_auto_phrase: None,
            line_break_anywhere: None,
            line_break_loose: None,
            text_overflow_ellipsis: false,
            box_orient_vertical: false,
            background_clip: super::BackgroundBox::BorderBox,
            background_origin: super::BackgroundBox::PaddingBox,
            background_fixed: false,
            background_local: false,
            first_letter: None,
            first_line: None,
            display_none: false,
            initial_letter: None,
            max_lines: 0,
            block_ellipsis: None,
            continue_collapse: false,
            continue_legacy: false,
            text_box_trim: [false, false],
            text_box_edge: [0, 0],
            vertical_shift: None,
            clip_rect: None,
            clip_path: None,
            container_type: ContainerType::Normal,
            container_names: Vec::new(),
            object_fit: ObjectFit::Fill,
            object_position: [Extent {
                px: 0.0,
                percent: 50.0,
            }; 2],
            underline: false,
            strikethrough: false,
            inset_percent: [0.0; 4],
            radius_percent: [0.0; 4],
            radius_px: [0.0; 4],
            radius_y: [None; 4],
            translate_percent: [0.0; 2],
            width_offset: 0.0,
            width_stretch: false,
            width_fit: false,
            height_fit: false,
            height_bounds_fit: [false; 2],
            height_stretch: [false; 3],
            size_keywords: [SizeKeyword::None; 3],
            calc_size: [None; 6],
            vertical_writing: false,
            writing_lr: false,
            writing_sideways: false,
            text_orientation: 0,
            glyphs_upright: 0,
            unicode_bidi: 0,
            central_baseline: false,
            counter_rotate: 0,
            svg_paint: std::collections::BTreeMap::new(),
            stacking: 0,
            filter_blur: 0.0,
            overflow_clip_margin: 0.0,
            overflow_clip_box: 0,
            contain_paint: false,
            contain_size: [false; 2],
            contain_intrinsic: [None; 2],
            fixed_inline_sides: None,
        }
    }
}
/** Serde skip test for the initial `background-origin`. */
fn is_padding_box(value: &super::BackgroundBox) -> bool {
    *value == super::BackgroundBox::PaddingBox
}
