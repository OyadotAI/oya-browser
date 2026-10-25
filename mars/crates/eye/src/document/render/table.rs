/** Automatic table layout: a span-aware grid, min/max column widths, border spacing and row heights. */
use super::flow::*;
use super::sizing::IntrinsicWidths;
use crate::document::style::{TableBox, VerticalAlign};

mod collapse;
mod columns;
pub(super) use collapse::resolve as collapse_borders;

/** Columns one table lays out; cells past it are skipped rather than widen the grid without bound. */
const GRID_COLUMNS: usize = 128;
/** Rows one table lays out, matching the parser's table bound. */
const GRID_ROWS: usize = crate::constants::TABLE_ROWS * 4;

/** One cell in the grid with its span. */
struct Slot<'a> {
    cell: &'a Node,
    row: usize,
    column: usize,
    rows: usize,
    columns: usize,
}
/** Rows in source order and their placed cells. */
struct Grid<'a> {
    rows: Vec<Row<'a>>,
    cells: Vec<Slot<'a>>,
    columns: usize,
    /** Row groups in grid order, each over its range of rows. */
    sections: Vec<Section<'a>>,
    /** Constraints the `<col>` and `<colgroup>` elements set, one per column they span. */
    elements: Vec<columns::Column>,
}
/** Whether a box lays out as a table: a `<table>`, or a `display: table` box with a
 * `table-row` or `table-cell` child, or with a caption beside one other child (MediaWiki's
 * thumbnails, whose width is the image's and not the caption's). One without is laid out as a
 * block, because CSS wraps its other children in a single anonymous cell, which is what a block
 * gives (the common `content: ""; display: table` clearfix among them). */
pub(super) fn is_table(context: &LayoutContext<'_>, node: &Node) -> bool {
    if node.tag == "table" {
        return true;
    }
    if node.style.table_box != TableBox::Table {
        return false;
    }
    let children = context.children(node.local_id);
    let parts = |kinds: &[TableBox]| {
        children
            .iter()
            .filter(|child| kinds.contains(&child.style.table_box))
            .count()
    };
    let others = children
        .iter()
        .filter(|child| !is_caption(child) && !super::flow::out_of_flow(child))
        .count();
    parts(&[
        TableBox::Row,
        TableBox::Cell,
        TableBox::RowGroup,
        TableBox::HeaderGroup,
        TableBox::FooterGroup,
    ]) > 0
        || (parts(&[TableBox::Caption]) > 0 && others == 1)
}
/** A caption: `<caption>` or a CSS `table-caption`. */
fn is_caption(node: &Node) -> bool {
    node.tag == "caption" || node.style.table_box == TableBox::Caption
}
/** A row: `<tr>`, or a CSS `table-row`. */
fn is_row(node: &Node) -> bool {
    node.tag == "tr" || node.style.table_box == TableBox::Row
}
/** A box that lays out as a table cell: `<td>`, `<th>` or `display: table-cell`. */
pub(super) fn is_cell_box(node: &Node) -> bool {
    ["td", "th"].contains(&node.tag.as_str()) || node.style.table_box == TableBox::Cell
}
/** A cell of this row: `<td>` and `<th>`, and any in-flow child of a CSS row, since CSS wraps
 * a stray child of a row in an anonymous cell rather than dropping it. */
