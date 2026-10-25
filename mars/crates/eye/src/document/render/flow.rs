/** Owned block and inline formatting lays out native controls and literal shaped text. */
pub(super) use super::{CONTENT_WIDTH, DisplayItem, MARGIN, PaintLine};
pub(super) use crate::{
    document::{Document, Node},
    protocol::{EngineError, EngineResult},
};

/** Each containing block owns one inline cursor and measured line height. */
pub(super) struct Flow {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub cursor: f64,
    pub line: f64,
    pub bottom: f64,
    /** The adjoining bottom (or parent top) margin already counted in `y`, which the next block collapses with. */
    pub collapsible: f64,
    /** Fragments on the current line, aligned when it finishes. */
    pub frags: Vec<super::line::Frag>,
    /** Finished lines whose offsets `settle` still has to apply. */
    pub finished: Vec<super::line::Finished>,
    /** The containing block's own font metrics, part of every line. */
    pub strut: Option<super::line::Strut>,
    /** The containing block's text-align. */
    pub align: super::line::Align,
    /** The block whose `text-overflow: ellipsis` truncates this flow's overflowing lines. */
    pub ellipsis: Option<usize>,
    /** Floats of this block formatting context that can still shorten lines. */
    pub floats: Vec<super::float::FloatBox>,
    /** Lowest bottom of floats pruned from `floats`, still honored by clearance. */
    pub cleared: Option<f64>,
    /** Definite content height of the containing block, for percentage heights. */
    pub definite: Option<f64>,
    /** Whether a space separates the next run: Some(true) after trailing whitespace, Some(false)
     * right after text, None after an element closed, where the parser dropped any whitespace-only text. */
    pub space: Option<bool>,
    /** Blink's unresolved BFC block offset: the parent's top margin collapses through to the first
     * child, so the parent's border edge is not placed until in-flow content resolves it. */
    pub strut_open: bool,
    /** Where the margin strut resolved, the first in-flow block's border top (Blink's resolved
     * BFC block offset); None while still open or when a line box resolved it in place. */
    pub resolved: Option<f64>,
    /** The border top `block_top` last computed, which `after_block` records on resolution. */
    last_top: std::cell::Cell<f64>,
    /** Floats placed while the margin strut was open, as (index in `floats`, item range, top
     * they were given); Blink positions them only once the BFC block offset resolves. */
    pub unpositioned: Vec<(usize, std::ops::Range<usize>, f64)>,
}
impl Flow {
    /** A block starts at its content origin with no invented glyph width. */
    pub fn new(x: f64, y: f64, width: f64) -> Self {
        Self {
            x,
            y,
            width,
            cursor: 0.0,
            line: 0.0,
            bottom: y,
            collapsible: 0.0,
            frags: Vec::new(),
            finished: Vec::new(),
            strut: None,
            align: super::line::Align::Left,
            ellipsis: None,
            floats: Vec::new(),
            cleared: None,
            definite: None,
            space: Some(false),
            strut_open: false,
            resolved: None,
            last_top: std::cell::Cell::new(y),
            unpositioned: Vec::new(),
        }
    }
    /** A child flow inside this one that shares its floats, unless it starts a new formatting context. */
    pub fn nested(&self, x: f64, y: f64, width: f64, fresh: bool) -> Self {
        let mut inner = Self::new(x, y, width);
        if !fresh {
            inner.floats = self.floats.clone();
            inner.cleared = self.cleared;
        }
        inner
    }
    /** Records a fragment placed on the current line. */
    pub fn push(&mut self, frag: super::line::Frag) {
        self.frags.push(frag);
    }
    /** The top of a block whose top margin collapses with the margin already counted above it. */
    pub fn block_top(&self, margin: f64) -> f64 {
        let top = self.y - self.collapsible + collapse(self.collapsible, margin);
        self.last_top.set(top);
        top
    }
    /** A block placed somewhere other than `block_top`, such as below floats it clears. */
    pub fn placed_at(&self, top: f64) {
        self.last_top.set(top);
    }
    /** Advances past a block and remembers its bottom margin for collapsing with the next one. */
    pub fn after_block(&mut self, bottom: f64, margin: f64) {
        if std::mem::take(&mut self.strut_open) {
            self.resolved = Some(self.last_top.get());
        }
        self.y = bottom + margin;
        self.collapsible = margin;
        self.bottom = self.bottom.max(self.y);
    }
    /** A completed line advances by its baseline-aligned height. */
    pub fn newline(&mut self) {
        if self.cursor > 0.0 || self.line > 0.0 || !self.frags.is_empty() {
            // A line box resolves an open margin strut where it stands.
            self.strut_open = false;
            let aligned = super::line::finish(self);
            self.y += self.line.max(aligned);
            self.bottom = self.bottom.max(self.y);
            self.collapsible = 0.0;
        }
        self.cursor = 0.0;
        self.line = 0.0;
        self.space = Some(false);
    }
}
/** Moves floats placed before the margin strut resolved down by `delta`, where the resolved
 * block offset puts them (Blink's PositionPendingFloats). A float before a block whose top
 * margin collapses through its parent sat that margin too high. */
