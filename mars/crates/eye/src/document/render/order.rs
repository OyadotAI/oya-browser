/** Order-modified document order (css-display-3 4.1): flex and grid items paint in the order
 * their `order` gives them, ties in document order, as Blink's `PaintAllPhasesAtomically`
 * children of a flex or grid box do. */
use crate::document::{Document, Position};

/** Each identity's paint position: its document position, except that the items of a flex or
 * grid container with any non-zero `order` trade positions among themselves, sorted by
 * `order`; absolutely positioned children are no items and keep theirs. Only siblings swap, so every box keeps its place against the rest of the tree.
 * `positions` maps identities to document positions. */
pub(super) fn modified(document: &Document, positions: &[Option<usize>]) -> Vec<Option<usize>> {
    let mut out = positions.to_vec();
    let mut groups = std::collections::BTreeMap::<usize, Vec<usize>>::new();
    for (position, node) in document.nodes.iter().enumerate() {
        let Some(parent) = node.parent else { continue };
        let style = &document.nodes[positions[parent].unwrap_or(position)].style;
        let out_of_flow = matches!(node.style.position, Position::Absolute | Position::Fixed);
        if (style.display_flex || style.display_grid) && !out_of_flow {
            groups.entry(parent).or_default().push(position);
        }
    }
    for members in groups.into_values() {
        let order = |position: &usize| document.nodes[*position].style.order;
        if members.iter().all(|position| order(position) == 0) {
            continue;
        }
        let mut sorted = members.clone();
        sorted.sort_by_key(|position| (order(position), *position));
        for (slot, position) in members.iter().zip(sorted) {
            out[document.nodes[position].local_id] = Some(*slot);
        }
    }
    out
}
