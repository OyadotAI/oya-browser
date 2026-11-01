mod bfc_fit;
/** Oya owns point-based block flow, wrapping and control placement. */
mod bidi;
mod block_in_inline;
mod blur;
mod body;
mod body_clip;
mod border_image;
mod canvas_layers;
mod casing;
mod clearance;
mod control;
pub(crate) mod degrade;
mod display;
mod effects;
mod first_letter;
mod first_line;
mod first_line_layout;
mod fixed;
mod flex;
mod flex_abspos;
mod flex_column;
mod flex_column_intrinsic;
mod flex_layout;
mod float;
mod flow;
mod forced_lines;
mod fragments;
mod frame;
mod generated;
mod glue;
mod gradient_tile;
mod grid;
mod hyphenate;
mod image;
mod indent_intrinsic;
mod initial_letter;
mod inline_root;
mod justify;
mod line;
mod line_clamp;
mod line_clamp_align;
mod lookahead;
mod marker;
mod multicol;
mod multicol_balance;
mod multicol_cut;
mod multicol_fragment;
mod multicol_intrinsic;
mod multicol_oof;
mod multicol_units;
mod nine_piece;
mod order;
mod path;
mod phrase;
mod plaintext;
mod positioned;
mod resize_wrappers;
mod root_box;
mod root_fit;
mod root_flow;
mod root_generated;
mod scroll_markers;
mod sizing;
mod sticky_container;
mod svg;
mod table;
mod text;
mod text_chains;
mod text_echo;
mod textarea_align;
mod upright_split;
mod validation;
pub mod vertical;
mod wrap;
use super::{Appearance, ColorScheme, Document, Node, RenderContext};
use crate::{
    constants::PAINT_LINES,
    protocol::{EngineError, EngineResult},
};
pub use border_image::DisplayBorderImage;
pub use display::{
    ClipRect, DisplayBackground, DisplayShadow, GradientStop, GradientTile, ImageSize,
    OpacityGroup, Pin, ScrollArea,
};
/// Agent-visibility helpers for the app; the document facade exports them when the app wires them.
#[allow(unused_imports)]
pub use display::{agent_visible, painted_bounds, sliced_shadow, spread_radius, wheel_target};
pub use effects::ScrollOffsets;
pub use frame::{embed_frame, frame_viewport};
pub use nine_piece::{NinePiece, nine_pieces};
use serde::{Deserialize, Serialize};
pub use svg::{FillRule, LineCap, LineJoin, PathCommand, VectorPath};
pub use text_chains::{ChainKey, ChainMember, TextChain, TextChains, chain_key, text_chains};

/// Default document margin in logical points, shared by paint and inspection.
pub const MARGIN: f64 = 32.0;
/// Readable text does not stretch across arbitrarily wide windows.
pub const CONTENT_WIDTH: f64 = 960.0;
/// Supported viewport bounds constrain layout work and geometry.
pub const MIN_VIEWPORT: f64 = 280.0;
/// Bounds snapshots even for programmatic window requests.
pub const MAX_VIEWPORT: f64 = 4096.0;
/// Point geometry is explicitly different from the legacy abstract text layout.
pub const UNIT: &str = "logical_points";

/// One immutable display list is consumed by native painting and agent inspection.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DisplayList {
    /// Top-left-origin logical-point geometry.
    pub unit: String,
    /// Viewport width used for this layout generation.
    pub width: f64,
    /// Full document height, before viewport scrolling.
    pub height: f64,
    /// Element boxes retain current document-local identities.
    pub items: Vec<DisplayItem>,
    /// Root/body measurements use layout extents independently of scroll overflow and paint IDs.
    #[serde(default)]
    pub resize_wrappers: mars_core::geometry::WrapperMetrics,
    /// Root or propagated body color covers the full document canvas, including empty viewport area.
    #[serde(default)]
    pub canvas_background: Option<String>,
    /// Scheme-sensitive text fallback remains consistent with the canvas color.
    #[serde(default)]
    pub canvas_foreground: Option<String>,
    /// The root's used color scheme. Native form controls follow it, not the OS appearance, as
    /// Chromium draws light controls on a page that never opted into dark.
    #[serde(default)]
    pub canvas_scheme: Appearance,
    /// Non-target body decoration remains separate when the root already supplies the canvas background.
    #[serde(default)]
    pub body_decoration: Option<DisplayDecoration>,
    /// The root element's border, painted under everything else like the body's decoration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_decoration: Option<DisplayDecoration>,
    /// The canvas's propagated gradient layers on a stand-in box over the whole document
    /// (render/canvas_layers.rs), painted over `canvas_background` and under everything else.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas_layers: Option<DisplayItem>,
    /// A layout bound cut the page short: later content, out-of-range boxes or excess lines are missing.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
    /// Scroll containers with somewhere to scroll, in no particular order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scrollers: Vec<ScrollArea>,
    /// The viewport's propagated `overflow-y` clips (CSS Overflow 3 section 3.3: the root's, or
    /// body's while the root's is visible), so a person's wheel and keys cannot scroll the
    /// document; scripts still can. Blink's `ScrollableArea::UserInputScrollable`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub viewport_locked: bool,
}

/// A formatting box can paint author decoration without inventing a DOM or agent identity.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DisplayDecoration {
    /// Logical coordinates share the same document origin as ordinary display items.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// None permits a border without double-painting a propagated body background.
    pub color: Option<String>,
    pub border: f64,
    pub border_color: Option<String>,
}