fn is_cell(row: &Node, cell: &Node) -> bool {
    if is_caption(cell) {
        return false;
    }
    ["td", "th"].contains(&cell.tag.as_str())
        || cell.style.table_box == TableBox::Cell
        || (!["table", "thead", "tbody", "tfoot", "tr"].contains(&row.tag.as_str())
            && !super::flow::out_of_flow(cell)
            && !is_row(cell)
            && group_kind(cell).is_none()
            && !is_column(cell))
}
/** A table: caption above, then the grid inside the table's own border and padding. */
pub(super) fn table(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
    forced: Option<BorderBox>,
) -> EngineResult<()> {
    flow.newline();
    let style = &node.style;
    let grid = grid(context, node)?;
    let spacing = spacing(node);
    let fixed_layout = fixed_layout(node);
    let columns = column_constraints(context, &grid, spacing, fixed_layout)?;
    let edges = horizontal_edges(node) + spacing * (grid.columns + 1) as f64;
    let (minimum, maximum) = columns::grid_min_max(&columns, fixed_layout);
    let (minimum, maximum) = (minimum + edges, maximum + edges);
    let room = (flow.width - style.margin[1] - style.margin[3]).max(0.0);
    let width = match forced {
        Some(bounds) => bounds.width,
        // A table's authored width is its border box, as in Chromium's table layout.
        None if style.width.is_some() => {
            let mut sized = node.clone();
            sized.style.box_sizing = crate::document::BoxSizing::BorderBox;
            border_width(&sized, flow.width, room, None).max(minimum)
        }
        None => maximum.min(room).max(minimum),
    };
    let (x, y) = match forced {
        Some(bounds) => (bounds.x, bounds.y),
        None => {
            let centered = centered(context, node);
            let mut probe = style.clone();
            probe.margin_auto[1] |= centered;
            probe.margin_auto[3] |= centered;
            (
                flow.x + super::body::margin_start(&probe, flow.width, width),
                flow.block_top(style.margin[0]),
            )
        }
    };
    let index = items.len();
    items.push(box_item(node, x, y, width, 0.0, Vec::new()));
    let sides = super::sizing::border_sides(style);
    let mut inner = Flow::new(
        x + sides[3] + style.padding[3],
        y + sides[0] + style.padding[0],
        (width - horizontal_edges(node)).max(0.0),
    );
    let (bottom_captions, top_captions): (Vec<_>, Vec<_>) = context
        .all_children(node.local_id)
        .into_iter()
        .filter(|child| is_caption(child))
        .partition(|caption| caption.style.caption_bottom);
    for caption in top_captions {
        node_layout(context, caption, &mut inner, items, None)?;
    }
    inner.newline();
    let widths = columns::widths(
        (inner.width - spacing * (grid.columns + 1) as f64).max(0.0),
        &columns,
        fixed_layout,
    );
    let mut bottom = place(context, &grid, &widths, (inner.x, inner.y, spacing), items)?;
    if !bottom_captions.is_empty() {
        let mut below = Flow::new(inner.x, bottom, inner.width);
        for caption in bottom_captions {
            node_layout(context, caption, &mut below, items, None)?;
        }
        below.newline();
        bottom = below.bottom.max(below.y);
    }
    let natural = bottom - y + style.padding[2] + sides[2];
    let specified = style
        .height
        .and_then(|height| height.resolve_size(flow.definite))
        .map(|height| super::sizing::border_height(node, height));
    let height = forced
        .and_then(|bounds| bounds.height)
        .unwrap_or(natural.max(specified.unwrap_or(0.0)));
    items[index].height = height;
    if forced.is_some() {
        flow.y = y + height;
        flow.collapsible = 0.0;
        flow.bottom = flow.bottom.max(flow.y);
    } else {
        flow.after_block(y + height, style.margin[2]);
    }
    Ok(())
}
/** Old `<center>` and `align=center` parents center a table like Chromium's -webkit-center. */
fn centered(context: &LayoutContext<'_>, node: &Node) -> bool {
    node.parent
        .and_then(|parent| context.node(parent).ok())
        .is_some_and(|parent| {
            parent.tag == "center"
                || parent.style.align == "webkit-center"
                || (parent
                    .attributes
                    .get("align")
                    .is_some_and(|a| a.eq_ignore_ascii_case("center")))
        })
}
/** Cell spacing: none when borders collapse. */
fn spacing(node: &Node) -> f64 {
    if node.style.border_collapse {
        0.0
    } else {
        node.style.border_spacing
    }
}
/** A grid row: a row box, or the anonymous row CSS 2.1 17.2.1 wraps around a run of cells
 * outside one, which borrows its parent's node and paints no box of its own. */
