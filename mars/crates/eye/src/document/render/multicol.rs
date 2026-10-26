/**
 * css-multicol container layout, after Blink's ColumnLayoutAlgorithm: the content lays out
 * as one column of the used column width (`column-span: all` boxes in place at full width),
 * then `multicol_fragment` cuts it into rows of balanced (or, under `column-fill: auto` with a
 * definite height, filled) columns between the spanners, with `column-rule`s in the gaps.
 * Abspos static positions and intrinsic sizes ignore the columns.
 */
use super::flow::{Flow, LayoutContext, node_layout};
use super::multicol_fragment::{Placed, Shape, fragment};
use crate::document::{DisplayItem, Node, style::Float};
use crate::protocol::EngineResult;

/** The tag of a column rule item, which the effects pass leaves undecorated: it paints only
the container's rule, never its backgrounds or shadows. */
pub(super) const RULE_TAG: &str = "::column-rule";

/** The used column geometry, css-multicol-1 3.4's pseudo-algorithm. */
#[derive(Clone, Copy)]
pub(super) struct Columns {
    /// N, the used column count (the most columns balancing may use).
    pub count: usize,
    /// W, the used column width.
    pub width: f64,
    /// The used gap; `normal` is 1em for multicol.
    pub gap: f64,
    /// Columns progress right to left under `direction: rtl`.
    pub rtl: bool,
}
impl Columns {
    /** N and W from `column-count`, `column-width` and the available width U. */
    pub fn used(node: &Node, available: f64) -> Self {
        let style = &node.style;
        let gap = if style.gap_set[0] {
            style.column_gap + style.gap_percent[0] * available / 100.0
        } else {
            style.font_size
        };
        let fit = |width: f64| ((available + gap) / (width + gap)).floor().max(1.0) as usize;
        let count = match (style.multicol.count, style.multicol.width) {
            (Some(count), None) => count as usize,
            (None, Some(width)) => fit(width),
            (Some(count), Some(width)) => (count as usize).min(fit(width)),
            (None, None) => 1,
        };
        let width = ((available - (count as f64 - 1.0) * gap) / count as f64).max(0.0);
        Self {
            count,
            width,
            gap,
            rtl: style.rtl,
        }
    }
    /** How far column `k` sits from the content box's left edge. */
    pub fn offset(&self, k: usize, available: f64) -> f64 {
        let step = k as f64 * (self.width + self.gap);
        if self.rtl {
            available - self.width - step
        } else {
            step
        }
    }
}

/** Whether `node` lays out its children in columns: a block container with a column count or
width (flex and grid containers take their own layout first).
ponytail: a vertical writing mode keeps one column; its columns would progress in the block
axis, which needs the writing-mode layout to fragment along x. */
pub(super) fn applies(node: &Node) -> bool {
    let style = &node.style;
    style.multicol.container()
        && !style.display_flex
        && !style.display_grid
        && !style.vertical_writing
}

/** The multicol content box a `column-span: all` box spans, when it is one in a multicol
formatting context (a forced flex or grid box never spans). */
pub(super) fn spanning(
    context: &LayoutContext<'_>,
    flow: &Flow,
    node: &Node,
    forced: Option<super::flow::BorderBox>,
) -> Option<(f64, f64)> {
    let spans = forced.is_none() && spanned(context, node).is_some();
    flow.span.filter(|_| spans)
}

/** The multicol container a `column-span: all` box spans: its nearest multicol ancestor,
unless a box between them starts a formatting context or contains fixed-position boxes
(a transform, filter or paint containment), which keeps it a plain box in its column. */
pub(super) fn spanned(context: &LayoutContext<'_>, node: &Node) -> Option<usize> {
    if !spans(node) {
        return None;
    }
    let mut current = node.parent.and_then(|parent| context.node(parent).ok());
    while let Some(ancestor) = current {
        if applies(ancestor) {
            return Some(ancestor.local_id);
        }
        let blocks = super::flow::new_context(ancestor)
            || super::positioned::contains_fixed(&ancestor.style)
            || ancestor.style.multicol.spanner_barrier;
        if blocks {
            return None;
        }
        current = ancestor.parent.and_then(|parent| context.node(parent).ok());
    }
    None
}

/** Lays out the multicol container `items[index]`'s content into `inner`, its content flow:
one column-wide flow with spanners in place at full width, then cut into column rows,
leaving `inner.y` and `inner.bottom` at the content end. */
pub(super) fn children(
    context: &mut LayoutContext<'_>,
    node: &Node,
    inner: &mut Flow,
    items: &mut Vec<DisplayItem>,
    index: usize,
) -> EngineResult<()> {
    let legend = rendered_legend(context, node);
    if let Some(legend) = legend {
        node_layout(context, legend, inner, items, None)?;
    }
    let shape = shape(node, inner);
    let start = items.len();
    let known = shape.fixed.or(shape.available.filter(|_| shape.fill));
    let mut flow = column_flow(inner, shape.columns, known);
    content(context, node, &mut flow, items, (index, legend))?;
    let mut pending = super::multicol_oof::take(context, node)?;
    super::multicol_oof::place_inside(context, &pending, items)?;
    let statics = &mut pending.outside;
    let (rows, height) = fragment(context, node, items, (start, index), (&shape, statics));
    super::multicol_oof::restore(context, pending);
    for row in rows.iter().rev() {
        rules(node, items, &shape, row, start);
    }
    inner.y = shape.top + height;
    inner.bottom = inner.bottom.max(inner.y);
    inner.collapsible = 0.0;
    Ok(())
}