fn position_floats(inner: &mut Flow, items: &mut [DisplayItem], delta: f64) {
    for (index, range, _) in std::mem::take(&mut inner.unpositioned) {
        if let Some(float) = inner.floats.get_mut(index) {
            float.top += delta;
            float.bottom += delta;
        }
        for item in items.iter_mut().take(range.end).skip(range.start) {
            super::line::translate(item, 0.0, delta);
        }
    }
}
/** The highest top at or below `y` where a formatting-context root with an authored width fits
 * between the floats, with that opportunity's left edge and width; an auto-width one fits
 * anywhere, shrinking to the room. Blink's LayoutNewFormattingContext also rejects an
 * opportunity shorter than the fragment, so an authored height is checked over its whole
 * extent; ponytail: an auto height is checked over one line, a trial layout per opportunity
 * would cover it. */
fn beside_floats(flow: &Flow, node: &Node, mut y: f64) -> (f64, f64, f64) {
    let style = &node.style;
    let height = specified_height(node, None, flow.definite);
    let room = |y: f64| match height {
        Some(height) => flow.room_at_height(y, height.max(super::float::LINE_FIT_SLACK)),
        None => flow.room_at(y),
    };
    if style.width.is_none() {
        let (left, width) = room(y);
        return (y, left, width);
    }
    let margins = style.margin[1] + style.margin[3];
    let needed = border_width(node, flow.width, flow.width, None) + margins;
    let extent = height.unwrap_or(0.0);
    for _ in 0..=flow.floats.len() {
        let (_, width) = room(y);
        let next = flow
            .floats
            .iter()
            .filter(|f| f.top < y + extent.max(super::float::LINE_FIT_SLACK) && f.bottom > y)
            .map(|f| f.bottom)
            .fold(f64::INFINITY, f64::min);
        if width + super::float::LINE_FIT_SLACK >= needed || !next.is_finite() {
            break;
        }
        y = next;
    }
    let (left, width) = room(y);
    (y, left, width)
}
/** css-overflow-3: `text-overflow` applies only to a block container that clips its overflow. */
pub(super) fn ellipsis_owner(node: &Node) -> Option<usize> {
    let clips = node.style.overflow != crate::document::style::Overflow::Visible;
    (node.style.text_overflow_ellipsis && clips).then_some(node.local_id)
}
/** CSS 2.1 margin collapsing: the largest positive margin plus the most negative one. */
pub(super) fn collapse(a: f64, b: f64) -> f64 {
    a.max(b).max(0.0) + a.min(b).min(0.0)
}
/** Items, document height, body decoration, and whether a bound cut layout short. */
pub(super) type Laid = (
    Vec<DisplayItem>,
    f64,
    Option<super::DisplayDecoration>,
    bool,
);
/**
 * Percentage paddings and margins resolve against their containing block's width, which only a
 * layout knows: a first layout measures those widths, the percentages become pixels, and a
 * second layout uses them. Bounded to that one extra pass; pages without them lay out once.
 */
