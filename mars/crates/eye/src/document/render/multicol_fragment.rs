use super::multicol::Columns;
use super::multicol_balance::{EPSILON, Row, Sizing, Unit, boundaries, column_height};
/**
 * Cuts a multicol container's content, laid out as one column with its spanners in place at
 * full width, into column rows separated by those spanners. Each part of the single column
 * (a column's strip, or a spanner's extent) becomes a `Piece` moved to where it shows; a box
 * cut by a piece boundary is sliced (`box-decoration-break: slice`): a copy per piece, clipped
 * to it, carrying only that piece's lines.
 */
use super::multicol_cut::Cutter;
use super::multicol_units::{anchors, forced, units};
use super::sizing::LayoutContext;
use crate::document::{DisplayItem, Node};

/** The most wrapped rows one column row makes, bounding a pathological column height. */
const MAX_ROWS: usize = 1000;
/** Where the columns go. */
pub(super) struct Shape {
    /// The content top, where the first row starts.
    pub top: f64,
    /// The used column count, width and gap.
    pub columns: Columns,
    /// The container's content-box left edge.
    pub content_left: f64,
    /// The container's content-box width.
    pub content_width: f64,
    /// The container's definite content height, which bounds the columns.
    pub available: Option<f64>,
    /// `column-fill: auto`: the last row fills its columns in turn at the available height.
    pub fill: bool,
    /// css-multicol-2 `column-height`, which fixes every column's height.
    pub fixed: Option<f64>,
    /// Columns past the count wrap into new rows (`column-wrap`).
    pub wrap: bool,
    /// The gap between wrapped rows; `normal` is 1em.
    pub row_gap: f64,
    /// Inside an enclosing multicol with known column heights: (the first row's height, every
    /// later row's), rows following the enclosing columns with no gap.
    pub outer: Option<(f64, f64)>,
}

/** How wrapped rows are sized. */
#[derive(Clone, Copy)]
struct Plan {
    /// The first row's column height; None sizes it to its tallest column.
    first: Option<f64>,
    /// Every later row's column height.
    rest: Option<f64>,
    /// The gap between rows.
    gap: f64,
    /// The last row shrinks to its tallest column instead of keeping its height.
    shrink_last: bool,
}

/** A placed column row: where it starts, its column height and how many columns it used. */
#[derive(Clone, Copy)]
pub(super) struct Placed {
    /// The row's top.
    pub top: f64,
    /// The column height.
    pub height: f64,
    /// Columns that hold content.
    pub used: usize,
}

/** A part of the single column and where it shows. */
#[derive(Clone, Copy, Debug)]
pub(super) struct Piece {
    /// Its first position in the single column.
    pub start: f64,
    /// Horizontal move.
    pub dx: f64,
    /// Where `start` lands.
    pub top: f64,
    /// How tall a slice's clip is: the column height for a column broken inside a box.
    pub clip: f64,
    /// The spanner a spanner piece holds; only it and its descendants show there.
    pub spanner: Option<usize>,
}

/** A spanner's extent in the single column, margins included. */
struct Spanner {
    /// Its node.
    local: usize,
    /// Its top margin edge.
    top: f64,
    /// Its bottom margin edge.
    bottom: f64,
    /// Its bottom border edge, below which nothing is inside it.
    border_bottom: f64,
}

/** Cuts `items[start..]` and the container's own lines (`items[owner]`) into rows of columns,
moving the out-of-flow `statics` positions with them; the placed rows and the content height. */
pub(super) fn fragment(
    context: &LayoutContext<'_>,
    container: &Node,
    items: &mut Vec<DisplayItem>,
    (start, owner): (usize, usize),
    (shape, statics): (&Shape, &mut [super::multicol_oof::Static]),
) -> (Vec<Placed>, f64) {
    let anchors = anchors(context, container, &items[start..]);
    let units = units(context, items, (start, owner), &anchors);
    let forced = forced(context, &items[start..]);
    let spanners = spanners(context, container, &items[start..]);
    let mut layout = Layout {
        pieces: Vec::new(),
        rows: Vec::new(),
        at: shape.top,
        placed: shape.top,
    };
    for spanner in &spanners {
        layout.row(
            items,
            (start, &units, &forced, &anchors),
            spanner.top,
            shape,
            false,
        );
        layout.span(spanner);
    }
    layout.row(
        items,
        (start, &units, &forced, &anchors),
        f64::INFINITY,
        shape,
        true,
    );
    let height = layout.placed - shape.top;
    let cutter = Cutter::new(context, &layout.pieces);
    let tail = items.split_off(start);
    for (item, anchor) in tail.into_iter().zip(anchors) {
        items.extend(cutter.cut(item, anchor));
    }
    cutter.move_lines(&mut items[owner]);
    for (_, (x, y)) in statics.iter_mut() {
        let (dx, dy) = cutter.shift(cutter.of(*y));
        (*x, *y) = (*x + dx, *y + dy);
    }
    (layout.rows, height)
}

