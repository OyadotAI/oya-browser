/** Computed styles are bounded shared data consumed by native layout, painting and control guards. */
use {
    super::flex_model::*,
    super::formatting::{ColorScheme, LineHeight},
    super::visual_model::*,
    crate::document::Appearance,
    serde::{Deserialize, Serialize},
};
/// Required resources retain their document order before acquisition.
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
    pub interactive: bool,
    /// Inherited: inside an `inert` subtree, which no `pointer-events` value makes actionable.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub inert: bool,
    pub inline: bool,
    pub font_size: f64,
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
    /// Main-axis content distribution (`justify-content`); `normal` is `Stretch`.
    pub justify_content: JustifyContent,
    /// Cross-axis content distribution of flex lines and grid rows (`align-content`).
    #[serde(default = "JustifyContent::normal")]
    pub align_content: JustifyContent,
    /// Cross-axis alignment controls the allocated item subtree as one unit.
    pub align_items: AlignItems,
    /// None retains the content minimum; explicit zero permits supported shrinking.
    pub min_width: Option<f64>,
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
    /// Inherited case mapping.
    #[serde(skip_serializing_if = "default")]
    pub text_transform: TextTransform,
    /// Inherited whitespace handling.
    #[serde(skip_serializing_if = "default")]
    pub white_space: WhiteSpace,
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
    /// Per-side literal border colors; None paints the default border color.
    #[serde(skip_serializing_if = "default")]
    pub border_colors: [Option<String>; 4],
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
    /// Named start and end lines of `grid-row` and `grid-column` (`content-start / content-end`),
    /// resolved against the container's line names at layout.
    #[serde(default, skip_serializing_if = "default")]
    pub grid_line_names: [[Option<String>; 2]; 2],
    /// Inline-axis alignment of grid items in their areas.
    #[serde(skip_serializing_if = "default")]
    pub justify_items: AlignItems,
    /// Size of background image layers.
    #[serde(skip_serializing_if = "default")]
    pub background_size: BackgroundSize,
    /// Position of background image layers.
    #[serde(skip_serializing_if = "default")]
    pub background_position: [Extent; 2],
    /// Inherited gap between table cells, in px.
    #[serde(skip_serializing_if = "default")]
    pub border_spacing: f64,
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
    /// A mask image shapes this box; Eye cannot apply masks, so its fills are not painted.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub masked: bool,
    /// The first `url()` mask layer and its geometry, painted with the box's fill as alpha.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask: Option<super::MaskLayer>,
    /// Tiling of background image layers.
    #[serde(skip_serializing_if = "default")]
    pub background_repeat: BackgroundRepeat,
    /// Own cross-axis alignment in a flex line or grid area; None is `auto` (the parent's `align-items`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align_self: Option<AlignItems>,
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
    /// Declared `word-break`: Some(true) for `break-all` or `break-word`, None when inherited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_break_all: Option<bool>,
    /// `text-overflow: ellipsis` (not inherited): a clipping block ends overflowing lines in "…".
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub text_overflow_ellipsis: bool,
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
    /// Percentage translation `[tx_percent, ty_percent]` of the box's own size.
    #[serde(skip_serializing_if = "default")]
    pub translate_percent: [f64; 2],
    /// Pixel part of a mixed `calc()` width such as `calc(70% - 12px)`, added once the
    /// percentage in `width` resolves; 0 for a plain width. Without it the whole declaration
    /// was refused and a two-column row stacked.
    #[serde(skip_serializing_if = "default")]
    pub width_offset: f64,
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
            interactive: true,
            inert: false,
            inline: false,
            font_size: crate::constants::CSS_ROOT_FONT_SIZE,
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
            box_sizing: BoxSizing::ContentBox,
            display_flex: false,
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::NoWrap,
            flex_reverse: false,
            wrap_reverse: false,
            flex_basis: FlexBasis::Auto,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            row_gap: 0.0,
            column_gap: 0.0,
            justify_content: JustifyContent::Stretch,
            align_content: JustifyContent::Stretch,
            align_items: AlignItems::Stretch,
            min_width: None,
            opacity: 1.0,
            rtl: false,
            position: Position::Static,
            inset: [None; 4],
            z_index: None,
            overflow: Overflow::Visible,
            radius: [0.0; 4],
            shadows: Vec::new(),
            backgrounds: Vec::new(),
            transform: None,
            text_decoration_none: false,
            font_weight: super::constants::WEIGHT_NORMAL,
            letter_spacing: 0.0,
            text_transform: TextTransform::None,
            white_space: WhiteSpace::Normal,
            font_family: FontFamily::Serif,
            font_families: Vec::new(),
            font_style: super::FontStyle::Normal,
            cursor_pointer: false,
            border_style_none: false,
            border_widths: [0.0; 4],
            border_colors: [None, None, None, None],
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
            grid_columns: GridTemplate::default(),
            grid_rows: GridTemplate::default(),
            grid_auto_rows: GridTemplate::default(),
            grid_auto_columns: GridTemplate::default(),
            grid_flow_column: false,
            grid_flow_dense: false,
            grid_column: GridLine::default(),
            grid_row: GridLine::default(),
            grid_areas: Vec::new(),
            grid_area_names: [None, None],
            grid_line_names: Default::default(),
            justify_items: AlignItems::Stretch,
            background_size: BackgroundSize::Auto,
            background_position: [Extent::default(); 2],
            background_repeat: BackgroundRepeat::Repeat,
            generated: Vec::new(),
            masked: false,
            mask: None,
            border_spacing: 0.0,
            border_collapse: false,
            table_layout_fixed: false,
            caption_bottom: false,
            quirk_top: false,
            font_authored: false,
            edges_authored: false,
            align_self: None,
            justify_self: None,
            order: 0,
            padding_percent: [0.0; 4],
            margin_percent: [0.0; 4],
            aspect_ratio: None,
            aspect_ratio_auto: false,
            overflow_wrap_break: None,
            word_break_all: None,
            text_overflow_ellipsis: false,
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
            translate_percent: [0.0; 2],
            width_offset: 0.0,
            svg_paint: std::collections::BTreeMap::new(),
            stacking: 0,
            filter_blur: 0.0,
        }
    }
}
