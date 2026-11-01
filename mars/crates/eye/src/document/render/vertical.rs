/**
 * Vertical writing modes (`vertical-rl`, `vertical-lr`). A box whose writing mode is vertical
 * while its parent's is horizontal roots an orthogonal flow (CSS Writing Modes 3). Its subtree
 * is laid out by the ordinary horizontal engine in logical coordinates, on a copy whose styles
 * are rotated (width and height swap, each box side moves to its logical side), then every box,
 * line and border maps back to physical coordinates: block progression runs right to left for
 * `vertical-rl` and left to right for `vertical-lr`, and lines run top to bottom. Painters draw a
 * mapped item's text turned a quarter clockwise (`DisplayItem::writing`).
 * ponytail: text is always set sideways (`text-orientation: mixed` draws CJK upright in
 * Blink), and a vertical root inside a vertical parent is not re-rooted; the whole subtree
 * shares one rotation.
 */
use super::flow::{BorderBox, Flow, LayoutContext, node_layout};
use crate::document::{
    DisplayItem, Document, Node, PathCommand,
    style::{ComputedStyle, Extent},
};
use crate::protocol::EngineResult;

/** Which side (top, right, bottom, left) of the physical box each logical side is, for
`vertical-rl`: block-start is the right edge and inline-start the top. */
const RL_SIDES: [usize; 4] = [1, 2, 3, 0];
/** The same for `vertical-lr`: block-start is the left edge. */
const LR_SIDES: [usize; 4] = [3, 2, 1, 0];
/** Which physical corner (top-left, top-right, bottom-right, bottom-left) each logical corner
is, for `vertical-rl`. */
const RL_CORNERS: [usize; 4] = [1, 2, 3, 0];
/** The same for `vertical-lr`. */
const LR_CORNERS: [usize; 4] = [0, 3, 2, 1];
/** The inverse of RL_SIDES, which turns a clockwise-rotated style back. */
const CCW_SIDES: [usize; 4] = [3, 0, 1, 2];
/** The inverse of RL_CORNERS. */
const CCW_CORNERS: [usize; 4] = [3, 0, 1, 2];
/** `DisplayItem::writing` for an item a counter-clockwise turn placed back inside a
`vertical-rl` subtree; the subtree's clockwise turn cancels it. */
pub const WRITING_CCW: u8 = 3;
/** `DisplayItem::writing` for a `vertical-rl` item. */
pub const WRITING_RL: u8 = 1;
/** `DisplayItem::writing` for a `vertical-lr` item. */
pub const WRITING_LR: u8 = 2;

/** How a subtree's logical layout maps onto its parent's coordinates. */
#[derive(Clone, Copy, PartialEq, Debug)]
enum Turn {
    /// `vertical-rl`: a quarter turn clockwise; blocks stack from the right edge.
    Clockwise,
    /// `vertical-lr`: the axes swap; blocks stack from the left edge.
    Transpose,
    /// A horizontal box inside a `vertical-rl` subtree: the clockwise turn undone.
    Counter,
}
impl Turn {
    /** The turn a root lays out with: its own vertical mode, or the undoing of its subtree's. */
    fn of(style: &ComputedStyle) -> Self {
        match (
            style.counter_rotate,
            style.writing_lr,
            style.writing_sideways,
        ) {
            (WRITING_RL, ..) => Self::Counter,
            (WRITING_CCW, ..) => Self::Clockwise,
            (WRITING_LR, ..) => Self::Transpose,
            // `sideways-lr` reads bottom to top, glyphs turned counter-clockwise.
            (_, true, true) => Self::Counter,
            (_, true, false) => Self::Transpose,
            _ => Self::Clockwise,
        }
    }
    /** Which side of the outer box each inner side is (top, right, bottom, left). */
    fn sides(self) -> [usize; 4] {
        match self {
            Self::Clockwise => RL_SIDES,
            Self::Transpose => LR_SIDES,
            Self::Counter => CCW_SIDES,
        }
    }
    /** Which outer corner each inner corner is (top-left, top-right, bottom-right, bottom-left). */
    fn corners(self) -> [usize; 4] {
        match self {
            Self::Clockwise => RL_CORNERS,
            Self::Transpose => LR_CORNERS,
            Self::Counter => CCW_CORNERS,
        }
    }
    /** An inner point in outer coordinates, for a root at `origin` whose outer box is `extent`
    (width, height). */
    fn point(self, origin: (f64, f64), extent: (f64, f64), (x, y): (f64, f64)) -> (f64, f64) {
        match self {
            Self::Clockwise => (origin.0 + extent.0 - y, origin.1 + x),
            Self::Transpose => (origin.0 + y, origin.1 + x),
            Self::Counter => (origin.0 + y, origin.1 + extent.1 - x),
        }
    }
    /** An inner rectangle in outer coordinates: the corner that lands top-left, sides swapped. */
    fn rect(self, origin: (f64, f64), extent: (f64, f64), [x, y, w, h]: [f64; 4]) -> [f64; 4] {
        let corner = match self {
            Self::Clockwise => (x, y + h),
            Self::Transpose => (x, y),
            Self::Counter => (x + w, y),
        };
        let (px, py) = self.point(origin, extent, corner);
        [px, py, h, w]
    }
    /** The writing flag an item mapped by this turn carries: its own, or the two cancelled. */
    fn flag(self, writing: u8) -> u8 {
        match (self, writing) {
            (Self::Clockwise, WRITING_CCW)
            | (Self::Counter, WRITING_RL)
            | (Self::Transpose, WRITING_LR) => 0,
            (_, 0) => match self {
                Self::Clockwise => WRITING_RL,
                Self::Transpose => WRITING_LR,
                Self::Counter => WRITING_CCW,
            },
            (_, kept) => kept,
        }
    }
}