/** Rows and spanners placed so far. */
struct Layout {
    /// Every piece, in single-column order.
    pieces: Vec<Piece>,
    /// Placed rows.
    rows: Vec<Placed>,
    /// The single-column position laid out so far.
    at: f64,
    /// The final position reached so far.
    placed: f64,
}
impl Layout {
    /** Places the row from `self.at` to `until` as columns. */
    fn row(
        &mut self,
        items: &[DisplayItem],
        (start, units, forced, anchors): (usize, &[Unit], &[f64], &[Option<f64>]),
        until: f64,
        shape: &Shape,
        last: bool,
    ) {
        let (top, until) = (self.at, until.max(self.at));
        let inside = |y: f64| y >= top - EPSILON && y < until;
        let units: Vec<Unit> = units.iter().copied().filter(|u| inside(u.top)).collect();
        let forced: Vec<f64> = forced.iter().copied().filter(|y| inside(*y)).collect();
        let free: Vec<&DisplayItem> = (items[start..].iter().zip(anchors))
            .filter_map(|(item, anchor)| anchor.is_none().then_some(item))
            .collect();
        let end = content_end(&free, (top, until), &units);
        let boxes = free.iter().filter(|item| item.height > 0.0);
        let starts: Vec<f64> = (boxes.map(|item| item.y))
            .chain(units.iter().map(|unit| unit.top))
            .filter(|y| inside(*y))
            .collect();
        let row = Row {
            units: &units,
            forced: &forced,
            starts: &starts,
            top,
            end,
        };
        if let Some((first, rest)) = shape.outer {
            let plan = Plan {
                first: Some(first),
                rest: Some(rest),
                gap: 0.0,
                shrink_last: true,
            };
            self.wrapped(row, shape, plan);
            self.at = until;
            return;
        }
        if shape.wrap {
            let fixed = shape.fixed.or(shape.available);
            let plan = Plan {
                first: fixed,
                rest: fixed,
                gap: shape.row_gap,
                shrink_last: false,
            };
            self.wrapped(row, shape, plan);
            self.at = until;
            return;
        }
        let used_height = self.placed - shape.top;
        let available = shape
            .fixed
            .or(shape.available.map(|h| (h - used_height).max(0.0)));
        let sizing = Sizing {
            count: shape.columns.count,
            available,
            fill: (last && shape.fill || shape.fixed.is_some()) && available.is_some(),
        };
        let height = if end > top {
            column_height(&row, &sizing)
        } else {
            0.0
        };
        let (bounds, _) = boundaries(&row, height.max(EPSILON));
        // The last row's columns reach the content box's end when its height is definite.
        let height = match available.filter(|_| last && end > top) {
            Some(rest) => height.max(rest),
            None => height,
        };
        self.columns(&bounds, shape, height, true);
        self.at = until;
    }
    /** css-multicol-2 `column-wrap: wrap`: rows of N columns stacked with the row gap, each
    `column-height` tall, else the container's definite height, else as tall as its tallest
    column when only forced breaks end columns. */
    fn wrapped(&mut self, row: Row<'_>, shape: &Shape, plan: Plan) {
        if row.end <= row.top {
            return self.columns(&[row.top, row.top], shape, 0.0, true);
        }
        let mut top = row.top;
        for index in 0..=MAX_ROWS {
            let fixed = if index == 0 { plan.first } else { plan.rest };
            let (columns, height, done) = wrap_row(Row { top, ..row }, shape, fixed, plan);
            self.columns(&columns, shape, height, done);
            if done {
                return;
            }
            self.placed += plan.gap;
            top = columns[columns.len() - 1];
        }
    }
    /** Adds the pieces of one row's columns and records the row; a slice in the last column
    ends with its content only when nothing follows in another row. */
    fn columns(&mut self, bounds: &[f64], shape: &Shape, height: f64, last: bool) {
        let count = bounds.len() - 1;
        for k in 0..count {
            let (start, end) = (bounds[k], bounds[k + 1]);
            let clip = if k + 1 < count || !last {
                height
            } else {
                end - start
            };
            self.pieces.push(Piece {
                start,
                dx: shape.columns.offset(k, shape.content_width),
                top: self.placed,
                clip,
                spanner: None,
            });
        }
        let used = if height > 0.0 { count } else { 0 };
        self.rows.push(Placed {
            top: self.placed,
            height,
            used,
        });
        self.placed += height;
    }
    /** Adds a spanner's piece below the row before it. */
    fn span(&mut self, spanner: &Spanner) {
        let length = spanner.bottom - spanner.top;
        self.pieces.push(Piece {
            start: spanner.top,
            dx: 0.0,
            top: self.placed,
            clip: length,
            spanner: Some(spanner.local),
        });
        self.placed += length;
        self.at = spanner.bottom;
    }
}

