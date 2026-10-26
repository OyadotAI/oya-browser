/** An element the table lays out as a cell although it is not one: CSS wraps it in an anonymous
 * cell, which Eye leaves out (table/anonymous.rs keeps a lone element as the cell itself). Its
 * margins sit inside that cell, so the cell is that much larger than the element's box
 * (flexbox_flex-formatting-interop-ref's 2em margins inset the text). */
use super::super::flow::*;

/** The element's margins `[top, right, bottom, left]` when it stands in for a cell. */
fn margins(cell: &Node) -> Option<[f64; 4]> {
    (!super::is_cell_box(cell)).then_some(cell.style.margin)
}
/** The block size its vertical margins add to the cell: both, since the cell is a formatting
 * context they cannot escape, or only the larger when the element is empty and they collapse
 * through it (anonymous-table-cell-margin-collapsing).
 *
 * ponytail: a child's margins that collapse through the element are not added. */
fn vertical(top: f64, bottom: f64, inner: f64) -> f64 {
    if inner > 0.0 {
        top + bottom
    } else {
        top.max(bottom)
    }
}
/** The cell's height at `width`: the element's at the width its margins leave, plus them. */
pub(super) fn measure(
    context: &mut LayoutContext<'_>,
    cell: &Node,
    width: f64,
) -> EngineResult<f64> {
    let Some([top, right, bottom, left]) = margins(cell) else {
        return context.measure_height(cell.local_id, width);
    };
    let inner = (width - left - right).max(0.0);
    let height = context.measure_height(cell.local_id, inner)?;
    Ok(height + vertical(top, bottom, height))
}
/** The element's box inside the cell's `bounds`: inset by its margins. */
pub(super) fn inset(cell: &Node, bounds: BorderBox) -> BorderBox {
    let Some([top, right, bottom, left]) = margins(cell) else {
        return bounds;
    };
    BorderBox {
        x: bounds.x + left,
        y: bounds.y + top,
        width: (bounds.width - left - right).max(0.0),
        height: bounds.height.map(|height| (height - top - bottom).max(0.0)),
    }
}