/** Whether `node` roots a vertical subtree: vertical itself and its parent horizontal. A
top-level box always roots one: the initial containing block is horizontal, and a vertical body
reaches layout as one synthetic top-level box (body.rs `container_root`). */
pub(super) fn is_root(context: &LayoutContext<'_>, node: &Node) -> bool {
    if !node.style.vertical_writing {
        return false;
    }
    node.parent
        .and_then(|parent| context.node(parent).ok())
        .is_none_or(|parent| !parent.style.vertical_writing)
}

/** A vertical subtree laid out at the physical border box `bounds` (or, without one, placed
as a block in `flow`): its logical layout, mapped into `items`. */
pub(super) fn layout(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
    forced: Option<BorderBox>,
) -> EngineResult<()> {
    // A block container's definite height is the margin box's room, so an auto-height box gets
    // it less its margins; a flex or grid container already hands over the item's own area.
    let parent = node.parent.and_then(|parent| context.node(parent).ok());
    let item = parent.is_some_and(|p| p.style.display_flex || p.style.display_grid);
    // An authored height resolves its percentage against the whole container height.
    let margins = match item || node.style.height.is_some() {
        true => 0.0,
        false => node.style.margin[0] + node.style.margin[2],
    };
    let room = forced
        .and_then(|bounds| bounds.height)
        .or(flow.definite.map(|height| (height - margins).max(0.0)));
    let available = room
        .or_else(|| scroller_room(context, node))
        .unwrap_or(context.viewport.1);
    // A flex or grid item measured before its area is known has an indefinite containing
    // block, so its percentage height behaves as auto (css-grid-2 6.6; Blink resolves it once
    // the area is sized).
    let percent_auto = item && room.is_none();
    // CSS Writing Modes 7.3: an orthogonal box shrinks to fit its available space, the container's
    // definite height or else the viewport's; only the document's root fills.
    let fit = forced.is_none_or(|bounds| bounds.height.is_none())
        && !(node.parent.is_none() && context.document.formatting.body.style.vertical_writing);
    // A forced box without a height leaves the logical width (the physical height) to the
    // subtree: fit-content in the room, as an orthogonal flow sizes (CSS Writing Modes 7.3).
    let logical_forced = match forced {
        Some(bounds) => Some(BorderBox {
            x: 0.0,
            y: 0.0,
            width: match bounds.height {
                Some(height) => height,
                None => {
                    logical(
                        context,
                        node,
                        (available, Some(flow.width)),
                        None,
                        (true, percent_auto),
                    )?
                    .1
                    .1
                }
            },
            height: Some(bounds.width),
        }),
        None => None,
    };
    let block = (available, Some(flow.width));
    let (laid, size) = logical(context, node, block, logical_forced, (fit, percent_auto))?;
    let (x, y) = match forced {
        Some(bounds) => (bounds.x, bounds.y),
        None if document_start(context, node) || is_document(context, node) => {
            place_document(node, flow, size)
        }
        None => place(node, flow, size, false),
    };
    let turn = Turn::of(&node.style);
    let viewport = context.viewport;
    for item in laid {
        // A box placed against the viewport maps back through the viewport, not the subtree.
        let mapped = match on_viewport(context, node, item.local_id) {
            true => map_item(item, turn, (0.0, 0.0), viewport),
            false => map_item(item, turn, (x, y), size),
        };
        items.push(mapped);
    }
    if let Some(bounds) = forced {
        flow.y = bounds.y + size.1;
        flow.bottom = flow.bottom.max(flow.y);
    }
    Ok(())
}