/// Native controls and text drawing use exactly these boxes.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DisplayItem {
    /// Measured content and border boxes for layout observers, independent from paint transforms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resize_box: Option<mars_core::geometry::BoxMetrics>,
    /// Canonical canvas pixels shared by native and agent presentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas: Option<mars_core::canvas::Bitmap>,
    /// Stable native identity within one page generation.
    pub local_id: usize,
    /// Supported element tag, never a native class name supplied by the page.
    pub tag: String,
    /// Semantic accessible role.
    pub role: String,
    /// Left edge in document coordinates.
    pub x: f64,
    /// Top edge in document coordinates.
    pub y: f64,
    /// Box width in logical points.
    pub width: f64,
    /// Box height in logical points.
    pub height: f64,
    /// Font selection chosen by owned element defaults.
    pub font_size: f64,
    /// Heading weight used for both measuring and drawing.
    pub bold: bool,
    /// New displays carry the engine's used line advance; older snapshots retain the native default.
    #[serde(default)]
    pub line_height: Option<f64>,
    /// Individually measured strings prevent a platform widget from choosing different wrapping.
    pub lines: Vec<PaintLine>,
    /// Literal colors come only from the owned CSS validator.
    #[serde(default)]
    pub color: Option<String>,
    /// Background and border use the exact same engine box as targeting.
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub border: f64,
    #[serde(default)]
    pub border_color: Option<String>,
    /// Error text belongs to this exact control box and never contains its value.
    #[serde(default)]
    pub validation: Vec<PaintLine>,
    /// Error rows extend the engine box without stretching the native control itself.
    #[serde(default)]
    pub control_height: Option<f64>,
    /// Effective opacity: this element's times every ancestor's, wrappers included.
    /// Painters that composite `stacking_opacity` groups divide those groups back out.
    #[serde(default = "full_opacity", skip_serializing_if = "is_opaque")]
    pub opacity: f64,
    /// Right-to-left base direction for native paragraph shaping.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rtl: bool,
    /// Inline SVG paint in absolute document coordinates; empty for every other element.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vector: Vec<VectorPath>,
    /// Corner radii top-left, top-right, bottom-right, bottom-left, clamped to the box.
    #[serde(default, skip_serializing_if = "display::is_square")]
    pub radius: [f64; 4],
    /// Vertical corner radii in the same order when some corner is elliptical; None when every
    /// corner is circular at `radius`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius_y: Option<[f64; 4]>,
    /// Per-side border widths top, right, bottom, left when they differ; `border` is 0 then.
    #[serde(default, skip_serializing_if = "display::is_square")]
    pub border_widths: [f64; 4],
    /// Per-side literal border colors matching `border_widths`; None paints `border_color`.
    #[serde(default, skip_serializing_if = "display::no_colors")]
    pub border_colors: [Option<String>; 4],
    /// Per-side line styles matching `border_widths`, so painters draw dotted, dashed, double and
    /// the 3D styles as Blink's `BoxBorderPainter` does instead of one solid band.
    #[serde(default, skip_serializing_if = "solid_borders")]
    pub border_styles: [crate::document::BorderStyle; 4],
    /// Box shadows, first listed on top as in CSS.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shadows: Vec<DisplayShadow>,
    /// Gradient and image layers over `background`, first listed on top, with resolved geometry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub backgrounds: Vec<DisplayBackground>,
    /// Absolute clip from overflow-clipping ancestors, already intersected; None paints unclipped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<ClipRect>,
    /// Absolute affine (a, b, c, d, e, f) applied after `clip`; pure translations are baked into geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<[f64; 6]>,
    /// The exact projected corners (top-left, top-right, bottom-right, bottom-left) of a box in a
    /// 3D context, where `transform` is only the affine that paints it. None for planar boxes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quad: Option<[[f64; 2]; 4]>,
    /// Where layout put the box when a transform (its own or an ancestor's) moved its paint
    /// away from there: layout APIs such as `offsetLeft` ignore transforms. None when the box
    /// paints where layout put it, at `x` and `y`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_origin: Option<[f64; 2]>,
    /// z-index of the stacking layer this item paints in; items are already in paint order.
    #[serde(default, skip_serializing_if = "display::is_zero")]
    pub z: i32,
    /// Enclosing translucent stacking contexts, outermost first, composited as groups by painters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stacking_opacity: Vec<OpacityGroup>,
    /// CSS numeric weight used for measuring and drawing; `bold` mirrors weight >= 600.
    #[serde(
        default = "display::normal_weight",
        skip_serializing_if = "display::is_normal_weight"
    )]
    pub font_weight: u16,
    /// Generic face shared by measurement and drawing.
    #[serde(default, skip_serializing_if = "display::is_system")]
    pub font_family: crate::platform::Family,
    /// Extra advance after each character in points.
    #[serde(default, skip_serializing_if = "display::is_zero_f64")]
    pub letter_spacing: f64,
    /// Extra advance after each space and no-break space (CSS `word-spacing`): layout widened
    /// the runs by it, so painters must space the glyphs to match.
    #[serde(default, skip_serializing_if = "display::is_zero_f64")]
    pub word_spacing: f64,
    /// CSS `text-autospace` shaped the runs: painters space ideograph-alpha boundaries as
    /// layout measured them.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub autospace: u8,
    /// Glyph orientation of a vertical item's runs (`platform::upright` codes): its upright
    /// characters advance one em and paint standing, from a split in `vertical.rs`.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub upright: u8,
    /// Lines are underlined: links unless the author removed text decoration, or authored underline.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
    /// Lines are struck through (`text-decoration: line-through`, `<s>`, `<del>`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub strikethrough: bool,
    /// Italic or oblique font posture.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    /// `object-fit` sizing rule for `<img>` elements.
    #[serde(default, skip_serializing_if = "display::is_fill")]
    pub object_fit: super::style::ObjectFit,
    /// `object-position` `[x_px, x_percent, y_px, y_percent]` for `<img>` elements, default `50% 50%`.
    #[serde(
        default = "display::default_object_position",
        skip_serializing_if = "display::is_center_object_position"
    )]
    pub object_position: [f64; 4],
    /// Content box `[x, y, width, height]` of an `<img>` element when padding or border insets it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_box: Option<[f64; 4]>,
    /// A replaced element whose author set `overflow: visible`: its image paints its whole
    /// object rectangle instead of being clipped to the content box (css-overflow overflow-img).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub overflow_visible: bool,
    /// A `url()` mask (an Image layer with resolved geometry): painters fill `background` only
    /// where the fetched mask image is opaque. Icon fonts' replacement, a colored box masked by an
    /// svg, otherwise paints as a solid square or not at all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<DisplayBackground>,
    /// `border-image`: painters draw its `nine_pieces` instead of the border once its source can
    /// paint (a gradient, or the image bytes fetched under this identity).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border_image: Option<DisplayBorderImage>,
    /// `background-clip` as (top, right, bottom, left) insets from the border box; the color
    /// and every layer paint only inside them.
    #[serde(default, skip_serializing_if = "is_zero_sides")]
    pub background_clip: [f64; 4],
    /// `background-clip: border-area`: backgrounds paint only under the border's ring.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub background_border_area: bool,
    /// A rectangle `[x, y, width, height]` in document points the color and every layer are
    /// also clipped to, inside the rounded `background-clip` area: a scrolled `local`
    /// background's `content-box` clip, the scrolled contents' content box within the rounded
    /// padding box. None adds no clip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_rect: Option<[f64; 4]>,
    /// An inline box fragment's left and right sides that a line break cut (`box-decoration-
    /// break: slice`): its outer shadows run on to those edges (`sliced_shadow`).
    #[serde(default, skip_serializing_if = "display::is_flush")]
    pub slice_cut: [bool; 2],
    /// A box kept only for its extent (a captioned table's wrapper, a table row or row group,
    /// which floats, offsets and lookups read) whose decoration other items paint: cell-clipped
    /// copies under each cell, or the table's grid box. Effects never decorate it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bare: bool,
    /// A decorated block whose own text runs paint later, with the inline content (at its
    /// `text_echo` item), so they are not covered by later sibling blocks' backgrounds. Its
    /// lines stay here for agents and text selection; painters skip them on this item.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub text_deferred: bool,
    /// A stand-in with no lines or decoration marking where painters draw the text of the
    /// nearest earlier item of this identity with `text_deferred`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub text_echo: bool,
    /// A fragment of an inline box that comes after a block inside it paints after that block's
    /// content: this is the identity of the block's last descendant, whose tree order it takes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paint_after: Option<usize>,
    /// The block container whose inline formatting context holds this item's runs (its own
    /// identity for a block): painters draw runs of one context as one shaped text, never runs
    /// of two (table cells side by side). Zero when unknown.
    #[serde(default, skip_serializing_if = "is_zero_identity")]
    pub inline_root: usize,
    /// `background-origin` as insets from the border box; None is the padding box.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_origin: Option<[f64; 4]>,
    /// The box sits in a vertical writing mode (`vertical::WRITING_RL` or `WRITING_LR`; 0 is
    /// horizontal): each line's `x`, `y` is the top-left of a run that reads top to bottom,
    /// `width` long, which painters draw turned a quarter clockwise.
    #[serde(default, skip_serializing_if = "is_horizontal")]
    pub writing: u8,
    /// Set on sticky and fixed boxes and everything inside them: how they follow the viewport's
    /// scroll, which hosts apply at paint time without a new layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<Pin>,
}
/** All-solid sides keep the established serialized shape. */
fn solid_borders(styles: &[crate::document::BorderStyle; 4]) -> bool {
    styles
        .iter()
        .all(|style| *style == crate::document::BorderStyle::Solid)
}
/** Items default to fully opaque. */
fn full_opacity() -> f64 {
    1.0
}
/** Fully opaque items keep their established serialized shape. */
fn is_opaque(opacity: &f64) -> bool {
    *opacity == 1.0
}

