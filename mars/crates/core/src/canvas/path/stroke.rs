/** Polygon stroke outlines preserve construction-time geometry and draw-time pen transforms. */
use super::*;
mod dashes;

/** Round outline tessellation stays below a quarter pixel chord error within admitted transforms. */
const ROUND_ERROR: f64 = 0.25;
/** A circle needs enough sides for small round joins, and a finite upper bound for huge pens. */
const ROUND_MIN: usize = 16;
/** Excessively large transformed round pens fail explicitly instead of producing coarse fake circles. */
const ROUND_MAX: usize = 1024;

/** Endpoint caps apply only to open contours. */
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}
/** Miter joins fall back to bevel when the miter length exceeds the saved limit. */
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

/** These pen properties belong to drawing state and participate in save/restore. */
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StrokeStyle {
    /// Width is measured in the current user coordinate system at stroke time.
    pub width: f64,
    /// Open contours choose a cap independently of their joins.
    pub cap: LineCap,
    /// Each interior vertex chooses an outer join.
    pub join: LineJoin,
    /// Ratio of miter length to half-width, with the Canvas default of ten.
    pub miter_limit: f64,
}

impl Default for StrokeStyle {
    /** A fresh context uses a one-unit, butt-capped, miter-joined pen. */
    fn default() -> Self {
        Self {
            width: 1.0,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter_limit: 10.0,
        }
    }
}

impl StrokeStyle {
    /** Invalid restored values must not enter outline arithmetic. */
    pub fn validate(&self) -> EngineResult<()> {
        if !self.width.is_finite()
            || self.width <= 0.0
            || !self.miter_limit.is_finite()
            || self.miter_limit <= 0.0
        {
            return Err(EngineError::invalid("Invalid canvas stroke style."));
        }
        Ok(())
    }
}

/** Geometry is already in canvas space; inverse-map it before constructing the current transformed pen. */
pub fn plan_stroke(
    size: [u32; 2],
    path: &Path,
    style: StrokeStyle,
    transform: Affine,
) -> EngineResult<PathPlan> {
    plan_stroke_dashed(size, path, style, transform, &DashStyle::default())
}

/** Dash splitting shares the ordinary cap/join rasterizer and preserves the stable solid-stroke API. */
pub fn plan_stroke_dashed(
    size: [u32; 2],
    path: &Path,
    style: StrokeStyle,
    transform: Affine,
    dash: &DashStyle,
) -> EngineResult<PathPlan> {
    path.validate()?;
    style.validate()?;
    dash.validate()?;
    let period = dash.total()?;
    let mut budget = dashes::Budget::default();
    let _dash_scratch = dashes::reservation(period > 0.0)?;
    if !transform.finite() {
        return Err(EngineError::invalid("Invalid canvas stroke transform."));
    }
    let _scratch = Allocation::new(OUTLINE_POINTS * std::mem::size_of::<Point>() * 2)?;
    let mut outline = Outline {
        contours: Vec::new(),
        count: 0,
        transform,
    };
    if let Some(inverse) = transform.inverse() {
        for subpath in &path.subpaths {
            outline.subpath(subpath, style, inverse, dash, period, &mut budget)?;
        }
    }
    raster::contours(
        size,
        outline.contours.iter().map(Vec::as_slice),
        FillRule::Nonzero,
    )?
    .add_work(budget.work())
}

/** All polygons are wound the same way, so their nonzero fill is a union rather than repeated alpha. */
struct Outline {
    contours: Vec<Vec<Point>>,
    count: usize,
    transform: Affine,
}

impl Outline {
    /** Duplicate points do not create a false join or divide by a zero segment length. */
    fn subpath(
        &mut self,
        subpath: &Subpath,
        style: StrokeStyle,
        inverse: Affine,
        dash: &DashStyle,
        period: f64,
        budget: &mut dashes::Budget,
    ) -> EngineResult<()> {
        if subpath.points.len() < 2 {
            return Ok(());
        }
        let mut points: Vec<_> = subpath
            .points
            .iter()
            .map(|point| inverse.map(*point))
            .collect();
        for point in &points {
            checked(*point)?;
        }
        points.dedup();
        if subpath.closed && points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        if period > 0.0 {
            for run in dashes::split(&points, subpath.closed, dash, period, budget)? {
                self.run(&run.points, run.closed, run.direction, style)?;
            }
            return Ok(());
        }
        self.run(&points, subpath.closed, [1.0, 0.0], style)
    }

    /** Each on-run uses joins internally and caps only at its actual dash endpoints. */
    fn run(
        &mut self,
        points: &[Point],
        closed: bool,
        direction: Point,
        style: StrokeStyle,
    ) -> EngineResult<()> {
        let mut points = points.to_vec();
        points.dedup();
        if points.len() == 1 {
            return if !closed {
                self.dot(points[0], direction, style)
            } else {
                Ok(())
            };
        }
        self.segments(&points, closed, style)?;
        self.joins(&points, closed, style)
    }

    /** Zero-length dashes retain their local direction so square caps rotate with the path. */
    fn dot(&mut self, center: Point, direction: Point, style: StrokeStyle) -> EngineResult<()> {
        let half = style.width / 2.0;
        match style.cap {
            LineCap::Butt => Ok(()),
            LineCap::Round => self.circle(center, half),
            LineCap::Square => {
                let tangent = scaled(direction, half);
                let normal = [-tangent[1], tangent[0]];
                self.polygon(vec![
                    add(add(center, tangent), normal),
                    add(add(center, tangent), scaled(normal, -1.0)),
                    add(add(center, scaled(tangent, -1.0)), scaled(normal, -1.0)),
                    add(add(center, scaled(tangent, -1.0)), normal),
                ])
            }
        }
    }