/** Whether an item of the vertical subtree rooted at `root` belongs to an out-of-flow box whose
containing block is the viewport: its rotated layout placed it against the rotated viewport,
which a vertical root's padding does not move (sizing-orthog references). */
fn on_viewport(context: &LayoutContext<'_>, root: &Node, local: usize) -> bool {
    let mut current = context.node(local).ok();
    while let Some(node) = current.filter(|node| node.local_id != root.local_id) {
        let anchored =
            super::positioned::containing_node(context, node).is_ok_and(|cb| cb.is_none());
        if super::flow::out_of_flow(node) && anchored && inset_both_axes(node) {
            return true;
        }
        current = node.parent.and_then(|parent| context.node(parent).ok());
    }
    false
}

/** Whether a box has an inset on each axis; one left at auto takes its static position, which
the subtree's own flow placed. */
fn inset_both_axes(node: &Node) -> bool {
    let inset = &node.style.inset;
    (inset[0].is_some() || inset[2].is_some()) && (inset[1].is_some() || inset[3].is_some())
}

/** CSS Writing Modes 7.3.1: with no definite room from its container, an orthogonal box's
available space is its nearest scroll container's fixed content height, else its fixed
max-height (raised to its min-height), never more than the initial containing block; None when
no scroller fixes either (the viewport then gives it). */
fn scroller_room(context: &LayoutContext<'_>, node: &Node) -> Option<f64> {
    let mut current = node.parent.and_then(|parent| context.node(parent).ok());
    while let Some(ancestor) = current {
        let style = &ancestor.style;
        if style.overflow != crate::document::Overflow::Visible {
            let bound = style.height.or(style.max_height)?.resolve_size(None)?;
            // Its min-height wins over the max, and the initial containing block caps both.
            let floor = style.min_height.and_then(|min| min.resolve_size(None));
            let bound = floor.map_or(bound, |floor| bound.max(floor));
            let content = match style.box_sizing {
                crate::document::BoxSizing::BorderBox => {
                    (bound - super::flow::vertical_edges(ancestor)).max(0.0)
                }
                _ => bound,
            };
            return Some(content.min(context.viewport.1));
        }
        current = ancestor.parent.and_then(|parent| context.node(parent).ok());
    }
    None
}

/** The border-box width a vertical root takes as an intrinsic size: its logical height,
after a layout in the space it would get. */
pub(super) fn intrinsic_width(context: &mut LayoutContext<'_>, node: &Node) -> EngineResult<f64> {
    let available = context.viewport.1;
    let parent = node.parent.and_then(|parent| context.node(parent).ok());
    let item = parent.is_some_and(|p| p.style.display_flex || p.style.display_grid);
    Ok(
        logical(context, node, (available, None), None, (true, item))?
            .1
            .0,
    )
}

/** Turns the rotated root's percentage logical width (its physical height) and its
 * percentage minimum and maximum into `auto`, for a measure against an indefinite size. */
fn auto_percent_width(document: &mut Document, root: usize, fit: bool) {
    let Some(node) = document.nodes.iter_mut().find(|node| node.local_id == root) else {
        return;
    };
    let style = &mut node.style;
    if style.percent_width {
        (style.width, style.percent_width) = (None, false);
        style.width_fit |= fit;
    }
    style.min_width_percent = None;
    if style.percent_max_width {
        (style.max_width, style.percent_max_width) = (None, false);
    }
}