/// Each line preserves Unicode text and its exact measured native position.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PaintLine {
    /// Literal rendered text, never executable source.
    pub text: String,
    /// Left edge in document coordinates.
    pub x: f64,
    /// Top edge in document coordinates.
    pub y: f64,
    /// Shaped width of this exact string/font combination.
    pub width: f64,
    /// A color for this run alone, over the item's: a list marker under a `::marker` rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/** Whether a small code is zero, the default a display item leaves out. */
fn is_zero_u8(value: &u8) -> bool {
    *value == 0
}
/** Serde skips an unknown inline formatting root. */
fn is_zero_identity(value: &usize) -> bool {
    *value == 0
}
/// Produces bounded native geometry without delegating HTML or flow to AppKit.
pub fn render(document: &Document, viewport: f64) -> EngineResult<DisplayList> {
    render_with_context(
        document,
        RenderContext {
            width: viewport,
            ..RenderContext::default()
        },
    )
}
/// Validated presentation facts drive layout and whole-canvas painting in one transaction.
pub fn render_with_context(
    document: &Document,
    context: RenderContext,
) -> EngineResult<DisplayList> {
    render_scrolled(document, context, &ScrollOffsets::new())
}
/// As `render_with_context`, painting each scroll container's contents at the offset `offsets`
/// asks for, clamped to its range. Layout never depends on them.
pub fn render_scrolled(
    document: &Document,
    context: RenderContext,
    offsets: &ScrollOffsets,
) -> EngineResult<DisplayList> {
    context.validate()?;
    laid_out(document, context, offsets)
}
/// As `render_scrolled` for a document embedded in a frame: `context` is the frame's content
/// box, which `RenderContext::validate_frame` bounds instead of the window bounds.
pub fn render_frame(document: &Document, context: RenderContext) -> EngineResult<DisplayList> {
    render_frame_scrolled(document, context, &ScrollOffsets::new())
}
/// As `render_frame`, painting each scroll container inside the frame at `offsets`: a scripted
/// frame's own page scrolls its containers like a window page does.
pub fn render_frame_scrolled(
    document: &Document,
    context: RenderContext,
    offsets: &ScrollOffsets,
) -> EngineResult<DisplayList> {
    context.validate_frame()?;
    laid_out(document, context, offsets)
}
/** Layout and paint order for a context its caller already validated. */
fn laid_out(
    document: &Document,
    context: RenderContext,
    offsets: &ScrollOffsets,
) -> EngineResult<DisplayList> {
    super::validate_styles(document)?;
    // Hosts lay out their shadow content with slotted light children in place.
    let flat = super::shadow::flatten(document);
    let document = flat.as_ref().unwrap_or(document);
    let degraded = degrade::degrade(document);
    let document = degraded.as_ref().unwrap_or(document);
    let sized = canvases(document);
    let document = sized.as_ref().unwrap_or(document);
    let quirky = quirk_margins(document);
    let document = quirky.as_ref().unwrap_or(document);
    let generated = root_generated::with_root_generated(document);
    let document = generated.as_ref().unwrap_or(document);
    let fitted = root_fit::fit_table_root(document, context)?;
    let document = fitted.as_ref().unwrap_or(document);
    let marked = scroll_markers::expand(document);
    let document = marked.as_ref().map_or(document, |marked| &marked.document);
    // Layout sees the anonymous table boxes; everything after it sees the document again.
    let anonymous = table::anonymous_boxes(document);
    let laid = anonymous.as_ref().map_or(document, |(fixed, _)| fixed);
    let rooted = body::container_root(laid);
    let (mut items, height, body_decoration, mut truncated, resize_wrappers) = match &rooted {
        Some((rooted, root)) => {
            let mut laid = flow::layout(rooted, context)?;
            laid.0.retain(|item| item.local_id != *root);
            laid
        }
        None => flow::layout(laid, context)?,
    };
    if let Some((_, first)) = &anonymous {
        items.retain(|item| item.local_id < *first);
    }
    fragments::split(document, &mut items);
    inline_root::mark(document, &mut items);
    // Zero-size text takes no room and paints nothing; hosts must not draw it at a default size.
    for item in items.iter_mut().filter(|item| item.font_size == 0.0) {
        item.lines.clear();
        item.line_height = item.line_height.filter(|advance| *advance > 0.0);
    }
    upright_split::split(&mut items);
    table_parts(document, &mut items)?;
    effects(document, &mut items)?;
    let (mut items, scrollers) =
        effects::stack(document, items, offsets, [context.width, context.height])?;
    fixed::anchor(document, &mut items, context);
    fixed::scroll_with_contents(document, &mut items, &scrollers);
    first_line::apply(document, &mut items);
    body_clip::apply(document, context, &mut items)?;
    scroll_markers::reclaim(marked.as_ref(), &mut items);
    validate_decoration(body_decoration.as_ref())?;
    let root_decoration = body::root_decoration(document, (context.width, context.height), height);
    validate_decoration(root_decoration.as_ref())?;
    let canvas_layers =
        canvas_layers::canvas_layers(document, (context.width, context.height), height);
    // Boxes outside the geometry bound or past the paint-line bound are dropped, not the page.
    let before = items.len();
    items.retain(|item| validate_geometry(std::slice::from_ref(item), 0.0).is_ok());
    truncated |= items.len() != before;
    truncated |= cap_lines(&mut items);
    let height = height.clamp(0.0, crate::constants::LAYOUT_EXTENT);
    validate_geometry(&items, height)?;
    let root = &document.formatting.html.style;
    let body = &document.formatting.body.style;
    let canvas_background = propagated(root, body).cloned().or_else(|| {
        // Chromium paints the initial canvas from the root's used scheme, which is light unless the
        // page opts into dark; the OS appearance alone never darkens a page.
        Some(match root.used_scheme {
            Appearance::Light => "#ffffff".into(),
            Appearance::Dark => "#000000".into(),
        })
    });
    let scheme = if root.color_scheme == ColorScheme::Normal {
        body
    } else {
        root
    };
    let canvas_foreground = scheme.color.clone().or_else(|| default_foreground(scheme));
    Ok(DisplayList {
        unit: UNIT.into(),
        width: context.width,
        height,
        items,
        resize_wrappers,
        canvas_background,
        canvas_foreground,
        canvas_scheme: root.used_scheme,
        body_decoration,
        root_decoration,
        canvas_layers,
        truncated,
        scrollers,
        viewport_locked: viewport_locked(root, body),
    })
}
/** The viewport takes the root's overflow, or body's when the root's is visible on both axes;
 * `hidden` and `clip` there stop user scrolling of the document. */