    /** Each segment supplies its rectangular ink; open endpoints supply caps once. */
    fn segments(&mut self, points: &[Point], closed: bool, style: StrokeStyle) -> EngineResult<()> {
        let segments = points.len() - usize::from(!closed);
        let half = style.width / 2.0;
        for index in 0..segments {
            let (mut from, mut to) = (points[index], points[(index + 1) % points.len()]);
            let unit = direction(from, to);
            let normal = [-unit[1] * half, unit[0] * half];
            if !closed && matches!(style.cap, LineCap::Square) {
                if index == 0 {
                    from = add(from, scaled(unit, -half));
                }
                if index + 1 == segments {
                    to = add(to, scaled(unit, half));
                }
            }
            self.polygon(vec![
                add(from, normal),
                add(to, normal),
                add(to, scaled(normal, -1.0)),
                add(from, scaled(normal, -1.0)),
            ])?;
        }
        if !closed && matches!(style.cap, LineCap::Round) {
            self.circle(points[0], half)?;
            self.circle(*points.last().expect("open stroke"), half)?;
        }
        Ok(())
    }

    /** Closing joins the final edge to the first instead of installing endpoint caps. */
    fn joins(&mut self, points: &[Point], closed: bool, style: StrokeStyle) -> EngineResult<()> {
        let start = usize::from(!closed);
        let end = points.len() - usize::from(!closed);
        for index in start..end {
            let previous = points[(index + points.len() - 1) % points.len()];
            let center = points[index];
            let next = points[(index + 1) % points.len()];
            self.join(
                center,
                direction(previous, center),
                direction(center, next),
                style,
            )?;
        }
        Ok(())
    }

    /** An outer miter intersects offset edge lines; the inner overlap is already covered by segments. */
    fn join(
        &mut self,
        center: Point,
        incoming: Point,
        outgoing: Point,
        style: StrokeStyle,
    ) -> EngineResult<()> {
        let turn = cross(incoming, outgoing);
        let half = style.width / 2.0;
        if matches!(style.join, LineJoin::Round) {
            return self.circle(center, half);
        }
        if turn == 0.0 {
            return Ok(());
        }
        let sign = -turn.signum();
        let first = add(
            center,
            [-incoming[1] * half * sign, incoming[0] * half * sign],
        );
        let last = add(
            center,
            [-outgoing[1] * half * sign, outgoing[0] * half * sign],
        );
        let mut polygon = vec![center, first];
        let delta = [last[0] - first[0], last[1] - first[1]];
        let miter = add(first, scaled(incoming, cross(delta, outgoing) / turn));
        let distance = (miter[0] - center[0]).hypot(miter[1] - center[1]);
        if matches!(style.join, LineJoin::Miter) && distance / half <= style.miter_limit {
            polygon.push(miter);
        }
        polygon.push(last);
        self.polygon(polygon)
    }

    /** Subdivide round caps and joins according to their transformed radius before allocating. */
    fn circle(&mut self, center: Point, radius: f64) -> EngineResult<()> {
        let [a, b, c, d, _, _] = self.transform.0;
        let projected = radius * (a.hypot(b) + c.hypot(d));
        let count = ((std::f64::consts::PI * (projected / (2.0 * ROUND_ERROR)).sqrt()).ceil()
            as usize)
            .max(ROUND_MIN);
        if count > ROUND_MAX || self.count.saturating_add(count) > OUTLINE_POINTS {
            return Err(quota());
        }
        let points = (0..count)
            .map(|index| {
                let angle = std::f64::consts::TAU * index as f64 / count as f64;
                add(center, [angle.cos() * radius, angle.sin() * radius])
            })
            .collect();
        self.polygon(points)
    }

    /** Validate an entire contour before retaining it; consistent orientation survives reflected transforms. */
    fn polygon(&mut self, mut points: Vec<Point>) -> EngineResult<()> {
        if self.count.saturating_add(points.len()) > OUTLINE_POINTS {
            return Err(quota());
        }
        for point in &mut points {
            *point = checked(self.transform.map(*point))?;
        }
        let base = points[0];
        let area: f64 = points
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let next = points[(index + 1) % points.len()];
                cross(
                    [point[0] - base[0], point[1] - base[1]],
                    [next[0] - base[0], next[1] - base[1]],
                )
            })
            .sum();
        if area < 0.0 {
            points.reverse();
        }
        self.count += points.len();
        self.contours.push(points);
        Ok(())
    }
}

/** Hypot avoids squaring long finite segments before normalization. */
fn direction(from: Point, to: Point) -> Point {
    let delta = [to[0] - from[0], to[1] - from[1]];
    scaled(delta, 1.0 / delta[0].hypot(delta[1]))
}
/** Small vector operations make the outline's offset and intersection formulas explicit. */
fn add(a: Point, b: Point) -> Point {
    [a[0] + b[0], a[1] + b[1]]
}
/** Scaling never mutates source path points. */
fn scaled(point: Point, amount: f64) -> Point {
    [point[0] * amount, point[1] * amount]
}
/** Signed cross products identify turns and consistent polygon orientation. */
fn cross(a: Point, b: Point) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