/** The subtree's logical items and its physical border-box size (width, height), laid out in
`available` logical width with the parent's physical content width, when known, as the
definite logical height percentages resolve against. */
fn logical(
    context: &LayoutContext<'_>,
    node: &Node,
    (available, block): (f64, Option<f64>),
    forced: Option<BorderBox>,
    (fit, percent_auto): (bool, bool),
) -> EngineResult<(Vec<DisplayItem>, (f64, f64))> {
    let mut rotated = rotated(context.document, node, fit);
    if percent_auto {
        auto_percent_width(&mut rotated, node.local_id, fit);
    }
    let mut inner = LayoutContext::new(&rotated)?;
    inner.viewport = (context.viewport.1, context.viewport.0);
    let root = inner.node(node.local_id)?;
    let mut flow = Flow::new(0.0, 0.0, available);
    flow.definite = block;
    let mut items = Vec::new();
    node_layout(&mut inner, root, &mut flow, &mut items, forced)?;
    flow.newline();
    super::line::settle(&mut inner, &mut flow, &mut items, 0)?;
    let room = (available, inner.viewport.1);
    super::positioned::place(&mut inner, &mut items, room)?;
    let size = items
        .iter()
        .find(|item| item.local_id == node.local_id)
        .map_or((0.0, 0.0), |item| (item.height, item.width));
    Ok((items, size))
}

/** Whether `node` is a vertical body's synthetic top-level box. */
fn is_document(context: &LayoutContext<'_>, node: &Node) -> bool {
    node.parent.is_none() && context.document.formatting.body.style.vertical_writing
}

/** Places a vertical document's synthetic root, whose margins its rotated layout already
holds: at the start edge, the right one for `vertical-rl`. */
fn place_document(node: &Node, flow: &mut Flow, (width, height): (f64, f64)) -> (f64, f64) {
    flow.newline();
    let y = flow.y;
    let x = if node.style.writing_lr {
        flow.x
    } else {
        flow.x + flow.width - width
    };
    flow.placed_at(y);
    flow.after_block(y + height, 0.0);
    (x, y)
}

/** Whether `node` is a vertical body's synthetic top-level box laid out right to left: the
document's block progression then starts at the viewport's right edge. */
fn document_start(context: &LayoutContext<'_>, node: &Node) -> bool {
    node.parent.is_none()
        && !node.style.writing_lr
        && context.document.formatting.body.style.vertical_writing
}

/** Places a block-level vertical root in its horizontal parent's flow and returns its border
box's top-left corner; `from_right` anchors a `vertical-rl` document at the right edge. */
fn place(
    node: &Node,
    flow: &mut Flow,
    (width, height): (f64, f64),
    from_right: bool,
) -> (f64, f64) {
    let style = &node.style;
    flow.newline();
    let y = flow.block_top(style.margin[0]);
    let free = (flow.width - width - style.margin[1] - style.margin[3]).max(0.0);
    let left = match (style.margin_auto[3], style.margin_auto[1]) {
        (true, true) => free / 2.0,
        (true, false) => free,
        _ if from_right => flow.width - width - style.margin[1],
        _ => style.margin[3],
    };
    flow.placed_at(y);
    flow.after_block(y + height, style.margin[2]);
    (flow.x + left, y)
}

/** The document holding just `root`'s subtree with every style rotated into logical axes. The
root keeps no margins (its parent's flow places it by its physical ones) and, when `fit`, sizes
its auto logical width to fit its content. Natural image sizes turn with the axes. */
fn rotated(document: &Document, root: &Node, fit: bool) -> Document {
    let turn = Turn::of(&root.style);
    // A counter root lays a horizontal subtree back out; its nodes are not marked again.
    let undoing = root.style.counter_rotate != 0;
    // A vertical document's synthetic root keeps its (the body's) margins, which collapse with
    // its children's inside the rotated layout.
    let document_root = root.parent.is_none() && document.formatting.body.style.vertical_writing;
    let mut inside = std::collections::BTreeSet::from([root.local_id]);
    let mut copy = document.clone();
    copy.nodes.retain(|node| {
        let keep = node.local_id == root.local_id
            || node.parent.is_some_and(|parent| inside.contains(&parent));
        if keep {
            inside.insert(node.local_id);
        }
        keep
    });
    for node in &mut copy.nodes {
        // A horizontal descendant of a vertical subtree turns with it, then roots its own
        // subtree that turns back (`Turn::Counter` or a second transpose).
        let horizontal = !node.style.vertical_writing && !undoing;
        let undoes = turn.flag(0);
        let counter = node.style.counter_rotate != 0;
        node.style = rotate_style(&node.style, turn, undoing);
        if horizontal && !counter && node.local_id != root.local_id {
            node.style.vertical_writing = true;
            node.style.counter_rotate = undoes;
        }
        node.image_size = node.image_size.map(|(width, height)| (height, width));
        if node.local_id == root.local_id && !document_root {
            node.style.margin = [0.0; 4];
            node.style.margin_auto = [false; 4];
        }
        if node.local_id == root.local_id {
            node.parent = None;
            node.style.width_fit |= fit && node.style.width.is_none();
            // A writing-mode root establishes a block formatting context (CSS Writing Modes
            // 3.2), so it holds its floats; the document's synthetic root already is one.
            node.style.flow_root |= !document_root;
        }
    }
    copy
}

