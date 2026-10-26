/**
 * `text-indent` in a block container's intrinsic widths, after Blink's ComputeMinMaxSizes: the
 * indent is room the first line takes before its content, so the max-content of that line and
 * the min-content of its first unbreakable piece both grow by it. A percentage indent resolves
 * against zero here, as the container's width is what is being found. Without it a float with
 * `text-indent: 3em` shrank to its text and the indented text overflowed it.
 */
use super::flow::*;
use super::sizing::IntrinsicWidths;

/** The fixed part of `node`'s first-line indent when it is a block container with a line to
 * indent: its own text or in-flow children (an empty inline-block has none). */
pub(super) fn indent(node: &Node, children: &[&Node]) -> f64 {
    let block = !node.style.inline || node.style.inline_block;
    let content = !node.text.trim().is_empty() || !children.is_empty();
    if block && content {
        node.style.text_indent.0
    } else {
        0.0
    }
}

/** `size` with min-content measured with the indent: the widest of `first` (the first piece on
 * the first line, the container's own text or its first child) widened by the indent, which may
 * be negative, and `rest`, the widest later piece. The max-content side is already in the line
 * `longest_line` measured; neither goes below zero. */
pub(super) fn first_line(
    node: &Node,
    children: &[&Node],
    (first, rest): (Option<f64>, f64),
    mut size: IntrinsicWidths,
) -> IntrinsicWidths {
    let indent = indent(node, children);
    if indent != 0.0 {
        size.min_content = (indent + first.unwrap_or(0.0)).max(rest).max(0.0);
        size.max_content = size.max_content.max(size.min_content);
    }
    size
}
