/** Whether a clearing block needs clearance (CSS 2.1 9.5.2): its hypothetical border edge is where
 * its top margin, collapsed with its first child's when nothing separates them, would put it. A
 * child's large top margin can clear the floats on its own, and then no clearance is added
 * (margin-collapse-clear-015: a 140px child margin already passes a 100px float). */
use super::flow::*;

/** The top margin that collapses at `node`'s top: its own, joined with its first in-flow block
 * child's when no border, padding, text or new formatting context separates them.
 *
 * ponytail: positive margins only (the largest wins); negative ones keep the box's own. */
pub(super) fn adjoining_top(context: &LayoutContext<'_>, node: &Node) -> f64 {
    let mut margin = node.style.margin[0];
    let mut current = node;
    for _ in 0..crate::constants::DOCUMENT_DEPTH {
        let Some(child) = first_child(context, current) else {
            break;
        };
        margin = margin.max(child.style.margin[0]);
        current = child;
    }
    margin
}
/** The first in-flow block child whose top margin meets `node`'s, if nothing separates them. */
fn first_child<'a>(context: &LayoutContext<'a>, node: &Node) -> Option<&'a Node> {
    let style = &node.style;
    let sides = super::sizing::border_sides(style);
    if style.padding[0] != 0.0 || sides[0] != 0.0 || new_context(node) {
        return None;
    }
    if !node.text.trim().is_empty() {
        return None;
    }
    let child = context.children(node.local_id).into_iter().find(|child| {
        !out_of_flow(child) && child.style.float == crate::document::style::Float::None
    })?;
    (!inline_level(child)).then_some(child)
}
