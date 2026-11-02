/** Table column widths, a port of Blink's table_layout_utils.cc (css-tables-3 "computing column
 * measures" and "width distribution"): cell constraints merge into column constraints, colspan
 * cells spread over their columns with the same distribution, percentages clamp to 100%, the
 * grid's min/max widths account for percentages, and the assignable width is handed out through
 * the min / percentage / specified / max guesses. `table-layout: fixed` uses the first row only
 * and Blink's fixed synchronisation.
 *
 * ponytail: merged columns only trim trailing `<col>`s; a column only spanned is sized like any. */
/** One cell's inline constraint, border box: Blink's CellInlineConstraint. */
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Cell {
    pub min: f64,
    pub max: f64,
    pub percent: Option<f64>,
    /** An authored length width. */
    pub constrained: bool,
    /** Border and padding a fixed table adds to a content-box percentage (Blink's
     * percent_border_padding); zero in auto tables. */
    pub percent_edges: f64,
}
/** One column's constraint after every cell is applied: Blink's TableTypes::Column. */
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Column {
    pub min: f64,
    pub max: f64,
    pub percent: Option<f64>,
    pub constrained: bool,
    /** The percentage's cell border and padding, added when it resolves. */
    percent_edges: f64,
    /** Whether any cell reached this column yet. */
    seen: bool,
}
impl Column {
    /** Blink's TableTypes::CreateColumn for a `<col>` or `<colgroup>`: min-width is its min, an
     * authored length (raised by min-width) its max and constraint, a nonzero percentage its
     * percentage. */
    pub(super) fn element(min: f64, length: Option<f64>, percent: Option<f64>) -> Self {
        let length = length.map(|length| length.max(min));
        Self {
            min,
            max: length.unwrap_or(0.0),
            percent: percent.filter(|percent| *percent != 0.0),
            constrained: length.is_some(),
            percent_edges: 0.0,
            // A width-less column still takes a spanning cell's share, as Blink's unset max does.
            seen: length.is_some() || min > 0.0,
        }
    }
    /** Blink's is_mergeable: in an auto table a column element with no width or percentage
     * adds no column of its own past the cells. */
    pub(super) fn mergeable(&self, fixed_table: bool) -> bool {
        !fixed_table && self.max == 0.0 && self.percent.is_none()
    }
    /** Blink's Column::Encompass: a column grows to hold a single-column cell. */
    fn encompass(&mut self, cell: Cell, fixed_table: bool) {
        if self.constrained && fixed_table {
            return;
        }
        if self.seen {
            self.min = self.min.max(cell.min);
            self.max = if self.constrained {
                self.max
                    .max(if cell.constrained { cell.max } else { cell.min })
            } else {
                self.max.max(cell.max)
            };
        } else {
            (self.min, self.max, self.seen) = (cell.min, cell.max, true);
        }
        self.max = self.max.max(self.min);
        if cell.percent > self.percent {
            self.percent = cell.percent;
            self.percent_edges = cell.percent_edges;
        }
        self.constrained |= cell.constrained;
    }
    /** Blink's ResolvePercentInlineSize: the percentage of the target plus its cell edges. */
    fn percent_size(&self, target: f64) -> f64 {
        self.min
            .max(self.percent.unwrap_or(0.0) * target / 100.0 + self.percent_edges)
    }
    /** A column with an authored length and no percentage. */
    fn is_fixed(&self) -> bool {
        self.constrained && self.percent.is_none()
    }
}

/** Column constraints from single cells `(column, cell)` and spanning cells
 * `(start, span, cell)`, in Blink's order: singles, spans by length then start, percent clamp. */
pub(super) fn constraints(
    elements: &[Column],
    count: usize,
    singles: &[(usize, Cell)],
    spans: &[(usize, usize, Cell)],
    spacing: f64,
    fixed: bool,
) -> Vec<Column> {
    let mut columns = elements.to_vec();
    columns.resize(count, Column::default());
    for (index, cell) in singles {
        columns[*index].encompass(*cell, fixed);
    }
    let mut spans = spans.to_vec();
    spans.sort_by_key(|(start, span, _)| (*span, *start));
    for (start, span, cell) in spans {
        let end = (start + span).min(count);
        let span = &mut columns[start..end];
        if fixed {
            spread_fixed(span, cell, spacing);
        } else {
            spread_auto(span, cell, spacing);
        }
    }
    clamp_percentages(&mut columns, fixed);
    columns
}
/** Auto tables: column percentages past a running total of 100% are cut; fixed tables scale
 * every percentage so they total 100%. */
