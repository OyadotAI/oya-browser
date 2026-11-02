/** The table wrapper box and the table grid box (CSS 2.1 17.4): captions sit in the wrapper,
 * outside the table box, so the table's background and borders paint around the grid only. */
use super::super::flow::*;

/**
 * Splits a captioned table's one item into the wrapper (its full extent, which floats and
 * lookups read, painting nothing) and, right after it, the grid box at `top` and `height`
 * carrying the table's background, borders and shadows. Without this a bottom caption sat on
 * the table's blue background (caption-side-applies-to-006).
 */
pub(in super::super) fn split_grid_box(
    items: &mut Vec<DisplayItem>,
    index: usize,
    top: f64,
    height: f64,
) {
    let Some(wrapper) = items.get_mut(index) else {
        return;
    };
    let mut grid = wrapper.clone();
    grid.y = top;
    grid.height = height;
    grid.lines = Vec::new();
    super::parts::strip(wrapper);
    items.insert(index + 1, grid);
}