fn viewport_locked(root: &super::ComputedStyle, body: &super::ComputedStyle) -> bool {
    use super::Overflow::{Clip, Hidden, Visible};
    let visible = root.overflow_x == Visible && root.overflow_y == Visible;
    let used = if visible { body } else { root };
    matches!(used.overflow_y, Hidden | Clip)
}
/** HTML's canvas natural size: its `width` and `height` attributes when they are valid
non-negative integers, else 300 by 150, read at every layout so a script's resize applies. The
bitmap itself is transparent until something draws on it. A video with no reported size yet
gets the same 300 by 150 default object size; CSS sizes it like any replaced element. */
fn canvases(document: &Document) -> Option<Document> {
    let canvas =
        |node: &Node| matches!(node.tag.as_str(), "canvas" | "video") && node.image_size.is_none();
    if !document.nodes.iter().any(canvas) {
        return None;
    }
    let mut copy = document.clone();
    for node in copy.nodes.iter_mut().filter(|node| canvas(node)) {
        let side = |name: &str, default: u32| {
            node.attributes
                .get(name)
                .and_then(|value| mars_core::canvas::dimension(value))
                .unwrap_or(default)
        };
        // A video's own size arrives with its metadata (the app sets it). Until then its
        // width and height attributes give its shape, as their mapped aspect-ratio does in
        // browsers, else HTML's default object size; a canvas always reads its attributes.
        node.image_size = Some((side("width", CANVAS_SIZE.0), side("height", CANVAS_SIZE.1)));
    }
    Some(copy)
}
/** HTML's default canvas bitmap size. */
const CANVAS_SIZE: (u32, u32) = (
    mars_core::canvas::DEFAULT_SIZE[0],
    mars_core::canvas::DEFAULT_SIZE[1],
);
/** A copy with the quirks-mode margin quirk applied: the UA top margin of the first box in the
 * body or a table cell is laid out as zero. None when no box has it. */
