/** CSS grid: explicit and auto-repeated tracks, line and span placement, row auto-flow and item alignment. */
use super::flow::*;
use crate::document::AlignItems;
use crate::document::style::{GridArea, GridLine, GridTemplate, Track, TrackSize};

/** Columns a grid lays out, explicit, repeated or implicit; items past it are clamped in. */
const GRID_COLUMNS: usize = 64;

/** One in-flow item and its resolved area in track indexes. */
struct Placed<'a> {
    node: &'a Node,
    row: usize,
    column: usize,
    rows: usize,
    columns: usize,
}
/** Lays out a grid container's children into its tracks and advances `inner` past the grid. */
pub(super) fn children(
    context: &mut LayoutContext<'_>,
    node: &Node,
    inner: &mut Flow,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    let style = &node.style;
    let mut kids = context.children(node.local_id);
    kids.retain(|child| {
        if out_of_flow(child) {
            context.defer(child.local_id, (inner.x, inner.y));
            false
        } else {
            true
        }
    });
    // `order` changes auto-placement order only; the stable sort keeps source order among equals.
    kids.sort_by_key(|child| child.style.order);
    let gap = (style.column_gap, style.row_gap);
    let mut columns = expand(&style.grid_columns, inner.width, gap.0);
    let placed = place(context, &kids, &mut columns, &style.grid_areas)?;
    let widths = size_columns(context, &columns, &placed, inner.width, gap.0)?;
    let lefts = offsets(inner.x, &widths, gap.0);
    let span_width = |p: &Placed<'_>| span(&widths, p.column, p.columns, gap.0);
    let row_count = placed.iter().map(|p| p.row + p.rows).max().unwrap_or(0);
    let mut heights = vec![0.0_f64; row_count];
    for (index, height) in heights.iter_mut().enumerate() {
        if let Some(track) = style.grid_rows.tracks.get(index)
            && let Some(length) = track.max.length(inner.definite)
        {
            *height = length;
        }
    }
    let mut natural = Vec::with_capacity(placed.len());
    for item in &placed {
        let justify = item.node.style.justify_self.unwrap_or(style.justify_items);
        let width = item_width(context, item.node, span_width(item), justify)?;
        let margins = item.node.style.margin;
        let height = context.measure_height(item.node.local_id, width)? + margins[0] + margins[2];
        natural.push((width, height));
        if item.rows == 1 {
            heights[item.row] = heights[item.row].max(height);
        }
    }
    flexible_rows(&style.grid_rows, &mut heights, inner.definite, gap.1);
    let tops = offsets(inner.y, &heights, gap.1);
    for (item, (width, height)) in placed.iter().zip(natural) {
        context.charge(1)?;
        let area_height = span(&heights, item.row, item.rows, gap.1);
        let margins = item.node.style.margin;
        let free_x = (span_width(item) - width - margins[1] - margins[3]).max(0.0);
        let justify = item.node.style.justify_self.unwrap_or(style.justify_items);
        let align = item.node.style.align_self.unwrap_or(style.align_items);
        let x = lefts[item.column] + margins[3] + offset(justify, free_x);
        let stretch = align == AlignItems::Stretch
            && item.node.style.height.is_none()
            && item.node.style.aspect_ratio.is_none();
        let free_y = (area_height - height).max(0.0);
        let y = tops[item.row] + margins[0] + if stretch { 0.0 } else { offset(align, free_y) };
        let forced_height = stretch.then_some((area_height - margins[0] - margins[2]).max(0.0));
        let mut target = Flow::new(x, y, width);
        node_layout(
            context,
            item.node,
            &mut target,
            items,
            Some(BorderBox {
                x,
                y,
                width,
                height: forced_height,
            }),
        )?;
    }
    let total = heights.iter().sum::<f64>() + gap.1 * heights.len().saturating_sub(1) as f64;
    inner.y += total;
    inner.bottom = inner.bottom.max(inner.y);
    Ok(())
}
/** Explicit tracks with any auto-fill group repeated as often as it fits the width. */
fn expand(template: &GridTemplate, width: f64, gap: f64) -> Vec<Track> {
    let mut tracks = template.tracks.clone();
    if !template.repeat.is_empty() {
        let fixed = |track: &Track| {
            (track.min.length(Some(width)))
                .or(track.max.length(Some(width)))
                .unwrap_or(0.0)
        };
        let group =
            template.repeat.iter().map(fixed).sum::<f64>() + gap * template.repeat.len() as f64;
        let others = tracks.iter().map(fixed).sum::<f64>() + gap * tracks.len() as f64;
        let room = width - others + gap;
        let times = if group > 0.0 {
            ((room / group).floor() as usize).max(1)
        } else {
            1
        };
        let times = times
            .min(GRID_COLUMNS / template.repeat.len().max(1))
            .max(1);
        let at = template.repeat_at.min(tracks.len());
        let repeated = (0..times).flat_map(|_| template.repeat.iter().copied());
        tracks.splice(at..at, repeated);
    }
    tracks.truncate(GRID_COLUMNS);
    tracks
}
/** Explicit lines first, then row-major auto-placement into the first free cells. */
fn place<'a>(
    context: &mut LayoutContext<'_>,
    kids: &[&'a Node],
    columns: &mut Vec<Track>,
    areas: &[GridArea],
) -> EngineResult<Vec<Placed<'a>>> {
    let auto = Track {
        min: TrackSize::Auto,
        max: TrackSize::Auto,
    };
    let named = areas.iter().map(|a| usize::from(a.column + a.columns));
    let named = named.max().unwrap_or(0).min(GRID_COLUMNS);
    while columns.len() < named {
        columns.push(auto);
    }
    let explicit = columns.len().max(1);
    let mut taken: Vec<Vec<bool>> = Vec::new();
    let mut placed = Vec::with_capacity(kids.len());
    let (mut row, mut column) = (0usize, 0usize);
    for &node in kids {
        context.charge(1)?;
        let (row_line, column_line) = lines(node, areas);
        let (col_start, col_span) = resolve(column_line, explicit);
        let (row_start, row_span) = resolve(row_line, usize::MAX);
        let col_span = col_span.min(GRID_COLUMNS);
        let width = columns
            .len()
            .max(col_span)
            .max(col_start.map_or(0, |c| c + col_span));
        let width = width.min(GRID_COLUMNS);
        let (r, c) = match (row_start, col_start) {
            (Some(r), Some(c)) => (r, c.min(width - col_span.min(width))),
            (None, Some(c)) => {
                let c = c.min(width - col_span.min(width));
                let mut r = if c < column { row + 1 } else { row };
                while !free(&taken, r, c, row_span, col_span) {
                    context.charge(1)?;
                    r += 1;
                }
                (r, c)
            }
            (Some(r), None) => {
                let mut c = 0;
                while c + col_span <= width && !free(&taken, r, c, row_span, col_span) {
                    c += 1;
                }
                (r, c.min(width - col_span.min(width)))
            }
            (None, None) => {
                loop {
                    context.charge(1)?;
                    if column + col_span > width {
                        row += 1;
                        column = 0;
                    }
                    if free(&taken, row, column, row_span, col_span) {
                        break;
                    }
                    column += 1;
                }
                (row, column)
            }
        };
        mark(&mut taken, r, c, row_span, col_span);
        if row_start.is_none() && col_start.is_none() || row_start.is_none() {
            row = r;
            column = c + col_span;
        }
        while columns.len() < c + col_span {
            columns.push(auto);
        }
        placed.push(Placed {
            node,
            row: r,
            column: c,
            rows: row_span,
            columns: col_span,
        });
    }
    Ok(placed)
}
/**
 * An item's row and column lines: those of the named area it asks for, else its own. A name
 * the container does not define places the item automatically.
 */
