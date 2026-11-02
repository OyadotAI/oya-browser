/** The box that limits how far a sticky box travels: its containing block (Blink's
`StickyContainer`, which is `ContainingBlock()`), not merely its parent. */
use crate::document::style::TableBox;
use crate::document::{Document, Node};

/** Whether `node` never contains a sticky box's travel: a table row or row group (a sticky cell
or row moves within its whole table, as `position-sticky-table-td-top` shows) or a non-atomic
inline box, which is not a block container. */
fn passes_through(node: &Node) -> bool {
    let style = &node.style;
    ["tr", "thead", "tbody", "tfoot"].contains(&node.tag.as_str())
        || matches!(style.table_box, TableBox::Row | TableBox::RowGroup)
        || (style.inline && !style.inline_block && !style.display_flex && !style.display_grid)
}

/** The identity of the sticky box `node`'s containing block, walking up through rows, row
groups and inline boxes; `stop` (its scroll container) always ends the walk. */
pub(super) fn sticky_container(
    document: &Document,
    positions: &[Option<usize>],
    node: &Node,
    stop: Option<usize>,
) -> Option<usize> {
    let lookup = |local: usize| {
        positions
            .get(local)
            .copied()
            .flatten()
            .map(|at| &document.nodes[at])
    };
    let mut current = node.parent?;
    while Some(current) != stop {
        let ancestor = lookup(current)?;
        match ancestor.parent {
            Some(up) if passes_through(ancestor) => current = up,
            _ => break,
        }
    }
    Some(current)
}
