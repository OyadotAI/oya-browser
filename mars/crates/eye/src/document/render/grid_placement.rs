/** CSS grid item placement (css-grid-2 section 8.5), following Blink's GridPlacement: resolve
 * each item's lines, place items fixed in both axes, then items locked to a row (or a column for
 * `grid-auto-flow: column`), then everything else with the auto-placement cursor, sparse or
 * dense. Implicit tracks take their sizes from `grid-auto-rows` and `grid-auto-columns`. */
use super::super::flow::*;
use super::GRID_COLUMNS;
use crate::document::ComputedStyle;
use crate::document::style::{
    ContentKeyword, GridArea, GridLine, GridTemplate, Track, TrackContent, TrackSize,
};

/** One in-flow item and its resolved area in track indexes. */
pub(super) struct Placed<'a> {
    pub node: &'a Node,
    pub row: usize,
    pub column: usize,
    pub rows: usize,
    pub columns: usize,
}
/** An implicit track: `auto` both ways. */
pub(super) const AUTO_TRACK: Track = Track {
    min: TrackSize::Auto,
    max: TrackSize::Auto,
    content: TrackContent {
        min: ContentKeyword::Auto,
        max: ContentKeyword::Auto,
        fit_limit: None,
    },
};
/** The size of implicit track `index` of an axis whose explicit grid has `explicit` tracks:
 * the `grid-auto-*` list repeated from the end of the explicit grid, or `auto`. */
pub(super) fn implicit_track(auto: &GridTemplate, explicit: usize, index: usize) -> Track {
    match auto.tracks.len() {
        0 => AUTO_TRACK,
        count => auto.tracks[index.saturating_sub(explicit) % count],
    }
}

/** One item's resolved position in the auto-flow axes: `major` is the axis the cursor steps
 * through line by line (rows for `row` flow), `minor` the one it walks within a line. */
#[derive(Clone, Copy)]
struct Area {
    major: Option<usize>,
    major_span: usize,
    minor: Option<usize>,
    minor_span: usize,
}

/** Cells taken so far, indexed `[major][minor]`. */
#[derive(Default)]
struct Occupancy(Vec<Vec<bool>>);
impl Occupancy {
    /** Whether every cell of an area is still free. */
    fn free(&self, major: usize, minor: usize, majors: usize, minors: usize) -> bool {
        (major..major + majors).all(|m| {
            self.0.get(m).is_none_or(|line| {
                (minor..minor + minors).all(|c| !line.get(c).copied().unwrap_or(false))
            })
        })
    }
    /** Claims an area's cells, growing implicit lines. */
    fn mark(&mut self, major: usize, minor: usize, majors: usize, minors: usize) {
        if self.0.len() < major + majors {
            self.0.resize(major + majors, Vec::new());
        }
        for line in &mut self.0[major..major + majors] {
            if line.len() < minor + minors {
                line.resize(minor + minors, false);
            }
            line[minor..minor + minors].fill(true);
        }
    }
}

/** Places `kids` in `style`'s grid. `columns` holds the explicit column tracks on entry and
 * every column, implicit ones included, on return; `rows` is the explicit row template's track
 * count once its auto-repeat group is repeated. */
pub(super) fn place<'a>(
    context: &mut LayoutContext<'_>,
    kids: &[&'a Node],
    style: &ComputedStyle,
    columns: &mut Vec<Track>,
    rows: usize,
) -> EngineResult<Vec<Placed<'a>>> {
    let areas = &style.grid_areas;
    let named = areas.iter().map(|a| usize::from(a.column + a.columns));
    let named = named.max().unwrap_or(0).min(GRID_COLUMNS);
    while columns.len() < named {
        columns.push(implicit_track(&style.grid_auto_columns, 0, columns.len()));
    }
    // Negative lines count back from the end of the explicit grid, which may have no tracks:
    // `-1` is then line 1 (LinkedIn's header is `grid-area: 1 / -1` in a template-less grid).
    let explicit_columns = columns.len();
    let explicit_rows = explicit_rows(style, rows);
    let line_names = [&style.grid_rows.names[..], &style.grid_columns.names[..]];
    let column_flow = style.grid_flow_column;
    let mut resolved = Vec::with_capacity(kids.len());
    for node in kids {
        let (row_line, column_line) = lines(node, areas, line_names);
        let (row, rows) = resolve(row_line, explicit_rows);
        let (column, cols) = resolve(column_line, explicit_columns);
        let cols = cols.min(GRID_COLUMNS);
        resolved.push(if column_flow {
            Area {
                major: column,
                major_span: cols,
                minor: row,
                minor_span: rows,
            }
        } else {
            Area {
                major: row,
                major_span: rows,
                minor: column,
                minor_span: cols,
            }
        });
    }
    let explicit_minor = if column_flow {
        explicit_rows
    } else {
        explicit_columns
    };
    let positions = auto_place(
        context,
        &mut resolved,
        explicit_minor,
        style.grid_flow_dense,
    )?;
    let mut placed = Vec::with_capacity(kids.len());
    for ((node, area), (major, minor)) in kids.iter().zip(&resolved).zip(positions) {
        let (row, column, rows, cols) = if column_flow {
            (minor, major, area.minor_span, area.major_span)
        } else {
            (major, minor, area.major_span, area.minor_span)
        };
        let column = column.min(GRID_COLUMNS.saturating_sub(cols));
        while columns.len() < column + cols {
            let index = columns.len();
            columns.push(implicit_track(
                &style.grid_auto_columns,
                explicit_columns,
                index,
            ));
        }
        placed.push(Placed {
            node,
            row,
            column,
            rows,
            columns: cols,
        });
    }
    Ok(placed)
}