fn lines(node: &Node, areas: &[GridArea]) -> (GridLine, GridLine) {
    let style = &node.style;
    let axis = |index: usize, own: GridLine| {
        let Some(name) = style.grid_area_names[index].as_deref() else {
            return own;
        };
        let Some(area) = areas.iter().find(|area| area.name == name) else {
            return GridLine::default();
        };
        let (start, count) = if index == 0 {
            (area.row, area.rows)
        } else {
            (area.column, area.columns)
        };
        GridLine {
            start: Some(i32::from(start) + 1),
            end: Some(i32::from(start + count) + 1),
            span: count,
        }
    };
    (axis(0, style.grid_row), axis(1, style.grid_column))
}
/** Zero-based start (None for auto) and span of one axis, against `count` explicit tracks. */
fn resolve(line: GridLine, count: usize) -> (Option<usize>, usize) {
    let index = |n: i32| -> usize {
        if n > 0 {
            (n - 1) as usize
        } else {
            let lines = count.saturating_add(1).min(GRID_COLUMNS + 1) as i32;
            (lines + n).max(0) as usize
        }
    };
    let span = usize::from(line.span.max(1));
    match (line.start, line.end) {
        (Some(start), Some(end)) => {
            let (a, b) = (index(start), index(end));
            let (a, b) = (a.min(b), a.max(b));
            (Some(a), (b - a).max(1))
        }
        (Some(start), None) => (Some(index(start)), span),
        (None, Some(end)) => (Some(index(end).saturating_sub(span)), span),
        (None, None) => (None, span),
    }
}
/** Whether every cell of an area is still free. */
fn free(taken: &[Vec<bool>], row: usize, column: usize, rows: usize, columns: usize) -> bool {
    (row..row + rows).all(|r| {
        taken.get(r).is_none_or(|line| {
            (column..column + columns).all(|c| !line.get(c).copied().unwrap_or(false))
        })
    })
}
/** Claims an area's cells, growing implicit rows. */
fn mark(taken: &mut Vec<Vec<bool>>, row: usize, column: usize, rows: usize, columns: usize) {
    if taken.len() < row + rows {
        taken.resize(row + rows, Vec::new());
    }
    for line in &mut taken[row..row + rows] {
        if line.len() < column + columns {
            line.resize(column + columns, false);
        }
        line[column..column + columns].fill(true);
    }
}
/** Column widths: fixed lengths, content-sized auto tracks, then `fr` shares of what is left. */
fn size_columns(
    context: &mut LayoutContext<'_>,
    tracks: &[Track],
    placed: &[Placed<'_>],
    width: f64,
    gap: f64,
) -> EngineResult<Vec<f64>> {
    let mut min_content = vec![0.0_f64; tracks.len()];
    let mut max_content = vec![0.0_f64; tracks.len()];
    for item in placed.iter().filter(|item| item.columns == 1) {
        let size = context.intrinsic_widths(item.node.local_id)?;
        let edges =
            horizontal_edges(item.node) + item.node.style.margin[1] + item.node.style.margin[3];
        let fixed = item
            .node
            .style
            .width
            .filter(|_| !item.node.style.percent_width);
        let (lo, hi) = match fixed {
            Some(width) => {
                let w = super::sizing::content_size(item.node, width, true) + edges;
                (w, w)
            }
            None => (size.min_content + edges, size.max_content + edges),
        };
        min_content[item.column] = min_content[item.column].max(lo);
        max_content[item.column] = max_content[item.column].max(hi);
    }
    let length = |size: TrackSize| size.length(Some(width));
    let mut sizes = tracks
        .iter()
        .enumerate()
        .map(|(i, t)| length(t.min).or(length(t.max)).unwrap_or(min_content[i]))
        .collect::<Vec<f64>>();
    let gaps = gap * tracks.len().saturating_sub(1) as f64;
    let mut free = width - gaps - sizes.iter().sum::<f64>();
    for (i, track) in tracks.iter().enumerate() {
        if free > 0.0 && track.max == TrackSize::Auto {
            let grow = (max_content[i] - sizes[i]).max(0.0).min(free);
            sizes[i] += grow;
            free -= grow;
        }
        if let (Some(max), Some(_)) = (length(track.max), length(track.min))
            && max > sizes[i]
            && free > 0.0
        {
            let grow = (max - sizes[i]).min(free);
            sizes[i] += grow;
            free -= grow;
        }
    }
    let fractions = tracks
        .iter()
        .map(|t| match t.max {
            TrackSize::Fr(f) => f,
            _ => 0.0,
        })
        .collect::<Vec<f64>>();
    let total = fractions.iter().sum::<f64>();
    if total > 0.0 {
        let leftover = width
            - gaps
            - sizes
                .iter()
                .zip(&fractions)
                .filter(|(_, f)| **f == 0.0)
                .map(|(s, _)| *s)
                .sum::<f64>();
        let unit = leftover.max(0.0) / total.max(1.0);
        for (size, fraction) in sizes.iter_mut().zip(&fractions) {
            if *fraction > 0.0 {
                *size = size.max(unit * fraction);
            }
        }
    } else if free > 0.0 {
        let autos = tracks.iter().filter(|t| t.max == TrackSize::Auto).count();
        for (size, track) in sizes.iter_mut().zip(tracks) {
            if autos > 0 && track.max == TrackSize::Auto {
                *size += free / autos as f64;
            }
        }
    }
    Ok(sizes)
}
/** `fr` rows share a definite height's leftover; otherwise they stay content sized. */
fn flexible_rows(template: &GridTemplate, heights: &mut [f64], definite: Option<f64>, gap: f64) {
    let Some(height) = definite else {
        return;
    };
    let fractions = (0..heights.len())
        .map(|i| match template.tracks.get(i).map(|t| t.max) {
            Some(TrackSize::Fr(f)) => f,
            _ => 0.0,
        })
        .collect::<Vec<f64>>();
    let total = fractions.iter().sum::<f64>();
    if total <= 0.0 {
        return;
    }
    let fixed = heights
        .iter()
        .zip(&fractions)
        .filter(|(_, f)| **f == 0.0)
        .map(|(h, _)| *h)
        .sum::<f64>();
    let unit =
        (height - fixed - gap * heights.len().saturating_sub(1) as f64).max(0.0) / total.max(1.0);
    for (h, f) in heights.iter_mut().zip(&fractions) {
        if *f > 0.0 {
            *h = h.max(unit * f);
        }
    }
}
/** Start offsets of consecutive tracks separated by `gap`. */
fn offsets(start: f64, sizes: &[f64], gap: f64) -> Vec<f64> {
    let mut at = start;
    sizes
        .iter()
        .map(|size| {
            let here = at;
            at += size + gap;
            here
        })
        .collect()
}
/** The extent of `count` tracks from `first`, gaps between them included. */
fn span(sizes: &[f64], first: usize, count: usize, gap: f64) -> f64 {
    let end = (first + count).min(sizes.len());
    sizes[first.min(end)..end].iter().sum::<f64>() + gap * count.saturating_sub(1) as f64
}
/** Border-box width in its area: stretched unless sized or aligned, else shrink-to-fit. */
fn item_width(
    context: &mut LayoutContext<'_>,
    node: &Node,
    area: f64,
    justify: AlignItems,
) -> EngineResult<f64> {
    let margins = node.style.margin[1] + node.style.margin[3];
    let room = (area - margins).max(0.0);
    if node.style.width.is_some() {
        return Ok(border_width(node, area, room, None));
    }
    if justify == AlignItems::Stretch && !["img", "svg"].contains(&node.tag.as_str()) {
        return Ok(border_width(node, area, room, None));
    }
    super::float::shrink_width(context, node, area, margins)
}
/** Offset of an aligned item inside its free space. */
fn offset(align: AlignItems, free: f64) -> f64 {
    match align {
        AlignItems::Center => free / 2.0,
        AlignItems::End => free,
        _ => 0.0,
    }
}
