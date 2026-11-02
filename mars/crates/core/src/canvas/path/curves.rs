/** Adaptive de Casteljau subdivision turns curves into bounded, construction-transformed path points. */
use super::*;

/** Measure approximation in bitmap pixels, independently of author transforms or pen width. */
const FLATNESS: f64 = 0.125;
/** A difficult curve fails explicitly rather than recursing indefinitely or silently coarsening. */
const DEPTH: usize = 20;
/** A pending subdivision contains its control polygon and its bounded depth. */
type Pending<const N: usize> = ([Point; N], usize);

impl Path {
    /** An empty path starts at the quadratic control point; the entire operation is admitted atomically. */
    pub fn quadratic_curve_to(
        &mut self,
        control: Point,
        end: Point,
        transform: Affine,
    ) -> EngineResult<()> {
        self.validate()?;
        let control = checked(transform.map(control))?;
        let end = checked(transform.map(end))?;
        let (start, new_subpath) = self.curve_start(control);
        self.append_curve([start, control, end], new_subpath)
    }

    /** Cubic control points use the transform at construction, while existing path points stay in place. */
    pub fn bezier_curve_to(
        &mut self,
        first: Point,
        second: Point,
        end: Point,
        transform: Affine,
    ) -> EngineResult<()> {
        self.validate()?;
        let first = checked(transform.map(first))?;
        let second = checked(transform.map(second))?;
        let end = checked(transform.map(end))?;
        let (start, new_subpath) = self.curve_start(first);
        self.append_curve([start, first, second, end], new_subpath)
    }

    /** Closing starts a fresh contour at its former start; a missing path starts at the first control point. */
    fn curve_start(&self, first_control: Point) -> (Point, bool) {
        match self.subpaths.last() {
            Some(subpath) if subpath.closed => (subpath.points[0], true),
            Some(subpath) => (*subpath.points.last().expect("validated path"), false),
            None => (first_control, true),
        }
    }

    /** Flatten completely before changing the current subpath, including its closed state. */
    fn append_curve<const N: usize>(
        &mut self,
        control: [Point; N],
        new_subpath: bool,
    ) -> EngineResult<()> {
        let start_points = usize::from(new_subpath);
        let remaining = POINTS
            .saturating_sub(self.count())
            .saturating_sub(start_points);
        let _allocation = Allocation::new(
            remaining * std::mem::size_of::<Point>()
                + (DEPTH + 1) * std::mem::size_of::<Pending<N>>(),
        )?;
        let points = flatten(control, remaining)?;
        self.admit(points.len() + start_points)?;
        if new_subpath {
            self.subpaths.push(Subpath {
                points: vec![control[0]],
                closed: false,
            });
        }
        self.subpaths
            .last_mut()
            .expect("curve subpath")
            .points
            .extend(points);
        Ok(())
    }
}

/** A bounded iterative stack avoids call-stack growth; every pending leaf already consumes point admission. */
fn flatten<const N: usize>(control: [Point; N], remaining: usize) -> EngineResult<Vec<Point>> {
    if remaining == 0 {
        return Err(quota());
    }
    let mut points = Vec::with_capacity(remaining);
    let mut stack = Vec::with_capacity(DEPTH + 1);
    stack.push((control, 0));
    while let Some((control, depth)) = stack.pop() {
        if flat(&control) {
            points.push(control[N - 1]);
            continue;
        }
        if depth == DEPTH || points.len() + stack.len() + 2 > remaining {
            return Err(quota());
        }
        let (left, right) = split(control);
        stack.push((right, depth + 1));
        stack.push((left, depth + 1));
    }
    Ok(points)
}

/** Distance to the finite chord preserves collinear backtracking that distance to an infinite line misses. */
fn flat<const N: usize>(control: &[Point; N]) -> bool {
    let start = control[0];
    let end = control[N - 1];
    control[1..N - 1]
        .iter()
        .all(|point| distance(*point, start, end) <= FLATNESS)
}

/** Clamping the projection detects controls beyond either endpoint and closed-endpoint loops. */
fn distance(point: Point, start: Point, end: Point) -> f64 {
    let delta = [end[0] - start[0], end[1] - start[1]];
    let length = delta[0] * delta[0] + delta[1] * delta[1];
    let amount = if length == 0.0 {
        0.0
    } else {
        (((point[0] - start[0]) * delta[0] + (point[1] - start[1]) * delta[1]) / length)
            .clamp(0.0, 1.0)
    };
    (point[0] - (start[0] + delta[0] * amount)).hypot(point[1] - (start[1] + delta[1] * amount))
}

/** Midpoint subdivision preserves endpoints exactly and keeps all generated points inside the control hull. */
fn split<const N: usize>(mut polygon: [Point; N]) -> ([Point; N], [Point; N]) {
    let (mut left, mut right) = ([[0.0; 2]; N], [[0.0; 2]; N]);
    left[0] = polygon[0];
    right[N - 1] = polygon[N - 1];
    for level in 1..N {
        for index in 0..N - level {
            polygon[index] = midpoint(polygon[index], polygon[index + 1]);
        }
        left[level] = polygon[0];
        right[N - 1 - level] = polygon[N - 1 - level];
    }
    (left, right)
}

/** Averaging halves avoids an intermediate sum larger than either admitted coordinate. */
fn midpoint(first: Point, second: Point) -> Point {
    [
        first[0] * 0.5 + second[0] * 0.5,
        first[1] * 0.5 + second[1] * 0.5,
    ]
}
