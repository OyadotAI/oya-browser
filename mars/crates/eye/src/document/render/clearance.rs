/** Whether a clearing block needs clearance (CSS 2.1 9.5.2): its hypothetical border edge is where
 * its top margin, collapsed with its first child's when nothing separates them, would put it. A
 * child's large top margin can clear the floats on its own, and then no clearance is added
 * (margin-collapse-clear-015: a 140px child margin already passes a 100px float). */
use super::flow::*;

/** The top margin that collapses at `node`'s top: its own, joined with its first in-flow block
 * children's when no border, padding, text or new formatting context separates them, as one
 * strut (total, most negative): a child's -1000px margin under its parent's 100px leaves -900px
 * (clear-on-parent-with-margins). */
pub(super) fn adjoining_strut(context: &LayoutContext<'_>, node: &Node) -> (f64, f64) {
    chain(context, node, false)
}
/** The collapsed top margin that decides clearance: `adjoining_strut`'s chain, reaching into a
 * first child that may be empty too (no-clearance-due-to-large-margin-after-left-right's
 * empty 185px-margin child clears the floats by itself). */
pub(super) fn adjoining_top(context: &LayoutContext<'_>, node: &Node) -> f64 {
    chain(context, node, true).0
}
/** The strut of `node`'s top margin and its first children's; `empty` also takes in a first
 * child that may be empty, which ends the chain. */
fn chain(context: &LayoutContext<'_>, node: &Node, empty: bool) -> (f64, f64) {
    let mut strut = join((0.0, 0.0), node.style.margin[0]);
    let mut current = node;
    for _ in 0..crate::constants::DOCUMENT_DEPTH {
        let Some((child, hollow)) = first_child(context, current) else {
            break;
        };
        if hollow && !empty {
            break;
        }
        strut = join(strut, child.style.margin[0]);
        if hollow {
            break;
        }
        current = child;
    }
    strut
}
/** The first in-flow block child whose top margin meets `node`'s, if nothing separates them,
 * and whether it may be empty (and so collapse its margins through itself). */
fn first_child<'a>(context: &LayoutContext<'a>, node: &Node) -> Option<(&'a Node, bool)> {
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
    let empty = child.text.trim().is_empty()
        && context.children(child.local_id).is_empty()
        && child.style.height.is_none();
    (!inline_level(child)).then_some((child, empty))
}