/**
 * The auto-placement algorithm (css-grid-2 8.5) on resolved areas, returning each item's
 * `(major, minor)` start. The minor axis is as wide as its explicit tracks, the largest
 * definite minor end and the largest minor span; the major axis grows as needed.
 */
fn auto_place(
    context: &mut LayoutContext<'_>,
    areas: &mut [Area],
    explicit_minor: usize,
    dense: bool,
) -> EngineResult<Vec<(usize, usize)>> {
    let mut minors = explicit_minor;
    for area in areas.iter() {
        minors = minors
            .max(area.minor_span)
            .max(area.minor.map_or(0, |start| start + area.minor_span));
    }
    let minors = minors.clamp(1, GRID_COLUMNS);
    for area in areas.iter_mut() {
        area.minor_span = area.minor_span.min(minors);
        if let Some(start) = area.minor {
            area.minor = Some(start.min(minors - area.minor_span));
        }
    }
    let mut taken = Occupancy::default();
    let mut positions = vec![None; areas.len()];
    // Step 1: items with a definite position in both axes.
    for (index, area) in areas.iter().enumerate() {
        if let (Some(major), Some(minor)) = (area.major, area.minor) {
            taken.mark(major, minor, area.major_span, area.minor_span);
            positions[index] = Some((major, minor));
        }
    }
    // Step 2: items locked to a major line; sparse packing keeps one cursor per line.
    let mut line_cursor = std::collections::BTreeMap::<usize, usize>::new();
    for (index, area) in areas.iter().enumerate() {
        let (Some(major), None) = (area.major, area.minor) else {
            continue;
        };
        let mut minor = if dense {
            0
        } else {
            line_cursor.get(&major).copied().unwrap_or(0)
        };
        while minor + area.minor_span <= minors
            && !taken.free(major, minor, area.major_span, area.minor_span)
        {
            context.charge(1)?;
            minor += 1;
        }
        let minor = minor.min(minors - area.minor_span);
        taken.mark(major, minor, area.major_span, area.minor_span);
        line_cursor.insert(major, minor + area.minor_span);
        positions[index] = Some((major, minor));
    }
    // Step 4: everything else with the auto-placement cursor.
    let (mut cursor_major, mut cursor_minor) = (0usize, 0usize);
    for (index, area) in areas.iter().enumerate() {
        if area.major.is_some() {
            continue;
        }
        context.charge(1)?;
        if dense {
            (cursor_major, cursor_minor) = (0, 0);
        }
        let spot = match area.minor {
            Some(minor) => {
                if !dense && minor < cursor_minor {
                    cursor_major += 1;
                }
                cursor_minor = minor;
                while !taken.free(cursor_major, minor, area.major_span, area.minor_span) {
                    context.charge(1)?;
                    cursor_major += 1;
                }
                (cursor_major, minor)
            }
            None => loop {
                if cursor_minor + area.minor_span > minors {
                    cursor_major += 1;
                    cursor_minor = 0;
                }
                if taken.free(cursor_major, cursor_minor, area.major_span, area.minor_span) {
                    break (cursor_major, cursor_minor);
                }
                context.charge(1)?;
                cursor_minor += 1;
            },
        };
        taken.mark(spot.0, spot.1, area.major_span, area.minor_span);
        if area.minor.is_none() {
            cursor_minor = spot.1 + area.minor_span;
        }
        positions[index] = Some(spot);
    }
    Ok(positions
        .into_iter()
        .map(Option::unwrap_or_default)
        .collect())
}

/**
 * An item's row and column lines: those of the named area it asks for, else its own. A name
 * the container does not define places the item automatically.
 */
