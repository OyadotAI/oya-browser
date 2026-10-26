/** HTML's percentage height quirk in layout (Blink's
 * LayoutBox::SkipContainingBlockForPercentHeightCalculation): in quirks mode an auto-height block
 * is skipped when its children resolve percentage heights, so they resolve against the nearest
 * ancestor whose height is known. */
use super::LayoutContext;
use crate::document::Node;

/**
 * The definite content height `node`'s children resolve percentages against when `node` itself
 * has an auto height: its own containing block's `definite`, passed through in quirks mode,
 * unless `node` is a table cell or a cell's child, out of flow, a flex or a grid container. None
 * in standards mode.
 */
pub(in crate::document::render) fn quirk_definite(
    context: &LayoutContext<'_>,
    node: &Node,
    definite: Option<f64>,
) -> Option<f64> {
    let style = &node.style;
    let skipped = context.document.formatting.quirks
        && style.height.is_none()
        && !style.display_flex
        && !style.display_grid
        && !super::super::table::is_cell_box(node)
        && !super::super::flow::out_of_flow(node)
        && !in_cell(context, node);
    definite.filter(|_| skipped)
}

/** Blink resolves a percentage against a table cell only for the cell's own children: one that
 * skipped an auto-height block to reach the cell has an indefinite basis. */
fn in_cell(context: &LayoutContext<'_>, node: &Node) -> bool {
    node.parent
        .and_then(|parent| context.node(parent).ok())
        .is_some_and(super::super::table::is_cell_box)
}
