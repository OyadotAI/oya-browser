/** Elliptical arcs become admitted transformed points without fixed-step or partial-path fallbacks. */
use super::*;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/** Match the curve approximation tolerance in bitmap coordinates. */
const FLATNESS: f64 = 0.125;
/** Keep distinct tangent directions even when a very small whole ellipse would fit inside one error bound. */
const MAX_SEGMENT_ANGLE: f64 = PI / 8.0;
/** The subdivision stack remains finite even for unrepresentably large transformed ellipses. */
const DEPTH: usize = 20;
/** An interval needs its angular endpoints and its current subdivision depth. */
type Pending = (f64, f64, usize);

impl Path {
    /** Circular arcs share sweep normalization and atomic admission with ellipses. */
    pub fn arc(
        &mut self,
        center: Point,
        radius: f64,
        angles: [f64; 2],
        counterclockwise: bool,
        transform: Affine,
    ) -> EngineResult<()> {
        self.ellipse(
            center,
            [radius, radius],
            0.0,
            angles,
            counterclockwise,
            transform,
        )
    }

    /** Connect to the arc start, then append the complete admitted arc as one path operation. */
    pub fn ellipse(
        &mut self,
        center: Point,
        radii: Point,
        rotation: f64,
        angles: [f64; 2],
        counterclockwise: bool,
        transform: Affine,
    ) -> EngineResult<()> {
        self.validate()?;
        validate(center, radii, rotation, angles, transform)?;
        if radii.contains(&0.0) {
            return self.line_to(center, transform);
        }
        let oval = Oval::new(center, radii, rotation, transform)?;
        let (start, sweep) = sweep(angles, counterclockwise);
        self.append_arc(&oval, start, sweep)
    }

    /** A closed contour starts its new subpath at its old first point before connecting to the arc. */
    fn append_arc(&mut self, oval: &Oval, start: f64, sweep: f64) -> EngineResult<()> {
        let previous = self.subpaths.last();
        let new_subpath = previous.is_none_or(|path| path.closed);
        let prefix = previous
            .filter(|path| path.closed)
            .map(|path| path.points[0]);
        let available = POINTS.saturating_sub(self.count());
        if available < 1 + usize::from(prefix.is_some()) {
            return Err(quota());
        }
        let _allocation = Allocation::new(
            available * std::mem::size_of::<Point>() + (DEPTH + 1) * std::mem::size_of::<Pending>(),
        )?;
        let mut points = Vec::with_capacity(available);
        if let Some(prefix) = prefix {
            points.push(prefix);
        }
        let first = oval.at(start)?;
        points.push(first);
        flatten(oval, start, sweep, available, &mut points)?;
        if sweep.abs() == TAU {
            *points.last_mut().expect("whole arc") = first;
        }
        self.admit(points.len())?;
        if new_subpath {
            self.subpaths.push(Subpath {
                points,
                closed: false,
            });
        } else {
            self.subpaths
                .last_mut()
                .expect("open arc subpath")
                .points
                .extend(points);
        }
        Ok(())
    }
}

/** Only the Web binding turns negative radii into DOM exceptions; direct callers still receive a checked error. */
fn validate(
    center: Point,
    radii: Point,
    rotation: f64,
    angles: [f64; 2],
    transform: Affine,
) -> EngineResult<()> {
    if center
        .into_iter()
        .chain(radii)
        .chain(angles)
        .chain([rotation])
        .any(|value| !value.is_finite())
        || radii.iter().any(|value| *value < 0.0)
        || !transform.finite()
    {
        return Err(EngineError::invalid("Invalid canvas ellipse arguments."));
    }
    Ok(())
}

/** Large directed sweeps and exact nonzero whole turns describe one complete circumference. */
fn sweep([start, end]: [f64; 2], counterclockwise: bool) -> (f64, f64) {
    let delta = end - start;
    let whole = if counterclockwise {
        -delta >= TAU
    } else {
        delta >= TAU
    } || start != end && delta.is_finite() && delta % TAU == 0.0;
    let first = start.rem_euclid(TAU);
    if whole {
        return (first, if counterclockwise { -TAU } else { TAU });
    }
    let positive = (end.rem_euclid(TAU) - first).rem_euclid(TAU);
    let amount = if counterclockwise && positive != 0.0 {
        positive - TAU
    } else {
        positive
    };
    (first, amount)
}

/** Transform the ellipse basis once so subdivision error is measured in final bitmap coordinates. */
struct Oval {
    center: Point,
    horizontal: Point,
    vertical: Point,
}

impl Oval {
    /** Basis vectors include axis rotation and the current transform's scale and shear. */
    fn new(center: Point, radii: Point, rotation: f64, transform: Affine) -> EngineResult<Self> {
        let (sin, cos) = rotation.sin_cos();
        let [a, b, c, d, _, _] = transform.0;
        let map = |[x, y]: Point| checked([a * x + c * y, b * x + d * y]);
        Ok(Self {
            center: checked(transform.map(center))?,
            horizontal: map([radii[0] * cos, radii[0] * sin])?,
            vertical: map([-radii[1] * sin, radii[1] * cos])?,
        })
    }

    /** Parametric angles preserve rotated and unequal-radius ellipse endpoints. */
    fn at(&self, angle: f64) -> EngineResult<Point> {
        let (sin, cos) = angle.sin_cos();
        checked([
            self.center[0] + self.horizontal[0] * cos + self.vertical[0] * sin,
            self.center[1] + self.horizontal[1] * cos + self.vertical[1] * sin,
        ])
    }

    /** The interpolation remainder is bounded by maximum second derivative times interval squared over eight. */
    fn flat(&self, start: f64, end: f64) -> bool {
        if (end - start).abs() > MAX_SEGMENT_ANGLE {
            return false;
        }
        let second = self.horizontal[0].hypot(self.horizontal[1]) * max_cos(start, end)
            + self.vertical[0].hypot(self.vertical[1])
                * max_cos(start - FRAC_PI_2, end - FRAC_PI_2);
        second * (end - start).powi(2) / 8.0 <= FLATNESS
    }
}

/** Every pending interval consumes one future endpoint; subdivision cannot exceed remaining point admission. */
fn flatten(
    oval: &Oval,
    start: f64,
    sweep: f64,
    available: usize,
    points: &mut Vec<Point>,
) -> EngineResult<()> {
    if sweep == 0.0 {
        return Ok(());
    }
    if points.len() == available {
        return Err(quota());
    }
    let mut stack = Vec::with_capacity(DEPTH + 1);
    stack.push((start, start + sweep, 0));
    while let Some((start, end, depth)) = stack.pop() {
        if oval.flat(start, end) {
            points.push(oval.at(end)?);
            continue;
        }
        if depth == DEPTH || points.len() + stack.len() + 2 > available {
            return Err(quota());
        }
        let middle = start * 0.5 + end * 0.5;
        stack.push((middle, end, depth + 1));
        stack.push((start, middle, depth + 1));
    }
    Ok(())
}

/** Cosine reaches absolute one at multiples of pi; otherwise its interval maximum is at an endpoint. */
fn max_cos(first: f64, second: f64) -> f64 {
    let (start, end) = (first.min(second), first.max(second));
    if (start / PI).ceil() <= (end / PI).floor() {
        1.0
    } else {
        start.cos().abs().max(end.cos().abs())
    }
}