#[derive(Clone, Copy)]
struct Row<'a> {
    node: &'a Node,
    anonymous: bool,
}
/** One row group in grid order: its box (none for an anonymous group) and its grid rows. */
pub(super) struct Section<'a> {
    pub node: Option<&'a Node>,
    pub rows: std::ops::Range<usize>,
}
/** A row group's rows, each with its cells, before they are placed into the grid. */
type Rows<'a> = Vec<(Row<'a>, Vec<&'a Node>)>;
/** The section kind Blink orders by: the first header group goes first, the first footer
 * group last, the rest in source order. */
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Body,
    Header,
    Footer,
}
/** A row group: `<tbody>`, `<thead>`, `<tfoot>` or a CSS `table-*-group`. */
fn group_kind(node: &Node) -> Option<Kind> {
    match (node.tag.as_str(), node.style.table_box) {
        ("thead", _) | (_, TableBox::HeaderGroup) => Some(Kind::Header),
        ("tfoot", _) | (_, TableBox::FooterGroup) => Some(Kind::Footer),
        ("tbody", _) | (_, TableBox::RowGroup) => Some(Kind::Body),
        _ => None,
    }
}
/** A column or column group, which takes no grid slot. */
fn is_column(node: &Node) -> bool {
    ["col", "colgroup"].contains(&node.tag.as_str()) || node.style.table_box == TableBox::Column
}
/** Blink's table box tree (CSS 2.1 17.2.1): a row joins as it is, and each run of cells
 * outside a row shares one anonymous row. */
fn add_row<'a>(
    context: &LayoutContext<'a>,
    rows: &mut Rows<'a>,
    parent: &'a Node,
    child: &'a Node,
) {
    if is_row(child) {
        let cells = context.children(child.local_id);
        let cells = cells
            .into_iter()
            .filter(|cell| is_cell(child, cell))
            .collect();
        rows.push((
            Row {
                node: child,
                anonymous: false,
            },
            cells,
        ));
    } else if is_cell(parent, child) {
        match rows.last_mut() {
            Some((row, cells)) if row.anonymous => cells.push(child),
            _ => rows.push((
                Row {
                    node: parent,
                    anonymous: true,
                },
                vec![child],
            )),
        }
    }
}
/** The table's row groups in grid order; a run of rows or cells directly in the table forms
 * an anonymous row group. */
fn sections<'a>(
    context: &LayoutContext<'a>,
    table: &'a Node,
) -> Vec<(Option<&'a Node>, Kind, Rows<'a>)> {
    let mut groups = Vec::new();
    let mut loose: Rows<'a> = Vec::new();
    for child in context.children(table.local_id) {
        if is_caption(child) || is_column(child) || super::flow::out_of_flow(child) {
            continue;
        }
        let Some(kind) = group_kind(child) else {
            add_row(context, &mut loose, table, child);
            continue;
        };
        if !loose.is_empty() {
            groups.push((None, Kind::Body, std::mem::take(&mut loose)));
        }
        let mut rows = Vec::new();
        for row in context.children(child.local_id) {
            add_row(context, &mut rows, child, row);
        }
        groups.push((Some(child), kind, rows));
    }
    if !loose.is_empty() {
        groups.push((None, Kind::Body, loose));
    }
    if let Some(at) = groups.iter().position(|group| group.1 == Kind::Header) {
        let header = groups.remove(at);
        groups.insert(0, header);
    }
    if let Some(at) = groups.iter().position(|group| group.1 == Kind::Footer) {
        let footer = groups.remove(at);
        groups.push(footer);
    }
    groups
}
/** A column element's span: its `span` attribute, at least one. */
fn column_span(node: &Node) -> usize {
    node.attributes
        .get("span")
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(1)
        .clamp(1, GRID_COLUMNS)
}
/** Blink's CreateColumn from one element's style, with a group's width as the fallback length
 * (auto tables only, as in Blink's VisitCol). */