/** The used glyph orientation of a node in a vertical subtree: `mixed` stands CJK upright and
`upright` every character; sideways writing, `text-orientation: sideways` and a horizontal
subtree turned back keep every glyph turned. */
fn glyphs_upright(style: &ComputedStyle, undoing: bool) -> u8 {
    use crate::platform::upright::{MIXED, TURNED, UPRIGHT};
    if undoing || !style.vertical_writing || style.writing_sideways {
        return TURNED;
    }
    match style.text_orientation {
        0 => MIXED,
        1 => UPRIGHT,
        _ => TURNED,
    }
}

/** One style with its physical axes turned into the subtree's logical ones. */
fn rotate_style(style: &ComputedStyle, turn: Turn, undoing: bool) -> ComputedStyle {
    let (sides, corners) = (turn.sides(), turn.corners());
    let pick = |values: [f64; 4]| sides.map(|side| values[side]);
    let mut out = style.clone();
    out.vertical_writing = false;
    out.counter_rotate = 0;
    out.central_baseline = !undoing && !style.writing_sideways;
    out.glyphs_upright = glyphs_upright(style, undoing);
    out.margin = pick(style.margin);
    out.margin_auto = sides.map(|side| style.margin_auto[side]);
    out.margin_percent = pick(style.margin_percent);
    out.padding = pick(style.padding);
    out.padding_percent = pick(style.padding_percent);
    out.border_widths = pick(style.border_widths);
    out.border_colors = sides.map(|side| style.border_colors[side].clone());
    out.border_styles = sides.map(|side| style.border_styles[side]);
    out.inset = sides.map(|side| style.inset[side]);
    out.inset_percent = pick(style.inset_percent);
    out.radius = corners.map(|corner| style.radius[corner]);
    out.radius_percent = corners.map(|corner| style.radius_percent[corner]);
    (out.overflow_x, out.overflow_y) = (style.overflow_y, style.overflow_x);
    out.aspect_ratio = style.aspect_ratio.map(|ratio| 1.0 / ratio);
    swap_sizes(style, &mut out);
    out
}

/** Width and height trade places: `width` keeps a length or a percentage with its offset,
`height` an extent, so each converts into the other's form. */
fn swap_sizes(style: &ComputedStyle, out: &mut ComputedStyle) {
    let extent = |width: Option<f64>, percent: bool, offset: f64| {
        width.map(|value| match percent {
            true => Extent {
                px: offset,
                percent: value,
            },
            false => Extent {
                px: value,
                percent: 0.0,
            },
        })
    };
    out.height = extent(style.width, style.percent_width, style.width_offset);
    out.min_height = extent(style.min_width, false, 0.0).or_else(|| {
        style.min_width_percent.map(|percent| Extent {
            px: style.min_width_offset,
            percent,
        })
    });
    out.max_height = extent(
        style.max_width,
        style.percent_max_width,
        style.max_width_offset,
    );
    let (width, percent, offset) = length(style.height);
    (out.width, out.percent_width, out.width_offset) = (width, percent, offset);
    let (min, min_percent, min_offset) = length(style.min_height);
    (out.min_width, out.min_width_percent) = match min_percent {
        true => (None, min),
        false => (min, None),
    };
    out.min_width_offset = min_offset;
    let (max, max_percent, max_offset) = length(style.max_height);
    (out.max_width, out.percent_max_width) = (max, max_percent);
    out.max_width_offset = max_offset;
    out.height_fit = style.width_fit || style.size_keywords[0] != Default::default();
    out.width_fit = style.height_fit;
    out.width_stretch = false;
    out.size_keywords = Default::default();
}