fn clamp_percentages(columns: &mut [Column], fixed: bool) {
    let mut total = 0.0;
    for column in columns.iter_mut() {
        if let Some(percent) = column.percent {
            let percent = if !fixed && percent + total > 100.0 {
                100.0 - total
            } else {
                percent
            };
            column.percent = Some(percent);
            total += percent;
        }
    }
    if fixed && total > 100.0 {
        for column in columns.iter_mut() {
            column.percent = column.percent.map(|percent| percent * 100.0 / total);
        }
    }
}
/** Blink's DistributeColspanCellToColumnsAuto: a spanning cell's surplus percentage goes to the
 * non-percent columns by max width, then its min and max widths, less the inner spacing, are
 * distributed like a table width and each column keeps the larger. */
fn spread_auto(span: &mut [Column], cell: Cell, spacing: f64) {
    let inner = spacing * span.len().saturating_sub(1) as f64;
    let (min, max) = ((cell.min - inner).max(0.0), (cell.max - inner).max(0.0));
    if let Some(percent) = cell.percent {
        let have = span.iter().filter_map(|c| c.percent).sum::<f64>();
        let others = span.iter().filter(|c| c.percent.is_none());
        let (count, basis) = others.fold((0, 0.0), |(n, sum), c| (n + 1, sum + c.max));
        let surplus = percent - have;
        if surplus > 0.0 && count > 0 {
            for column in span.iter_mut().filter(|c| c.percent.is_none()) {
                column.percent = Some(if basis > 0.0 {
                    surplus * column.max / basis
                } else {
                    surplus / count as f64
                });
            }
        }
    }
    let sizes = distribute_auto(min, span, true);
    for (column, size) in span.iter_mut().zip(sizes) {
        column.min = column.min.max(size);
    }
    let sizes = distribute_auto(max, span, cell.constrained);
    for (column, size) in span.iter_mut().zip(sizes) {
        column.max = column.max.max(column.min).max(size);
        column.seen = true;
    }
}
/** Blink's DistributeColspanCellToColumnsFixed: an even split into columns no cell sized yet. */
fn spread_fixed(span: &mut [Column], cell: Cell, spacing: f64) {
    let inner = spacing * span.len().saturating_sub(1) as f64;
    let count = span.len().max(1) as f64;
    let min = if cell.constrained {
        (cell.min - inner).max(0.0)
    } else {
        0.0
    };
    let max = (cell.max - inner).max(0.0);
    for column in span.iter_mut() {
        if !column.seen {
            column.constrained |= cell.constrained;
            (column.min, column.max, column.seen) = (min / count, max / count, true);
        }
        if column.percent.is_none() && !column.constrained {
            column.percent = cell.percent.map(|percent| percent / count);
        }
    }
}

/** Blink's ComputeGridInlineMinMax without the undistributable spacing and edges: the column
 * sums, with the max widened so percentage columns can reach their share. */
pub(super) fn grid_min_max(columns: &[Column], fixed: bool) -> (f64, f64) {
    let (mut min, mut max, mut estimate, mut others, mut percents) = (0.0, 0.0, 0.0_f64, 0.0, 0.0);
    for column in columns {
        min += if fixed && column.is_fixed() {
            column.max
        } else {
            column.min
        };
        match column.percent.filter(|percent| *percent > 0.0) {
            Some(percent) if column.max > 0.0 => {
                estimate = estimate.max(100.0 / percent * (column.max - column.percent_edges))
            }
            Some(_) => {}
            None => others += column.max,
        }
        max += column.max;
        percents += column.percent.unwrap_or(0.0);
    }
    let percents = f64::min(percents, 100.0);
    if percents > 0.0 {
        if others > 0.0 {
            max = f64::max(
                max,
                if percents >= 100.0 {
                    TABLE_MAX_INLINE
                } else {
                    100.0 / (100.0 - percents) * others
                },
            );
        }
        max = max.max(estimate);
    }
    (min, max.max(min))
}
/** Blink's kTableMaxInlineSize: the max width of a table whose percentages total 100%. */
const TABLE_MAX_INLINE: f64 = 1_000_000.0;

