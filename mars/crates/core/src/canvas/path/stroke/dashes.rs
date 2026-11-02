/** User-space dash splitting bounds every transition and retained point before allocating stroke runs. */
use super::*;

/** Tiny patterns cannot expand one admitted path into unbounded native work or storage. */
const SPLITS: usize = OUTLINE_POINTS;

/** A run is open at gap boundaries; a whole closed contour keeps its seam join. */
pub(super) struct Run {
    pub(super) points: Vec<Point>,
    pub(super) closed: bool,
    pub(super) direction: Point,
}

/** One budget spans every subpath, including initial phase scans and zero-length transitions. */
#[derive(Default)]
pub(super) struct Budget {
    transitions: usize,
    points: usize,
}
impl Budget {
    /** A bounded transition is charged even when it emits no visible geometry. */
    fn step(&mut self) -> EngineResult<()> {
        if self.transitions == SPLITS {
            return Err(quota());
        }
        self.transitions += 1;
        Ok(())
    }
    /** Points are admitted before Vec growth, including both endpoints of a zero-length dash. */
    fn point(&mut self, points: &mut Vec<Point>, point: Point) -> EngineResult<()> {
        if self.points == SPLITS {
            return Err(quota());
        }
        checked(point)?;
        self.points += 1;
        points.push(point);
        Ok(())
    }
    /** Planning adds this charge to pixel work before mutable backing ownership is acquired. */
    pub(super) fn work(&self) -> usize {
        self.transitions + self.points
    }
}

/** Reserve worst-case run headers, Vec growth and temporary copies within the process allocation ceiling. */
pub(super) fn reservation(active: bool) -> EngineResult<Allocation> {
    Allocation::new(if active {
        SPLITS * (std::mem::size_of::<Point>() * 8 + std::mem::size_of::<Run>() * 2)
    } else {
        0
    })
}

/** The phase starts anew on each contour, with zero-length ink entries kept for cap dots. */
struct Phase<'a> {
    pattern: &'a [f64],
    index: usize,
    remaining: f64,
}
impl<'a> Phase<'a> {
    /** Euclidean remainder handles large positive and negative offsets without walking whole periods. */
    fn new(dash: &'a DashStyle, total: f64, budget: &mut Budget) -> EngineResult<Self> {
        let mut phase = Self {
            pattern: &dash.segments,
            index: 0,
            remaining: dash.segments[0],
        };
        let mut offset = dash.offset.rem_euclid(total);
        while offset > phase.remaining || (offset == phase.remaining && phase.remaining > 0.0) {
            offset -= phase.remaining;
            phase.next(budget)?;
        }
        phase.remaining -= offset;
        Ok(phase)
    }
    /** Advancing through zero entries still consumes a finite transition allowance. */
    fn next(&mut self, budget: &mut Budget) -> EngineResult<()> {
        budget.step()?;
        self.index = (self.index + 1) % self.pattern.len();
        self.remaining = self.pattern[self.index];
        Ok(())
    }
    /** Canonical even indices paint while odd indices leave gaps. */
    fn ink(&self) -> bool {
        self.index.is_multiple_of(2)
    }
}

/** Split a complete contour before outlining so closed-seam dash pieces can be joined atomically. */
pub(super) fn split(
    points: &[Point],
    closed: bool,
    dash: &DashStyle,
    total: f64,
    budget: &mut Budget,
) -> EngineResult<Vec<Run>> {
    if points.len() < 2 {
        return Ok(Vec::new());
    }
    let mut phase = Phase::new(dash, total, budget)?;
    let starts_in_ink = phase.ink();
    let mut current = if starts_in_ink {
        Some(run(points[0], direction(points[0], points[1]), budget)?)
    } else {
        None
    };
    let mut runs = Vec::new();
    let edges = points.len() - usize::from(!closed);
    for index in 0..edges {
        edge(
            points[index],
            points[(index + 1) % points.len()],
            &mut phase,
            &mut current,
            &mut runs,
            budget,
        )?;
    }
    let ends_in_ink = current.is_some();
    runs.extend(current);
    seam(&mut runs, closed && starts_in_ink && ends_in_ink);
    Ok(runs)
}

/** Each edge may contain many transitions, but each transition and generated endpoint is admitted first. */
fn edge(
    from: Point,
    to: Point,
    phase: &mut Phase<'_>,
    current: &mut Option<Run>,
    runs: &mut Vec<Run>,
    budget: &mut Budget,
) -> EngineResult<()> {
    budget.step()?;
    let length = (to[0] - from[0]).hypot(to[1] - from[1]);
    let tangent = direction(from, to);
    let mut travelled = 0.0;
    while length - travelled > phase.remaining {
        let next = travelled + phase.remaining;
        if phase.remaining > 0.0 && next <= travelled {
            return Err(quota());
        }
        travelled = next;
        let fraction = travelled / length;
        let point = [
            from[0] + (to[0] - from[0]) * fraction,
            from[1] + (to[1] - from[1]) * fraction,
        ];
        if let Some(mut ink) = current.take() {
            budget.point(&mut ink.points, point)?;
            runs.push(ink);
        } else {
            *current = Some(run(point, tangent, budget)?);
        }
        phase.next(budget)?;
    }
    phase.remaining -= length - travelled;
    if let Some(ink) = current {
        budget.point(&mut ink.points, to)?;
    }
    Ok(())
}

/** The first point is budgeted before allocating even a one-point run. */
fn run(point: Point, direction: Point, budget: &mut Budget) -> EngineResult<Run> {
    let mut points = Vec::new();
    budget.point(&mut points, point)?;
    Ok(Run {
        points,
        closed: false,
        direction,
    })
}

/** Ink crossing a closed contour's origin is one run with an interior join, never two capped ends. */
fn seam(runs: &mut Vec<Run>, joins: bool) {
    if !joins {
        return;
    }
    if runs.len() == 1 {
        let run = &mut runs[0];
        run.closed = true;
        if run.points.first() == run.points.last() {
            run.points.pop();
        }
    } else if runs.len() > 1 {
        let first = runs.remove(0);
        let last = runs.last_mut().expect("remaining closed-seam run");
        last.points.extend(first.points.into_iter().skip(1));
    }
}
