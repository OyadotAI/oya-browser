/** Table part backgrounds (CSS 2.1 17.5.1, Blink's TableCellPainter::PaintBackgroundForTablePart):
 * a column group, column, row group or row paints its background only under its cells, never in
 * the border spacing between them, positioned against the part's own box (so an image tiles
 * across the whole column or row) and clipped to each cell. */
use super::super::flow::*;
use super::{Grid, Slot};

/** Where the grid's columns and rows sit once placed. */
pub(super) struct Geometry<'g> {
    /** Each column's left edge. */
    pub lefts: &'g [f64],
    /** Each column's width. */
    pub widths: &'g [f64],
    /** Each row's top edge. */
    pub tops: &'g [f64],
    /** Each row's height. */
    pub heights: &'g [f64],
    /** The left edge and width row and row group boxes span. */
    pub span: (f64, f64),
}

/** Makes an item a bare extent: it keeps its geometry for lookups and paints nothing. */
pub(super) fn strip(item: &mut DisplayItem) {
    item.bare = true;
    item.background = None;
    item.backgrounds = Vec::new();
    item.shadows = Vec::new();
    item.border = 0.0;
    item.border_widths = [0.0; 4];
}

/** Whether a part has a background to paint under its cells. */
fn paints(node: &Node) -> bool {
    node.style
        .background
        .as_deref()
        .is_some_and(|color| color != "transparent")
        || !node.style.backgrounds.is_empty()
}

/** The column elements for each grid column: its `<colgroup>` (when it holds `<col>`s) and its
 * own element (the `<col>`, or a `<colgroup>` without columns), in grid order. */
pub(super) fn column_nodes<'a>(
    context: &LayoutContext<'a>,
    table: &Node,
) -> Vec<(Option<&'a Node>, &'a Node)> {
    let mut out = Vec::new();
    for child in context.children(table.local_id) {
        if !super::is_column(child) {
            continue;
        }
        let cols: Vec<_> = context.children(child.local_id);
        let cols: Vec<_> = cols
            .into_iter()
            .filter(|kid| super::is_column(kid))
            .collect();
        if cols.is_empty() || child.tag == "col" {
            out.extend(std::iter::repeat_n(
                (None, child),
                super::column_span(child),
            ));
        } else {
            for col in cols {
                out.extend(std::iter::repeat_n(
                    (Some(child), col),
                    super::column_span(col),
                ));
            }
        }
        if out.len() > super::GRID_COLUMNS {
            break;
        }
    }
    out
}

/** A part's whole box, painted under one cell: a background item clipped to `cell`. */
fn under(node: &Node, rect: [f64; 4], cell: [f64; 4]) -> DisplayItem {
    let mut item = box_item(node, rect[0], rect[1], rect[2], rect[3], Vec::new());
    item.border = 0.0;
    item.border_widths = [0.0; 4];
    item.clip = Some(crate::document::render::ClipRect {
        x: cell[0],
        y: cell[1],
        width: cell[2],
        height: cell[3],
        radius: [0.0; 4],
    });
    item
}

/** The rows' extent from `first` to `last` inclusive, as (top, height). */
fn rows_extent(geometry: &Geometry<'_>, first: usize, last: usize) -> (f64, f64) {
    let top = geometry.tops[first];
    (top, geometry.tops[last] + geometry.heights[last] - top)
}

/** Pushes, under one cell at `cell` (x, y, width, height), the backgrounds of its column group,
 * column, row group and row, bottom first. */
pub(super) fn paint_under(
    grid: &Grid<'_>,
    geometry: &Geometry<'_>,
    slot: &Slot<'_>,
    cell: [f64; 4],
    items: &mut Vec<DisplayItem>,
) {
    if geometry.tops.is_empty() {
        return;
    }
    columns_under(grid, geometry, slot.column, cell, items);
    rows_under(grid, geometry, slot.row, cell, items);
}

/** The column group's and column's backgrounds under one cell of `column`. */
fn columns_under(
    grid: &Grid<'_>,
    geometry: &Geometry<'_>,
    column: usize,
    cell: [f64; 4],
    items: &mut Vec<DisplayItem>,
) {
    let Some((group, own)) = grid.column_boxes.get(column) else {
        return;
    };
    let (top, height) = rows_extent(geometry, 0, geometry.tops.len() - 1);
    if let Some(group) = group.filter(|group| paints(group)) {
        let same = |(_, (g, _)): &(usize, &(Option<&Node>, &Node))| {
            g.is_some_and(|g| std::ptr::eq(g, group))
        };
        let mut members = grid.column_boxes.iter().enumerate().filter(same);
        let first = members.next().map_or(column, |(index, _)| index);
        let last = members.next_back().map_or(first, |(index, _)| index);
        let left = geometry.lefts[first].min(geometry.lefts[last]);
        let right = (geometry.lefts[first] + geometry.widths[first])
            .max(geometry.lefts[last] + geometry.widths[last]);
        items.push(under(group, [left, top, right - left, height], cell));
    }
    if paints(own) {
        let (left, width) = (geometry.lefts[column], geometry.widths[column]);
        items.push(under(own, [left, top, width, height], cell));
    }
}

/** The row group's and row's backgrounds under one cell starting in `row`. */
fn rows_under(
    grid: &Grid<'_>,
    geometry: &Geometry<'_>,
    row: usize,
    cell: [f64; 4],
    items: &mut Vec<DisplayItem>,
) {
    let (left, width) = geometry.span;
    let section = grid.sections.iter().find(|s| s.rows.contains(&row));
    if let Some(section) = section
        && let Some(node) = section.node.filter(|node| paints(node))
    {
        let (top, height) = rows_extent(geometry, section.rows.start, section.rows.end - 1);
        items.push(under(node, [left, top, width, height], cell));
    }
    let part = grid.rows[row];
    if !part.anonymous && paints(part.node) {
        let (top, height) = rows_extent(geometry, row, row);
        items.push(under(part.node, [left, top, width, height], cell));
    }
}
