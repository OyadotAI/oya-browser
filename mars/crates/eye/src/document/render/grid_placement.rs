/** CSS grid item placement (css-grid-2 section 8.5), following Blink's GridPlacement: resolve
 * each item's lines, place items fixed in both axes, then items locked to a row (or a column for
 * `grid-auto-flow: column`), then everything else with the auto-placement cursor, sparse or
 * dense. Implicit tracks take their sizes from `grid-auto-rows` and `grid-auto-columns`. */
use super::super::flow::*;
use super::GRID_COLUMNS;
use crate::document::ComputedStyle;
use crate::document::style::{
    ContentKeyword, GridArea, GridLine, GridTemplate, LineName, Track, TrackContent, TrackSize,
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
    let row_names = axis_names(style, 0, rows);
    let column_names = axis_names(style, 1, explicit_columns);
    let line_names = [&row_names[..], &column_names[..]];
    let column_flow = style.grid_flow_column;
    let mut resolved = Vec::with_capacity(kids.len());
    for node in kids {
        let explicit = [explicit_rows, explicit_columns];
        let (mut row_line, mut column_line) = lines(node, areas, line_names, explicit);
        for (axis, line) in [(0, &mut row_line), (1, &mut column_line)] {
            subgrid_span(node, axis, line);
        }
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
    let mut minors = minors.clamp(1, GRID_COLUMNS);
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
        // Past the last minor line the grid grows implicit tracks (Blink's
        // `PlaceGridItemsLockedToMajorAxis`), up to the track bound.
        let last = GRID_COLUMNS.saturating_sub(area.minor_span);
        while minor < last && !taken.free(major, minor, area.major_span, area.minor_span) {
            context.charge(1)?;
            minor += 1;
        }
        minors = minors.max(minor + area.minor_span).min(GRID_COLUMNS);
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
 * An item's row and column lines: those of the named area it asks for, else its own, with
 * named lines resolved against the `explicit` `[row, column]` track counts.
 */
fn lines(
    node: &Node,
    areas: &[GridArea],
    line_names: [&[(String, u16)]; 2],
    explicit: [usize; 2],
) -> (GridLine, GridLine) {
    let style = &node.style;
    let axis = |index: usize, own: GridLine| {
        let names = line_names[index];
        let Some(name) = style.grid_area_names[index].as_deref() else {
            let sides = &style.grid_line_names[index];
            return named_lines(sides, names, own, explicit[index]);
        };
        let Some(area) = areas.iter().find(|area| area.name == name) else {
            // No such area: the name may still name lines (`name-start` and `name-end`).
            let line = LineName {
                name: name.to_owned(),
                nth: 0,
                span: false,
            };
            let both = [Some(line.clone()), Some(line)];
            return named_lines(&both, names, GridLine::default(), explicit[index]);
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
/**
 * An item's named lines resolved against the container's `lines` in an axis of `count`
 * explicit tracks, after Blink's `GridLineResolver`: a bare `name` is `name-start` (or
 * `name-end`), else the first `name`, else the first implicit line; `name n` counts lines of
 * that name with every implicit line taken to carry it; `span name` counts from the opposite
 * line once that is definite, and is a span of 1 otherwise (css-grid-2 8.3). A subgrid then
 * clamps what lands past its edges.
 */
fn named_lines(
    sides: &[Option<LineName>; 2],
    lines: &[(String, u16)],
    own: GridLine,
    count: usize,
) -> GridLine {
    let last = i32::try_from(count).unwrap_or(i32::MAX - 1) + 1;
    let [start, end] = sides;
    let mut placed = own;
    if let Some(line) = start.as_ref().filter(|line| !line.span) {
        placed.start = Some(named_line(line, "-start", lines, last));
        if end.is_none() && placed.end.is_none() && line.nth == 0 {
            let name = format!("{}-end", line.name);
            placed.end = named(lines, &name, last).first().copied();
        }
    }
    if let Some(line) = end.as_ref().filter(|line| !line.span) {
        placed.end = Some(named_line(line, "-end", lines, last));
    }
    // A negative numeric line counts back from the last explicit one.
    let positive = |n: i32| if n > 0 { n } else { (last + 1 + n).max(1) };
    if let (Some(line), Some(opposite)) = (start.as_ref().filter(|l| l.span), placed.end) {
        placed.start = Some(named_span(line, positive(opposite), true, lines, last));
    }
    if let (Some(line), Some(opposite)) = (end.as_ref().filter(|l| l.span), placed.start) {
        placed.end = Some(named_span(line, positive(opposite), false, lines, last));
    }
    placed
}
/** The explicit lines, at most `last`, that carry `name`, ascending. */
fn named(lines: &[(String, u16)], name: &str, last: i32) -> Vec<i32> {
    let mut found = lines
        .iter()
        .filter(|(line_name, _)| line_name == name)
        .map(|(_, line)| i32::from(*line))
        .filter(|line| *line <= last)
        .collect::<Vec<_>>();
    found.sort_unstable();
    found.dedup();
    found
}
/** A definite named side, `last` being the last explicit line (Blink's `ResolveGridPosition`
 * and `ResolveNamedGridLinePosition`). */
fn named_line(line: &LineName, suffix: &str, lines: &[(String, u16)], last: i32) -> i32 {
    if line.nth == 0 {
        let first = |name: &str| named(lines, name, last).first().copied();
        return first(&format!("{}{suffix}", line.name))
            .or_else(|| first(&line.name))
            .unwrap_or(last + 1);
    }
    let found = named(lines, &line.name, last);
    let (have, wanted) = (found.len() as i32, line.nth.abs());
    match (line.nth > 0, wanted <= have) {
        (true, true) => found[(wanted - 1) as usize],
        (true, false) => last + wanted - have,
        (false, true) => found[(have - wanted) as usize],
        // ponytail: lines before the explicit grid would add implicit tracks there, which this
        // grid never makes; they clamp to its first line.
        (false, false) => 1,
    }
}
/** The line `span name` reaches from the definite `opposite` line, backwards for a start side
 * (Blink's `LookBackForNamedGridLine` and `LookAheadForNamedGridLine`). */
fn named_span(
    line: &LineName,
    opposite: i32,
    start_side: bool,
    lines: &[(String, u16)],
    last: i32,
) -> i32 {
    let found = named(lines, &line.name, last);
    let wanted = line.nth.max(1) as usize;
    if start_side {
        let from = (opposite - 1).min(last);
        let before = found
            .iter()
            .rev()
            .filter(|line| **line <= from)
            .collect::<Vec<_>>();
        let missing = wanted.saturating_sub(before.len()) as i32;
        before
            .get(wanted - 1)
            .map_or(1 - missing, |line| **line)
            .max(1)
    } else {
        let after = found
            .iter()
            .filter(|line| **line > opposite)
            .collect::<Vec<_>>();
        let missing = wanted.saturating_sub(after.len()) as i32;
        after
            .get(wanted - 1)
            .map_or(opposite.max(last) + missing, |line| **line)
    }
}
/** The explicit row count: the template's `rows`, or more when a named area reaches past them. */
fn explicit_rows(style: &ComputedStyle, rows: usize) -> usize {
    let areas = style
        .grid_areas
        .iter()
        .map(|area| usize::from(area.row + area.rows));
    areas.max().unwrap_or(0).max(rows)
}
/** An axis's line names (0 rows, 1 columns) for `count` tracks, then the implicit ones every
 * named area gives its edges, `name-start` and `name-end` (css-grid-2 7.3.2). Explicit names
 * come first, so a name on both kinds of line finds the explicit one first. */
fn axis_names(style: &ComputedStyle, axis: usize, count: usize) -> Vec<(String, u16)> {
    let template = if axis == 0 {
        &style.grid_rows
    } else {
        &style.grid_columns
    };
    let mut names = template.line_names(count);
    for area in style.grid_areas.iter().filter(|area| !area.name.is_empty()) {
        let (start, span) = if axis == 0 {
            (area.row, area.rows)
        } else {
            (area.column, area.columns)
        };
        names.push((format!("{}-start", area.name), start + 1));
        names.push((format!("{}-end", area.name), start + span + 1));
    }
    names
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
    subgrid: bool,
) -> [(Option<usize>, Option<usize>); 2] {
    let names = [axis_names(style, 0, rows), axis_names(style, 1, columns)];
    let line_names = [&names[0][..], &names[1][..]];
    // A template's areas extend the explicit grid, except a subgrid's, fixed at its span.
    let named = style
        .grid_areas
        .iter()
        .map(|a| usize::from(a.column + a.columns));
    let explicit = if subgrid {
        [rows, columns]
    } else {
        [explicit_rows(style, rows), named.fold(columns, usize::max)]
    };
    let (row, column) = lines(node, &style.grid_areas, line_names, explicit);
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
    [axis(row, explicit[0]), axis(column, explicit[1])]
}
/** An auto-spanned subgrid's span in a subgridded axis follows its line-name lists
 * (css-grid-2 9.1); with both lines given the span is theirs. */
fn subgrid_span(node: &Node, axis: usize, line: &mut GridLine) {
    if line.span <= 1
        && (line.start.is_none() || line.end.is_none())
        && let Some(span) = super::subgrid::auto_span(node, axis)
    {
        line.span = span;
    }
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
    let names = [axis_names(style, 0, count), axis_names(style, 1, count)];
    let line_names = [&names[0][..], &names[1][..]];
    let (row, column) = lines(node, &style.grid_areas, line_names, [count, count]);
    resolve(if axis == 0 { row } else { column }, count)
}