/** Column widths for an assignable width (Blink's SynchronizeAssignableTableInlineSizeAndColumns). */
pub(super) fn widths(target: f64, columns: &[Column], fixed: bool) -> Vec<f64> {
    if fixed {
        distribute_fixed(target, columns)
    } else {
        distribute_auto(target, columns, true)
    }
}

/** The css-tables-3 guesses, in order. */
#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Guess {
    Min,
    Percentage,
    Specified,
    Max,
    AboveMax,
}
/** Blink's DistributeInlineSizeToComputedInlineSizeAuto: find the first guess at least as wide
 * as the target and grow the columns that guess grows, by their share of its increase. */
fn distribute_auto(target: f64, columns: &[Column], constrained: bool) -> Vec<f64> {
    let mut sizes = [0.0; 4];
    let mut increases = [0.0; 4];
    let (mut auto_max, mut fixed_max, mut percent_total) = (0.0, 0.0, 0.0);
    let (mut autos, mut fixeds, mut percents) = (0, 0, 0);
    for column in columns {
        if column.percent.is_some() {
            let size = column.percent_size(target);
            sizes = [
                sizes[0] + column.min,
                sizes[1] + size,
                sizes[2] + size,
                sizes[3] + size,
            ];
            increases[1] += size - column.min;
            percents += 1;
            percent_total += column.percent.unwrap_or(0.0);
        } else if column.constrained {
            sizes = [
                sizes[0] + column.min,
                sizes[1] + column.min,
                sizes[2] + column.max,
                sizes[3] + column.max,
            ];
            increases[2] += column.max - column.min;
            fixeds += 1;
            fixed_max += column.max;
        } else {
            sizes = [
                sizes[0] + column.min,
                sizes[1] + column.min,
                sizes[2] + column.min,
                sizes[3] + column.max,
            ];
            increases[3] += column.max - column.min;
            autos += 1;
            auto_max += column.max;
        }
    }
    let target = target.max(sizes[0]);
    let guesses = [Guess::Min, Guess::Percentage, Guess::Specified, Guess::Max];
    let guess = guesses
        .into_iter()
        .zip(sizes)
        .find(|(_, size)| *size >= target)
        .map_or(Guess::AboveMax, |(guess, _)| guess);
    let grow = |column: &Column, kind: Guess| match kind {
        Guess::Percentage => column.percent.is_some(),
        Guess::Specified => column.percent.is_none() && column.constrained,
        _ => column.percent.is_none() && !column.constrained,
    };
    let base = |column: &Column, below: Guess| {
        if column.percent.is_some() && below > Guess::Percentage {
            column.percent_size(target)
        } else if column.constrained && column.percent.is_none() && below > Guess::Specified {
            column.max
        } else {
            column.min
        }
    };
    match guess {
        Guess::Min => columns.iter().map(|c| c.min).collect(),
        Guess::Max if target == sizes[3] => columns
            .iter()
            .map(|c| {
                if c.percent.is_some() {
                    c.percent_size(target)
                } else {
                    c.max
                }
            })
            .collect(),
        Guess::Percentage | Guess::Specified | Guess::Max => {
            let index = guess as usize;
            let room = target - sizes[index - 1];
            let count = columns.iter().filter(|c| grow(c, guess)).count().max(1) as f64;
            let increase = increases[index];
            let mut out: Vec<f64> = columns
                .iter()
                .map(|column| {
                    if !grow(column, guess) {
                        return base(column, guess);
                    }
                    let own = match guess {
                        Guess::Percentage => column.percent_size(target) - column.min,
                        _ => column.max - column.min,
                    };
                    let delta = if increase > 0.0 {
                        room * own / increase
                    } else {
                        room / count
                    };
                    column.min + delta
                })
                .collect();
            settle(&mut out, columns, target, |c| grow(c, guess));
            out
        }
        Guess::AboveMax => above_max(
            target,
            columns,
            sizes[3],
            (autos, fixeds, percents),
            (auto_max, fixed_max, percent_total),
            constrained,
        ),
    }
}
/** The past-max case: auto columns grow by max width, else (when the target is a constraint)
 * fixed columns, else percentage columns by their percentage. */
