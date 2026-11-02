/**
 * A multicol container's min- and max-content widths, after Blink's
 * ColumnLayoutAlgorithm::ComputeMinMaxSizes: the column content's widths, where a
 * `column-width` caps the minimum and floors the maximum, take every column and gap (the
 * minimum only with an auto `column-width`); spanners are sized apart and only widen the result.
 * ponytail: a box holding a spanner counts as spanner content only, not as column content.
 */
use super::flow::LayoutContext;
use super::multicol::{Columns, spanned};
use super::sizing::{IntrinsicWidths, horizontal_edges, intrinsic_contribution};
use crate::document::Node;
use crate::protocol::EngineResult;

/** The container's widths from `content`, its widths as a plain block. */
pub(super) fn intrinsic(
    context: &mut LayoutContext<'_>,
    node: &Node,
    content: IntrinsicWidths,
) -> EngineResult<IntrinsicWidths> {
    let spanners = spanners(context, node);
    if spanners.is_empty() {
        return Ok(multiplied(node, content));
    }
    let mut columns = IntrinsicWidths::default();
    for child in context.children(node.local_id) {
        if !spanners.iter().any(|s| inside(context, s, child.local_id)) {
            encompass(&mut columns, contribution(context, node, child)?);
        }
    }
    let mut size = multiplied(node, columns);
    for spanner in spanners {
        let parent = spanner
            .parent
            .and_then(|p| context.node(p).ok())
            .unwrap_or(node);
        encompass(&mut size, contribution(context, parent, spanner)?);
    }
    Ok(size)
}

/** Column widths widened to the container's: every column and the gaps between them. */
fn multiplied(node: &Node, content: IntrinsicWidths) -> IntrinsicWidths {
    let multicol = &node.style.multicol;
    let mut size = content;
    if let Some(width) = multicol.width {
        size.min_content = size.min_content.min(width);
        size.max_content = size.max_content.max(width).max(size.min_content);
    }
    let count = f64::from(multicol.count.unwrap_or(1));
    let gap = Columns::used(node, 0.0).gap * (count - 1.0);
    if multicol.width.is_none() {
        size.min_content = size.min_content * count + gap;
    }
    size.max_content = size.max_content * count + gap;
    size
}

/** A child's margin-box widths in its parent. */
fn contribution(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    child: &Node,
) -> EngineResult<IntrinsicWidths> {
    let raw = context.intrinsic_widths(child.local_id)?;
    let value = intrinsic_contribution(parent, child, raw);
    let extra = horizontal_edges(child) + child.style.margin[1] + child.style.margin[3];
    Ok(IntrinsicWidths {
        min_content: value.min_content + extra,
        max_content: value.max_content + extra,
    })
}

/** Widens `size` to hold `other`. */
fn encompass(size: &mut IntrinsicWidths, other: IntrinsicWidths) {
    size.min_content = size.min_content.max(other.min_content);
    size.max_content = size.max_content.max(other.max_content);
}

/** The descendants spanning this container. */
fn spanners<'a>(context: &LayoutContext<'a>, node: &Node) -> Vec<&'a Node> {
    let mut found = Vec::new();
    let mut stack = context.children(node.local_id);
    while let Some(child) = stack.pop() {
        if spanned(context, child) == Some(node.local_id) {
            found.push(child);
        } else if !super::flow::new_context(child) {
            stack.extend(context.children(child.local_id));
        }
    }
    found
}

/** Whether `spanner` is `local` or sits inside it. */
fn inside(context: &LayoutContext<'_>, spanner: &Node, local: usize) -> bool {
    let mut current = Some(spanner);
    while let Some(node) = current {
        if node.local_id == local {
            return true;
        }
        current = node.parent.and_then(|parent| context.node(parent).ok());
    }
    false
}