/** An extent as `width`'s (value, is percentage, pixel offset) form. */
fn length(extent: Option<Extent>) -> (Option<f64>, bool, f64) {
    match extent {
        Some(Extent { px, percent }) if percent != 0.0 => (Some(percent), true, px),
        Some(Extent { px, .. }) => (Some(px), false, 0.0),
        None => (None, false, 0.0),
    }
}

/** One inner item mapped to outer coordinates by `turn`, for a root at `origin` whose outer
box is `extent` (width, height). */
fn map_item(
    mut item: DisplayItem,
    turn: Turn,
    origin: (f64, f64),
    extent: (f64, f64),
) -> DisplayItem {
    let rect = |r: [f64; 4]| turn.rect(origin, extent, r);
    let before = (item.x, item.y);
    [item.x, item.y, item.width, item.height] = rect([item.x, item.y, item.width, item.height]);
    // Gradient geometry is relative to the item's top-left: through the turn and back.
    let relative = |x: f64, y: f64| {
        let (px, py) = turn.point(origin, extent, (before.0 + x, before.1 + y));
        (px - item.x, py - item.y)
    };
    for layer in &mut item.backgrounds {
        turn_layer(layer, &relative);
    }
    let advance = item.line_height.unwrap_or(item.font_size);
    // Text an inner turn already turned stands advance wide and its width tall.
    let turned = item.writing != 0;
    for line in item.lines.iter_mut().chain(item.validation.iter_mut()) {
        let (w, h) = match turned {
            true => (advance, line.width),
            false => (line.width, advance),
        };
        let [x, y, _, _] = rect([line.x, line.y, w, h]);
        (line.x, line.y) = (x, y);
    }
    if let Some(clip) = &mut item.clip {
        [clip.x, clip.y, clip.width, clip.height] = rect([clip.x, clip.y, clip.width, clip.height]);
    }
    item.content_box = item.content_box.map(rect);
    map_sides(&mut item, turn);
    let point = |x: f64, y: f64| turn.point(origin, extent, (x, y));
    for path in &mut item.vector {
        map_path(&mut path.commands, &point);
    }
    item.writing = turn.flag(item.writing);
    item
}

/** A gradient layer's geometry through `relative` (an item-relative point mapping); a radial
gradient's radii trade axes. Image layers keep their placement. */
fn turn_layer(
    layer: &mut crate::document::DisplayBackground,
    relative: &dyn Fn(f64, f64) -> (f64, f64),
) {
    use crate::document::DisplayBackground;
    match layer {
        DisplayBackground::Linear { x0, y0, x1, y1, .. } => {
            (*x0, *y0) = relative(*x0, *y0);
            (*x1, *y1) = relative(*x1, *y1);
        }
        DisplayBackground::Radial { cx, cy, rx, ry, .. } => {
            (*cx, *cy) = relative(*cx, *cy);
            (*rx, *ry) = (*ry, *rx);
        }
        DisplayBackground::Image { .. } => {}
    }
}

/** Inner box sides and corners onto their outer ones. */
fn map_sides(item: &mut DisplayItem, turn: Turn) {
    let (sides, corners) = (turn.sides(), turn.corners());
    let (widths, colors, styles, radius) = (
        item.border_widths,
        item.border_colors.clone(),
        item.border_styles,
        item.radius,
    );
    for (logical, physical) in sides.into_iter().enumerate() {
        item.border_widths[physical] = widths[logical];
        item.border_colors[physical] = colors[logical].clone();
        item.border_styles[physical] = styles[logical];
    }
    for (logical, physical) in corners.into_iter().enumerate() {
        item.radius[physical] = radius[logical];
    }
}

/** Every point of a path through `point`. */
fn map_path(commands: &mut [PathCommand], point: &dyn Fn(f64, f64) -> (f64, f64)) {
    for command in commands {
        match command {
            PathCommand::Move { x, y } | PathCommand::Line { x, y } => (*x, *y) = point(*x, *y),
            PathCommand::Cubic {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                (*x1, *y1) = point(*x1, *y1);
                (*x2, *y2) = point(*x2, *y2);
                (*x, *y) = point(*x, *y);
            }
            PathCommand::Close => {}
        }
    }
}