/** A fieldset's rendered legend, which sits above the columns rather than in them. */
fn rendered_legend<'a>(context: &LayoutContext<'a>, node: &Node) -> Option<&'a Node> {
    (node.tag == "fieldset")
        .then(|| context.children(node.local_id))?
        .into_iter()
        .find(|child| child.tag == "legend" && !child.removed())
}

/** Lays the container's content (generated boxes, own text, children but the legend) out in
the single column flow and settles its lines. */
fn content(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
    (index, legend): (usize, Option<&Node>),
) -> EngineResult<()> {
    let start = items.len();
    super::generated::generated(context, node, flow, items, false)?;
    let own = super::text::own_text(node);
    items[index].lines = super::text::text(context, &own, node, flow, index)?;
    for child in context.children(node.local_id) {
        if legend.is_none_or(|legend| legend.local_id != child.local_id) {
            node_layout(context, child, flow, items, None)?;
        }
    }
    super::generated::generated(context, node, flow, items, true)?;
    flow.newline();
    super::line::settle(context, flow, items, start)
}

/** Where and how the container's columns go, from its style and its content flow. */
fn shape(node: &Node, inner: &Flow) -> Shape {
    let style = &node.style;
    let multicol = &style.multicol;
    let available = available_height(node, inner);
    Shape {
        top: inner.y,
        columns: Columns::used(node, inner.width),
        content_left: inner.x,
        content_width: inner.width,
        available,
        fill: multicol.fill_auto,
        outer: outer_rows(inner, available.or(multicol.height)),
        fixed: multicol.height,
        wrap: multicol.wraps(),
        row_gap: if style.gap_set[1] {
            style.row_gap
        } else {
            style.font_size
        },
    }
}

/** The single column-wide flow the content lays out in, a fresh formatting context whose
spanners span the container's content box. */
fn column_flow(inner: &Flow, columns: Columns, known: Option<f64>) -> Flow {
    let mut flow = inner.nested(inner.x, inner.y, columns.width, true);
    flow.fragmentainer = known.map(|height| (inner.y, height));
    flow.strut = inner.strut;
    flow.align = inner.align;
    flow.definite = inner.definite;
    flow.span = Some((inner.x, inner.width));
    flow
}

/** Inside an enclosing multicol whose column height is known, an auto-height container's
rows follow its columns: (the first row's height, to the enclosing column's end, and every
later row's). */
fn outer_rows(inner: &Flow, own: Option<f64>) -> Option<(f64, f64)> {
    let (start, height) = inner.fragmentainer.filter(|_| own.is_none())?;
    if height <= 0.0 {
        return None;
    }
    let passed = ((inner.y - start) / height).floor() + 1.0;
    Some((start + passed * height - inner.y, height))
}

/** The definite column height: the container's definite content height, else its max-height. */
fn available_height(node: &Node, inner: &Flow) -> Option<f64> {
    inner.definite.or_else(|| {
        let max = node.style.max_height?.resolve_size(None)?;
        let edges = super::flow::vertical_edges(node);
        let border_box = node.style.box_sizing == crate::document::BoxSizing::BorderBox;
        Some(if border_box {
            (max - edges).max(0.0)
        } else {
            max
        })
    })
}

/** An in-flow block-level child with `column-span: all`, which spans every column. */
fn spans(child: &Node) -> bool {
    child.style.multicol.span_all
        && !child.removed()
        && !super::flow::out_of_flow(child)
        && child.style.float == Float::None
        && !super::flow::inline_level(child)
}

/** Paints a rule centered in each gap between two used columns, the row's height tall,
inserted at `at` so the columns' content paints over it. */
fn rules(node: &Node, items: &mut Vec<DisplayItem>, shape: &Shape, row: &Placed, at: usize) {
    let Placed { top, height, used } = *row;
    let multicol = &node.style.multicol;
    let width = super::sizing::snap_border(multicol.used_rule_width());
    let Some(line) = multicol.rule_style.filter(|_| width > 0.0) else {
        return;
    };
    let columns = shape.columns;
    let color = multicol.rule_color.clone().or(node.style.color.clone());
    for k in 0..used.saturating_sub(1) {
        let left = shape.content_left + columns.offset(k, shape.content_width);
        let middle = if columns.rtl {
            left - columns.gap / 2.0
        } else {
            left + columns.width + columns.gap / 2.0
        };
        let mut rule =
            super::flow::box_item(node, middle - width / 2.0, top, width, height, Vec::new());
        rule.tag = RULE_TAG.into();
        rule.background = None;
        rule.border = 0.0;
        rule.border_widths = [0.0, 0.0, 0.0, width];
        rule.border_colors = [None, None, None, color.clone()];
        rule.border_color = color.clone();
        rule.border_styles = [line; 4];
        rule.background_clip = [0.0; 4];
        rule.background_origin = None;
        items.insert(at + k, rule);
    }
}