pub(super) fn layout(
    document: &Document,
    presentation: crate::document::RenderContext,
) -> EngineResult<Laid> {
    // The page's loaded fonts must be on this thread before any run is measured.
    crate::document::style::install_fonts(&document.formatting);
    let collapsed = super::table::collapse_borders(document)?;
    let document = collapsed.as_ref().unwrap_or(document);
    let body = super::sizing::has_percent_sides(&document.formatting.body.style);
    let nodes = document.nodes.iter().any(|node| {
        super::sizing::has_percent_sides(&node.style)
            || node
                .style
                .generated
                .iter()
                .any(|generated| super::sizing::has_percent_sides(&generated.style))
    });
    if !body && !nodes {
        return layout_once(document, presentation);
    }
    let items = if nodes {
        layout_once(document, presentation)?.0
    } else {
        Vec::new()
    };
    let resolved = super::sizing::resolve_percent_sides(document, &items, presentation.width);
    layout_once(&resolved, presentation)
}
/** Top-level children share a virtual body formatting context and one request budget. */
fn layout_once(
    document: &Document,
    presentation: crate::document::RenderContext,
) -> EngineResult<Laid> {
    let mut context = LayoutContext::new(document)?;
    let mut items = Vec::new();
    let body = &document.formatting.body.style;
    let authored = document.formatting.body_box_authored;
    let (x, y, width) = if authored {
        super::body::content(body, presentation.width)
    } else {
        (
            MARGIN,
            MARGIN,
            (presentation.width - MARGIN * 2.0).min(CONTENT_WIDTH),
        )
    };
    let mut flow = Flow::new(x, y, width);
    flow.strut = Some(strut(body));
    flow.align = alignment(body);
    flow.definite = body
        .height
        .and_then(|height| height.resolve_size(Some(presentation.height)))
        .map(|height| super::sizing::content_height(body, height));
    if authored && body.padding[0] == 0.0 && super::sizing::border_sides(body)[0] == 0.0 {
        // The body's top margin collapses with its first child's, as in Chromium.
        flow.collapsible = body.margin[0];
    }
    if !body.hidden {
        let mut run = None;
        for node in document.nodes.iter().filter(|node| node.parent.is_none()) {
            if !inline_level(node) {
                root_line(&mut context, &mut flow, &mut items, run.take())?;
            } else if run.is_none() {
                run = Some(items.len());
            }
            // A page past the work bound keeps what was laid out instead of being refused.
            if let Err(error) = node_layout(&mut context, node, &mut flow, &mut items, None) {
                if !context.exhausted() {
                    return Err(error);
                }
                break;
            }
        }
        if context.exhausted() {
            context.reserve();
            run = None;
        }
        root_line(&mut context, &mut flow, &mut items, run)?;
    }
    flow.newline();
    super::line::settle(&mut context, &mut flow, &mut items, 0)?;
    flow.bottom = flow.bottom.max(flow.floats_bottom());
    let truncated = context.exhausted();
    let positioned = match super::positioned::place(
        &mut context,
        &mut items,
        (presentation.width, presentation.height),
    ) {
        Ok(bottom) => bottom,
        Err(_) if context.exhausted() => 0.0,
        Err(error) => return Err(error),
    };
    flow.bottom = flow.bottom.max(positioned);
    if truncated {
        let painted = items
            .iter()
            .map(|item| item.y + item.height)
            .filter(|bottom| bottom.is_finite())
            .fold(0.0, f64::max);
        flow.bottom = flow.bottom.max(painted);
    }
    let bottom = if authored {
        body.padding[2] + super::sizing::border_sides(body)[2] + body.margin[2]
    } else {
        MARGIN
    };
    let height = super::body::clipped_height(document, flow.definite)
        .map_or(flow.bottom.max(flow.y), |clip| y + clip)
        + bottom;
    let decoration = super::body::decoration(document, x, y, width, height - body.margin[2]);
    Ok((items, height, decoration, context.exhausted()))
}
/** Consecutive root-level inline nodes share one anonymous body line, aligned like body text. */
fn root_line(
    context: &mut LayoutContext<'_>,
    flow: &mut Flow,
    items: &mut [DisplayItem],
    run: Option<usize>,
) -> EngineResult<()> {
    if run.is_none() {
        return Ok(());
    }
    flow.newline();
    super::line::settle(context, flow, items, 0)
}
/** The font metrics every line of a block includes. */
pub(super) fn strut(style: &crate::document::ComputedStyle) -> super::line::Strut {
    super::line::Strut {
        ascent: super::sizing::text_ascent(style),
        height: super::sizing::advance(style),
        font: style.font_size,
    }
}
/** text-align resolved against the base direction; justify lays out as start. */
pub(super) fn alignment(style: &crate::document::ComputedStyle) -> super::line::Align {
    use super::line::Align;
    match (style.align.as_str(), style.rtl) {
        ("center" | "webkit-center", _) => Align::Center,
        ("right", _) | ("start", true) | ("end", false) => Align::Right,
        _ => Align::Left,
    }
}
/** Lays out one node, then records whether source whitespace separates it from the next run:
 * only when its own text ended in a space or whitespace followed it in the source, so adjacent
 * inline elements (highlighted code tokens) touch as in Chromium. */
