/**
 * Where one column row breaks into columns (css-break-3 inside a multicol container). Lines and
 * monolithic boxes are the unbreakable units; anything else may break anywhere between them,
 * and a forced break (`break-before`/`break-after: column`) always breaks. Balancing searches
 * the shortest column height that fits the row in N columns, growing by the minimal space
 * shortage as Blink's column balancer does.
 * ponytail: no orphans, widows or `break-*: avoid` between siblings; add them as penalties.
 */
/** Slack under which two edges count as the same position, absorbing float rounding. */
pub(super) const EPSILON: f64 = 0.01;
/** The fewest pixels a column grows by per balancing pass, so balancing always ends. */
const MIN_GROWTH: f64 = 1.0 / 64.0;
/** The shortest column, so a zero available height still makes progress. */
const MIN_COLUMN_HEIGHT: f64 = 1.0;
/** The most columns a row makes, after Blink's kMaxNumberOfColumns guard. */
const MAX_COLUMNS: usize = 1000;
/** The most balancing passes; each grows the columns by at least one shortage. */
const MAX_BALANCE_PASSES: usize = 256;

/** An unbreakable extent in the single-column layout. */
#[derive(Clone, Copy, Debug)]
pub(super) struct Unit {
    /// Its top.
    pub top: f64,
    /// Its bottom.
    pub bottom: f64,
}

/** One row's break inputs: its units and forced breaks, and the extent it covers. */
#[derive(Clone, Copy)]
pub(super) struct Row<'a> {
    /// The unbreakable units in the row.
    pub units: &'a [Unit],
    /// Positions that must break, ascending.
    pub forced: &'a [f64],
    /// Where content starts: the tops of the row's boxes and units.
    pub starts: &'a [f64],
    /// The row's first position in the single column.
    pub top: f64,
    /// Where the row's content ends.
    pub end: f64,
}

/** How a row's column height is chosen. */
pub(super) struct Sizing {
    /// N, the column count balancing aims for.
    pub count: usize,
    /// The definite column height, when the container's height bounds it.
    pub available: Option<f64>,
    /// Fill columns in turn at the available height instead of balancing.
    pub fill: bool,
}

/** The used column height: the available height when filling, else the balanced height,
bounded by the available height. */
pub(super) fn column_height(row: &Row<'_>, sizing: &Sizing) -> f64 {
    let bound = sizing.available.map(|h| h.max(MIN_COLUMN_HEIGHT));
    if sizing.fill
        && let Some(bound) = bound
    {
        return bound;
    }
    let count = sizing.count.max(1);
    let tallest = row
        .units
        .iter()
        .map(|u| u.bottom - u.top)
        .fold(0.0, f64::max);
    let mut height = ((row.end - row.top) / count as f64)
        .max(tallest)
        .max(MIN_COLUMN_HEIGHT);
    for _ in 0..MAX_BALANCE_PASSES {
        if bound.is_some_and(|bound| height >= bound) {
            break;
        }
        let (bounds, shortage) = boundaries(row, height);
        if bounds.len() - 1 <= count && !overflows(&bounds, height) {
            break;
        }
        height += shortage.max(MIN_GROWTH);
    }
    bound.map_or(height, |bound| height.min(bound))
}

/** Whether a column holds more than the column height: chained unbreakable content pushed
past its end, which balancing grows the columns to fit. */
fn overflows(bounds: &[f64], height: f64) -> bool {
    bounds
        .windows(2)
        .any(|pair| pair[1] - pair[0] > height + EPSILON)
}

/** The column boundaries a column height gives, first the row's top and last its end, with
the smallest shortage that would have let a column take more. */
pub(super) fn boundaries(row: &Row<'_>, height: f64) -> (Vec<f64>, f64) {
    let mut bounds = vec![row.top];
    let mut shortage = f64::INFINITY;
    let mut current = row.top;
    while bounds.len() <= MAX_COLUMNS {
        let forced = row
            .forced
            .iter()
            .copied()
            .find(|at| *at > current + EPSILON && *at < row.end - EPSILON);
        let limit = current + height;
        if forced.is_none_or(|at| at > limit + EPSILON) && row.end - current <= height + EPSILON {
            break;
        }
        let next = match forced.filter(|at| *at <= limit + EPSILON) {
            Some(at) => at,
            None => {
                let short = straddling(row.units, limit).map(|u| u.bottom - limit);
                shortage = short.fold(shortage, f64::min);
                let next = next_break(row, current, limit);
                if next > limit + EPSILON {
                    shortage = shortage.min(next - limit);
                }
                next
            }
        };
        current = next.max(current + MIN_GROWTH);
        bounds.push(current);
    }
    bounds.push(row.end.max(current));
    (
        bounds,
        if shortage.is_finite() {
            shortage
        } else {
            MIN_GROWTH
        },
    )
}

/** Units cut by a break at `at`. */
fn straddling(units: &[Unit], at: f64) -> impl Iterator<Item = &Unit> {
    units
        .iter()
        .filter(move |u| u.top + EPSILON < at && at < u.bottom - EPSILON)
}

/** The last break at or above `limit` that cuts no unit, moved up to the top of a unit that
would be cut; past a unit taller than the column when nothing else fits. A break with no unit
before it in the column (only margins and empty space) is no valid break either, so a first
line after a top margin stays in the first column. */
fn next_break(row: &Row<'_>, start: f64, limit: f64) -> f64 {
    let mut at = limit;
    while let Some(top) = straddling(row.units, at).map(|u| u.top).reduce(f64::min) {
        let content = row
            .starts
            .iter()
            .any(|y| *y >= start - EPSILON && *y < top - EPSILON);
        if !content {
            return past_monolithic(row.units, limit);
        }
        at = top;
    }
    at
}

/** A break after every unit cut at `limit`: monolithic content overflows its column. */
fn past_monolithic(units: &[Unit], limit: f64) -> f64 {
    let mut at = limit;
    while let Some(bottom) = straddling(units, at).map(|u| u.bottom).reduce(f64::max) {
        at = bottom;
    }
    at
}
