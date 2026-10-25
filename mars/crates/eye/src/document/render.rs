/** Oya owns point-based block flow, wrapping and control placement. */
mod blur;
mod body;
mod control;
pub(crate) mod degrade;
mod display;
mod effects;
mod flex;
mod flex_column;
mod flex_layout;
mod float;
mod flow;
mod generated;
mod grid;
mod image;
mod line;
mod marker;
mod path;
mod positioned;
mod sizing;
mod svg;
mod table;
mod text;
mod validation;
mod wrap;
use super::{Appearance, ColorScheme, Document, Node, RenderContext};
use crate::{
    constants::PAINT_LINES,
    protocol::{EngineError, EngineResult},
};
pub use display::{
    ClipRect, DisplayBackground, DisplayShadow, GradientStop, ImageSize, OpacityGroup, Pin,
    ScrollArea,
};
/// Agent-visibility helpers for the app; the document facade exports them when the app wires them.
#[allow(unused_imports)]
pub use display::{agent_visible, painted_bounds, wheel_target};
pub use effects::ScrollOffsets;
use serde::{Deserialize, Serialize};
pub use svg::{FillRule, LineCap, LineJoin, PathCommand, VectorPath};

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
    /// A `url()` mask (an Image layer with resolved geometry): painters fill `background` only
    /// where the fetched mask image is opaque. Icon fonts' replacement, a colored box masked by an
    /// svg, otherwise paints as a solid square or not at all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<DisplayBackground>,
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
    super::validate_styles(document)?;
    // Hosts lay out their shadow content with slotted light children in place.
    let flat = super::shadow::flatten(document);
    let document = flat.as_ref().unwrap_or(document);
    let degraded = degrade::degrade(document);
    let document = degraded.as_ref().unwrap_or(document);
    let quirky = quirk_margins(document);
    let document = quirky.as_ref().unwrap_or(document);
    let rooted = body::container_root(document);
    let (mut items, height, body_decoration, mut truncated) = match &rooted {
        Some((rooted, root)) => {
            let mut laid = flow::layout(rooted, context)?;
            laid.0.retain(|item| item.local_id != *root);
            laid
        }
        None => flow::layout(document, context)?,
    };
    // Zero-size text takes no room and paints nothing; hosts must not draw it at a default size.
    for item in items.iter_mut().filter(|item| item.font_size == 0.0) {
        item.lines.clear();
        item.line_height = item.line_height.filter(|advance| *advance > 0.0);
    }
    effects(document, &mut items)?;
    let (mut items, scrollers) = effects::stack(document, items, offsets)?;
    validate_decoration(body_decoration.as_ref())?;
    // Boxes outside the geometry bound or past the paint-line bound are dropped, not the page.
    let before = items.len();
    items.retain(|item| validate_geometry(std::slice::from_ref(item), 0.0).is_ok());
    truncated |= items.len() != before;
    truncated |= cap_lines(&mut items);
    let height = height.clamp(0.0, crate::constants::LAYOUT_EXTENT);
    validate_geometry(&items, height)?;
    let root = &document.formatting.html.style;
    let body = &document.formatting.body.style;
    let canvas_background = opaque(&root.background)
        .or_else(|| opaque(&body.background))
        .cloned()
        .or_else(|| {
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
        canvas_background,
        canvas_foreground,
        canvas_scheme: root.used_scheme,
        body_decoration,
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
        item.rtl = node.style.rtl;
        item.opacity = wrappers;
        // A `::before`/`::after` box shares its element's identity but has its own opacity, which
        // pages use to hide hover and focus overlays; without it they paint as solid blocks.
        if let Some(style) = effects::generated_style(node, item) {
            item.opacity *= style.opacity;
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
/** Transparent root paint delegates to the HTML body's canvas propagation rule. */
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