fn column_element(node: &Node, fallback: Option<f64>) -> columns::Column {
    let style = &node.style;
    let length = style.width.filter(|_| !style.percent_width).or(fallback);
    let percent = style.width.filter(|_| style.percent_width);
    let min = style.min_width.unwrap_or(0.0);
    columns::Column::element(min, length, percent)
}
/** Blink's ColumnConstraintsBuilder: each `<col>` sets the columns it spans; a `<colgroup>`
 * without columns spans its own `span`, and its width falls to columns without one. */
fn column_elements(context: &LayoutContext<'_>, table: &Node, fixed: bool) -> Vec<columns::Column> {
    let mut out = Vec::new();
    for child in context.children(table.local_id) {
        if !is_column(child) {
            continue;
        }
        let kids = context.children(child.local_id);
        let cols: Vec<_> = kids.into_iter().filter(|kid| is_column(kid)).collect();
        if cols.is_empty() || child.tag == "col" {
            let column = column_element(child, None);
            out.extend(std::iter::repeat_n(column, column_span(child)));
            continue;
        }
        let group = column_element(child, None);
        let fallback = (!fixed && group.constrained).then_some(group.max);
        for col in cols {
            let column = column_element(col, fallback);
            out.extend(std::iter::repeat_n(column, column_span(col)));
        }
        if out.len() > GRID_COLUMNS {
            break;
        }
    }
    out
}
/** Rows and cells placed into grid slots, skipping slots taken by earlier row spans. */
fn grid<'a>(context: &mut LayoutContext<'a>, table: &Node) -> EngineResult<Grid<'a>> {
    let table = context.node(table.local_id)?;
    let mut rows = Vec::new();
    let mut sections = Vec::new();
    for (node, _, group) in self::sections(context, table) {
        let start = rows.len().min(GRID_ROWS);
        rows.extend(group);
        rows.truncate(GRID_ROWS);
        sections.push(Section {
            node,
            rows: start..rows.len(),
        });
    }
    let ends = sections
        .iter()
        .flat_map(|section| section.rows.clone().map(|_| section.rows.end))
        .collect::<Vec<_>>();
    let mut taken: Vec<Vec<bool>> = vec![Vec::new(); rows.len()];
    let mut cells = Vec::new();
    let mut columns = 0;
    for (row, (_, row_cells)) in rows.iter().enumerate() {
        context.charge(1)?;
        let mut column = 0;
        for &cell in row_cells {
            while taken[row].get(column).copied().unwrap_or(false) {
                column += 1;
            }
            let span = |name: &str| {
                cell.attributes
                    .get(name)
                    .and_then(|v| v.trim().parse::<usize>().ok())
            };
            // A row span stops at the end of its section; `rowspan=0` reaches it.
            let left = ends[row] - row;
            let spans = (
                span("rowspan").map_or(1, |n| if n == 0 { left } else { n.min(left) }),
                span("colspan").unwrap_or(1).clamp(1, GRID_COLUMNS),
            );
            if column + spans.1 > GRID_COLUMNS {
                break;
            }
            for line in taken.iter_mut().skip(row).take(spans.0) {
                context.charge(spans.1)?;
                if line.len() < column + spans.1 {
                    line.resize(column + spans.1, false);
                }
                line[column..column + spans.1].fill(true);
            }
            cells.push(Slot {
                cell,
                row,
                column,
                rows: spans.0,
                columns: spans.1,
            });
            column += spans.1;
            columns = columns.max(column);
        }
    }
    // Column elements past the cells stay unless they are mergeable (Blink trims those).
    let fixed = fixed_layout(table);
    let mut elements = column_elements(context, table, fixed);
    while elements.len() > columns && elements.last().is_some_and(|c| c.mergeable(fixed)) {
        elements.pop();
    }
    elements.truncate(GRID_COLUMNS);
    Ok(Grid {
        rows: rows.into_iter().map(|(row, _)| row).collect(),
        cells,
        columns: columns.max(elements.len()).max(1),
        sections,
        elements,
    })
}
/** `table-layout: fixed` applies only to a table with an authored width (Blink's
 * IsFixedTableLayout). */
