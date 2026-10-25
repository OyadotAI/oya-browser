/** `border-collapse: collapse`, after Blink's TableBorders (layout/table/table_borders.cc): every
 * grid edge takes the widest border any cell, row, row group or the table puts on it (css-tables-3 border
 * conflict resolution by width), each cell then uses half of the edges around it, and the table
 * half of its outer edges. Resolved once before layout by rewriting those widths into a copy of
 * the document, so sizing and painting both see them.
 *
 * ponytail: no `hidden` and no column borders. */
use super::super::flow::*;
use super::{Grid, grid, is_anonymous, is_table};
use crate::document::BorderStyle;

/** One grid edge's winning border: width, color (None is the box's text color) and style. */
#[derive(Clone, Default)]
struct Edge {
    width: f64,
    color: Option<String>,
    style: BorderStyle,
    /** Which kind of box set it: table 0, row group 1, row 2, cell 3. */
    rank: u8,
}
impl Edge {
    /** css-tables-3 conflict resolution (Blink's IsSourceOnSideBetter): the wider border wins,
     * then the stronger style, then the box nearer the cell; a full tie keeps the first, the
     * cell above or to the left. */
    fn beats(&self, other: &Edge) -> bool {
        let strength = |style: BorderStyle| match style {
            BorderStyle::Double => 8,
            BorderStyle::Solid => 7,
            BorderStyle::Dashed => 6,
            BorderStyle::Dotted => 5,
            BorderStyle::Ridge => 4,
            BorderStyle::Outset => 3,
            BorderStyle::Groove => 2,
            BorderStyle::Inset => 1,
        };
        if self.width != other.width {
            return self.width > other.width;
        }
        if self.style != other.style {
            return strength(self.style) > strength(other.style);
        }
        self.rank > other.rank
    }
}
/** The winning border on each horizontal and vertical edge of one table's grid. */
struct Edges {
    columns: usize,
    /** `(rows + 1) * columns` edges above each row and below the last. */
    across: Vec<Edge>,
    /** `rows * (columns + 1)` edges left of each column and right of the last. */
    down: Vec<Edge>,
}
impl Edges {
    /** Empty edges for a grid of `rows` by `columns`. */
    fn new(rows: usize, columns: usize) -> Self {
        Self {
            columns,
            across: vec![Edge::default(); (rows + 1) * columns],
            down: vec![Edge::default(); rows * (columns + 1)],
        }
    }
    /** Merges a box's borders `[top, right, bottom, left]` over rows `r0..r1`, columns `c0..c1`. */
    fn merge(&mut self, (r0, r1): (usize, usize), (c0, c1): (usize, usize), sides: [Edge; 4]) {
        let columns = self.columns;
        for column in c0..c1 {
            raise(&mut self.across[r0 * columns + column], &sides[0]);
            raise(&mut self.across[r1 * columns + column], &sides[2]);
        }
        for row in r0..r1 {
            raise(&mut self.down[row * (columns + 1) + c0], &sides[3]);
            raise(&mut self.down[row * (columns + 1) + c1], &sides[1]);
        }
    }
    /** Blink's GetCellBorders: the winning edge on each side of the area, at half its width. */
    fn half(&self, (r0, r1): (usize, usize), (c0, c1): (usize, usize)) -> [Edge; 4] {
        let columns = self.columns;
        let best = |edges: &mut dyn Iterator<Item = &Edge>| {
            let mut edge = edges.fold(Edge::default(), |best, edge| {
                if edge.beats(&best) {
                    edge.clone()
                } else {
                    best
                }
            });
            edge.width /= 2.0;
            edge
        };
        [
            best(&mut (c0..c1).map(|c| &self.across[r0 * columns + c])),
            best(&mut (r0..r1).map(|r| &self.down[r * (columns + 1) + c1])),
            best(&mut (c0..c1).map(|c| &self.across[r1 * columns + c])),
            best(&mut (r0..r1).map(|r| &self.down[r * (columns + 1) + c0])),
        ]
    }
}
/** Keeps the winner of two borders on one edge. */
fn raise(edge: &mut Edge, side: &Edge) {
    if side.width > 0.0 && side.beats(edge) {
        *edge = side.clone();
    }
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
fn table_changes(table: &Node, grid: &Grid<'_>) -> Vec<(usize, [Edge; 4], bool)> {
    let (rows, columns) = (grid.rows.len(), grid.columns);
    if rows == 0 {
        return Vec::new();
    }
    let mut edges = Edges::new(rows, columns);
    // An anonymous table has no border of its own; its wrapping block keeps its border.
    let anonymous = is_anonymous(table);
    if !anonymous {
        edges.merge((0, rows), (0, columns), sides(table, 0));
    }
    // Blink's collapsed borders take the widest of table, row group, row and cell borders.
    for section in &grid.sections {
        if let Some(group) = section.node.filter(|_| !section.rows.is_empty()) {
            edges.merge(
                (section.rows.start, section.rows.end),
                (0, columns),
                sides(group, 1),
            );
        }
    }
    for (index, row) in grid
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| !row.anonymous)
    {
        edges.merge((index, index + 1), (0, columns), sides(row.node, 2));
    }
    for slot in &grid.cells {
        let area = (slot.row, slot.row + slot.rows);
        edges.merge(
            area,
            (slot.column, slot.column + slot.columns),
            sides(slot.cell, 3),
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
    if !anonymous {
        out.push((table.local_id, edges.half((0, rows), (0, columns)), true));
    }
    out
}
/** A box's borders `[top, right, bottom, left]`, a color it leaves unset being its text color. */
fn sides(node: &Node, rank: u8) -> [Edge; 4] {
    let style = &node.style;
    let widths = super::super::sizing::border_sides(style);
    std::array::from_fn(|side| Edge {
        width: widths[side],
        color: style.border_colors[side]
            .clone()
            .or_else(|| style.border_color.clone())
            .or_else(|| style.color.clone()),
        style: style.border_styles[side],
        rank,
    })
}
/** Writes resolved half borders into a style. A collapsed table has no padding; where the
 * table's own border lost to a cell's, its half is kept as padding so it is not painted in the
 * table's color. */
fn apply(style: &mut crate::document::ComputedStyle, half: [Edge; 4], table: bool) {
    let own = super::super::sizing::border_sides(style);
    if style.border > 0.0 {
        // Per-side widths paint with per-side colors, so carry the shorthand's color over.
        style.border_colors = std::array::from_fn(|_| style.border_color.clone());
    }
    style.border = 0.0;
    if !table {
        // A cell paints each edge in the winner's color and style (Blink's collapsed borders).
        for (side, edge) in half.into_iter().enumerate() {
            style.border_widths[side] = edge.width;
            if edge.width > 0.0 {
                style.border_style_none = false;
                style.border_colors[side] = edge.color;
                style.border_styles[side] = edge.style;
            }
        }
        return;
    }
    for (side, edge) in half.iter().enumerate() {
        let wins = own[side] >= edge.width * 2.0 && own[side] > 0.0;
        style.border_widths[side] = if wins { edge.width } else { 0.0 };
        style.padding[side] = if wins { 0.0 } else { edge.width };
    }
}