fn above_max(
    target: f64,
    columns: &[Column],
    max_total: f64,
    (autos, fixeds, percents): (usize, usize, usize),
    (auto_max, fixed_max, percent_total): (f64, f64, f64),
    constrained: bool,
) -> Vec<f64> {
    let room = target - max_total;
    let mut out: Vec<f64> = columns
        .iter()
        .map(|c| {
            if c.percent.is_some() {
                c.percent_size(target)
            } else {
                c.max
            }
        })
        .collect();
    let share = |weight: f64, basis: f64, count: usize| {
        if basis > 0.0 {
            room * weight / basis
        } else {
            room / count.max(1) as f64
        }
    };
    let pick: Box<dyn Fn(&Column) -> bool> = if autos > 0 {
        Box::new(|c: &Column| c.percent.is_none() && !c.constrained)
    } else if fixeds > 0 && constrained {
        Box::new(|c: &Column| c.percent.is_none() && c.constrained)
    } else if percents > 0 {
        Box::new(|c: &Column| c.percent.is_some())
    } else {
        return out;
    };
    for (size, column) in out.iter_mut().zip(columns) {
        if !pick(column) {
            continue;
        }
        *size += if autos > 0 {
            share(column.max, auto_max, autos)
        } else if fixeds > 0 && constrained {
            share(column.max, fixed_max, fixeds)
        } else {
            share(column.percent.unwrap_or(0.0), percent_total, percents)
        };
    }
    settle(&mut out, columns, target, pick);
    out
}
/** Gives any rounding remainder to the last growing column, as Blink's remaining_deficit does. */
fn settle(out: &mut [f64], columns: &[Column], target: f64, grows: impl Fn(&Column) -> bool) {
    let total = out.iter().sum::<f64>();
    if let Some(last) = columns.iter().rposition(grows) {
        out[last] += target - total;
    }
}
/** Blink's SynchronizeAssignableTableInlineSizeAndColumnsFixed: fixed columns take their width
 * (scaled when nothing else can absorb the rest), percentages theirs, auto columns split the
 * remainder evenly. */
fn distribute_fixed(target: f64, columns: &[Column]) -> Vec<f64> {
    let fixed = |c: &Column| c.is_fixed() && c.max > 0.0;
    let zero = |c: &Column| c.constrained && c.max == 0.0;
    let autos = columns
        .iter()
        .filter(|c| c.percent.is_none() && !fixed(c) && !zero(c))
        .count();
    let percent_total = columns
        .iter()
        .filter(|c| c.percent.is_some())
        .map(|c| c.percent_size(target))
        .sum::<f64>();
    let fixed_total = columns
        .iter()
        .filter(|c| c.percent.is_none() && fixed(c))
        .map(|c| c.max)
        .sum::<f64>();
    let mut out = vec![0.0; columns.len()];
    let mut assigned = 0.0;
    let room = (target - percent_total).max(0.0);
    let scale = if (fixed_total < room && autos == 0) || fixed_total > target {
        room / fixed_total.max(f64::EPSILON)
    } else {
        1.0
    };
    for (size, column) in out.iter_mut().zip(columns) {
        if column.percent.is_none() && fixed(column) {
            *size = column.max * scale;
            assigned += *size;
        }
    }
    if assigned >= target {
        return out;
    }
    let left = target - assigned;
    let scale = if (percent_total < left && autos == 0) || percent_total > left {
        left / percent_total.max(f64::EPSILON)
    } else {
        1.0
    };
    for (size, column) in out.iter_mut().zip(columns) {
        if column.percent.is_some() {
            *size = column.percent_size(target) * scale;
            assigned += *size;
        }
    }
    let all_zero = columns.iter().all(zero);
    let rest = |c: &Column| c.percent.is_none() && !fixed(c) && (!zero(c) || all_zero);
    let count = columns.iter().filter(|c| rest(c)).count();
    for (size, column) in out.iter_mut().zip(columns) {
        if rest(column) {
            *size = (target - assigned) / count as f64;
        }
    }
    let assigned_total = out.iter().sum::<f64>();
    if let Some(last) = columns
        .iter()
        .rposition(|c| rest(c) || c.percent.is_some() || fixed(c))
    {
        out[last] += target - assigned_total;
    }
    out
}