fn fixed_layout(node: &Node) -> bool {
    node.style.table_layout_fixed && node.style.width.is_some()
}
/** Blink's CreateCellInlineConstraint: the border-box min (min-content, raised by min-width)
 * and max (an authored length, else max-content, capped by max-width), its percentage width,
 * and whether a length was authored. Fixed tables skip min-content. */
fn cell_constraint(
    context: &mut LayoutContext<'_>,
    cell: &Node,
    fixed: bool,
) -> EngineResult<columns::Cell> {
    let edges = horizontal_edges(cell);
    let style = &cell.style;
    let length = style.width.filter(|_| !style.percent_width);
    let authored = length.map(|width| super::sizing::content_size(cell, width, true) + edges);
    let content = if fixed && authored.is_some() {
        IntrinsicWidths::default()
    } else {
        context.intrinsic_widths(cell.local_id)?
    };
    let mut min = if fixed {
        0.0
    } else {
        let floor = style.min_width;
        (content.min_content + edges)
            .max(floor.map_or(0.0, |w| super::sizing::content_size(cell, w, true) + edges))
    };
    let mut max = authored.unwrap_or(content.max_content.max(content.min_content) + edges);
    if let Some(cap) = style.max_width.filter(|_| !style.percent_max_width) {
        let cap = super::sizing::content_size(cell, cap, true) + edges;
        max = max.min(cap);
        min = min.min(cap);
    }
    Ok(columns::Cell {
        min,
        max: max.max(min),
        percent: style.width.filter(|_| style.percent_width),
        constrained: authored.is_some(),
    })
}
/** Column constraints from every cell (only the first row's in a fixed table). */
fn column_constraints(
    context: &mut LayoutContext<'_>,
    grid: &Grid<'_>,
    spacing: f64,
    fixed: bool,
) -> EngineResult<Vec<columns::Column>> {
    let mut singles = Vec::new();
    let mut spans = Vec::new();
    for slot in &grid.cells {
        context.charge(slot.columns)?;
        if slot.cell.hidden() || (fixed && slot.row > 0) {
            continue;
        }
        let cell = cell_constraint(context, slot.cell, fixed)?;
        if slot.columns == 1 {
            singles.push((slot.column, cell));
        } else {
            spans.push((slot.column, slot.columns, cell));
        }
    }
    Ok(columns::constraints(
        &grid.elements,
        grid.columns,
        &singles,
        &spans,
        spacing,
        fixed,
    ))
}
/** Lays out every cell at its column span width, sizes rows, then aligns cells vertically; returns the bottom. */
fn place(
    context: &mut LayoutContext<'_>,
    grid: &Grid<'_>,
    widths: &[f64],
    (x0, y0, spacing): (f64, f64, f64),
    items: &mut Vec<DisplayItem>,
) -> EngineResult<f64> {
    let mut lefts = Vec::with_capacity(widths.len());
    let mut x = x0 + spacing;
    for width in widths {
        lefts.push(x);
        x += width + spacing;
    }
    let span_width = |slot: &Slot<'_>| {
        widths[slot.column..slot.column + slot.columns]
            .iter()
            .sum::<f64>()
            + spacing * (slot.columns - 1) as f64
    };
    // Row heights from each cell's natural height at its width, authored heights included.
    let mut heights = grid
        .rows
        .iter()
        .map(|row| {
            row.node
                .style
                .height
                .filter(|_| !row.anonymous)
                .and_then(|h| h.resolve(None))
                .unwrap_or(0.0)
        })
        .collect::<Vec<f64>>();
    let mut measured = Vec::with_capacity(grid.cells.len());
    for slot in &grid.cells {
        let width = span_width(slot);
        let height = if slot.cell.hidden() {
            0.0
        } else {
            context.measure_height(slot.cell.local_id, width)?
        };
        measured.push(height);
        if slot.rows == 1 {
            heights[slot.row] = heights[slot.row].max(height);
        }
    }
    for (slot, height) in grid.cells.iter().zip(&measured) {
        if slot.rows > 1 {
            let span = &mut heights[slot.row..slot.row + slot.rows];
            let have = span.iter().sum::<f64>() + spacing * (slot.rows - 1) as f64;
            let extra = (height - have).max(0.0) / slot.rows as f64;
            span.iter_mut().for_each(|h| *h += extra);
        }
    }
    let mut tops = Vec::with_capacity(heights.len());
    let mut y = y0 + spacing;
    for height in &heights {
        tops.push(y);
        y += height + spacing;
    }
    // A row box spans its columns, inside the outer border spacing (Blink's row fragments).
    let total = (x - x0 - 2.0 * spacing).max(0.0);
    for (row, node) in grid
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| !row.anonymous)
    {
        context.charge(1)?;
        items.push(box_item(
            node.node,
            x0 + spacing,
            tops[row],
            total,
            heights[row],
            Vec::new(),
        ));
    }
    for (slot, natural) in grid.cells.iter().zip(measured) {
        if slot.cell.hidden() {
            continue;
        }
        let height = heights[slot.row..slot.row + slot.rows].iter().sum::<f64>()
            + spacing * (slot.rows - 1) as f64;
        let start = items.len();
        context.layout_at(
            slot.cell.local_id,
            BorderBox {
                x: lefts[slot.column],
                y: tops[slot.row],
                width: span_width(slot),
                height: Some(height),
            },
            items,
        )?;
        // Alignment moves the content by its own height, not the authored cell height.
        let natural = context
            .content_height(slot.cell.local_id)
            .unwrap_or(natural)
            .min(natural);
        let offset = match row_align(context, slot.cell) {
            VerticalAlign::Top | VerticalAlign::TextTop => 0.0,
            VerticalAlign::Bottom | VerticalAlign::TextBottom => height - natural,
            VerticalAlign::Baseline => 0.0,
            _ => (height - natural) / 2.0,
        }
        .max(0.0);
        if offset > 0.0 {
            if let Some(cell) = items.get_mut(start) {
                for line in &mut cell.lines {
                    line.y += offset;
                }
            }
            for item in items.iter_mut().skip(start + 1) {
                super::line::translate(item, 0.0, offset);
            }
        }
    }
    Ok(y)
}
/** A cell's vertical-align, or its row's `valign` when the cell keeps the default. */
fn row_align(context: &LayoutContext<'_>, cell: &Node) -> VerticalAlign {
    if cell.attributes.contains_key("valign") || cell.style.vertical_align != VerticalAlign::Middle
    {
        return cell.style.vertical_align;
    }
    let row = cell.parent.and_then(|parent| context.node(parent).ok());
    match row
        .and_then(|row| row.attributes.get("valign"))
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("top") => VerticalAlign::Top,
        Some("bottom") => VerticalAlign::Bottom,
        Some("baseline") => VerticalAlign::Baseline,
        _ => VerticalAlign::Middle,
    }
}
/** Intrinsic widths of a whole table: the column sums plus spacing and edges. */
pub(super) fn intrinsic(
    context: &mut LayoutContext<'_>,
    node: &Node,
) -> EngineResult<IntrinsicWidths> {
    let grid = grid(context, node)?;
    let fixed = fixed_layout(node);
    let columns = column_constraints(context, &grid, spacing(node), fixed)?;
    let (min, max) = columns::grid_min_max(&columns, fixed);
    let edges = spacing(node) * (grid.columns + 1) as f64;
    Ok(IntrinsicWidths {
        min_content: min + edges,
        max_content: max + edges,
    })
}