pub(super) fn node_layout(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
    forced: Option<BorderBox>,
) -> EngineResult<()> {
    let inline_run = forced.is_none()
        && !node.hidden()
        && !out_of_flow(node)
        && node.style.float == crate::document::style::Float::None
        && inline_level(node);
    if inline_run && atomic_inline(node) && flow.cursor > 0.0 && flow.space == Some(true) {
        // The space a preceding run ended with is drawn before an atomic box too; only words
        // added it, so Google's "Day: <a inline-block>" ran together.
        let face = super::sizing::face_for(&node.style, Some(" "));
        flow.cursor += context.measure(" ", face)?;
        flow.space = Some(false);
    }
    dispatch(context, node, flow, items, forced)?;
    if inline_run {
        flow.space = Some(node.space_after || flow.space == Some(true));
    }
    Ok(())
}
/** Forced flex border boxes bypass authored sizing, while ordinary flow preserves its cursor. */
fn dispatch(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
    forced: Option<BorderBox>,
) -> EngineResult<()> {
    context.charge(1)?;
    if node.hidden() {
        return Ok(());
    }
    if forced.is_none() && out_of_flow(node) {
        let position = if flow.cursor > 0.0 && !inline_level(node) {
            (flow.x, flow.y + flow.line)
        } else {
            // A collapsible space before the box is kept (the box is no break), so the static
            // position is after it, as Blink's inline static positions are.
            let pending = flow.cursor > 0.0 && flow.space == Some(true);
            let space = if pending {
                let face = super::sizing::face_for(&node.style, Some(" "));
                context.measure(" ", face)?
            } else {
                0.0
            };
            (flow.left() + flow.cursor + space, flow.y)
        };
        context.defer(node.local_id, position);
        return Ok(());
    }
    if forced.is_none() && node.style.float != crate::document::style::Float::None {
        return super::float::float(context, node, flow, items);
    }
    if node.tag == "br" {
        flow.line = flow.line.max(super::sizing::advance(&node.style));
        flow.newline();
        return Ok(());
    }
    if node.tag == "img" {
        return super::image::image(context, node, flow, items, forced);
    }
    if node.tag == "svg" {
        return super::svg::svg(context, node, flow, items, forced);
    }
    if super::table::is_table(context, node) {
        return super::table::table(context, node, flow, items, forced);
    }
    if super::control::has_content(context, node) {
        return super::control::content_box(context, node, flow, items, forced);
    }
    if ["input", "button", "select", "textarea"].contains(&node.tag.as_str()) {
        return super::control::control(context, node, flow, items, forced);
    }
    if inline_level(node) && forced.is_none() {
        return super::text::inline(context, node, flow, items);
    }
    block(context, node, flow, items, forced)
}
/** Boxes placed on a line as one unit: inline-blocks, replaced elements and controls. */
fn atomic_inline(node: &Node) -> bool {
    node.style.inline_block
        || ["img", "svg", "input", "button", "select", "textarea"].contains(&node.tag.as_str())
}
/** Whitespace-only text, which a flex or grid container never makes an item of, even under
 * `white-space: pre` (the newlines between nodejs.org's grid code lines added blank rows). */
pub(super) fn blank_text(node: &Node) -> bool {
    node.tag == "#text" && node.text.trim().is_empty()
}
/** Absolute and fixed boxes leave normal flow and are placed against their containing block later. */
pub(super) fn out_of_flow(node: &Node) -> bool {
    matches!(
        node.style.position,
        crate::document::Position::Absolute | crate::document::Position::Fixed
    )
}
/** Inline-level boxes share line boxes: inline elements, labels and inline-block controls.
 * ponytail: labels and controls are inline-level by tag because the cascade has no
 * inline-block default yet, so an authored `display: block` on them is not honored here. */