fn quirk_margins(document: &Document) -> Option<Document> {
    if !document.nodes.iter().any(|node| node.style.quirk_top) {
        return None;
    }
    let mut copy = document.clone();
    for node in copy.nodes.iter_mut().filter(|node| node.style.quirk_top) {
        node.style.margin[0] = 0.0;
    }
    Some(copy)
}
/** Keeps at most PAINT_LINES runs in paint order; true when any were dropped. */
fn cap_lines(items: &mut [DisplayItem]) -> bool {
    let mut left = PAINT_LINES;
    let mut dropped = false;
    for item in items {
        let count = item.lines.len() + item.validation.len();
        if count <= left {
            left -= count;
            continue;
        }
        dropped = true;
        item.validation.clear();
        item.lines.truncate(left);
        left -= item.lines.len();
    }
    dropped
}
/** Effective opacity and base direction come from the actual document after layout. */
fn effects(document: &Document, items: &mut [DisplayItem]) -> EngineResult<()> {
    let wrappers = document.formatting.html.style.opacity * document.formatting.body.style.opacity;
    let positions = sizing::positions(document);
    for item in items {
        let node = find(document, &positions, item.local_id)?;
        item.rtl = paragraph(document, &positions, node)?.style.rtl;
        // Layout hides boxes by zeroing their opacity (a line clamp, an ellipsis), so keep that.
        item.opacity *= wrappers;
        // A `::before`/`::after` box shares its element's identity but has its own opacity, which
        // pages use to hide hover and focus overlays; without it they paint as solid blocks.
        let generated = effects::generated_style(node, item);
        if let Some(style) = generated {
            item.opacity *= style.opacity;
        }
        // `visibility: hidden` keeps the box's room but paints nothing of it.
        if generated.map_or(node.style.invisible, |style| style.invisible) {
            item.opacity = 0.0;
        }
        let mut current = Some(node);
        while let Some(node) = current {
            item.opacity *= node.style.opacity;
            current = match node.parent {
                Some(parent) => Some(find(document, &positions, parent)?),
                None => None,
            };
        }
    }
    Ok(())
}
/** The box whose direction is a run's base direction when painted: an inline box's or text's
nearest block or embedding inline (`unicode-bidi` other than normal, or a `dir` attribute, which
isolates). A `unicode-bidi: normal` inline's own `direction` does nothing; shaping such an RTL
span right to left reordered its text (bidi-normal-001). */
fn paragraph<'a>(
    document: &'a Document,
    positions: &[Option<usize>],
    mut node: &'a Node,
) -> EngineResult<&'a Node> {
    let embeds = |node: &Node| node.style.unicode_bidi != 0 || node.attributes.contains_key("dir");
    while node.style.inline && !node.style.inline_block && !embeds(node) {
        let Some(parent) = node.parent else {
            break;
        };
        node = find(document, positions, parent)?;
    }
    Ok(node)
}
/** Filled table parts considered per render; later ones paint no background box. Real tables
 * have a handful; past this a page is hostile, and table_parts runs after layout's budget. */
