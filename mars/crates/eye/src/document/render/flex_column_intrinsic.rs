/** Intrinsic widths of multi-line column flex containers: their items break into side-by-side
lines, so the max-content width is the sum of the line widths, not the widest item. */
use super::{
    flex::{self, FlexItem},
    flex_column::{fit_width, item},
    flex_layout::own_text,
    sizing::{LayoutContext, content_size},
};
use crate::{
    document::{FlexDirection, FlexWrap, Node, style::Extent},
    protocol::EngineResult,
};

/** The content-box height a column container's lines break against: its own fixed height, or
failing that its fixed `max-height` (Blink wraps at the maximum when the height is indefinite).
None when neither is a length, so every item sits on one line. */
pub(super) fn break_size(node: &Node) -> Option<f64> {
    fixed(node, node.style.height).or_else(|| fixed(node, node.style.max_height))
}

/** A height property's content-box length when it is one without a containing block. */
fn fixed(node: &Node, extent: Option<Extent>) -> Option<f64> {
    extent
        .and_then(|extent| extent.resolve_size(None))
        .map(|value| content_size(node, value, false))
}

/** Whether the container is a wrapping column, the one case these widths apply to. */
fn column_wrap(node: &Node) -> bool {
    node.style.display_flex
        && node.style.flex_direction == FlexDirection::Column
        && node.style.flex_wrap == FlexWrap::Wrap
}

/** Max-content width of a wrapping column (Blink's `ComputeMinMaxSizeOfMultilineColumnContainer`):
lay the items out with `largest` (the widest max-content contribution) as their available width,
break them into lines at the break size, and add up the lines' widths and the gaps between them.
A balanced column breaks at an unbounded size, its `flex-line-count` making the lines. None for
any other container, or one without a break size, which keeps the widest item. */
pub(super) fn max_content(
    context: &mut LayoutContext<'_>,
    node: &Node,
    children: &[&Node],
    largest: f64,
) -> EngineResult<Option<f64>> {
    let balanced = matches!(flex::Breaking::of(node), flex::Breaking::Balance(_));
    let unbounded = balanced.then_some(crate::constants::LAYOUT_EXTENT);
    let Some(size) = break_size(node).or(unbounded).filter(|_| column_wrap(node)) else {
        return Ok(None);
    };
    let mut items: Vec<&Node> = children
        .iter()
        .copied()
        .filter(|child| !super::flow::blank_text(child) && !own_text(node, child))
        .collect();
    items.sort_by_key(|child| child.style.order);
    let (mut records, mut crosses) = (Vec::<FlexItem>::new(), Vec::new());
    for child in &items {
        let width = fit_width(context, node, child, (largest, largest))?;
        crosses.push(width + child.style.margin[1] + child.style.margin[3]);
        records.push(item(
            context,
            node,
            child,
            width,
            fixed(node, node.style.height),
        )?);
    }
    let lines = flex::break_lines_in(&records, size, node.style.row_gap, flex::Breaking::of(node));
    let widths = lines
        .iter()
        .map(|range| crosses[range.clone()].iter().copied().fold(0.0, f64::max))
        .sum::<f64>();
    let gaps = node.style.column_gap * lines.len().saturating_sub(1) as f64;
    Ok(Some(widths + gaps))
}
