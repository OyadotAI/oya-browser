/** Automatic table layout: a span-aware grid, min/max column widths, border spacing and row heights. */
use super::flow::*;
use super::sizing::IntrinsicWidths;
use crate::document::style::{TableBox, VerticalAlign};

mod anonymous;
mod collapse;
mod columns;
mod frame;
mod parts;
mod stand_in;
mod wrapper;
pub(super) use anonymous::fixup as anonymous_boxes;
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
    /** The elements behind `elements`: each column's group and own element. */
    column_boxes: Vec<(Option<&'a Node>, &'a Node)>,
    /** Whether the table collapses its borders, so cells paint whole grid edges. */
    collapsed: bool,
    /** `direction: rtl`: the first column sits at the right (css-tables-3 column order). */
    rtl: bool,
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
        return wraps_anonymous_table(context, node);
    }
    let children = context.children(node.local_id);
    // An empty table is still a table: its width shrinks to its edges (empty-table-height).
    if children.is_empty() && node.text.trim().is_empty() && !node.tag.starts_with("::") {
        return true;
    }
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
        // Columns take no box of their own, so a table of columns alone paints none of them
        // (table-column-rendering-001).
        TableBox::Column,
    ]) > 0
        || (parts(&[TableBox::Caption]) > 0 && others == 1)
}
/** A block whose in-flow children are all table parts, with a row, cell or row group among
 * them: CSS 2.1 17.2.1 (Blink's LayoutTable::CreateAnonymousWithParent) wraps them in one
 * anonymous table, which this box lays out in place, so its cells sit side by side.
 *
 * ponytail: only the all-table-parts case; a run of parts between other content still stacks. */
fn wraps_anonymous_table(context: &LayoutContext<'_>, node: &Node) -> bool {
    let style = &node.style;
    if style.table_box != TableBox::None
        || style.display_flex
        || style.display_grid
        || !node.text.trim().is_empty()
        || [
            "td", "th", "tr", "thead", "tbody", "tfoot", "caption", "col", "colgroup",
        ]
        .contains(&node.tag.as_str())
    {
        return false;
    }
    let children = context.children(node.local_id);
    let mut in_flow = children
        .iter()
        .filter(|c| !super::flow::out_of_flow(c))
        .peekable();
    in_flow.peek().is_some()
        && in_flow
            .clone()
            .all(|child| !matches!(child.style.table_box, TableBox::None | TableBox::Table))
        && in_flow
            .any(|child| !matches!(child.style.table_box, TableBox::Caption | TableBox::Column))
}
/** Defers the absolute and fixed children of the table and of its rows and row groups at the
grid's start: they take no grid slot, and a blockified abspos cell (CSS2 top-applies-to-007)
otherwise never laid out at all.
ponytail: a row's out-of-flow child takes the grid's start, not its row's, as static position. */
fn defer_out_of_flow(context: &mut LayoutContext<'_>, node: &Node, at: (f64, f64)) {
    for child in context.children(node.local_id) {
        if super::flow::out_of_flow(child) {
            context.defer(child.local_id, at);
        } else if is_row(child) || group_kind(child).is_some() {
            defer_out_of_flow(context, child, at);
        }
    }
}
/** A `display: table` box laid out as a block (it holds only its own text, which is its one
 * anonymous cell): like any table its width shrinks to fit (inline-table-height-002-ref). */