const TABLE_PART_LIMIT: usize = 1_024;
/**
 * Blink's `TableSectionPainter` and `TableColumnPainter`: a row group, column or column group
 * has no box in this layout, yet its background paints behind the cells it covers, above the
 * table's own and below rows and cells. Each such part with a background gets one box over its
 * cells, placed just before the table's first row or cell, so `display: table-row-group;
 * background: black` fills the section instead of vanishing.
 *
 * Every table's cells and column ranges are found once per render, not once per part, and at
 * most `TABLE_PART_LIMIT` parts are considered, so thousands of filled parts stay linear.
 *
 * ponytail: one box over the part's cells, not one clipped copy per cell, so border-spacing
 * gaps fill too; split per cell if a spaced table with a section background ever shows it.
 */
fn table_parts(document: &Document, items: &mut Vec<DisplayItem>) -> EngineResult<()> {
    use super::style::TableBox;
    let positions = sizing::positions(document);
    let painted: std::collections::HashSet<usize> = items.iter().map(|i| i.local_id).collect();
    let mut tables: Option<std::collections::HashMap<usize, TableFacts>> = None;
    let mut added = Vec::new();
    let mut considered = 0;
    for node in &document.nodes {
        let group = matches!(
            node.style.table_box,
            TableBox::RowGroup | TableBox::HeaderGroup | TableBox::FooterGroup
        );
        let filled = node.style.background.is_some() || !node.style.backgrounds.is_empty();
        if !(group || node.style.table_box == TableBox::Column)
            || !filled
            || painted.contains(&node.local_id)
        {
            continue;
        }
        considered += 1;
        if considered > TABLE_PART_LIMIT {
            break;
        }
        let Some(table) = table_of(document, &positions, node)? else {
            continue;
        };
        let tables = match &mut tables {
            Some(tables) => tables,
            None => tables.insert(table_facts(document, &positions, items)?),
        };
        let Some(facts) = tables.get(&table) else {
            continue;
        };
        let cells = &facts.cells;
        let Some(&(first, _)) = cells.first() else {
            continue;
        };
        let rect = if group {
            let mut within = Vec::new();
            for &(index, _) in cells {
                if descends(document, &positions, items[index].local_id, node.local_id)? {
                    within.push(rect_of(&items[index]));
                }
            }
            union(within)
        } else {
            column_rect(items, cells, facts.columns.get(&node.local_id).copied())
        };
        if let Some([x, y, width, height]) = rect {
            added.push((first, flow::box_item(node, x, y, width, height, Vec::new())));
        }
    }
    // Same slot: later parts insert first, so document order (columns, then groups) survives.
    for (at, item) in added.into_iter().rev() {
        items.insert(at, item);
    }
    Ok(())
}
/** The nearest `display: table` ancestor of a table part. */
fn table_of(
    document: &Document,
    positions: &[Option<usize>],
    node: &Node,
) -> EngineResult<Option<usize>> {
    let mut current = node.parent;
    while let Some(local) = current {
        let ancestor = find(document, positions, local)?;
        if ancestor.style.table_box == super::style::TableBox::Table || ancestor.tag == "table" {
            return Ok(Some(local));
        }
        current = ancestor.parent;
    }
    Ok(None)
}
/** One table's paint facts, shared by all of its parts. */
#[derive(Default)]
struct TableFacts {
    /** Indexes of its row and cell items in paint order, each flagged true for a cell. */
    cells: Vec<(usize, bool)>,
    /** The leaf column range `[start, end)` of each column and column group, by identity. */
    columns: std::collections::HashMap<usize, (usize, usize)>,
}
/** Every table that has rows or cells, with its facts, from one pass over the items and one
 * parent-to-children index of the document. */
fn table_facts(
    document: &Document,
    positions: &[Option<usize>],
    items: &[DisplayItem],
) -> EngineResult<std::collections::HashMap<usize, TableFacts>> {
    use super::style::TableBox;
    let mut tables = std::collections::HashMap::<usize, TableFacts>::new();
    for (index, item) in items.iter().enumerate() {
        let node = find(document, positions, item.local_id)?;
        if !matches!(node.style.table_box, TableBox::Row | TableBox::Cell) {
            continue;
        }
        if let Some(table) = table_of(document, positions, node)? {
            let cell = node.style.table_box == TableBox::Cell;
            tables.entry(table).or_default().cells.push((index, cell));
        }
    }
    let mut children = std::collections::HashMap::<usize, Vec<&Node>>::new();
    for node in &document.nodes {
        if let Some(parent) = node.parent {
            children.entry(parent).or_default().push(node);
        }
    }
    for (table, facts) in &mut tables {
        facts.columns = column_ranges(&children, *table);
    }
    Ok(tables)
}
/** Whether `ancestor` is on `local`'s parent chain. */
fn descends(
    document: &Document,
    positions: &[Option<usize>],
    local: usize,
    ancestor: usize,
) -> EngineResult<bool> {
    let mut current = find(document, positions, local)?.parent;
    while let Some(parent) = current {
        if parent == ancestor {
            return Ok(true);
        }
        current = find(document, positions, parent)?.parent;
    }
    Ok(false)
}
/** An item's border box as `[x, y, width, height]`. */
fn rect_of(item: &DisplayItem) -> [f64; 4] {
    [item.x, item.y, item.width, item.height]
}
/** The smallest rectangle holding every one given; None for none. */
fn union(rects: impl IntoIterator<Item = [f64; 4]>) -> Option<[f64; 4]> {
    rects.into_iter().fold(None, |acc, [x, y, w, h]| {
        let [ax, ay, aw, ah] = acc.unwrap_or([x, y, w, h]);
        let (left, top) = (ax.min(x), ay.min(y));
        let (right, bottom) = ((ax + aw).max(x + w), (ay + ah).max(y + h));
        Some([left, top, right - left, bottom - top])
    })
}
/** The leaf columns (`span` each) every column and column group of `table` covers, by identity,
 * from its column elements in document order: Blink's `LayoutTableColumn` visit order. */