fn lines(
    node: &Node,
    areas: &[GridArea],
    line_names: [&[(String, u16)]; 2],
) -> (GridLine, GridLine) {
    let style = &node.style;
    let axis = |index: usize, own: GridLine| {
        let Some(name) = style.grid_area_names[index].as_deref() else {
            return named_lines(&style.grid_line_names[index], line_names[index], own);
        };
        let Some(area) = areas.iter().find(|area| area.name == name) else {
            // No such area: the name may still name lines (`name-start` and `name-end`).
            let both = [Some(name.to_owned()), Some(name.to_owned())];
            return named_lines(&both, line_names[index], GridLine::default());
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
/** An item's named lines resolved against the container's: `name`, else `name-start` for a
 * start and `name-end` for an end, as CSS Grid matches implicit names. Notion's hero places
 * every region by names like `content-start / content-end`; unresolved names stay automatic. */
fn named_lines(names: &[Option<String>; 2], lines: &[(String, u16)], own: GridLine) -> GridLine {
    let find = |name: &str, suffix: &str| {
        let line = |wanted: &str| {
            lines
                .iter()
                .find(|(n, _)| n == wanted)
                .map(|(_, l)| i32::from(*l))
        };
        line(name).or_else(|| line(&format!("{name}{suffix}")))
    };
    let mut placed = own;
    if let Some(start) = names[0].as_deref().and_then(|name| find(name, "-start")) {
        placed.start = Some(start);
        if names[1].is_none() && placed.end.is_none() {
            placed.end = names[0].as_deref().and_then(|name| find(name, "-end"));
        }
    }
    if let Some(end) = names[1].as_deref().and_then(|name| find(name, "-end")) {
        placed.end = Some(end);
    }
    placed
}
/** The explicit row count: the template's `rows`, or more when a named area reaches past them. */
fn explicit_rows(style: &ComputedStyle, rows: usize) -> usize {
    let areas = style
        .grid_areas
        .iter()
        .map(|area| usize::from(area.row + area.rows));
    areas.max().unwrap_or(0).max(rows)
}
/** A 1-based line number as a zero-based line index; negative numbers count back from the end
 * of the `count` explicit tracks. */
fn line_index(n: i32, count: usize) -> usize {
    if n > 0 {
        (n - 1) as usize
    } else {
        let lines = count.saturating_add(1).min(GRID_COLUMNS + 1) as i32;
        (lines + n).max(0) as usize
    }
}
/**
 * An absolutely positioned child's `[row, column]` start and end line indexes, following Blink's
 * `ComputeOutOfFlowItemPlacement` (css-grid-2 9.1): there is no auto-placement, and an `auto`
 * line, or a span with nothing to count from, is None, which the caller takes as the padding
 * edge. `explicit` is the explicit `[row, column]` track count after auto-repeat.
 * ponytail: `span 1` reads as `auto` next to a line, since the style keeps no "span authored"
 * bit; keep one in GridLine if a page needs `2 / span 1` on an abspos child.
 */
pub(super) fn out_of_flow_lines(
    node: &Node,
    style: &ComputedStyle,
    [rows, columns]: [usize; 2],
) -> [(Option<usize>, Option<usize>); 2] {
    let line_names = [&style.grid_rows.names[..], &style.grid_columns.names[..]];
    let (row, column) = lines(node, &style.grid_areas, line_names);
    let axis = |line: GridLine, count: usize| {
        let span = usize::from(line.span.max(1));
        // A negative line before the first one does not exist, so it is `auto` too.
        let index = |n: i32| (n > 0 || count as i32 + 1 + n >= 0).then(|| line_index(n, count));
        match (line.start.and_then(index), line.end.and_then(index)) {
            (Some(a), Some(b)) => (Some(a.min(b)), Some(a.max(b).max(a.min(b) + 1))),
            (Some(start), None) if line.end.is_none() => {
                (Some(start), (span > 1).then(|| start + span))
            }
            (None, Some(end)) if line.start.is_none() => {
                ((span > 1).then(|| end.saturating_sub(span)), Some(end))
            }
            lines => lines,
        }
    };
    [axis(row, explicit_rows(style, rows)), axis(column, columns)]
}
/** Zero-based start (None for auto) and span of one axis, against `count` explicit tracks. */
fn resolve(line: GridLine, count: usize) -> (Option<usize>, usize) {
    let index = |n: i32| line_index(n, count);
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
/** A grid-lanes item's zero-based start (None when auto-placed) and span in the lane axis
 * (`axis` 0 for rows, 1 for columns) against `count` explicit lanes; the stacking axis has no
 * lines, so only this one resolves (css-grid-3 masonry placement). */
pub(super) fn lane_lines(
    node: &Node,
    style: &ComputedStyle,
    axis: usize,
    count: usize,
) -> (Option<usize>, usize) {
    let line_names = [&style.grid_rows.names[..], &style.grid_columns.names[..]];
    let (row, column) = lines(node, &style.grid_areas, line_names);
    resolve(if axis == 0 { row } else { column }, count)
}