pub(super) fn inline_level(node: &Node) -> bool {
    node.style.inline_block
        || !node.style.display_flex
            && (node.style.inline
                || node.tag == "label"
                || ["input", "button", "select", "textarea"].contains(&node.tag.as_str()))
}
/** Blocks provide the same content origin to normal and flexible child formatting. */
pub(super) fn block(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
    forced: Option<BorderBox>,
) -> EngineResult<()> {
    flow.newline();
    let style = &node.style;
    let fresh = forced.is_some() || new_context(node);
    // CSS 2.1 9.5.2: clearance places the border edge at the cleared floats' bottom, absorbing
    // the top margin, when the collapsed position would be above it.
    let clearing = forced
        .is_none()
        .then(|| flow.clearance(style.clear))
        .flatten();
    if clearing.is_some() {
        // A cleared child past adjoining floats resolves its parent's open strut first, where the
        // parent stands, so its own margin no longer collapses through (Blink's clearance rules).
        flow.strut_open = false;
    }
    let cleared = clearing.filter(|clear| *clear > flow.block_top(style.margin[0]));
    if let Some(clear) = cleared {
        flow.y = clear;
        flow.collapsible = 0.0;
        flow.placed_at(clear);
    }
    let above = (flow.y - flow.collapsible, flow.collapsible);
    let mut y = forced
        .map(|bounds| bounds.y)
        .or(cleared)
        .unwrap_or_else(|| flow.block_top(style.margin[0]));
    // A new formatting context sits beside floats instead of under them, in the first layout
    // opportunity its authored width fits (Blink's LayoutNewFormattingContext).
    let (left, room) = if fresh && forced.is_none() {
        let (top, left, room) = beside_floats(flow, node, y);
        y = top;
        (left, room)
    } else {
        (flow.x, flow.width)
    };
    let width = match forced {
        Some(bounds) => bounds.width,
        // A content-sized width shrinks to fit the room, as floats do.
        None if style.width_fit => super::float::shrink_width(
            context,
            node,
            room.min(flow.width),
            style.margin[1] + style.margin[3],
        )?,
        None => border_width(
            node,
            flow.width,
            (room - style.margin[1] - style.margin[3]).max(0.0),
            None,
        ),
    };
    let x = forced
        .map(|bounds| bounds.x)
        .unwrap_or_else(|| left + start_margin(context, node, room, width));
    let sides = super::sizing::border_sides(style);
    let specified = specified_height(node, forced, flow.definite)
        .or_else(|| aspect_height(node, width, flow.definite));
    let mut inner = flow.nested(
        x + style.padding[3] + sides[3],
        y + style.padding[0] + sides[0],
        (width - horizontal_edges(node)).max(0.0),
        fresh,
    );
    inner.strut = Some(strut(style));
    inner.align = alignment(style);
    inner.ellipsis = ellipsis_owner(node);
    inner.definite = specified.map(|height| (height - vertical_edges(node)).max(0.0));
    // A flex container is its own formatting context, so no margin collapses through it.
    let collapses = forced.is_none() && !fresh;
    let through_top = collapses && style.padding[0] == 0.0 && sides[0] == 0.0;
    if through_top {
        // A first child's top margin collapses through a parent with no top padding or border.
        inner.collapsible = y - flow.y + flow.collapsible;
        inner.strut_open = true;
    }
    let index = items.len();
    items.push(box_item(node, x, y, width, 0.0, Vec::new()));
    if style.display_grid {
        super::grid::children(context, node, &mut inner, items)?;
    } else if style.display_flex {
        super::flex_layout::children(
            context,
            node,
            &mut inner,
            items,
            specified.map(|height| (height - vertical_edges(node)).max(0.0)),
            (clamp_height(node, 0.0, flow.definite) - vertical_edges(node)).max(0.0),
        )?;
    } else {
        super::generated::generated(context, node, &mut inner, items, false)?;
        let lines = super::text::text(context, &node.text, node, &mut inner, index)?;
        items[index].lines = lines;
        if !super::generated::is_generated(node) {
            for child in context.children(node.local_id) {
                node_layout(context, child, &mut inner, items, None)?;
            }
        }
        super::generated::generated(context, node, &mut inner, items, true)?;
    }
    inner.newline();
    super::line::settle(context, &mut inner, items, index + 1)?;
    if through_top && let Some(top) = inner.resolved {
        // The collapsed margins resolved at the first child's border edge, which is where this
        // box's border edge sits too (Blink resolves both at one BFC block offset).
        position_floats(&mut inner, items, top - y);
        y = top;
        items[index].y = top;
        flow.placed_at(top);
    } else if through_top && inner.strut_open && flow.strut_open {
        // Still unresolved: the floats wait for the outer box, which shares their list.
        flow.unpositioned.append(&mut inner.unpositioned);
    }
    if style.list_item
        && let Some(line) = super::marker::marker(context, node, (inner.x, y), &items[index..])?
    {
        items[index].lines.push(line);
    }
    if fresh {
        inner.bottom = inner.bottom.max(inner.floats_bottom());
    } else {
        flow.floats = std::mem::take(&mut inner.floats);
        flow.cleared = inner.cleared;
    }
    // A last child's bottom margin passes through a parent with no bottom padding or border.
    let through = collapses
        && style.padding[2] == 0.0
        && sides[2] == 0.0
        && specified.is_none()
        && inner.bottom <= inner.y;
    let trailing = if through { inner.collapsible } else { 0.0 };
    let natural =
        (inner.bottom - trailing - y + style.padding[2] + sides[2]).max(vertical_edges(node));
    // A table cell's authored height is a minimum: Blink sizes the cell to the larger of it and
    // its content, and aligns the content by its own height (recorded here for the table).
    let cell =
        forced.is_none_or(|bounds| bounds.height.is_none()) && super::table::is_cell_box(node);
    let specified = specified.map(|height| if cell { height.max(natural) } else { height });
    context.record_content_height(node.local_id, natural);
    let height = clamp_height(node, specified.unwrap_or(natural), flow.definite);
    // Blink's FinishLayout: a block size other than the intrinsic one (min-height grew it, or
    // max-height cut it) separates the last child's bottom margin, which is then dropped.
    let trailing = if height == natural { trailing } else { 0.0 };
    items[index].height = height;
    super::generated::absolute(context, node, items, index)?;
    if forced.is_some() {
        flow.y = y + height;
        flow.collapsible = 0.0;
        flow.bottom = flow.bottom.max(flow.y);
    } else if height == 0.0 {
        // An empty block's own top and bottom margins collapse with the ones around it.
        let (base, previous) = above;
        let combined = collapse(collapse(previous, style.margin[0]), style.margin[2]);
        flow.y = base + combined;
        flow.collapsible = combined;
        flow.bottom = flow.bottom.max(flow.y);
    } else {
        flow.after_block(y + height, collapse(style.margin[2], trailing));
    }
    Ok(())
}
/** The inline-start margin, with a legacy `<center>` parent centering narrower blocks. */
fn start_margin(context: &LayoutContext<'_>, node: &Node, room: f64, width: f64) -> f64 {
    let legacy = node
        .parent
        .and_then(|parent| context.node(parent).ok())
        .is_some_and(|parent| parent.style.align == "webkit-center");
    if legacy && !node.style.margin_auto.iter().any(|auto| *auto) {
        let mut centered = node.style.clone();
        centered.margin_auto[1] = true;
        centered.margin_auto[3] = true;
        return super::body::margin_start(&centered, room, width);
    }
    super::body::margin_start(&node.style, room, width)
}
/** Boxes that start a block formatting context: they contain floats and are not overlapped by outside ones. */
pub(super) fn new_context(node: &Node) -> bool {
    let style = &node.style;
    style.overflow != crate::document::Overflow::Visible
        || style.display_flex
        || style.display_grid
        || style.inline_block
        || style.flow_root
        || style.table_box == crate::document::style::TableBox::Table
        || style.float != crate::document::style::Float::None
        || out_of_flow(node)
        || ["td", "th", "caption", "button", "fieldset"].contains(&node.tag.as_str())
}
/** A forced height, else the authored height as a border-box extent, if it is definite. */
fn specified_height(node: &Node, forced: Option<BorderBox>, definite: Option<f64>) -> Option<f64> {
    if let Some(height) = forced.and_then(|bounds| bounds.height) {
        return Some(height);
    }
    let content = node.style.height?.resolve_size(definite)?;
    Some(clamp_height(
        node,
        super::sizing::border_height(node, content),
        definite,
    ))
}
/**
 * The border-box height `aspect-ratio` gives an automatic height at this border-box width: the
 * ratio sizes the content box unless `box-sizing: border-box` makes it size the border box.
 */
