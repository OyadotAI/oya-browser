/** Tangent arcs use the existing atomic elliptical-arc append after resolving the current user-space point. */
use super::*;

impl Path {
    /** Construct the shortest circle tangent to the incoming and outgoing rays, with Canvas degeneracies. */
    pub fn arc_to(
        &mut self,
        first: Point,
        second: Point,
        radius: f64,
        transform: Affine,
    ) -> EngineResult<()> {
        self.validate()?;
        if !radius.is_finite() || !transform.finite() {
            return Err(invalid());
        }
        let mapped = checked(transform.map(first))?;
        checked(transform.map(second))?;
        if radius < 0.0 {
            if self.subpaths.is_empty() {
                self.move_to(first, transform)?;
            }
            return Err(EngineError::new(
                "canvas_radius",
                "Canvas arc radius must be nonnegative.",
                "Use a nonnegative radius.",
            ));
        }
        let Some(subpath) = self.subpaths.last() else {
            return self.move_to(first, transform);
        };
        let current = if subpath.closed {
            subpath.points[0]
        } else {
            *subpath.points.last().expect("validated path")
        };
        if radius == 0.0 || first == second || current == mapped {
            return self.line_to(first, transform);
        }
        let inverse = transform.inverse().ok_or_else(invalid)?;
        let current = checked(inverse.map(current))?;
        let Some(arc) = tangent(current, first, second, radius)? else {
            return self.line_to(first, transform);
        };
        self.arc(
            arc.center,
            radius,
            arc.angles,
            arc.counterclockwise,
            transform,
        )
    }
}

/** Only derived circle geometry crosses into the shared arc subdivision code. */
struct Tangent {
    center: Point,
    angles: [f64; 2],
    counterclockwise: bool,
}

/** Normalized rays and atan2 avoid unstable acos rounding for almost-collinear directions. */
fn tangent(
    current: Point,
    first: Point,
    second: Point,
    radius: f64,
) -> EngineResult<Option<Tangent>> {
    let incoming = unit([current[0] - first[0], current[1] - first[1]])?;
    let outgoing = unit([second[0] - first[0], second[1] - first[1]])?;
    let turn = incoming[0] * outgoing[1] - incoming[1] * outgoing[0];
    if turn == 0.0 {
        return Ok(None);
    }
    let dot = (incoming[0] * outgoing[0] + incoming[1] * outgoing[1]).clamp(-1.0, 1.0);
    let distance = radius / (turn.abs().atan2(dot) * 0.5).tan();
    let start = checked([
        first[0] + incoming[0] * distance,
        first[1] + incoming[1] * distance,
    ])?;
    let end = checked([
        first[0] + outgoing[0] * distance,
        first[1] + outgoing[1] * distance,
    ])?;
    let side = -turn.signum();
    let center = checked([
        start[0] + incoming[1] * radius * side,
        start[1] - incoming[0] * radius * side,
    ])?;
    let angle = |point: Point| (point[1] - center[1]).atan2(point[0] - center[0]);
    Ok(Some(Tangent {
        center,
        angles: [angle(start), angle(end)],
        counterclockwise: turn > 0.0,
    }))
}

/** A zero, infinite or numerically lost ray cannot safely define a tangent circle. */
fn unit(vector: Point) -> EngineResult<Point> {
    let length = vector[0].hypot(vector[1]);
    if !length.is_finite() || length == 0.0 {
        return Err(invalid());
    }
    Ok([vector[0] / length, vector[1] / length])
}

/** Inverse-dependent geometry outside this implementation's numeric range is refused explicitly. */
fn invalid() -> EngineError {
    EngineError::new(
        "canvas_geometry",
        "Canvas tangent arc exceeds this build's numeric range.",
        "Use finite geometry and an invertible current transform.",
    )
}
