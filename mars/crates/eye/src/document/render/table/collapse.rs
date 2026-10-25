/** `border-collapse: collapse`, after Blink's TableBorders (layout/table/table_borders.cc): every
 * grid edge takes the widest border any cell, row, row group or the table puts on it (css-tables-3 border
 * conflict resolution by width), each cell then uses half of the edges around it, and the table
 * half of its outer edges. Resolved once before layout by rewriting those widths into a copy of
 * the document, so sizing and painting both see them.
 *
 * ponytail: width wins and nothing else (no style ranking, `hidden` or column borders); the winning edge paints in each cell's own color. */
use super::super::flow::*;
use super::{Grid, grid, is_table};

/** Widest border on each horizontal and vertical edge of one table's grid. */
struct Edges {
    columns: usize,
    /** `(rows + 1) * columns` edges above each row and below the last. */
    across: Vec<f64>,
    /** `rows * (columns + 1)` edges left of each column and right of the last. */
    down: Vec<f64>,
}
impl Edges {
    /** Empty edges for a grid of `rows` by `columns`. */
    fn new(rows: usize, columns: usize) -> Self {
        Self {
            columns,
            across: vec![0.0; (rows + 1) * columns],
            down: vec![0.0; rows * (columns + 1)],
        }
    }
    /** Merges a box's border widths `[top, right, bottom, left]` over rows `r0..r1`, columns `c0..c1`. */
    fn merge(&mut self, (r0, r1): (usize, usize), (c0, c1): (usize, usize), sides: [f64; 4]) {
        let columns = self.columns;
        for column in c0..c1 {
            raise(&mut self.across[r0 * columns + column], sides[0]);
            raise(&mut self.across[r1 * columns + column], sides[2]);
        }
        for row in r0..r1 {
            raise(&mut self.down[row * (columns + 1) + c0], sides[3]);
            raise(&mut self.down[row * (columns + 1) + c1], sides[1]);
        }
    }
    /** Blink's GetCellBorders: the widest edge on each side of the area, halved. */
    fn half(&self, (r0, r1): (usize, usize), (c0, c1): (usize, usize)) -> [f64; 4] {
        let columns = self.columns;
        let widest = |edges: &mut dyn Iterator<Item = f64>| edges.fold(0.0_f64, f64::max) / 2.0;
        [
            widest(&mut (c0..c1).map(|c| self.across[r0 * columns + c])),
            widest(&mut (r0..r1).map(|r| self.down[r * (columns + 1) + c1])),
            widest(&mut (c0..c1).map(|c| self.across[r1 * columns + c])),
            widest(&mut (r0..r1).map(|r| self.down[r * (columns + 1) + c0])),
        ]
    }
}
/** Keeps the wider of two border widths. */
fn raise(edge: &mut f64, width: f64) {
    *edge = edge.max(width);
}

/** The document with every collapsed table's cell and table borders resolved, or None when no
 * table collapses its borders. */
pub(in super::super) fn resolve(document: &Document) -> EngineResult<Option<Document>> {
    if !document.nodes.iter().any(|node| node.style.border_collapse) {
        return Ok(None);
    }
    let mut context = LayoutContext::new(document)?;
    let mut changes = Vec::new();
    for node in &document.nodes {
        if node.style.border_collapse && !node.hidden() && is_table(&context, node) {
            let table = grid(&mut context, node)?;
            changes.extend(table_changes(node, &table));
        }
    }
    let mut resolved = document.clone();
    let positions = super::super::sizing::positions(document);
    for (local, sides, table) in changes {
        let Some(node) = positions
            .get(local)
            .copied()
            .flatten()
            .and_then(|at| resolved.nodes.get_mut(at))
        else {
            continue;
        };
        apply(&mut node.style, sides, table);
    }
    Ok(Some(resolved))
}
/** Each cell's and the table's resolved half borders, as (identity, sides, is the table). */
fn table_changes(table: &Node, grid: &Grid<'_>) -> Vec<(usize, [f64; 4], bool)> {
    let (rows, columns) = (grid.rows.len(), grid.columns);
    if rows == 0 {
        return Vec::new();
    }
    let mut edges = Edges::new(rows, columns);
    edges.merge((0, rows), (0, columns), sides(table));
    // Blink's collapsed borders take the widest of table, row group, row and cell borders.
    for section in &grid.sections {
        if let Some(group) = section.node.filter(|_| !section.rows.is_empty()) {
            edges.merge(
                (section.rows.start, section.rows.end),
                (0, columns),
                sides(group),
            );
        }
    }
    for (index, row) in grid
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| !row.anonymous)
    {
        edges.merge((index, index + 1), (0, columns), sides(row.node));
    }
    for slot in &grid.cells {
        let area = (slot.row, slot.row + slot.rows);
        edges.merge(
            area,
            (slot.column, slot.column + slot.columns),
            sides(slot.cell),
        );
    }
    let mut out: Vec<_> = grid
        .cells
        .iter()
        .map(|slot| {
            let area = (slot.row, slot.row + slot.rows);
            let span = (slot.column, slot.column + slot.columns);
            (slot.cell.local_id, edges.half(area, span), false)
        })
        .collect();
    out.push((table.local_id, edges.half((0, rows), (0, columns)), true));
    out
}
/** A box's border widths `[top, right, bottom, left]`. */
fn sides(node: &Node) -> [f64; 4] {
    super::super::sizing::border_sides(&node.style)
}
/** Writes resolved half borders into a style. A collapsed table has no padding; where the
 * table's own border lost to a cell's, its half is kept as padding so it is not painted in the
 * table's color. */
fn apply(style: &mut crate::document::ComputedStyle, half: [f64; 4], table: bool) {
    let own = super::super::sizing::border_sides(style);
    if style.border > 0.0 {
        // Per-side widths paint with per-side colors, so carry the shorthand's color over.
        style.border_colors = std::array::from_fn(|_| style.border_color.clone());
    }
    style.border = 0.0;
    if !table {
        style.border_widths = half;
        return;
    }
    for side in 0..4 {
        let wins = own[side] >= half[side] * 2.0 && own[side] > 0.0;
        style.border_widths[side] = if wins { half[side] } else { 0.0 };
        style.padding[side] = if wins { 0.0 } else { half[side] };
    }
}