pub(super) fn shrinks(node: &Node) -> bool {
    node.style.table_box == TableBox::Table && node.tag != "table"
}
/** Whether this table box is the anonymous table its block wraps around its table parts. */
fn is_anonymous(node: &Node) -> bool {
    node.tag != "table" && node.style.table_box != TableBox::Table
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
    let anonymous = is_anonymous(node);
    let centered = centered(context, node);
    let (width, x, y) = frame::frame(context, node, flow, forced, ((minimum, maximum), centered))?;
    let index = items.len();
    items.push(box_item(node, x, y, width, 0.0, Vec::new()));
    let sides = super::sizing::border_sides(style);
    // Captions sit outside the table box's border and padding (CSS 2.1 17.4), in the wrapper.
    let (bottom_captions, top_captions): (Vec<_>, Vec<_>) = context
        .all_children(node.local_id)
        .into_iter()
        .filter(|child| is_caption(child))
        .partition(|caption| caption.style.caption_bottom);
    let captioned = !anonymous && (!top_captions.is_empty() || !bottom_captions.is_empty());
    // An anonymous table's box is its wrapping block, whose captions stay in its content box.
    let content = (
        x + sides[3] + style.padding[3],
        y + sides[0] + style.padding[0],
    );
    let inner_width = (width - horizontal_edges(node)).max(0.0);
    let frame = if anonymous {
        (content.0, content.1, inner_width)
    } else {
        (x, y, width)
    };
    let mut above = Flow::new(frame.0, frame.1, frame.2);
    for caption in top_captions {
        node_layout(context, caption, &mut above, items, None)?;
    }
    above.newline();
    let after = if above.bottom > frame.1 {
        above.bottom.max(above.y)
    } else {
        frame.1
    };
    let top = if anonymous { y } else { after };
    let inner_top = if anonymous {
        after
    } else {
        top + sides[0] + style.padding[0]
    };
    let grid_width = if anonymous {
        let own = horizontal_edges(node);
        (maximum - own).min(inner_width).max(minimum - own)
    } else {
        inner_width
    };
    let widths = columns::widths(
        (grid_width - spacing * (grid.columns + 1) as f64).max(0.0),
        &columns,
        fixed_layout,
    );
    // Blink's table block size counts the captions, which subtract from the grid's share.
    // min-height and max-height hold the table's block size like any box's (Blink's table
    // block size is clamped by them; a min-height alone is a floor). A flex or grid item's
    // container applies them itself, after measuring the table's content.
    let floor = style.min_height.filter(|_| !layout_item(context, node));
    let specified = style
        .height
        .and_then(|height| height.resolve_size(flow.definite))
        .map(|height| super::sizing::border_height(node, height))
        .or_else(|| floor.map(|_| 0.0))
        .map(|height| clamp_height(node, height, flow.definite));
    // Only an authored or forced height needs the bottom captions measured ahead of placing.
    let sized = specified.is_some() || forced.is_some_and(|bounds| bounds.height.is_some());
    let captions = (top - y)
        + if sized && !anonymous {
            caption_heights(context, &bottom_captions, width)?
        } else {
            0.0
        };
    let wanted = forced.and_then(|bounds| bounds.height).or(specified);
    // The rows share what the table's height leaves after captions, edges and padding.
    let target =
        wanted.map(|total| total - captions - (inner_top - top) - style.padding[2] - sides[2]);
    let mut bottom = place(
        context,
        &grid,
        &widths,
        (content.0, inner_top, (spacing, row_spacing(node)), target),
        items,
    )?;
    // An absolute or fixed child takes no grid slot; its static position is the grid's start.
    defer_out_of_flow(context, node, (content.0, inner_top));
    if anonymous {
        let mut below = Flow::new(frame.0, bottom, frame.2);
        for caption in &bottom_captions {
            node_layout(context, caption, &mut below, items, None)?;
        }
        below.newline();
        bottom = below.bottom.max(below.y);
    }
    let natural = bottom - top + style.padding[2] + sides[2];
    let mut below = Flow::new(x, top + natural, width);
    let total = forced
        .and_then(|bounds| bounds.height)
        .unwrap_or((natural + captions).max(specified.unwrap_or(0.0)));
    let grid_height = (total - captions).max(natural);
    below.y = top + grid_height;
    if !anonymous {
        for caption in bottom_captions {
            node_layout(context, caption, &mut below, items, None)?;
        }
    }
    below.newline();
    let height = (below.bottom.max(below.y) - y).max(total);
    // The item is the table wrapper box (floats and lookups read its extent); a captioned
    // table paints its grid box apart, so its background stays off the captions.
    items[index].height = height;
    if captioned {
        wrapper::split_grid_box(items, index, top, grid_height);
    }
    if forced.is_some() {
        flow.y = y + height;
        flow.collapsible = 0.0;
        flow.bottom = flow.bottom.max(flow.y);
    } else {
        flow.after_block(y + height, style.margin[2]);
    }
    Ok(())
}
/** Whether a box is a flex or grid item, whose container sizes it. */
fn layout_item(context: &LayoutContext<'_>, node: &Node) -> bool {
    node.parent
        .and_then(|parent| context.node(parent).ok())
        .is_some_and(|parent| parent.style.display_flex || parent.style.display_grid)
}
/** The block size bottom captions take at the table's width, measured before they are placed. */
fn caption_heights(
    context: &mut LayoutContext<'_>,
    captions: &[&Node],
    width: f64,
) -> EngineResult<f64> {
    let mut probe = Flow::new(0.0, 0.0, width);
    let mut scratch = Vec::new();
    for caption in captions {
        node_layout(context, caption, &mut probe, &mut scratch, None)?;
    }
    probe.newline();
    Ok(probe.bottom.max(probe.y))
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
/** Row spacing: `border-spacing`'s second length, else its first; none when borders collapse. */
fn row_spacing(node: &Node) -> f64 {
    if node.style.border_collapse {
        0.0
    } else {
        node.style
            .border_spacing_rows
            .unwrap_or(node.style.border_spacing)
    }
}
/** Column spacing: `border-spacing`'s first length; none when borders collapse. */
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
        let mut cells: Vec<&Node> = cells
            .into_iter()
            .filter(|cell| is_cell(child, cell))
            .collect();
        // A CSS row holding only its own text: that text is its one anonymous cell, which the
        // row lays out itself (table-vertical-align-baseline-009).
        if cells.is_empty() && child.tag != "tr" && !child.text.trim().is_empty() {
            cells.push(child);
        }
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
fn column_elements(boxes: &[(Option<&Node>, &Node)], fixed: bool) -> Vec<columns::Column> {
    boxes
        .iter()
        .map(|(group, own)| {
            let group = group.map(|group| column_element(group, None));
            let fallback = group.filter(|group| !fixed && group.constrained);
            column_element(own, fallback.map(|group| group.max))
        })
        .collect()
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
    let mut column_boxes = parts::column_nodes(context, table);
    let mut elements = column_elements(&column_boxes, fixed);
    while elements.len() > columns && elements.last().is_some_and(|c| c.mergeable(fixed)) {
        elements.pop();
    }
    elements.truncate(GRID_COLUMNS);
    column_boxes.truncate(elements.len());
    Ok(Grid {
        rows: rows.into_iter().map(|(row, _)| row).collect(),
        cells,
        columns: columns.max(elements.len()).max(1),
        sections,
        elements,
        column_boxes,
        collapsed: table.style.border_collapse,
        rtl: table.style.rtl,
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
    // An element standing in for the anonymous cell around it: its authored width is that
    // cell's content, so it is the cell's minimum too, not a cell width (inline-table-width-002a).
    let stand_in = !is_cell_box(cell);
    if stand_in && let Some(authored) = authored {
        min = min.max(authored);
    }
    let mut max = authored.unwrap_or(content.max_content.max(content.min_content) + edges);
    // Its margins sit inside that anonymous cell, so they widen it.
    if stand_in && !fixed {
        // Intrinsic sizes resolve a percentage margin against zero (calc-margins-table-caption).
        let margins = style
            .fixed_inline_sides
            .map_or(style.margin[1] + style.margin[3], |fixed| {
                fixed[2] + fixed[3]
            });
        (min, max) = (min + margins, max + margins);
    }
    if let Some(cap) = style.max_width.filter(|_| !style.percent_max_width) {
        let cap = super::sizing::content_size(cell, cap, true) + edges;
        max = max.min(cap);
        min = min.min(cap);
    }
    Ok(columns::Cell {
        min,
        max: max.max(min),
        percent: style.width.filter(|_| style.percent_width),
        constrained: authored.is_some() && !stand_in,
        // Only fixed tables add a content-box cell's edges to its percentage (Blink).
        percent_edges: if fixed && style.box_sizing != crate::document::BoxSizing::BorderBox {
            edges
        } else {
            0.0
        },
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
        if slot.cell.removed() || (fixed && slot.row > 0) {
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
/** Blink's DistributeTableBlockSizeToRows for auto rows: a table taller than its rows gives
the excess to them in proportion to their heights, or equally when every row is empty. */
fn grow_rows(heights: &mut [f64], room: f64) {
    let have = heights.iter().sum::<f64>();
    let extra = room - have;
    if extra <= 0.0 || heights.is_empty() {
        return;
    }
    let count = heights.len() as f64;
    for height in heights.iter_mut() {
        *height += if have > 0.0 {
            extra * *height / have
        } else {
            extra / count
        };
    }
}
/** Lays out every cell at its column span width, sizes rows, then aligns cells vertically; returns the bottom. */
fn place(
    context: &mut LayoutContext<'_>,
    grid: &Grid<'_>,
    widths: &[f64],
    (x0, y0, (spacing, rows_gap), target): (f64, f64, (f64, f64), Option<f64>),
    items: &mut Vec<DisplayItem>,
) -> EngineResult<f64> {
    let mut lefts = Vec::with_capacity(widths.len());
    let mut x = x0 + spacing;
    for width in widths {
        lefts.push(x);
        x += width + spacing;
    }
    if grid.rtl {
        for (left, width) in lefts.iter_mut().zip(widths) {
            *left = x0 + x - *left - width;
        }
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
    let mut baselines = Vec::with_capacity(grid.cells.len());
    let mut rows = vec![RowBaseline::default(); heights.len()];
    for slot in &grid.cells {
        let width = span_width(slot);
        let height = if slot.cell.removed() {
            0.0
        } else {
            stand_in::measure(context, slot.cell, width)?
        };
        let baseline = cell_baseline(context, slot, width, height)?;
        if let Some(baseline) = baseline {
            rows[slot.row].add(baseline, (slot.rows == 1).then_some(height - baseline));
        }
        measured.push(height);
        baselines.push(baseline);
        if slot.rows == 1 {
            heights[slot.row] = heights[slot.row].max(height);
        }
    }
    for (height, row) in heights.iter_mut().zip(&rows) {
        *height = height.max(row.size());
    }
    for (slot, height) in grid.cells.iter().zip(&measured) {
        if slot.rows > 1 {
            let span = &mut heights[slot.row..slot.row + slot.rows];
            let have = span.iter().sum::<f64>() + rows_gap * (slot.rows - 1) as f64;
            let extra = (height - have).max(0.0) / slot.rows as f64;
            span.iter_mut().for_each(|h| *h += extra);
        }
    }
    if let Some(target) = target {
        let gaps = rows_gap * (heights.len() + 1) as f64;
        grow_rows(&mut heights, target - gaps);
    }
    let mut tops = Vec::with_capacity(heights.len());
    let mut y = y0 + rows_gap;
    for height in &heights {
        tops.push(y);
        y += height + rows_gap;
    }
    // A row box spans its columns, inside the outer border spacing (Blink's row fragments).
    let total = (x - x0 - 2.0 * spacing).max(0.0);
    // Row groups paint first, under their rows and cells (Blink's TableSectionPainter), and give
    // a positioned group a box to offset and to contain its absolute descendants.
    // ponytail: one box over the group's rows, so border spacing between its rows takes its
    // background; paint it per cell when a page shows the gap.
    for section in &grid.sections {
        let (Some(node), Some(&first), Some(last)) = (
            section.node,
            tops.get(section.rows.start),
            section.rows.end.checked_sub(1),
        ) else {
            continue;
        };
        if section.rows.is_empty() {
            continue;
        }
        context.charge(1)?;
        let bottom = tops[last] + heights[last];
        items.push(part_box(node, x0 + spacing, first, total, bottom - first));
        parts::strip(items.last_mut().expect("just pushed"));
    }
    for (row, node) in grid
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| !row.anonymous)
    {
        context.charge(1)?;
        items.push(part_box(
            node.node,
            x0 + spacing,
            tops[row],
            total,
            heights[row],
        ));
        parts::strip(items.last_mut().expect("just pushed"));
    }
    for ((slot, natural), baseline) in grid.cells.iter().zip(measured).zip(baselines) {
        if slot.cell.removed() {
            continue;
        }
        let height = heights[slot.row..slot.row + slot.rows].iter().sum::<f64>()
            + rows_gap * (slot.rows - 1) as f64;
        let geometry = parts::Geometry {
            lefts: &lefts,
            widths,
            tops: &tops,
            heights: &heights,
            span: (x0 + spacing, total),
        };
        // In a right-to-left table a spanning cell starts at its last column, the leftmost.
        let last = (slot.column + slot.columns - 1).min(lefts.len() - 1);
        let left = lefts[if grid.rtl { last } else { slot.column }];
        let rect = [left, tops[slot.row], span_width(slot), height];
        parts::paint_under(grid, &geometry, slot, rect, items);
        let start = items.len();
        let bounds = BorderBox {
            x: left,
            y: tops[slot.row],
            width: span_width(slot),
            height: Some(height),
        };
        context.layout_at(
            slot.cell.local_id,
            stand_in::inset(slot.cell, bounds),
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
            VerticalAlign::Baseline => rows[slot.row]
                .ascent
                .zip(baseline)
                .map_or(0.0, |(a, b)| a - b),
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
        if grid.collapsed
            && let Some(edges) = collapsed_edges(slot.cell, start, items)
        {
            items.push(edges);
        }
    }
    Ok(y)
}
/** A row or row group's box: its background only, since in the separated model those boxes
 * have no borders (CSS 2.1 17.6.1) and in the collapsed one their widths already joined the
 * cells' edges (collapse.rs), so painting them again would draw each border twice. */
fn part_box(node: &Node, x: f64, y: f64, width: f64, height: f64) -> DisplayItem {
    let mut item = box_item(node, x, y, width, height, Vec::new());
    item.border = 0.0;
    item.border_widths = [0.0; 4];
    item
}
/**
 * A collapsed cell's borders as Blink's CollapsedBorderPainter draws them: each edge at its whole
 * width, centered on the grid line, rather than the half the cell's box holds for layout. The
 * cell's own box keeps its background and drops its half borders; the edges paint over it as one
 * more item of the cell (a 36px orange border painted 18px wide made a fixed table's cell 36px
 * narrower than its reference).
 *
 * ponytail: each cell paints all four of its edges in their winners' colors, so a shared edge
 * paints twice and an overlap goes to the later cell in paint (DOM) order; Blink paints each
 * edge once in grid order, which row-group-order.html (a thead after a tbody) shows.
 */
fn collapsed_edges(cell: &Node, start: usize, items: &mut [DisplayItem]) -> Option<DisplayItem> {
    let own = items.get_mut(start)?;
    let half = super::sizing::border_sides(&cell.style);
    if own.local_id != cell.local_id || half.iter().all(|width| *width <= 0.0) {
        return None;
    }
    own.border_widths = [0.0; 4];
    let mut edges = own.clone();
    edges.background = None;
    edges.backgrounds = Vec::new();
    edges.shadows = Vec::new();
    edges.lines = Vec::new();
    edges.vector = Vec::new();
    edges.validation = Vec::new();
    edges.x -= half[3];
    edges.y -= half[0];
    edges.width += half[1] + half[3];
    edges.height += half[0] + half[2];
    edges.border_widths = half.map(|width| super::sizing::snap_border(width * 2.0));
    Some(edges)
}
/** Blink's RowBaselineTabulator: the tallest ascent and descent of a row's baseline-aligned
 * cells (a row-spanning cell adds its ascent only). */
#[derive(Clone, Copy, Default)]
struct RowBaseline {
    ascent: Option<f64>,
    descent: f64,
}
impl RowBaseline {
    /** Takes one baseline-aligned cell's baseline and, unless it spans rows, its descent. */
    fn add(&mut self, baseline: f64, descent: Option<f64>) {
        self.ascent = Some(self.ascent.map_or(baseline, |a| a.max(baseline)));
        self.descent = self.descent.max(descent.unwrap_or(0.0));
    }
    /** The block size the aligned cells need: ascent plus descent. */
    fn size(&self) -> f64 {
        self.ascent.map_or(0.0, |a| a + self.descent)
    }
}
/** A baseline-aligned cell's baseline from its border-box top: its first line's, else the end of
 * its content box (css-tables-3 row layout); None for other alignments and for empty cells. */
fn cell_baseline(
    context: &mut LayoutContext<'_>,
    slot: &Slot<'_>,
    width: f64,
    height: f64,
) -> EngineResult<Option<f64>> {
    let cell = slot.cell;
    let empty = cell.text.trim().is_empty() && context.children(cell.local_id).is_empty();
    if cell.removed() || empty || row_align(context, cell) != VerticalAlign::Baseline {
        return Ok(None);
    }
    let end = height - cell.style.padding[2] - super::sizing::border_sides(&cell.style)[2];
    Ok(Some(
        super::flex_layout::first_baseline(context, cell, width)?.unwrap_or(end),
    ))
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