fn column_ranges(
    children: &std::collections::HashMap<usize, Vec<&Node>>,
    table: usize,
) -> std::collections::HashMap<usize, (usize, usize)> {
    let span = |node: &Node| {
        node.attributes
            .get("span")
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(1)
            .max(1)
    };
    let is_column = |node: &&Node| node.style.table_box == super::style::TableBox::Column;
    let kids = |local: usize| children.get(&local).into_iter().flatten().copied();
    let mut ranges = std::collections::HashMap::new();
    let mut at: usize = 0;
    for part in kids(table).filter(is_column) {
        let inner: Vec<&Node> = kids(part.local_id).filter(is_column).collect();
        let own = if inner.is_empty() {
            span(part)
        } else {
            inner
                .iter()
                .fold(0, |sum: usize, n| sum.saturating_add(span(n)))
        };
        let end = at.saturating_add(own);
        ranges.entry(part.local_id).or_insert((at, end));
        for leaf in &inner {
            let next = at.saturating_add(span(leaf));
            ranges.entry(leaf.local_id).or_insert((at, next));
            at = next;
        }
        if inner.is_empty() {
            at = end;
        }
    }
    ranges
}
/** A column's box: its leaf columns' x extents (each column starts at a distinct cell left
 * edge) across the full height of the table's cells. */
fn column_rect(
    items: &[DisplayItem],
    cells: &[(usize, bool)],
    range: Option<(usize, usize)>,
) -> Option<[f64; 4]> {
    let (start, end) = range?;
    let cells: Vec<&DisplayItem> = cells
        .iter()
        .filter(|(_, cell)| *cell)
        .map(|&(i, _)| &items[i])
        .collect();
    let mut lefts: Vec<f64> = cells.iter().map(|c| c.x).collect();
    lefts.sort_by(f64::total_cmp);
    lefts.dedup_by(|a, b| (*a - *b).abs() < crate::constants::LAYOUT_EPSILON);
    let [_, top, _, height] = union(cells.iter().map(|c| rect_of(c)))?;
    let columns = (start..end.min(lefts.len())).filter_map(|k| {
        let width = cells
            .iter()
            .filter(|c| (c.x - lefts[k]).abs() < crate::constants::LAYOUT_EPSILON)
            .map(|c| c.width)
            .fold(f64::INFINITY, f64::min);
        width.is_finite().then_some([lefts[k], top, width, height])
    });
    union(columns)
}
/** Identity lookup never assumes local identities equal Vec indexes after scripts run. */
fn find<'a>(
    document: &'a Document,
    positions: &[Option<usize>],
    local: usize,
) -> EngineResult<&'a Node> {
    positions
        .get(local)
        .copied()
        .flatten()
        .map(|position| &document.nodes[position])
        .ok_or_else(|| EngineError::invalid("Invalid layout node identity."))
}

/// Agents never target `aria-hidden=true` subtrees or the inside of an svg drawing.
pub fn agent_hidden(document: &Document, node: &Node) -> bool {
    let mut current = Some(node);
    let mut first = true;
    while let Some(node) = current {
        if node
            .attributes
            .get("aria-hidden")
            .is_some_and(|value| value.eq_ignore_ascii_case("true"))
            || (!first && node.tag == "svg")
        {
            return true;
        }
        first = false;
        current = node
            .parent
            .and_then(|parent| {
                mars_core::dom::local_position(&document.nodes, parent, |node| node.local_id)
            })
            .map(|position| &document.nodes[position]);
    }
    false
}
/** Decoration shares the finite geometry and validated literal colors of actionable display items. */
fn validate_decoration(decoration: Option<&DisplayDecoration>) -> EngineResult<()> {
    if let Some(decoration) = decoration {
        for value in [
            decoration.x,
            decoration.y,
            decoration.width,
            decoration.height,
            decoration.border,
        ] {
            validation::bounded(value)?;
        }
    }
    Ok(())
}
/** The background color the canvas takes: the root's, else the body's (CSS Backgrounds 2.11.2).
 * A root with no box (`display: none`) gives none, nor does its body; a body without a box
 * of its own (`display: none` or `contents`) does not propagate. */
fn propagated<'a>(
    root: &'a super::ComputedStyle,
    body: &'a super::ComputedStyle,
) -> Option<&'a String> {
    if root.hidden {
        return None;
    }
    opaque(&root.background).or_else(|| {
        (!body.hidden && !body.display_contents)
            .then(|| opaque(&body.background))
            .flatten()
    })
}
/** A color that paints: neither absent nor `transparent`; a transparent root's paint
 * delegates to the body's. */