fn aspect_height(node: &Node, width: f64, definite: Option<f64>) -> Option<f64> {
    let ratio = node.style.aspect_ratio?;
    let height = if node.style.box_sizing == crate::document::BoxSizing::BorderBox {
        (width / ratio).max(vertical_edges(node))
    } else {
        (width - horizontal_edges(node)).max(0.0) / ratio + vertical_edges(node)
    };
    Some(clamp_height(node, height, definite))
}
/** min-height and max-height bound the border-box height; min wins over max. */
pub(super) fn clamp_height(node: &Node, height: f64, definite: Option<f64>) -> f64 {
    let bound = |extent: Option<crate::document::style::Extent>| {
        extent
            .and_then(|extent| extent.resolve_size(definite))
            .map(|value| super::sizing::border_height(node, value))
    };
    let mut height = height;
    if let Some(max) = bound(node.style.max_height) {
        height = height.min(max);
    }
    if let Some(min) = bound(node.style.min_height) {
        height = height.max(min);
    }
    height
}
/** Every rendering collaborator uses the same forced-box and intrinsic sizing contract. */
pub(super) use super::sizing::{
    BorderBox, LayoutContext, border_width, horizontal_edges, vertical_edges,
};
/** One representation supplies both agent bounds and the native paint path. */
pub(super) fn box_item(
    node: &Node,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    lines: Vec<PaintLine>,
) -> DisplayItem {
    let face = super::sizing::face(&node.style);
    DisplayItem {
        local_id: node.local_id,
        tag: node.tag.clone(),
        role: super::role(node).into(),
        x,
        y,
        width,
        height,
        font_size: node.style.font_size,
        bold: node.style.bold,
        line_height: Some(super::sizing::advance(&node.style)),
        lines,
        color: node.style.color.clone().or_else(|| {
            if node.style.color_scheme == crate::document::ColorScheme::Normal {
                return None;
            }
            if node.href.is_some() {
                Some(
                    match node.style.used_scheme {
                        crate::document::Appearance::Light => "#0000ee",
                        crate::document::Appearance::Dark => "#9e9eff",
                    }
                    .into(),
                )
            } else {
                super::default_foreground(&node.style)
            }
        }),
        // A url() mask keeps the fill it shapes; any other mask hides it, as Eye cannot apply it.
        background: node
            .style
            .background
            .clone()
            .filter(|_| !node.style.masked || node.style.mask.is_some()),
        border: if node.style.border_style_none {
            0.0
        } else {
            super::sizing::snap_border(node.style.border)
        },
        border_color: node.style.border_color.clone(),
        validation: Vec::new(),
        control_height: None,
        opacity: 1.0,
        rtl: false,
        vector: Vec::new(),
        radius: [0.0; 4],
        border_widths: if node.style.border == 0.0 && !node.style.border_style_none {
            node.style.border_widths.map(super::sizing::snap_border)
        } else {
            [0.0; 4]
        },
        border_colors: if node.style.border == 0.0 {
            node.style.border_colors.clone()
        } else {
            Default::default()
        },
        border_styles: node.style.border_styles,
        shadows: Vec::new(),
        backgrounds: Vec::new(),
        clip: None,
        transform: None,
        pin: None,
        z: 0,
        stacking_opacity: Vec::new(),
        font_weight: face.weight,
        font_family: face.family,
        letter_spacing: face.spacing,
        underline: !node.style.text_decoration_none
            && (node.style.underline || (node.tag == "a" && node.href.is_some())),
        strikethrough: !node.style.text_decoration_none && node.style.strikethrough,
        italic: face.italic,
        object_fit: crate::document::style::ObjectFit::Fill,
        object_position: super::display::default_object_position(),
        content_box: None,
        mask: None,
    }
}