/** One wrapped row from `rest.top` on: its column boundaries, its column height and whether
it is the last row. The last row balances under `column-fill: balance`, else may shrink. */
fn wrap_row(rest: Row<'_>, shape: &Shape, fixed: Option<f64>, plan: Plan) -> (Vec<f64>, f64, bool) {
    let count = shape.columns.count.max(1);
    let mut height = fixed.unwrap_or(f64::INFINITY).max(EPSILON);
    let (mut bounds, _) = boundaries(&rest, height);
    let last = bounds.len() - 1 <= count;
    if last && let Some(balanced) = last_row_balance(&rest, shape, fixed) {
        height = balanced;
        bounds = boundaries(&rest, balanced).0;
    }
    let taken = (bounds.len() - 1).min(count);
    bounds.truncate(taken + 1);
    let tallest = bounds
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .fold(0.0, f64::max);
    if !height.is_finite() || last && plan.shrink_last && shape.fill {
        height = tallest;
    }
    (bounds, height, last)
}

/** The balanced column height of the last wrapped row, which `column-fill: balance`
balances within the fixed row height; None when the row fills. */
fn last_row_balance(rest: &Row<'_>, shape: &Shape, fixed: Option<f64>) -> Option<f64> {
    let fixed = fixed.filter(|_| !shape.fill)?;
    let sizing = Sizing {
        count: shape.columns.count,
        available: Some(fixed),
        fill: false,
    };
    Some(column_height(rest, &sizing).max(EPSILON))
}

/** The bottom of the row's content between `top` and `until` (trailing margins truncate at a
column end); `top` when the row holds nothing. */
fn content_end(items: &[&DisplayItem], (top, until): (f64, f64), units: &[Unit]) -> f64 {
    let boxes = items
        .iter()
        .filter(|item| item.y < until && item.y + item.height > top)
        .map(|item| (item.y + item.height).min(until));
    let lines = units.iter().map(|unit| unit.bottom);
    boxes.chain(lines).fold(top, f64::max)
}

/** The container's spanners, outermost only, in order. */
fn spanners(context: &LayoutContext<'_>, container: &Node, items: &[DisplayItem]) -> Vec<Spanner> {
    let mut found: Vec<Spanner> = Vec::new();
    for item in items.iter().filter(|item| !item.tag.starts_with("::")) {
        let Ok(node) = context.node(item.local_id) else {
            continue;
        };
        let inside = found.last().is_some_and(|last| item.y < last.border_bottom);
        if inside || super::multicol::spanned(context, node) != Some(container.local_id) {
            continue;
        }
        // Margins between adjacent spanners collapsed in the single column; the earlier
        // spanner's piece keeps the shared part.
        let after = found.last().map_or(f64::NEG_INFINITY, |last| last.bottom);
        let border_bottom = item.y + item.height;
        found.push(Spanner {
            local: node.local_id,
            top: (item.y - node.style.margin[0]).max(after),
            bottom: (border_bottom + node.style.margin[2]).max(after),
            border_bottom,
        });
    }
    found
}