fn opaque(color: &Option<String>) -> Option<&String> {
    color
        .as_ref()
        .filter(|color| color.as_str() != "transparent")
}
/** The initial text color of the used scheme, as Chromium's CanvasText; never the OS's own. */
fn default_foreground(style: &super::ComputedStyle) -> Option<String> {
    Some(match style.used_scheme {
        Appearance::Light => "#000000".into(),
        Appearance::Dark => "#ffffff".into(),
    })
}

/// Semantic roles expose the same interactions as the visible native controls.
/// An explicit `role` attribute wins when its first token is a known ARIA role, except on native
/// form controls, whose role decides which native widget and actions they get.
pub fn role(node: &Node) -> &'static str {
    let native = ["input", "button", "select", "textarea"].contains(&node.tag.as_str());
    if !native
        && let Some(explicit) = node.attributes.get("role").and_then(|value| {
            let token = value.split_ascii_whitespace().next()?.to_ascii_lowercase();
            ARIA_ROLES.iter().find(|role| **role == token).copied()
        })
    {
        return explicit;
    }
    match node.tag.as_str() {
        "h1" | "h2" | "h3" => "heading",
        "table" => "table",
        "tr" => "row",
        "td" => "cell",
        "th" if node
            .attributes
            .get("scope")
            .is_some_and(|scope| scope == "row") =>
        {
            "rowheader"
        }
        "th" => "columnheader",
        "caption" => "caption",
        "thead" | "tbody" | "tfoot" => "rowgroup",
        "div" | "form" | "section" | "main" | "fieldset" => "group",
        "a" if node.href.is_some() => "link",
        "img" | "svg" => "image",
        "input" if node.input_type() == "password" => "password",
        "input" if node.input_type() == "checkbox" => "checkbox",
        "input" if node.input_type() == "radio" => "radio",
        "input" if node.input_type() == "submit" => "button",
        "input" | "textarea" => "textbox",
        "select" => "combobox",
        "option" => "option",
        "button" => "button",
        _ => "text",
    }
}

/// WAI-ARIA 1.2 roles a page may declare; anything else falls back to the element's own role.
const ARIA_ROLES: [&str; 70] = [
    "alert",
    "alertdialog",
    "application",
    "article",
    "banner",
    "blockquote",
    "button",
    "caption",
    "cell",
    "checkbox",
    "code",
    "columnheader",
    "combobox",
    "complementary",
    "contentinfo",
    "definition",
    "dialog",
    "document",
    "feed",
    "figure",
    "form",
    "generic",
    "grid",
    "gridcell",
    "group",
    "heading",
    "img",
    "image",
    "link",
    "list",
    "listbox",
    "listitem",
    "log",
    "main",
    "marquee",
    "math",
    "menu",
    "menubar",
    "menuitem",
    "menuitemcheckbox",
    "menuitemradio",
    "meter",
    "navigation",
    "none",
    "note",
    "option",
    "paragraph",
    "presentation",
    "progressbar",
    "radio",
    "radiogroup",
    "region",
    "row",
    "rowgroup",
    "rowheader",
    "scrollbar",
    "search",
    "searchbox",
    "separator",
    "slider",
    "spinbutton",
    "status",
    "switch",
    "tab",
    "table",
    "tablist",
    "tabpanel",
    "textbox",
    "timer",
    "tooltip",
];
/** Private styles and arithmetic must never send NaN, infinity or excessive extents to AppKit. */
fn validate_geometry(items: &[DisplayItem], height: f64) -> EngineResult<()> {
    validation::bounded(height)?;
    for item in items {
        validation::offset(item.x)?;
        validation::offset(item.y)?;
        for value in [item.width, item.height, item.font_size, item.border] {
            validation::bounded(value)?;
        }
        if let Some(advance) = item.line_height {
            validation::bounded(advance)?;
            if advance <= 0.0 {
                return Err(EngineError::invalid("Invalid rendered line advance."));
            }
        }
        if let Some(height) = item.control_height {
            validation::bounded(height)?;
        }
        if !(0.0..=1.0).contains(&item.opacity) {
            return Err(EngineError::invalid("Invalid rendered opacity."));
        }
        for path in &item.vector {
            validation::vector(path)?;
        }
        display::validate(item)?;
        for line in item.lines.iter().chain(&item.validation) {
            validation::offset(line.x)?;
            validation::offset(line.y)?;
            validation::bounded(line.width)?;
        }
    }
    Ok(())
}
/** Serde skip test for insets that are all zero. */
fn is_zero_sides(sides: &[f64; 4]) -> bool {
    *sides == [0.0; 4]
}
/** A `unicode-bidi` keyword as its `ComputedStyle::unicode_bidi` code. */
pub(in crate::document) fn bidi_mode(value: &str) -> Option<u8> {
    bidi::mode(value)
}
/** The used line advance layout gives a style (its font's normal line spacing for `normal`), so
`lh` units match the lines they measure. */
pub(in crate::document) fn line_advance(style: &super::ComputedStyle) -> f64 {
    sizing::advance(style)
}
/** Serde skip test for horizontal text. */
fn is_horizontal(writing: &u8) -> bool {
    *writing == 0
}
