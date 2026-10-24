/** Automatic table layout: a span-aware grid, min/max column widths, border spacing and row heights. */
use super::flow::*;
use super::sizing::IntrinsicWidths;
use crate::document::style::VerticalAlign;

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
    rows: Vec<&'a Node>,
    cells: Vec<Slot<'a>>,
    columns: usize,
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
    let columns = column_widths(context, &grid, spacing)?;
    let edges = horizontal_edges(node) + spacing * (grid.columns + 1) as f64;
    let minimum = columns.iter().map(|c| c.min_content).sum::<f64>() + edges;
    let maximum = columns.iter().map(|c| c.max_content).sum::<f64>() + edges;
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
    for caption in context
        .all_children(node.local_id)
        .into_iter()
        .filter(|child| child.tag == "caption")
    {
        node_layout(context, caption, &mut inner, items, None)?;
    }
    inner.newline();
    let fixed = fixed_columns(&grid);
    let widths = distribute(
        &columns,
        &fixed,
        (inner.width - spacing * (grid.columns + 1) as f64).max(0.0),
    );
    let bottom = place(context, &grid, &widths, (inner.x, inner.y, spacing), items)?;
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
/** Rows and cells placed into grid slots, skipping slots taken by earlier row spans. */
fn grid<'a>(context: &mut LayoutContext<'a>, table: &Node) -> EngineResult<Grid<'a>> {
    let mut rows = context.descendants(table.local_id);
    rows.retain(|node| {
        node.tag == "tr" && !node.hidden() && owner(context, node) == Some(table.local_id)
    });
    rows.truncate(GRID_ROWS);
    let mut taken: Vec<Vec<bool>> = vec![Vec::new(); rows.len()];
    let mut cells = Vec::new();
    let mut columns = 0;
    for (row, node) in rows.iter().enumerate() {
        context.charge(1)?;
        let mut column = 0;
        for cell in context.children(node.local_id) {
            if !["td", "th"].contains(&cell.tag.as_str()) {
                continue;
            }
            while taken[row].get(column).copied().unwrap_or(false) {
                column += 1;
            }
            let span = |name: &str, limit: usize| {
                cell.attributes
                    .get(name)
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(1)
                    .clamp(1, limit.max(1))
            };
            let spans = (
                span("rowspan", rows.len() - row),
                span("colspan", GRID_COLUMNS),
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
    Ok(Grid {
        rows,
        cells,
        columns: columns.max(1),
    })
}
/** The nearest table ancestor, so a nested table's rows belong to it and not to the outer grid. */
fn owner(context: &LayoutContext<'_>, node: &Node) -> Option<usize> {
    let mut current = node.parent;
    while let Some(local) = current {
        let ancestor = context.node(local).ok()?;
        if ancestor.tag == "table" {
            return Some(local);
        }
        current = ancestor.parent;
    }
    None
}
/** A cell's border-box min and max widths; an authored width fixes both, never below min-content. */
fn cell_widths(context: &mut LayoutContext<'_>, cell: &Node) -> EngineResult<IntrinsicWidths> {
    let content = context.intrinsic_widths(cell.local_id)?;
    let edges = horizontal_edges(cell);
    let min = content.min_content + edges;
    let max = content.max_content.max(content.min_content) + edges;
    Ok(
        match cell.style.width.filter(|_| !cell.style.percent_width) {
            Some(width) => {
                let fixed = super::sizing::content_size(cell, width, true) + edges;
                IntrinsicWidths {
                    min_content: min.max(fixed),
                    max_content: min.max(fixed),
                }
            }
            None => IntrinsicWidths {
                min_content: min,
                max_content: max,
            },
        },
    )
}
/** Column min and max widths: single-span cells first, then spans spread their excess evenly. */
fn column_widths(
    context: &mut LayoutContext<'_>,
    grid: &Grid<'_>,
    spacing: f64,
) -> EngineResult<Vec<IntrinsicWidths>> {
    let mut columns = vec![IntrinsicWidths::default(); grid.columns];
    for pass in [false, true] {
        for slot in grid.cells.iter().filter(|slot| (slot.columns > 1) == pass) {
            context.charge(slot.columns)?;
            if slot.cell.hidden() {
                continue;
            }
            let widths = cell_widths(context, slot.cell)?;
            let span = &mut columns[slot.column..slot.column + slot.columns];
            let gaps = spacing * (slot.columns - 1) as f64;
            let have_min = span.iter().map(|c| c.min_content).sum::<f64>() + gaps;
            let have_max = span.iter().map(|c| c.max_content).sum::<f64>() + gaps;
            let count = span.len() as f64;
            for column in span.iter_mut() {
                column.min_content += (widths.min_content - have_min).max(0.0) / count;
                column.max_content += (widths.max_content - have_max).max(0.0) / count;
                column.max_content = column.max_content.max(column.min_content);
            }
        }
    }
    Ok(columns)
}
/** Column widths for an available width: between min and max, then extra space in proportion to
 * max, given only to columns without an authored width while any exist. */
fn distribute(columns: &[IntrinsicWidths], fixed: &[bool], available: f64) -> Vec<f64> {
    let min = columns.iter().map(|c| c.min_content).sum::<f64>();
    let max = columns.iter().map(|c| c.max_content).sum::<f64>();
    if available <= min {
        return columns.iter().map(|c| c.min_content).collect();
    }
    if available <= max {
        let share = (available - min) / (max - min).max(f64::EPSILON);
        return columns
            .iter()
            .map(|c| c.min_content + (c.max_content - c.min_content) * share)
            .collect();
    }
    let extra = available - max;
    let flexible = fixed.iter().any(|fixed| !fixed);
    let grows = |index: usize| !flexible || !fixed.get(index).copied().unwrap_or(false);
    let basis = columns
        .iter()
        .enumerate()
        .filter(|(index, _)| grows(*index))
        .map(|(_, c)| c.max_content)
        .sum::<f64>();
    let count = (0..columns.len())
        .filter(|index| grows(*index))
        .count()
        .max(1);
    columns
        .iter()
        .enumerate()
        .map(|(index, c)| {
            if !grows(index) {
                c.max_content
            } else if basis > 0.0 {
                c.max_content + extra * c.max_content / basis
            } else {
                c.max_content + extra / count as f64
            }
        })
        .collect()
}
/** Columns a single-span cell gives an authored pixel width. */
fn fixed_columns(grid: &Grid<'_>) -> Vec<bool> {
    let mut fixed = vec![false; grid.columns];
    for slot in grid.cells.iter().filter(|slot| slot.columns == 1) {
        if slot.cell.style.width.is_some() && !slot.cell.style.percent_width {
            fixed[slot.column] = true;
        }
    }
    fixed
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
            row.style
                .height
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
    let total = x - x0;
    for (row, node) in grid.rows.iter().enumerate() {
        context.charge(1)?;
        items.push(box_item(
            node,
            x0,
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
    let columns = column_widths(context, &grid, spacing(node))?;
    let edges = horizontal_edges(node) + spacing(node) * (grid.columns + 1) as f64;
    let min = columns.iter().map(|c| c.min_content).sum::<f64>() + edges;
    let max = columns.iter().map(|c| c.max_content).sum::<f64>() + edges;
    Ok(IntrinsicWidths {
        min_content: (min - horizontal_edges(node)).max(0.0),
        max_content: (max - horizontal_edges(node)).max(0.0),
    })
}
