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
    /** The most negative margin in `collapsible`'s strut, tagged with the `collapsible` it was
     * recorded for: a strut adjoins its largest positive and most negative margins (CSS 2.1
     * 8.3.1), which one sum loses. Any other `collapsible` (a reset) means its own sign. */
    pub negative: (f64, f64),
    /** Fragments on the current line, aligned when it finishes. */
    pub frags: Vec<super::line::Frag>,
    /** Empty inline boxes on the current line: they add their line-height to a line that has
     * other content, but never make a line of their own (empty-inline-003). */
    pub phantoms: Vec<super::line::Frag>,
    /** Out-of-flow boxes met on the current line: (identity, fragments before it, line top),
    so their static position follows the line's alignment and bidi order when it settles. */
    pub statics: Vec<(usize, usize, f64, f64)>,
    /** Finished lines whose offsets `settle` still has to apply. */
    pub finished: Vec<super::line::Finished>,
    /** The containing block's own font metrics, part of every line. */
    pub strut: Option<super::line::Strut>,
    /** The containing block's text-align. */
    pub align: super::justify::Aligned,
    /** The block whose `text-overflow: ellipsis` truncates this flow's overflowing lines. */
    pub ellipsis: Option<usize>,
    /** The block's `direction` is `rtl`, so its lines lay out right to left (render/bidi.rs). */
    pub rtl: bool,
    /** `vertical-lr` text laid out turned: a line's over side is the physical right, while
     * block flow runs left to right, so each line's contents mirror across it. */
    pub mirror_lines: bool,
    /** The `text-indent` the current line started with, which a right-to-left line takes off
     * its right edge once it is aligned. */
    pub indent: f64,
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
    /** `margin-trim: block-start` on this flow's container, until its first line or non-empty
     * block: the children adjoining its start drop their margins. */
    pub trim_start: bool,
    /** Floats placed while the margin strut was open, as (index in `floats`, item range, top
     * they were given); Blink positions them only once the BFC block offset resolves. */
    pub unpositioned: Vec<(usize, std::ops::Range<usize>, f64)>,
    /** Sides (left, right) of floats adjoining the open margin strut, here or in an ancestor
     * whose strut is still open (Blink's adjoining object types). */
    pub adjoining: [bool; 2],
    /** Blink's self_collapsing_child_had_clearance: the last in-flow children are self-collapsing
     * and one of them had clearance, so their margins no longer collapse through this box's
     * block-end edge but count toward its height (CSS 2.1 8.3.1). */
    pub cleared_collapsing: bool,
    /** Preserved white space hanging at the end of the current line. */
    pub hang: super::line::Hang,
    /** `text-box-trim` still to apply here: (the first formatted line's start, the last one's
     * end), from this block's style or passed down through edges without border or padding. */
    pub trim: [bool; 2],
    /** The end cut the last formatted line would take, with the nested last blocks whose
     * height shrinks with it, and the line's bottom (infinite once a nested block checked it);
     * None once anything but a trimmable line came last. */
    pub tail: Option<(f64, Vec<usize>, f64)>,
    /** An inline box left the current line empty: Blink wraps it in an anonymous block with no
     * formatted line, which still takes a pending `text-box-trim` start or end. */
    pub empty_run: bool,
    /** The last scalar of the line's text when text (not an atomic inline) ends it, which
     * decides whether the next atomic or element's run may break from it (`glue.rs`). */
    pub last: Option<char>,
    /** The run `last` ends may break between any two letters (`word-break: break-all`,
     * `line-break: anywhere`, `overflow-wrap`), which makes its end a break opportunity too. */
    pub last_fill: bool,
    /** The multicol content box (left, width) a `column-span: all` box in this formatting
     * context spans; None outside a multicol container. */
    pub span: Option<(f64, f64)>,
    /** The enclosing multicol's columns when their height is known: (the first column's top,
     * the column height), in this flow's coordinates; a nested multicol's rows end where they
     * do. None outside such columns. */
    pub fragmentainer: Option<(f64, f64)>,
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
            negative: (0.0, 0.0),
            frags: Vec::new(),
            phantoms: Vec::new(),
            statics: Vec::new(),
            finished: Vec::new(),
            strut: None,
            align: Default::default(),
            ellipsis: None,
            rtl: false,
            mirror_lines: false,
            indent: 0.0,
            floats: Vec::new(),
            cleared: None,
            definite: None,
            space: Some(false),
            strut_open: false,
            resolved: None,
            last_top: std::cell::Cell::new(y),
            trim_start: false,
            unpositioned: Vec::new(),
            adjoining: [false; 2],
            cleared_collapsing: false,
            hang: super::line::Hang::default(),
            trim: [false, false],
            tail: None,
            empty_run: false,
            last: None,
            last_fill: false,
            span: None,
            fragmentainer: None,
        }
    }
    /** A child flow inside this one that shares its floats, unless it starts a new formatting context. */
    pub fn nested(&self, x: f64, y: f64, width: f64, fresh: bool) -> Self {
        let mut inner = Self::new(x, y, width);
        if !fresh {
            inner.floats = self.floats.clone();
            inner.cleared = self.cleared;
            inner.span = self.span;
        }
        inner.fragmentainer = self.fragmentainer;
        inner
    }
    /** Records a fragment placed on the current line. */
    pub fn push(&mut self, frag: super::line::Frag) {
        self.last = None;
        self.last_fill = false;
        self.frags.push(frag);
        self.empty_run = false;
        self.hang = super::line::Hang::default();
    }
    /** The top of a block whose top margin collapses with the margin already counted above it. */
    pub fn block_top(&self, margin: f64) -> f64 {
        let margin = self.trimmed(margin);
        let top = self.y - self.collapsible + join(self.strut(), margin).0;
        self.last_top.set(top);
        top
    }
    /** A block placed somewhere other than `block_top`, such as below floats it clears. */
    pub fn placed_at(&self, top: f64) {
        self.last_top.set(top);
    }
    /** Advances past a block and remembers its bottom margin for collapsing with the next one. */
    pub fn after_block(&mut self, bottom: f64, margin: f64) {
        self.after_block_strut(bottom, (margin, margin.min(0.0)));
    }
    /** As `after_block`, with the bottom strut as (total, most negative margin). */
    pub fn after_block_strut(&mut self, bottom: f64, (margin, negative): (f64, f64)) {
        self.trim_start = false;
        if std::mem::take(&mut self.strut_open) {
            self.resolved = Some(self.last_top.get());
        }
        self.y = bottom + margin;
        self.set_strut((margin, negative));
        self.bottom = self.bottom.max(self.y);
        self.cleared_collapsing = false;
    }
    /** A completed line advances by its baseline-aligned height. */
    pub fn newline(&mut self) {
        if self.cursor > 0.0 || self.line > 0.0 || !self.frags.is_empty() {
            // A line box resolves an open margin strut where it stands.
            self.strut_open = false;
            self.trim_start = false;
            let aligned = super::line::finish(self);
            self.y += self.line.max(aligned);
            if let Some(tail) = &mut self.tail {
                tail.2 = tail.2.min(self.y);
            }
            self.bottom = self.bottom.max(self.y);
            self.collapsible = 0.0;
            self.cleared_collapsing = false;
        }
        self.phantoms.clear();
        self.cursor = 0.0;
        self.line = 0.0;
        self.space = Some(false);
        self.hang = super::line::Hang::default();
    }
    /** Ends the line at a soft wrap opportunity, where its hanging white space hangs whole
     * (a forced break or the paragraph's end hangs only what overflows). */
    pub fn wrap(&mut self) {
        self.hang.soft = true;
        self.newline();
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
/** The highest top at or below `y` where a formatting-context root fits between the floats,
 * with the left edge and room its margins and width resolve against, after Blink's
 * LayoutNewFormattingContext: with no float beside it the root's margins apply from the
 * content edge (and may be negative); with floats the margins only shrink the opportunity,
 * measured from the content edge, so a margin under a float costs nothing. An opportunity is
 * rejected when the border box crosses a float or is taller than it. Ponytail: an auto
 * height is checked over one line; a trial layout per opportunity would cover it. */
fn beside_floats(flow: &Flow, node: &Node, mut y: f64) -> (f64, f64, f64) {
    use super::float::LINE_FIT_SLACK;
    let style = &node.style;
    let height = specified_height(node, None, flow.definite);
    let extent = height.unwrap_or(0.0).max(LINE_FIT_SLACK);
    let (margin_left, margin_right) = (style.margin[3], style.margin[1]);
    let place = |y: f64| {
        let (left, width) = match height {
            Some(_) => flow.room_at_height(y, extent),
            None => flow.room_at(y),
        };
        let (floats_left, floats_right) = (left > flow.x, left + width < flow.x + flow.width);
        let (line_left, line_right) = if !floats_left && !floats_right {
            (left + margin_left, left + width - margin_right)
        } else {
            (
                left.max(flow.x + margin_left.max(0.0)),
                (left + width).min(flow.x + flow.width - margin_right.max(0.0)),
            )
        };
        let size = (line_right - line_left).max(0.0);
        let room = (size + margin_left + margin_right).max(0.0);
        // An auto width fills the opportunity, which still rejects it when the margins leave
        // its border box over a float (CSS 2.1 9.5: a root's border box never overlaps a
        // float's margin box), even one past the containing block's edge.
        let border = border_width(node, flow.width, size, None);
        // An auto height an `aspect-ratio` gives follows the width this opportunity allows.
        let extent = match height {
            None => {
                aspect_height(node, border, flow.definite).map_or(extent, |h| h.max(LINE_FIT_SLACK))
            }
            Some(_) => extent,
        };
        let (x0, x1) = (
            line_left + LINE_FIT_SLACK,
            line_left + border - LINE_FIT_SLACK,
        );
        let overlaps = flow
            .floats
            .iter()
            .any(|f| f.x1 > f.x0 && f.top < y + extent && f.bottom > y && f.x0 < x1 && f.x1 > x0);
        let wide = (floats_left || floats_right) && border > width + LINE_FIT_SLACK;
        // Margins wider than the room beside the floats leave no opportunity there at all
        // (floats-wrap-bfc-with-margin-005).
        let squeezed = (floats_left || floats_right) && line_right - line_left < -LINE_FIT_SLACK;
        let fits = !overlaps && !wide && !squeezed;
        (fits, line_left - margin_left, room, extent)
    };
    for _ in 0..=flow.floats.len() {
        let (fits, left, room, extent) = place(y);
        let next = flow
            .floats
            .iter()
            .filter(|f| f.top < y + extent && f.bottom > y)
            .map(|f| f.bottom)
            .fold(f64::INFINITY, f64::min);
        if fits || !next.is_finite() {
            return (y, left, room);
        }
        y = next;
    }
    let (_, left, room, _) = place(y);
    (y, left, room)
}
/** css-overflow-3: `text-overflow` applies only to a block container that clips its overflow. */
pub(super) fn ellipsis_owner(node: &Node) -> Option<usize> {
    let clips = node.style.overflow != crate::document::style::Overflow::Visible;
    (node.style.text_overflow_ellipsis && clips).then_some(node.local_id)
}
/** A margin strut `(total, most negative)` joined with one more margin: the largest positive
 * margin plus the most negative one (CSS 2.1 8.3.1). abspos-022's 2, -4, 0, 14, -4 and 2px
 * margins collapse to 10px, where pairwise sums gave 8. */
pub(super) fn join((total, negative): (f64, f64), margin: f64) -> (f64, f64) {
    let positive = (total - negative).max(0.0).max(margin);
    let negative = negative.min(margin.min(0.0));
    (positive + negative, negative)
}
impl Flow {
    /** The open strut as (total, most negative margin). */
    pub fn strut(&self) -> (f64, f64) {
        let (tagged, negative) = self.negative;
        if tagged == self.collapsible {
            (self.collapsible, negative)
        } else {
            (self.collapsible, self.collapsible.min(0.0))
        }
    }
    /** A child's margin as this flow's `margin-trim: block-start` leaves it: zero while the
     * child still adjoins the container's start. */
    pub fn trimmed(&self, margin: f64) -> f64 {
        if self.trim_start { 0.0 } else { margin }
    }
    /** Records a strut as `collapsible` and its most negative margin. */
    pub fn set_strut(&mut self, (total, negative): (f64, f64)) {
        self.collapsible = total;
        self.negative = (total, negative);
    }
}
/** Items, document height, body decoration, and whether a bound cut layout short. */
pub(super) type Laid = (
    Vec<DisplayItem>,
    f64,
    Option<super::DisplayDecoration>,
    bool,
    mars_core::geometry::WrapperMetrics,
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
    let html = super::sizing::has_percent_sides(&document.formatting.html.style);
    if !html && !body && !nodes {
        return layout_once(document, presentation);
    }
    let wrappers =
        (html || body).then(|| super::sizing::resolve_wrapper_sides(document, presentation.width));
    let document = wrappers.as_ref().unwrap_or(document);
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
    context.viewport = (presentation.width, presentation.height);
    let mut items = Vec::new();
    let body = &wrapper_body(document);
    let authored = document.formatting.body_box_authored;
    // The body lays out inside the root element's margin, border and padding.
    let root = super::body::root_edges(document, (presentation.width, presentation.height));
    let inside = (presentation.width - root[1] - root[3]).max(0.0);
    let (x, y, width) = if authored {
        super::body::content(body, inside)
    } else {
        (MARGIN, MARGIN, (inside - MARGIN * 2.0).min(CONTENT_WIDTH))
    };
    let viewport = (presentation.width, presentation.height);
    // The root's `::before` boxes come first, above the body (render/root_flow.rs).
    let content = (root[3], root[0], inside);
    let before = super::root_flow::root_boxes(&mut context, document, &mut items, content, false)?;
    let body_margin = if authored { body.margin[0] } else { MARGIN };
    let y = y + super::root_flow::body_shift(&before, root[0], body_margin);
    let flowed_y = y + root[0];
    let [x, y] = super::root_box::body_origin(document, viewport, [x + root[3], flowed_y]);
    let mut flow = Flow::new(x, y, width);
    flow.strut = Some(strut(body));
    flow.align = alignment(body);
    flow.rtl = body.rtl;
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
        let top_level = document.nodes.iter().filter(|node| node.parent.is_none());
        for node in top_level.filter(|node| !super::root_generated::is_root_box(node)) {
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
    // The body's content box ends at its definite height, else at its in-flow content: the body
    // is no formatting context root, so floats overflow it into the root's.
    let content_end = flow
        .definite
        .map_or(flow.bottom.max(flow.y), |height| y + height);
    let floats_end = flow.floats_bottom();
    flow.bottom = flow.bottom.max(floats_end);
    let truncated = context.exhausted();
    context.body_block = positioned_body_block(body, [x, y, width], content_end);
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
    let (edge, margin) = if authored {
        (
            body.padding[2] + super::sizing::border_sides(body)[2],
            body.margin[2],
        )
    } else {
        (0.0, MARGIN)
    };
    let body_end = content_end + edge;
    // The root's block formatting context holds the body's margin box and the floats past it.
    let height = super::body::clipped_height(document, flow.definite).map_or_else(
        || (body_end + margin).max(flow.bottom.max(flow.y)),
        |clip| y + clip + edge + margin,
    ) + root[2];
    let height = (body_end + margin + root[2]).max(height);
    // The root's `::after` boxes follow the body's margin box.
    let after = (root[3], body_end + margin, inside);
    let after = super::root_flow::root_boxes(&mut context, document, &mut items, after, true)?;
    let height = height.max(after.end + after.margin + root[2]);
    let top = collapsed_body_edge(document, &items, authored, 0).unwrap_or(y);
    let end = collapsed_body_edge(document, &items, authored, 2).unwrap_or(body_end);
    let decoration = super::body::decoration(document, x, top, width, end);
    let in_flow_end = (body_end + margin)
        .max(floats_end)
        .max(after.end + after.margin);
    let resize_wrappers = super::resize_wrappers::measure(
        document,
        viewport,
        body,
        [width, top, end],
        in_flow_end - (y - flowed_y),
        context.exhausted(),
    );
    // This document has resolved percentage edges and these boxes have their final layout sizes.
    for item in &mut items {
        let node = context.node(item.local_id)?;
        if !item.bare && super::effects::generated_style(node, item).is_none() {
            item.resize_box = Some(super::effects::resize_box(item, node));
        }
    }
    Ok((
        items,
        height,
        decoration,
        context.exhausted(),
        resize_wrappers,
    ))
}
/** The padding box of a positioned body whose content box starts at (`x`, `y`), `width` wide,
and ends at `end`; None for a static body, whose top-level absolute boxes use the viewport. */
fn positioned_body_block(
    body: &crate::document::ComputedStyle,
    [x, y, width]: [f64; 3],
    end: f64,
) -> Option<[f64; 4]> {
    if body.position == crate::document::Position::Static && body.transform.is_none() {
        return None;
    }
    let [pt, pr, pb, pl] = body.padding;
    Some([
        x - pl,
        y - pt,
        width + pl + pr,
        (end - y).max(0.0) + pt + pb,
    ])
}
/** Where the body's border box starts (`side` 0) or ends (`side` 2) when the margin on that side
collapses with its first or last in-flow child's (no border or padding between them, and an
auto height for the end): at that block's border edge, as in Chromium, so the body's background
does not show in the collapsed margin. */
fn collapsed_body_edge(
    document: &Document,
    items: &[DisplayItem],
    authored: bool,
    side: usize,
) -> Option<f64> {
    let body = &document.formatting.body.style;
    let open = body.padding[side] == 0.0 && super::sizing::border_sides(body)[side] == 0.0;
    if !authored || !open || (side == 2 && body.height.is_some()) {
        return None;
    }
    let mut flowing = document.nodes.iter().filter(|node| {
        node.parent.is_none()
            && !node.removed()
            && !out_of_flow(node)
            && node.style.float == crate::document::style::Float::None
    });
    let child = if side == 0 {
        flowing.next()
    } else {
        flowing.next_back()
    }?;
    if inline_level(child) {
        return None;
    }
    let item = items.iter().find(|item| item.local_id == child.local_id)?;
    Some(if side == 0 {
        item.y
    } else {
        item.y + item.height
    })
}
/** The body style the wrapper lays out: a vertical body's margins belong to its rotated box
instead (body.rs `layout_only`), so the wrapper leaves them out. */
fn wrapper_body(document: &Document) -> crate::document::ComputedStyle {
    let mut body = document.formatting.body.style.clone();
    if body.vertical_writing {
        body.margin = [0.0; 4];
        body.margin_auto = [false; 4];
    }
    body
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
/** Whether something after the last line (a `<br clear>`'s clearance) left this flow's end
 * below that line's bottom `end`. */
pub(super) fn cleared_past(flow: &Flow, end: f64) -> bool {
    flow.y > end + super::line::LINE_EPSILON
}
/** The font metrics every line of a block includes. */
pub(super) fn strut(style: &crate::document::ComputedStyle) -> super::line::Strut {
    super::line::Strut::of(style)
}
/** text-align and text-align-last resolved against the base direction. */
pub(super) fn alignment(style: &crate::document::ComputedStyle) -> super::justify::Aligned {
    super::justify::aligned(style)
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
        && !node.removed()
        && !out_of_flow(node)
        && node.style.float == crate::document::style::Float::None
        && inline_level(node);
    if inline_run && atomic_inline(node) && flow.cursor > 0.0 && flow.space == Some(true) {
        // The space a preceding run ended with is drawn before an atomic box too; only words
        // added it, so Google's "Day: <a inline-block>" ran together.
        // The white space is the parent's text, so it takes the parent's font: under
        // `font-size: 0` it has no width (table-vertical-align-baseline-009).
        // A top-level box's white space is the body's.
        let parent = node.parent.and_then(|parent| context.node(parent).ok());
        let body = &context.document.formatting.body.style;
        let style = parent.map_or(body, |parent| &parent.style);
        let face = super::sizing::face_for(style, Some(" "));
        flow.cursor += context.measure(" ", face)?;
        flow.space = Some(false);
    }
    dispatch(context, node, flow, items, forced)?;
    if inline_run {
        flow.space = Some(node.space_after || flow.space == Some(true));
        if flow.frags.is_empty() && flow.cursor == 0.0 {
            flow.empty_run = true;
            flow.tail = None;
        }
    }
    Ok(())
}
/** Where an out-of-flow box met in this flow would have started: below the current line for a
block-level box, else after the line so far and a kept `space`; at the start edge, which is the
right one in a right-to-left block (whose boxes then align their right edge there). */
fn static_position(flow: &Flow, node: &Node, space: f64) -> (f64, f64) {
    let block_level = !inline_level(node);
    let (left, width) = flow.room_at(flow.y);
    match (flow.rtl, block_level) {
        (false, true) if flow.cursor > 0.0 => (flow.x, flow.y + flow.line),
        // A block-level box starts at the content edge: floats shorten lines, not blocks
        // (CSS2 abspos-028).
        (false, true) => (flow.x, flow.y),
        (false, false) if flow.cursor == 0.0 && flow.frags.is_empty() => {
            (empty_line_start(flow, left, width), flow.y)
        }
        (false, false) => (flow.left() + flow.cursor + space, flow.y),
        (true, true) if flow.cursor > 0.0 => (flow.x + flow.width, flow.y + flow.line),
        (true, true) => (flow.x + flow.width, flow.y),
        (true, false) if flow.cursor == 0.0 && flow.frags.is_empty() => {
            (empty_line_start(flow, left, width), flow.y)
        }
        (true, false) => (left + width - flow.cursor - space, flow.y),
    }
}
/** Where an inline-level box alone on a line starts: the line's start edge (its right one in a
right-to-left block, where the box ends there) moved by its `text-align`, as the empty line is
aligned in the room floats leave (inline-level-absolute-in-block-level-context-002, -008).
Content that later joins the line moves it again in `line::place_statics`. */
fn empty_line_start(flow: &Flow, left: f64, width: f64) -> f64 {
    use super::line::Align;
    match flow.align.last {
        Align::Center => left + width / 2.0,
        Align::Right => left + width,
        Align::Left => left,
        Align::Justify(_) if flow.rtl => left + width,
        Align::Justify(_) => left,
    }
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
    if node.removed() {
        return Ok(());
    }
    if forced.is_none() && out_of_flow(node) {
        // A collapsible space before the box is kept (the box is no break), so the static
        // position is after it, as Blink's inline static positions are.
        let pending = flow.cursor > 0.0 && flow.space == Some(true) && inline_level(node);
        let space = if pending {
            let face = super::sizing::face_for(&node.style, Some(" "));
            context.measure(" ", face)?
        } else {
            0.0
        };
        let position = static_position(flow, node, space);
        context.defer(node.local_id, position);
        if flow.rtl {
            context.mark_static_end(node.local_id);
        }
        if inline_level(node) {
            flow.statics
                .push((node.local_id, flow.frags.len(), flow.y, space));
        }
        return Ok(());
    }
    if forced.is_none() && node.style.float != crate::document::style::Float::None {
        return super::float::float(context, node, flow, items);
    }
    // A vertical subtree in a horizontal parent lays out in its own rotated space; an inline
    // one reaches here through its atomic inline's forced box.
    if super::vertical::is_root(context, node) && (forced.is_some() || !inline_level(node)) {
        return super::vertical::layout(context, node, flow, items, forced);
    }
    if node.tag == "br" {
        // Blink's EnsureTextMetrics: a forced break directly in the block sits in the root box,
        // which always has the strut's metrics, so the br's own font and line-height count
        // only inside an inline box.
        let in_box = node
            .parent
            .and_then(|parent| context.node(parent).ok())
            .is_some_and(|parent| parent.style.inline);
        let own = super::sizing::advance(&node.style);
        flow.line = flow.line.max(match flow.strut {
            Some(strut) if !in_box => strut.height,
            _ => own,
        });
        flow.newline();
        return Ok(());
    }
    if replaced_image(node) {
        return super::image::image(context, node, flow, items, forced);
    }
    if node.tag == "svg" {
        return super::svg::svg(context, node, flow, items, forced);
    }
    // An inline-table goes on the line as an atomic inline; its table layout runs inside.
    if forced.is_none() && node.style.inline_block && super::table::is_table(context, node) {
        return super::text::atomic(context, node, flow, items);
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
/** Replaced boxes sized from a natural size: images, canvases from their bitmap's `width` and
`height` (render.rs `canvases`), videos from their media's frame size (or the default object
size before it is known), and embedded frames. */
pub(super) fn replaced_image(node: &Node) -> bool {
    matches!(node.tag.as_str(), "img" | "canvas" | "video") || frame(node)
}
/** A box showing another document (HTML's iframe, or an object or embed the runtime loaded a
document or drawing into): a replaced box whose content the runtime paints into its content box.
Without a natural size it takes CSS 2.1's 300 by 150 default, never its fallback content's size. */
pub(super) fn frame(node: &Node) -> bool {
    node.tag == "iframe" || node.embedded && (node.tag == "object" || node.tag == "embed")
}
/** Boxes placed on a line as one unit: inline-blocks, replaced elements and controls. */
fn atomic_inline(node: &Node) -> bool {
    node.style.inline_block
        || frame(node)
        || [
            "img", "canvas", "video", "svg", "input", "button", "select", "textarea",
        ]
        .contains(&node.tag.as_str())
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
    if std::mem::take(&mut flow.empty_run) {
        flow.trim[0] = false;
    }
    let style = &node.style;
    let fresh = forced.is_some() || new_context(node);
    // CSS 2.1 9.5.2: clearance places the border edge at the cleared floats' bottom, absorbing
    // the top margin, when the collapsed position would be above it.
    let clearing = forced
        .is_none()
        .then(|| flow.clearance(style.clear))
        .flatten();
    // Blink's HasClearancePastAdjoiningFloats: clearing a float that adjoins the open strut is
    // certain clearance, whatever the margin; otherwise clearance applies only when the
    // hypothetical border edge sits above the floats.
    let past_adjoining = flow.strut_open && clears_adjoining(flow, style.clear);
    let cleared = clearing.filter(|clear| {
        let margin = super::clearance::adjoining_top(context, node);
        past_adjoining || *clear > flow.block_top(margin)
    });
    if let Some(clear) = cleared {
        // Clearance separates the child's margin from its parent's open strut, which resolves
        // where the parent stands (Blink's ResolveBfcBlockOffset at the previous position).
        flow.strut_open = false;
        flow.y = clear;
        flow.collapsible = 0.0;
        flow.placed_at(clear);
    }
    let above = (flow.y - flow.collapsible, flow.strut());
    let mut y = forced
        .map(|bounds| bounds.y)
        .or(cleared)
        .unwrap_or_else(|| flow.block_top(style.margin[0]));
    // A new formatting context sits beside floats instead of under them, in the first layout
    // opportunity its authored width fits (Blink's LayoutNewFormattingContext).
    let (left, room) = if let Some(frame) = super::multicol::spanning(context, flow, node, forced) {
        frame
    } else if fresh && forced.is_none() {
        let (top, left, room) = match separated_from_adjoining(flow, node, cleared) {
            Some(placed) => placed,
            None => beside_floats(flow, node, y),
        };
        y = top;
        super::bfc_fit::refit(context, node, flow, (top, (left, room)))?
    } else {
        (flow.x, flow.width)
    };
    let width = match forced {
        Some(bounds) => bounds.width,
        // A content-sized width shrinks to fit the room, as floats do.
        None if style.width_fit || super::table::shrinks(node) => super::float::shrink_width(
            context,
            node,
            room.min(flow.width),
            style.margin[1] + style.margin[3],
        )?,
        None => {
            let available = (room - style.margin[1] - style.margin[3]).max(0.0);
            let aspect = aspect_width(context, node, flow.definite)?;
            let natural =
                aspect.or_else(|| super::sizing::transferred_width(node, flow.definite, available));
            // A width the ratio transfers from a definite height may overflow the container.
            let room = aspect.map_or(available, |w| available.max(w + horizontal_edges(node)));
            border_width(node, flow.width, room, natural)
        }
    };
    let x = forced
        .map(|bounds| bounds.x)
        .unwrap_or_else(|| left + start_margin(context, node, room, width));
    let sides = super::sizing::border_sides(style);
    let stretch = super::sizing::stretch_height(context, node, flow.definite);
    let authored = specified_height(node, forced, flow.definite).or_else(|| {
        super::sizing::stretched(node, stretch).map(|h| clamp_height(node, h, flow.definite))
    });
    // CSS Sizing 4's automatic minimum in the ratio-dependent axis: with `min-height: auto` and
    // visible overflow, a height the ratio gives grows to fit the content.
    let grows = authored.is_none()
        && style.min_height.is_none()
        && !style.height_bounds_fit[0]
        && !style.height_stretch[1]
        && style.overflow == crate::document::Overflow::Visible;
    let specified = authored.or_else(|| aspect_height(node, width, flow.definite));
    let mut inner = flow.nested(
        x + style.padding[3] + sides[3],
        y + style.padding[0] + sides[0],
        (width - horizontal_edges(node)).max(0.0),
        fresh,
    );
    inner.strut = Some(strut(style));
    // Blink's SetInitialTextBoxTrim: a trim passed down stops at border or padding, and the
    // block's own `text-box-trim` adds to it.
    let passed = [(0, 0), (1, 2)].map(|(side, edge)| {
        forced.is_none() && flow.trim[side] && style.padding[edge] == 0.0 && sides[edge] == 0.0
    });
    inner.trim = [
        style.text_box_trim[0] || passed[0],
        style.text_box_trim[1] || passed[1],
    ];
    inner.align = alignment(style);
    inner.ellipsis = ellipsis_owner(node);
    inner.rtl = style.rtl;
    if let Some(rtl) = super::plaintext::paragraph_rtl(context, node) {
        let mut paragraph = style.clone();
        paragraph.rtl = rtl;
        inner.align = alignment(&paragraph);
        inner.rtl = rtl;
    }
    inner.mirror_lines = style.writing_lr && !style.writing_sideways;
    // A column flex item whose flexed height is indefinite lends its children no percentage basis.
    // A grid still stretches its tracks to that height: its items resolve against their areas.
    let indefinite = forced.is_some()
        && context.indefinite_item.take() == Some(node.local_id)
        && !style.display_grid;
    let basis = specified.filter(|_| !indefinite);
    inner.definite = basis
        .map(|height| (height - vertical_edges(node)).max(0.0))
        .or_else(|| super::sizing::quirk_definite(context, node, flow.definite));
    // A flex container is its own formatting context, so no margin collapses through it.
    let collapses = forced.is_none() && !fresh;
    let through_top = collapses && style.padding[0] == 0.0 && sides[0] == 0.0;
    if through_top && cleared.is_some() {
        // Clearance fixed this box's border edge; its children's adjoining top margins are
        // spent there and leave its first child at its content top.
        inner.set_strut(super::clearance::adjoining_strut(context, node));
    } else if through_top {
        // A first child's top margin collapses through a parent with no top padding or border.
        let joined = join(flow.strut(), style.margin[0]);
        inner.set_strut((y - flow.y + flow.collapsible, joined.1));
        inner.strut_open = true;
        if flow.strut_open {
            inner.adjoining = flow.adjoining;
        }
    }
    inner.trim_start = style.margin_trim[0];
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
            basis.map(|height| (height - vertical_edges(node)).max(0.0)),
            (clamp_height(node, 0.0, flow.definite) - vertical_edges(node)).max(0.0),
        )?;
    } else if super::multicol::applies(node) {
        super::multicol::children(context, node, &mut inner, items, index)?;
    } else {
        let letter = super::first_letter::open(context, node, forced.is_some());
        let outer_open = flow.finished.is_empty() && flow.frags.is_empty();
        let line = super::first_line_layout::open(context, node, (forced.is_some(), outer_open));
        super::generated::generated(context, node, &mut inner, items, false)?;
        let own = super::first_letter::lead(context, node, &mut inner, items)?;
        let lines = super::text::text(context, &own, node, &mut inner, index)?;
        items[index].lines = lines;
        if !super::generated::is_generated(node) {
            for child in context.children(node.local_id) {
                node_layout(context, child, &mut inner, items, None)?;
            }
        }
        super::generated::generated(context, node, &mut inner, items, true)?;
        super::first_letter::close(context, letter);
        super::first_line_layout::close(context, line);
    }
    inner.newline();
    if style.margin_trim[1] {
        trim_end(&mut inner);
    }
    super::line::settle(context, &mut inner, items, index + 1)?;
    super::line::trim_end(&mut inner, items, style.text_box_trim[1]);
    super::line_clamp::clamp(context, node, items, index, &mut inner)?;
    if through_top
        && cleared.is_none()
        && let Some(top) = inner.resolved
    {
        // The collapsed margins resolved at the first child's border edge, which is where this
        // box's border edge sits too (Blink resolves both at one BFC block offset).
        position_floats(&mut inner, items, top - y);
        y = top;
        items[index].y = top;
        flow.placed_at(top);
    } else if through_top && inner.strut_open && flow.strut_open {
        // Still unresolved: the floats wait for the outer box, which shares their list.
        flow.unpositioned.append(&mut inner.unpositioned);
        flow.adjoining = [0, 1].map(|side| flow.adjoining[side] || inner.adjoining[side]);
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
        && !inner.cleared_collapsing
        && inner.bottom <= inner.y;
    let trailing = if through { inner.collapsible } else { 0.0 };
    let trailing_strut = if through { inner.strut() } else { (0.0, 0.0) };
    let natural =
        (inner.bottom - trailing - y + style.padding[2] + sides[2]).max(vertical_edges(node));
    let natural = super::sizing::contained_height(node, natural);
    // A table cell's authored height is a minimum: Blink sizes the cell to the larger of it and
    // its content, and aligns the content by its own height (recorded here for the table).
    let cell =
        forced.is_none_or(|bounds| bounds.height.is_none()) && super::table::is_cell_box(node);
    let specified = specified.map(|height| {
        if cell {
            height.max(natural)
        } else if grows {
            height.max(calc_size_through(style, MIN_HEIGHT, natural))
        } else {
            height
        }
    });
    context.record_content_height(node.local_id, natural);
    let automatic = calc_size_through(style, HEIGHT, natural);
    let height = clamp_height(node, specified.unwrap_or(automatic), flow.definite);
    // A box with an aspect ratio has the height the ratio gives it as its content height.
    let intrinsic = aspect_height(node, width, flow.definite).unwrap_or(natural);
    let height = super::sizing::clamp_content_bounds(node, height, intrinsic, flow.definite);
    let height = super::sizing::clamp_stretch(node, height, stretch);
    // Blink's FinishLayout: a block size other than the intrinsic one (min-height grew it, or
    // max-height cut it) separates the last child's bottom margin, which is then dropped.
    let trailing_strut = if height == natural {
        trailing_strut
    } else {
        (0.0, 0.0)
    };
    items[index].height = height;
    if forced.is_none() {
        // Blink's UpdateTextBoxTrim: any in-flow block takes the first formatted line, and
        // only a last block that leaves its end cut to its parent passes it on.
        flow.trim[0] = false;
        flow.tail = inner
            .tail
            .take()
            .filter(|tail| {
                passed[1]
                    && specified.is_none()
                    && !style.text_box_trim[1]
                    && !cleared_past(&inner, tail.2)
            })
            .map(|(cut, mut nested, _)| {
                nested.push(index);
                (cut, nested, f64::INFINITY)
            });
    }
    super::generated::absolute(context, node, items, index)?;
    if forced.is_some() {
        flow.y = y + height;
        flow.collapsible = 0.0;
        flow.bottom = flow.bottom.max(flow.y);
    } else if height == 0.0 && !fresh {
        // An empty block's own top and bottom margins collapse with the ones around it; a new
        // formatting context's never collapse through it (CSS 2.1 8.3.1).
        // With clearance its border edge sits at the floats' bottom and the strut before the
        // clearance is separated: the margins restart at its own top margin edge (Blink's
        // ComputeInflowPosition for a self-collapsing cleared child).
        let (base, previous) = match cleared {
            Some(clear) => (clear - style.margin[0], (0.0, 0.0)),
            None => above,
        };
        // Children whose margins collapsed through it already joined its top margin; else its
        // own top and bottom margins join the strut before it.
        let own = [flow.trimmed(style.margin[0]), flow.trimmed(style.margin[2])];
        let combined = if through_top && through && cleared.is_none() && !style.margin_trim[1] {
            join(trailing_strut, own[1])
        } else {
            join(join(previous, own[0]), own[1])
        };
        flow.y = base + combined.0;
        flow.set_strut(combined);
        flow.bottom = flow.bottom.max(flow.y);
        flow.cleared_collapsing |= cleared.is_some();
    } else {
        flow.after_block_strut(y + height, join(trailing_strut, style.margin[2]));
    }
    Ok(())
}
/** `margin-trim: block-end`: the margins of the children adjoining the container's end are
 * dropped, so its content ends at the last child's border edge (css-box-4). */
fn trim_end(inner: &mut Flow) {
    let (total, _) = inner.strut();
    if total != 0.0 && inner.bottom == inner.y {
        inner.y -= total;
        inner.bottom = inner.y;
    }
    inner.set_strut((0.0, 0.0));
}
/** Blink's HandleNewFormattingContext abort: a formatting-context root with a top margin,
 * met while floats adjoin the open margin strut, is first tried with that margin collapsing
 * (the floats moving down with it). When it does not fit beside them there, its margin is
 * separated like clearance: the strut resolves where it stands, the floats stay, and the root
 * takes the first opportunity from there, its margin ignored. None when the margins adjoin. */
fn separated_from_adjoining(
    flow: &mut Flow,
    node: &Node,
    cleared: Option<f64>,
) -> Option<(f64, f64, f64)> {
    let adjoining = flow.strut_open && flow.adjoining.iter().any(|side| *side);
    if !adjoining || cleared.is_some() || node.style.margin[0] == 0.0 {
        return None;
    }
    // Floats sit at the strut so far, so trying the root there with them unmoved is the same
    // as trying it at the collapsed position with them moved down by the margin.
    let separated = flow.y;
    let placed = beside_floats(flow, node, separated);
    if placed.0 <= separated {
        return None;
    }
    flow.strut_open = false;
    flow.placed_at(placed.0);
    Some(placed)
}
/** Whether `clear` names a side with a float adjoining the open margin strut. */
fn clears_adjoining(flow: &Flow, clear: crate::document::style::Clear) -> bool {
    use crate::document::style::Clear;
    let [left, right] = flow.adjoining;
    match clear {
        Clear::None => false,
        Clear::Left => left,
        Clear::Right => right,
        Clear::Both => left || right,
    }
}
/** The nearest ancestor that is no inline box: a block inside an inline takes its containing
 * block's direction, not the inline's (CSS 2.1 9.2.1.1). None for the body's children. */
fn containing_block<'a>(context: &LayoutContext<'a>, node: &Node) -> Option<&'a Node> {
    let mut current = node.parent.and_then(|parent| context.node(parent).ok());
    while let Some(ancestor) = current {
        if !ancestor.style.inline || ancestor.style.inline_block {
            return Some(ancestor);
        }
        current = ancestor.parent.and_then(|parent| context.node(parent).ok());
    }
    None
}
/** The inline-start margin, with a legacy `<center>` parent centering narrower blocks. In a
 * right-to-left parent an over-constrained box keeps its right margin and the left one gives
 * way (CSS 2.1 10.3.3). */
fn start_margin(context: &LayoutContext<'_>, node: &Node, room: f64, width: f64) -> f64 {
    let parent = node.parent.and_then(|parent| context.node(parent).ok());
    let auto = node.style.margin_auto;
    let body = &context.document.formatting.body.style;
    let rtl = containing_block(context, node).map_or(body.rtl, |block| block.style.rtl);
    if rtl && !auto[1] && !auto[3] {
        return room - width - node.style.margin[1];
    }
    let legacy = parent.is_some_and(|parent| parent.style.align == "webkit-center");
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
        || style.multicol.container()
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
    let height = if ratio_sizes_border_box(node) {
        (width / ratio).max(vertical_edges(node))
    } else {
        (width - horizontal_edges(node)).max(0.0) / ratio + vertical_edges(node)
    };
    let height = calc_size_through(&node.style, HEIGHT, height);
    Some(clamp_height(node, height, definite))
}
/** Whether `aspect-ratio` sizes the border box: under `box-sizing: border-box`, unless the
value says `auto`, which makes a non-replaced box's ratio size its content box (CSS Sizing 4). */
pub(super) fn ratio_sizes_border_box(node: &Node) -> bool {
    node.style.box_sizing == crate::document::BoxSizing::BorderBox && !node.style.aspect_ratio_auto
}
/** The content-box width `aspect-ratio` gives an auto-width block whose height is definite (CSS
Sizing 4's transferred size, Blink's ComputeInlineSizeForFragment), raised to its min-content
as the automatic minimum size does; None when the width does not come from the ratio. */
fn aspect_width(
    context: &mut LayoutContext<'_>,
    node: &Node,
    definite: Option<f64>,
) -> EngineResult<Option<f64>> {
    let style = &node.style;
    let Some(ratio) = style
        .aspect_ratio
        .filter(|_| style.width.is_none() && !style.width_fit)
    else {
        return Ok(None);
    };
    let Some(height) = specified_height(node, None, definite) else {
        return Ok(None);
    };
    let content = if ratio_sizes_border_box(node) {
        height * ratio - horizontal_edges(node)
    } else {
        (height - vertical_edges(node)).max(0.0) * ratio
    };
    let automatic = super::sizing::min_width_auto(style)
        && style.overflow == crate::document::Overflow::Visible;
    let minimum = if automatic {
        let content = context.intrinsic_widths(node.local_id)?.min_content;
        calc_size_through(style, MIN_WIDTH, content)
    } else {
        0.0
    };
    let content = calc_size_through(style, WIDTH, content);
    Ok(Some(content.max(minimum).max(0.0)))
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
        resize_box: None,
        canvas: node.canvas.clone(),
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
        radius_y: None,
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
        quad: None,
        layout_origin: None,
        pin: None,
        z: 0,
        stacking_opacity: Vec::new(),
        font_weight: face.weight,
        font_family: face.family,
        letter_spacing: face.spacing,
        word_spacing: node.style.word_spacing,
        autospace: face.autospace,
        upright: face.upright,
        underline: !node.style.text_decoration_none
            && (node.style.underline || (node.tag == "a" && node.href.is_some())),
        strikethrough: !node.style.text_decoration_none && node.style.strikethrough,
        decoration: super::display::decoration_line(&node.style),
        outline: None,
        italic: face.italic,
        object_fit: crate::document::style::ObjectFit::Fill,
        object_position: super::display::default_object_position(),
        content_box: None,
        overflow_visible: false,
        mask: None,
        border_image: None,
        background_clip: background_insets(node, node.style.background_clip),
        background_border_area: node.style.background_clip == BackgroundBox::BorderArea,
        background_rect: None,
        slice_cut: [false; 2],
        bare: false,
        text_deferred: false,
        text_echo: false,
        paint_after: None,
        inline_root: 0,
        background_origin: (node.style.background_origin != BackgroundBox::PaddingBox)
            .then(|| background_insets(node, node.style.background_origin)),
        writing: 0,
    }
}
use crate::document::style::BackgroundBox;
use crate::document::style::{HEIGHT, MIN_HEIGHT, MIN_WIDTH, WIDTH, calc_size_through};
/** A background box as insets from the border box: none for the border box (and the border
area, which paints within it), the border widths for the padding box, and the padding too for
the content box. */
fn background_insets(node: &Node, area: BackgroundBox) -> [f64; 4] {
    let sides = super::sizing::border_sides(&node.style);
    match area {
        BackgroundBox::BorderBox | BackgroundBox::BorderArea => [0.0; 4],
        BackgroundBox::PaddingBox => sides,
        BackgroundBox::ContentBox => std::array::from_fn(|i| sides[i] + node.style.padding[i]),
    }
}
